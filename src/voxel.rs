//! The 3D world: a voxel island made of the same materials as the 2D game, following
//! falling-sand rules in three dimensions. Sand and gravel tumble, water and lava flow
//! and level out, lava meeting water turns to stone and obsidian, fire spreads through
//! trees and leaves ash.
//!
//! The island is a fixed SX x SY x SZ grid of voxels, meshed in 16^3 chunks. Faces carry
//! their own light: ambient occlusion at the corners, a sun shadow traced towards a
//! fixed sun, and darkness underground.

use macroquad::prelude::*;

use crate::materials::{Kind, Material as M};
use crate::noise::{fbm, hash3, Rng};

pub const SX: i32 = 192;
pub const SY: i32 = 80;
pub const SZ: i32 = 192;
pub const SEA: i32 = 26;
const CS: i32 = 16;
const CX: i32 = SX / CS;
const CY: i32 = SY / CS;
const CZ: i32 = SZ / CS;
/// Direction the sunlight comes from (towards the sun), for baked shadows.
pub const SUN: [f32; 3] = [0.55, 0.8, 0.3];

#[inline]
pub fn mat(v: u8) -> M {
    // Every byte stored in the grid came from a `Material as u8`.
    if v <= crate::materials::LAST_MATERIAL as u8 {
        unsafe { std::mem::transmute::<u8, M>(v) }
    } else {
        M::Stone
    }
}

/// Voxels the wizard stands on and light can't pass through.
#[inline]
pub fn opaque(m: M) -> bool {
    matches!(m.kind(), Kind::Static | Kind::Powder) && !matches!(m, M::Glass | M::Flower | M::Plant | M::Mushroom | M::Sprout)
}

/// Tiny decorations drawn as small cubes: flowers and tufts.
#[inline]
fn tuft(m: M) -> bool {
    matches!(m, M::Flower | M::Plant | M::Mushroom)
}

pub struct Vox {
    pub cells: Vec<u8>,
    pub life: Vec<u8>,
    /// Highest opaque voxel in each column (for sky light).
    top: Vec<i32>,
    pub seed: u32,
    dirty: Vec<bool>,
    meshes: Vec<Vec<Mesh>>,
    water: Vec<Vec<Mesh>>,
    active: Vec<u32>,
    queued: Vec<bool>,
    pub rng: Rng,
    pub tick: u32,
    /// Burning voxels this step (drawn as flames).
    pub fires: Vec<u32>,
    /// Steam puffs and sparks requested by the sim, consumed by the game.
    pub puffs: Vec<(Vec3, u8)>,
    /// Edited since the last save.
    pub edited: bool,
}

impl Vox {
    pub fn idx(x: i32, y: i32, z: i32) -> usize {
        ((y * SZ + z) * SX + x) as usize
    }

    pub fn inside(x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < SX && y < SY && z < SZ
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> M {
        if !Self::inside(x, y, z) {
            return if y < 0 { M::Basalt } else { M::Empty };
        }
        mat(self.cells[Self::idx(x, y, z)])
    }

    pub fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        if x < 0 || z < 0 || x >= SX || z >= SZ {
            return true; // invisible walls around the island
        }
        opaque(self.get(x, y, z))
    }

    pub fn new(seed: u64) -> Self {
        let n = (SX * SY * SZ) as usize;
        let mut v = Vox {
            cells: vec![M::Empty as u8; n],
            life: vec![0; n],
            top: vec![0; (SX * SZ) as usize],
            seed: (seed as u32) ^ 0x3D3D,
            dirty: vec![true; (CX * CY * CZ) as usize],
            meshes: (0..CX * CY * CZ).map(|_| Vec::new()).collect(),
            water: (0..CX * CY * CZ).map(|_| Vec::new()).collect(),
            active: Vec::new(),
            queued: vec![false; n],
            rng: Rng::new(seed ^ 0x77),
            tick: 0,
            fires: Vec::new(),
            puffs: Vec::new(),
            edited: false,
        };
        v.generate();
        v
    }

    // ---------- generation ----------

    /// Ground height and biome of a column: 0 grassland, 1 desert, 2 snow, 3 beach.
    fn column(&self, x: i32, z: i32) -> (i32, u8) {
        let s = self.seed;
        let (fx, fz) = (x as f32, z as f32);
        let (dx, dz) = (fx / SX as f32 - 0.5, fz / SZ as f32 - 0.5);
        let r = (dx * dx + dz * dz).sqrt() * 2.0 + (fbm(fx * 0.015, fz * 0.015, 3, s.wrapping_add(9)) - 0.5) * 0.5;
        let land = (1.0 - ((r - 0.55) / 0.35).clamp(0.0, 1.0)).powf(1.5);
        let hills = fbm(fx * 0.025, fz * 0.025, 4, s) * 22.0;
        let peaks = (fbm(fx * 0.011, fz * 0.011 + 30.0, 3, s.wrapping_add(3)) - 0.48).max(0.0) * 110.0;
        let h = (SEA as f32 - 14.0 + (hills + 12.0 + peaks) * land).clamp(3.0, SY as f32 - 6.0) as i32;
        let temp = fbm(fx * 0.007, fz * 0.007 + 70.0, 2, s.wrapping_add(5));
        let biome = if h <= SEA + 1 {
            3
        } else if h > SEA + 24 {
            2
        } else if temp < 0.37 {
            1
        } else {
            0
        };
        (h, biome)
    }

    fn generate(&mut self) {
        let s = self.seed;
        let mut cols = vec![(0, 0u8); (SX * SZ) as usize];
        for z in 0..SZ {
            for x in 0..SX {
                let (h, biome) = self.column(x, z);
                cols[(z * SX + x) as usize] = (h, biome);
                for y in 0..SY {
                    let m = if y > h {
                        if y <= SEA {
                            M::Water
                        } else {
                            M::Empty
                        }
                    } else if y < 2 {
                        M::Basalt
                    } else {
                        let d = h - y;
                        // Caves: two crossed noise fields make winding tunnels and halls.
                        let c = fbm(x as f32 * 0.06, y as f32 * 0.1, 2, s.wrapping_add(11)) + fbm(z as f32 * 0.06, y as f32 * 0.1 + 33.0, 2, s.wrapping_add(12));
                        let deep = ((SEA - y) as f32 / 20.0).clamp(0.0, 0.12);
                        if d > 3 && y > 2 && c > 1.24 - deep {
                            if y < 7 {
                                M::Lava
                            } else {
                                M::Empty
                            }
                        } else {
                            let hh = hash3(x * 7 + z, y, s);
                            match biome {
                                _ if d == 0 && biome == 0 => M::Grass,
                                _ if d == 0 && biome == 2 => M::Snow,
                                1 | 3 if d < 4 => M::Sand,
                                _ if d < 3 => M::Dirt,
                                _ if hh % 97 == 0 && y < 20 => M::Gold,
                                _ if hh % 61 == 0 => M::Coal,
                                _ if hh % 13 == 0 && d > 6 => M::Gravel,
                                _ if y < 9 => M::Basalt,
                                _ => M::Stone,
                            }
                        }
                    };
                    self.cells[Self::idx(x, y, z)] = m as u8;
                }
            }
        }
        // Trees, flowers and landmarks.
        for gz in 0..SZ / 6 {
            for gx in 0..SX / 6 {
                let h = hash3(gx, gz, s.wrapping_add(21));
                let x = gx * 6 + (h % 5) as i32;
                let z = gz * 6 + ((h >> 4) % 5) as i32;
                let (top, biome) = cols[(z * SX + x) as usize];
                if top <= SEA + 1 || self.get(x, top, z) == M::Empty {
                    continue;
                }
                let grove = fbm(x as f32 * 0.03, z as f32 * 0.03, 2, s.wrapping_add(22));
                match biome {
                    0 if h % 100 < 34 => {
                        if h % 41 == 0 {
                            self.great_tree(x, top + 1, z, h);
                        } else if grove > 0.6 {
                            self.blossom(x, top + 1, z, h);
                        } else {
                            self.oak(x, top + 1, z, h);
                        }
                    }
                    2 if h % 100 < 30 => self.pine(x, top + 1, z, h),
                    1 if h % 100 < 10 => {
                        for k in 0..(3 + h % 3) as i32 {
                            self.put(x, top + 1 + k, z, M::Plant);
                        }
                    }
                    _ => {}
                }
            }
        }
        for z in 0..SZ {
            for x in 0..SX {
                let (top, biome) = cols[(z * SX + x) as usize];
                let h = hash3(x, z, s.wrapping_add(23));
                if biome == 0 && self.get(x, top, z) == M::Grass && self.get(x, top + 1, z) == M::Empty {
                    let meadow = fbm(x as f32 * 0.04, z as f32 * 0.04, 2, s.wrapping_add(24)) > 0.55;
                    if h % if meadow { 4 } else { 23 } == 0 {
                        self.put(x, top + 1, z, M::Flower);
                    } else if h % 5 == 1 {
                        self.put(x, top + 1, z, M::Plant);
                    }
                }
            }
        }
        self.stones(s);
        for z in 0..SZ {
            for x in 0..SX {
                self.recompute_top(x, z);
            }
        }
        // Let lava and water settle against the caves they were carved into.
        for i in 0..self.cells.len() {
            if matches!(mat(self.cells[i]), M::Lava) {
                self.queue(i);
            }
        }
    }

    /// A ring of rune-carved standing stones on a hilltop, and a stone arch.
    fn stones(&mut self, s: u32) {
        let mut best = (0, 0, 0);
        for k in 0..400 {
            let h = hash3(k, 5, s);
            let (x, z) = (20 + (h % (SX as u32 - 40)) as i32, 20 + ((h >> 12) % (SZ as u32 - 40)) as i32);
            let (top, biome) = self.column(x, z);
            if biome == 0 && top > best.1 && top < SEA + 22 {
                best = (x, top, z);
            }
        }
        let (cx, top, cz) = best;
        if top == 0 {
            return;
        }
        for k in 0..7 {
            let a = k as f32 / 7.0 * std::f32::consts::TAU;
            let (x, z) = ((cx as f32 + a.cos() * 7.0) as i32, (cz as f32 + a.sin() * 7.0) as i32);
            let base = (0..SY).rev().find(|&y| opaque(self.get(x, y, z))).unwrap_or(top);
            let hgt = 4 + (hash3(k, 9, s) % 4) as i32;
            for y in base + 1..=base + hgt {
                let rune = y == base + hgt - 1 || y == base + 2;
                self.put(x, y, z, if rune { M::Rune } else { M::Stone });
            }
        }
        self.put(cx, top + 1, cz, M::Altar);
    }

    fn put(&mut self, x: i32, y: i32, z: i32, m: M) {
        if Self::inside(x, y, z) {
            self.cells[Self::idx(x, y, z)] = m as u8;
        }
    }

    fn put_soft(&mut self, x: i32, y: i32, z: i32, m: M) {
        if Self::inside(x, y, z) && matches!(self.get(x, y, z), M::Empty | M::Flower | M::Plant) {
            self.cells[Self::idx(x, y, z)] = m as u8;
        }
    }

    fn blob(&mut self, (cx, cy, cz): (i32, i32, i32), r: f32, ry: f32, m: M, h: u32) {
        let ri = r.ceil() as i32;
        let ryi = ry.ceil() as i32;
        for y in -ryi..=ryi {
            for z in -ri..=ri {
                for x in -ri..=ri {
                    let e = (x * x + z * z) as f32 / (r * r) + (y * y) as f32 / (ry * ry);
                    let jag = (hash3(x + cx * 3, y + z * 5, h) % 100) as f32 / 100.0 * 0.4;
                    if e <= 1.0 + jag - 0.2 {
                        self.put_soft(cx + x, cy + y, cz + z, m);
                    }
                }
            }
        }
    }

    fn oak(&mut self, x: i32, y: i32, z: i32, h: u32) {
        let t = 4 + (h >> 8) as i32 % 3;
        for k in 0..t {
            self.put(x, y + k, z, M::Wood);
        }
        self.blob((x, y + t, z), 2.8, 2.2, M::Leaves, h);
    }

    fn blossom(&mut self, x: i32, y: i32, z: i32, h: u32) {
        let t = 4 + (h >> 8) as i32 % 3;
        let (mut wx, mut wz) = (x, z);
        for k in 0..t {
            if k > 1 && hash3(k, 1, h) % 3 == 0 {
                wx += if hash3(k, 2, h) % 2 == 0 { 1 } else { -1 };
            }
            if k > 1 && hash3(k, 3, h) % 4 == 0 {
                wz += if hash3(k, 4, h) % 2 == 0 { 1 } else { -1 };
            }
            self.put(wx, y + k, wz, M::Wood);
        }
        self.blob((wx, y + t, wz), 3.2, 2.0, M::Blossom, h);
        for k in 0..8 {
            let (px, pz) = (x + (hash3(k, 5, h) % 9) as i32 - 4, z + (hash3(k, 6, h) % 9) as i32 - 4);
            if let Some(gy) = (0..SY).rev().find(|&yy| opaque(self.get(px, yy, pz))) {
                if self.get(px, gy, pz) == M::Grass {
                    self.put_soft(px, gy + 1, pz, M::Blossom);
                }
            }
        }
    }

    fn pine(&mut self, x: i32, y: i32, z: i32, h: u32) {
        let t = 6 + (h >> 8) as i32 % 4;
        for k in 0..t {
            self.put(x, y + k, z, M::Wood);
        }
        for k in 2..t + 2 {
            let r = ((t + 2 - k) as f32 * 0.45).max(0.6);
            let ri = r.ceil() as i32;
            for dz in -ri..=ri {
                for dx in -ri..=ri {
                    if ((dx * dx + dz * dz) as f32) <= r * r {
                        self.put_soft(x + dx, y + k, z + dz, M::Leaves);
                    }
                }
            }
        }
    }

    fn great_tree(&mut self, x: i32, y: i32, z: i32, h: u32) {
        let t = 11 + (h >> 8) as i32 % 4;
        for k in -1..t {
            for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                self.put(x + dx, y + k, z + dz, M::Wood);
            }
            if k < 2 {
                for (dx, dz) in [(-1, 0), (2, 1), (0, -1), (1, 2)] {
                    self.put(x + dx, y + k, z + dz, M::Wood);
                }
            }
        }
        self.blob((x, y + t, z), 5.0, 3.0, M::Leaves, h);
        for (k, (dx, dz)) in [(-4, 2), (4, -1), (1, 4), (-1, -4)].iter().enumerate() {
            let by = y + t - 3 + (k as i32 % 2);
            for j in 0..4 {
                self.put(x + dx * j / 4, by + j / 2, z + dz * j / 4, M::Wood);
            }
            self.blob((x + dx, by + 1, z + dz), 3.0, 2.0, M::Leaves, h.wrapping_add(k as u32));
        }
    }

    // ---------- editing ----------

    fn recompute_top(&mut self, x: i32, z: i32) {
        let t = (0..SY).rev().find(|&y| opaque(self.get(x, y, z))).unwrap_or(-1);
        self.top[(z * SX + x) as usize] = t;
    }

    pub fn top(&self, x: i32, z: i32) -> i32 {
        if x < 0 || z < 0 || x >= SX || z >= SZ {
            return -1;
        }
        self.top[(z * SX + x) as usize]
    }

    fn mark(&mut self, x: i32, y: i32, z: i32) {
        for (dx, dy, dz) in [(0, 0, 0), (1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            if Self::inside(nx, ny, nz) {
                self.dirty[((ny / CS * CZ + nz / CS) * CX + nx / CS) as usize] = true;
            }
        }
    }

    fn queue(&mut self, i: usize) {
        if !self.queued[i] {
            self.queued[i] = true;
            self.active.push(i as u32);
        }
    }

    fn wake(&mut self, x: i32, y: i32, z: i32) {
        for (dx, dy, dz) in [(0, 0, 0), (1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            if Self::inside(nx, ny, nz) {
                self.queue(Self::idx(nx, ny, nz));
            }
        }
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, m: M) {
        if !Self::inside(x, y, z) {
            return;
        }
        let i = Self::idx(x, y, z);
        self.cells[i] = m as u8;
        self.life[i] = match m {
            M::Fire => self.rng.range(20, 50) as u8,
            _ => 0,
        };
        self.edited = true;
        self.mark(x, y, z);
        self.wake(x, y, z);
        self.recompute_top(x, z);
    }

    fn swap(&mut self, a: (i32, i32, i32), b: (i32, i32, i32)) {
        let (i, j) = (Self::idx(a.0, a.1, a.2), Self::idx(b.0, b.1, b.2));
        self.cells.swap(i, j);
        self.life.swap(i, j);
        self.mark(a.0, a.1, a.2);
        self.mark(b.0, b.1, b.2);
        self.wake(a.0, a.1, a.2);
        self.wake(b.0, b.1, b.2);
        if a.0 != b.0 || a.2 != b.2 {
            self.recompute_top(a.0, a.2);
        }
        self.recompute_top(b.0, b.2);
    }

    /// Set fire to a voxel if it can burn.
    pub fn ignite(&mut self, x: i32, y: i32, z: i32) {
        let m = self.get(x, y, z);
        if m.flammable() {
            if m == M::Grass {
                self.set(x, y, z, M::Dirt);
                if self.get(x, y + 1, z) == M::Empty {
                    self.set(x, y + 1, z, M::Fire);
                }
            } else {
                self.set(x, y, z, M::Fire);
            }
        } else if m == M::Empty && opaque(self.get(x, y - 1, z)) {
            self.set(x, y, z, M::Fire);
        }
    }

    // ---------- simulation ----------

    /// One fixed step of the falling-sand rules over every voxel that might move.
    pub fn step(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        let list = std::mem::take(&mut self.active);
        for &i in &list {
            self.queued[i as usize] = false;
        }
        self.fires.clear();
        let mut budget = 40_000;
        for i in list {
            budget -= 1;
            if budget == 0 {
                break;
            }
            let i = i as usize;
            let x = (i as i32) % SX;
            let z = (i as i32 / SX) % SZ;
            let y = i as i32 / (SX * SZ);
            match mat(self.cells[i]) {
                M::Sand | M::Gravel | M::Ash | M::Rubble | M::Gunpowder | M::Seed => self.powder(x, y, z),
                M::Water => self.liquid(x, y, z, 60),
                M::Oil => self.liquid(x, y, z, 50),
                M::Lava => {
                    if self.lava(x, y, z) {
                        continue;
                    }
                    if self.tick % 3 == 0 {
                        self.liquid(x, y, z, 14);
                    } else {
                        self.queue(i);
                    }
                }
                M::Fire => self.fire(x, y, z),
                _ => {}
            }
        }
    }

    fn free(&self, x: i32, y: i32, z: i32) -> bool {
        Self::inside(x, y, z) && matches!(self.get(x, y, z), M::Empty | M::Flower | M::Plant | M::Fire)
    }

    fn sinkable(&self, x: i32, y: i32, z: i32, heavy: bool) -> bool {
        self.free(x, y, z) || heavy && Self::inside(x, y, z) && matches!(self.get(x, y, z), M::Water | M::Oil)
    }

    fn powder(&mut self, x: i32, y: i32, z: i32) {
        if self.sinkable(x, y - 1, z, true) {
            self.swap((x, y, z), (x, y - 1, z));
            return;
        }
        let r = self.rng.range(0, 4);
        for k in 0..4 {
            let (dx, dz) = [(1, 0), (-1, 0), (0, 1), (0, -1)][((r + k) % 4) as usize];
            if self.free(x + dx, y, z + dz) && self.sinkable(x + dx, y - 1, z + dz, true) {
                self.swap((x, y, z), (x + dx, y - 1, z + dz));
                return;
            }
        }
    }

    fn liquid(&mut self, x: i32, y: i32, z: i32, spread: u8) {
        let i = Self::idx(x, y, z);
        let me = self.get(x, y, z);
        // Oil floats: water above oil sinks through it.
        if me == M::Water && self.get(x, y - 1, z) == M::Oil {
            self.swap((x, y, z), (x, y - 1, z));
            return;
        }
        if self.free(x, y - 1, z) {
            self.life[i] = 0;
            self.swap((x, y, z), (x, y - 1, z));
            return;
        }
        let r = self.rng.range(0, 4);
        for k in 0..4 {
            let (dx, dz) = [(1, 0), (-1, 0), (0, 1), (0, -1)][((r + k) % 4) as usize];
            if self.free(x + dx, y, z + dz) && self.free(x + dx, y - 1, z + dz) {
                self.life[i] = 0;
                self.swap((x, y, z), (x + dx, y - 1, z + dz));
                return;
            }
        }
        // Spread sideways for a while, then settle.
        if self.life[i] < spread {
            for k in 0..4 {
                let (dx, dz) = [(1, 0), (-1, 0), (0, 1), (0, -1)][((r + k) % 4) as usize];
                if self.free(x + dx, y, z + dz) {
                    self.life[i] += 1;
                    self.swap((x, y, z), (x + dx, y, z + dz));
                    return;
                }
            }
        }
    }

    /// Lava ignites what it touches and turns water to steam; quenched from above it
    /// becomes obsidian, otherwise stone. Returns true if it solidified.
    fn lava(&mut self, x: i32, y: i32, z: i32) -> bool {
        for (dx, dy, dz) in [(0, 1, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, -1, 0)] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            match self.get(nx, ny, nz) {
                M::Water => {
                    self.set(nx, ny, nz, M::Empty);
                    self.puffs.push((vec3(nx as f32 + 0.5, ny as f32 + 0.5, nz as f32 + 0.5), 0));
                    self.set(x, y, z, if dy > 0 { M::Obsidian } else { M::Stone });
                    return true;
                }
                m if m.flammable() && self.rng.chance(0.1) => self.ignite(nx, ny, nz),
                _ => {}
            }
        }
        false
    }

    fn fire(&mut self, x: i32, y: i32, z: i32) {
        let i = Self::idx(x, y, z);
        self.fires.push(i as u32);
        for (dx, dy, dz) in [(0, 1, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, -1, 0)] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            let m = self.get(nx, ny, nz);
            if m == M::Water {
                self.set(x, y, z, M::Empty);
                self.puffs.push((vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5), 0));
                return;
            }
            let p = match m {
                M::Leaves | M::Blossom | M::Plant | M::Flower => 0.12,
                M::Wood => 0.035,
                M::Oil | M::Gunpowder | M::Gas => 0.5,
                M::Grass => 0.05,
                _ => 0.0,
            };
            if p > 0.0 && self.rng.chance(p) {
                self.ignite(nx, ny, nz);
            }
        }
        let l = self.life[i].saturating_sub(1);
        self.life[i] = l;
        if l == 0 {
            let ash = opaque(self.get(x, y - 1, z)) && self.rng.chance(0.25);
            self.set(x, y, z, if ash { M::Ash } else { M::Empty });
            self.puffs.push((vec3(x as f32 + 0.5, y as f32 + 0.8, z as f32 + 0.5), 1));
        } else {
            self.queue(i);
        }
    }

    // ---------- meshing ----------

    /// Rebuild up to `budget` dirty chunk meshes, nearest to `near` first.
    pub fn remesh(&mut self, near: Vec3, budget: usize) {
        let mut dirty: Vec<(i32, usize)> = self
            .dirty
            .iter()
            .enumerate()
            .filter(|(_, d)| **d)
            .map(|(k, _)| {
                let k = k as i32;
                let (cx, cz, cy) = (k % CX, (k / CX) % CZ, k / (CX * CZ));
                let c = vec3((cx * CS + 8) as f32, (cy * CS + 8) as f32, (cz * CS + 8) as f32);
                ((c - near).length() as i32, k as usize)
            })
            .collect();
        dirty.sort_unstable();
        for (_, k) in dirty.into_iter().take(budget) {
            self.dirty[k] = false;
            let (opaque_m, water_m) = self.mesh_chunk(k as i32);
            self.meshes[k] = opaque_m;
            self.water[k] = water_m;
        }
    }

    pub fn dirty_left(&self) -> usize {
        self.dirty.iter().filter(|d| **d).count()
    }

    fn sun_shadow(&self, p: [f32; 3]) -> f32 {
        let mut q = p;
        for _ in 0..28 {
            q = [q[0] + SUN[0], q[1] + SUN[1], q[2] + SUN[2]];
            if q[1] >= SY as f32 {
                return 1.0;
            }
            let m = self.get(q[0].floor() as i32, q[1].floor() as i32, q[2].floor() as i32);
            if opaque(m) && !matches!(m, M::Leaves | M::Blossom) {
                return 0.55;
            }
            if matches!(m, M::Leaves | M::Blossom) {
                return 0.72; // dappled shade under trees
            }
        }
        1.0
    }

    fn mesh_chunk(&self, k: i32) -> (Vec<Mesh>, Vec<Mesh>) {
        let (cx, cz, cy) = (k % CX, (k / CX) % CZ, k / (CX * CZ));
        let mut solid = MeshBuf::default();
        let mut water = MeshBuf::default();
        // (normal, u, v, face brightness)
        const FACES: [([i32; 3], [i32; 3], [i32; 3], f32); 6] = [
            ([0, 1, 0], [1, 0, 0], [0, 0, 1], 1.0),
            ([0, -1, 0], [1, 0, 0], [0, 0, 1], 0.5),
            ([1, 0, 0], [0, 1, 0], [0, 0, 1], 0.84),
            ([-1, 0, 0], [0, 1, 0], [0, 0, 1], 0.7),
            ([0, 0, 1], [1, 0, 0], [0, 1, 0], 0.76),
            ([0, 0, -1], [1, 0, 0], [0, 1, 0], 0.64),
        ];
        for y in cy * CS..(cy + 1) * CS {
            for z in cz * CS..(cz + 1) * CS {
                for x in cx * CS..(cx + 1) * CS {
                    let m = self.get(x, y, z);
                    if m == M::Empty || m == M::Fire {
                        continue;
                    }
                    let h = hash3(x * 31 + z, y, 7);
                    let base = shade(m, h);
                    if tuft(m) {
                        // A small cube sitting on the ground, swaying colour by hash.
                        let col = if m == M::Flower { [[236, 84, 104], [250, 214, 80], [120, 150, 250], [246, 244, 236], [196, 110, 230]][(h % 5) as usize] } else { base };
                        let hgt = if m == M::Plant { 0.3 + (h % 3) as f32 * 0.14 } else { 0.3 };
                        let wid = if m == M::Plant { 0.14 } else { 0.22 };
                        let (ox, oz) = (0.2 + ((h >> 8) % 6) as f32 * 0.1, 0.2 + ((h >> 12) % 6) as f32 * 0.1);
                        let lit = if y > self.top(x, z) { 1.0 } else { 0.5 };
                        solid.cube(vec3(x as f32 + ox, y as f32, z as f32 + oz), vec3(wid, hgt, wid), col, lit);
                        continue;
                    }
                    let liquid = matches!(m, M::Water | M::Glass | M::Oil | M::Acid);
                    for (n, u, v, bright) in FACES {
                        let (nx, ny, nz) = (x + n[0], y + n[1], z + n[2]);
                        let nm = self.get(nx, ny, nz);
                        if liquid {
                            // No walls of water at the edge of the world.
                            if nm == m || opaque(nm) || nx < 0 || nz < 0 || nx >= SX || nz >= SZ || ny < 0 {
                                continue;
                            }
                        } else if opaque(nm) && !(nx < 0 || nz < 0 || nx >= SX || nz >= SZ) {
                            continue;
                        }
                        // Light: open sky above this face's neighbour, sun shadow, corners.
                        let sky = if ny > self.top(nx, nz) { 1.0 } else { (0.5 - ((self.top(nx, nz) - ny) as f32 * 0.03)).max(0.18) };
                        let centre = [x as f32 + 0.5 + n[0] as f32 * 0.6, y as f32 + 0.5 + n[1] as f32 * 0.6, z as f32 + 0.5 + n[2] as f32 * 0.6];
                        let sun = if sky >= 1.0 && !liquid { self.sun_shadow(centre) } else { 1.0 };
                        let glow = matches!(m, M::Lava | M::Crystal | M::Rune | M::Mushroom | M::Altar);
                        let mut corners = [[0.0f32; 3]; 4];
                        let mut ao = [1.0f32; 4];
                        for (c, (su, sv)) in [(-1, -1), (1, -1), (1, 1), (-1, 1)].iter().enumerate() {
                            let p = [
                                x as f32 + 0.5 + n[0] as f32 * 0.5 + (u[0] * su + v[0] * sv) as f32 * 0.5,
                                y as f32 + 0.5 + n[1] as f32 * 0.5 + (u[1] * su + v[1] * sv) as f32 * 0.5,
                                z as f32 + 0.5 + n[2] as f32 * 0.5 + (u[2] * su + v[2] * sv) as f32 * 0.5,
                            ];
                            corners[c] = p;
                            if !liquid && !glow {
                                let s1 = self.solid(nx + u[0] * su, ny + u[1] * su, nz + u[2] * su) as i32;
                                let s2 = self.solid(nx + v[0] * sv, ny + v[1] * sv, nz + v[2] * sv) as i32;
                                let cc = self.solid(nx + u[0] * su + v[0] * sv, ny + u[1] * su + v[1] * sv, nz + u[2] * su + v[2] * sv) as i32;
                                let level = if s1 == 1 && s2 == 1 { 0 } else { 3 - (s1 + s2 + cc) };
                                ao[c] = [0.5, 0.66, 0.83, 1.0][level as usize];
                            }
                        }
                        let light = if glow { 1.25 } else { bright * sky * sun };
                        let buf = if liquid { &mut water } else { &mut solid };
                        let alpha = match m {
                            M::Water => 150,
                            M::Glass => 110,
                            M::Oil => 220,
                            _ => 255,
                        };
                        // Water's top sits a little low so it reads as a surface.
                        if m == M::Water && n[1] == 1 {
                            for c in corners.iter_mut() {
                                c[1] -= 0.12;
                            }
                        }
                        buf.quad(corners, base, alpha, light, ao);
                    }
                }
            }
        }
        (solid.finish(), water.finish())
    }

    pub fn draw(&self, cam: Vec3, mat_solid: &Material, mat_water: &Material) {
        let visible = |k: usize| {
            let k = k as i32;
            let (cx, cz, cy) = (k % CX, (k / CX) % CZ, k / (CX * CZ));
            let c = vec3((cx * CS + 8) as f32, (cy * CS + 8) as f32, (cz * CS + 8) as f32);
            (c - cam).length() < 230.0
        };
        gl_use_material(mat_solid);
        for (k, ms) in self.meshes.iter().enumerate() {
            if visible(k) {
                for m in ms {
                    draw_mesh(m);
                }
            }
        }
        gl_use_material(mat_water);
        for (k, ms) in self.water.iter().enumerate() {
            if visible(k) {
                for m in ms {
                    draw_mesh(m);
                }
            }
        }
        gl_use_default_material();
    }

    /// First voxel hit along a ray: (hit, empty voxel just before it).
    pub fn raycast(&self, from: Vec3, dir: Vec3, max: f32, hit_liquid: bool) -> Option<((i32, i32, i32), (i32, i32, i32))> {
        let mut p = from;
        let step = dir.normalize_or_zero() * 0.1;
        let mut prev = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let mut t = 0.0;
        while t < max {
            p += step;
            t += 0.1;
            let c = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            if c == prev {
                continue;
            }
            let m = self.get(c.0, c.1, c.2);
            if opaque(m) || hit_liquid && matches!(m.kind(), Kind::Liquid) || (c.0 < 0 || c.2 < 0 || c.0 >= SX || c.2 >= SZ) {
                return Some((c, prev));
            }
            prev = c;
        }
        None
    }

    // ---------- saving ----------

    /// Run-length encoded voxels.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = b"FWV1".to_vec();
        let mut i = 0;
        while i < self.cells.len() {
            let v = self.cells[i];
            let mut run = 1;
            while i + run < self.cells.len() && self.cells[i + run] == v && run < 65535 {
                run += 1;
            }
            out.push(v);
            out.extend_from_slice(&(run as u16).to_le_bytes());
            i += run;
        }
        out
    }

    pub fn decode(&mut self, data: &[u8]) -> bool {
        if data.len() < 4 || &data[..4] != b"FWV1" {
            return false;
        }
        let mut cells = Vec::with_capacity(self.cells.len());
        for c in data[4..].chunks_exact(3) {
            let run = u16::from_le_bytes([c[1], c[2]]) as usize;
            let v = if c[0] <= crate::materials::LAST_MATERIAL as u8 { c[0] } else { M::Stone as u8 };
            cells.extend(std::iter::repeat(v).take(run));
        }
        if cells.len() != self.cells.len() {
            return false;
        }
        self.cells = cells;
        self.life.fill(0);
        self.dirty.fill(true);
        for z in 0..SZ {
            for x in 0..SX {
                self.recompute_top(x, z);
            }
        }
        for i in 0..self.cells.len() {
            if matches!(mat(self.cells[i]), M::Fire | M::Lava | M::Water | M::Sand) {
                self.queue(i);
            }
        }
        true
    }
}

/// A voxel's colour, with per-voxel variation.
fn shade(m: M, h: u32) -> [u8; 3] {
    let b = match m {
        M::Grass => [86, 168, 70],
        M::Leaves => [62, 142, 62],
        M::Water => [44, 110, 210],
        M::Lava => [255, 120, 30],
        m => m.base_color(),
    };
    let v = m.variation().min(0.5) * 0.7;
    let k = 1.0 + ((h & 255) as f32 / 255.0 - 0.5) * v;
    [(b[0] as f32 * k).min(255.0) as u8, (b[1] as f32 * k).min(255.0) as u8, (b[2] as f32 * k).min(255.0) as u8]
}

/// Collects quads into meshes of at most ~60k vertices each.
#[derive(Default)]
struct MeshBuf {
    done: Vec<Mesh>,
    verts: Vec<Vertex>,
    idx: Vec<u16>,
}

impl MeshBuf {
    fn quad(&mut self, c: [[f32; 3]; 4], col: [u8; 3], alpha: u8, light: f32, ao: [f32; 4]) {
        if self.verts.len() > 60_000 {
            self.flush();
        }
        let b = self.verts.len() as u16;
        for k in 0..4 {
            let l = light * ao[k];
            let rgb = [(col[0] as f32 * l).min(255.0) as u8, (col[1] as f32 * l).min(255.0) as u8, (col[2] as f32 * l).min(255.0) as u8];
            self.verts.push(Vertex { position: vec3(c[k][0], c[k][1], c[k][2]), uv: vec2(0.0, 0.0), color: [rgb[0], rgb[1], rgb[2], alpha], normal: vec4(0.0, 0.0, 0.0, 0.0) });
        }
        // Split along the brighter diagonal so AO gradients don't crease.
        if ao[0] + ao[2] > ao[1] + ao[3] {
            self.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        } else {
            self.idx.extend_from_slice(&[b + 1, b + 2, b + 3, b + 1, b + 3, b]);
        }
    }

    fn cube(&mut self, o: Vec3, s: Vec3, col: [u8; 3], light: f32) {
        let (x0, y0, z0, x1, y1, z1) = (o.x, o.y, o.z, o.x + s.x, o.y + s.y, o.z + s.z);
        let faces = [
            ([[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]], 1.0),
            ([[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]], 0.84),
            ([[x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1]], 0.7),
            ([[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], 0.76),
            ([[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]], 0.64),
        ];
        for (c, b) in faces {
            self.quad(c, col, 255, light * b, [1.0; 4]);
        }
    }

    fn flush(&mut self) {
        if self.verts.is_empty() {
            return;
        }
        self.done.push(Mesh { vertices: std::mem::take(&mut self.verts), indices: std::mem::take(&mut self.idx), texture: None });
    }

    fn finish(mut self) -> Vec<Mesh> {
        self.flush();
        self.done
    }
}
