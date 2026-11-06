//! The infinite world.
//!
//! The world is made of 64x64 storage chunks generated on demand. Only a window of
//! WCX x WCY chunks around the camera lives in a dense array and is simulated; when the
//! camera moves, the window shifts, chunks leaving it are stored (if modified) and new
//! ones are loaded from storage or generated. Inside the window, 16x16 sim chunks sleep
//! when nothing in them changed, so settled terrain costs nothing.
//!
//! All public methods take global cell coordinates.

use std::collections::HashMap;

use crate::cell::{Cell, FIRE_FLOWING, FIRE_FUEL};
use crate::materials::{Kind, Material as M};
use crate::noise::Rng;
use crate::worldgen::Generator;

pub const CHUNK: i32 = 64;
const CU: usize = CHUNK as usize;
pub const WCX: i32 = 10;
pub const WCY: i32 = 8;
pub const WW: usize = (WCX * CHUNK) as usize;
pub const WH: usize = (WCY * CHUNK) as usize;
pub const SC: usize = 16;
pub const SCW: usize = WW / SC;
pub const SCH: usize = WH / SC;
const SPC: usize = CU / SC;

const N4: [(i32, i32); 4] = [(0, 1), (-1, 0), (1, 0), (0, -1)];
const N8: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

struct Stored {
    cells: Vec<Cell>,
    wall: Vec<u8>,
    dirty: bool,
}

pub struct World {
    pub cells: Vec<Cell>,
    /// Background wall shade per cell; 0 means open sky.
    pub wall: Vec<u8>,
    /// Global coordinate of the window's top-left cell.
    pub ox: i32,
    pub oy: i32,
    /// Surface height for each window column.
    pub surf: Vec<i32>,
    store: HashMap<(i32, i32), Stored>,
    dirty: Vec<bool>,
    pub gen: Generator,
    active: Vec<bool>,
    next: Vec<bool>,
    pub rng: Rng,
    pub clock: u8,
    pub frame: u64,
    pub fire_count: usize,
    pub active_chunks: usize,
    /// Explosions requested by the simulation (gunpowder), consumed by the game.
    pub explosions: Vec<(i32, i32, i32)>,
}

impl World {
    pub fn new(seed: u64, gx: f32, gy: f32) -> Self {
        let pcx = (gx.floor() as i32).div_euclid(CHUNK);
        let pcy = (gy.floor() as i32).div_euclid(CHUNK);
        let mut w = World {
            cells: vec![Cell::EMPTY; WW * WH],
            wall: vec![0; WW * WH],
            ox: (pcx - 5) * CHUNK,
            oy: (pcy - 4) * CHUNK,
            surf: vec![0; WW],
            store: HashMap::new(),
            dirty: vec![false; (WCX * WCY) as usize],
            gen: Generator::new(seed),
            active: vec![false; SCW * SCH],
            next: vec![false; SCW * SCH],
            rng: Rng::new(seed ^ 0x5151),
            clock: 0,
            frame: 0,
            fire_count: 0,
            active_chunks: 0,
            explosions: Vec::new(),
        };
        for j in 0..WCY {
            for i in 0..WCX {
                let (c, wl) = w.gen.chunk(w.ox / CHUNK + i, w.oy / CHUNK + j);
                w.blit(i, j, &c, &wl);
            }
        }
        w.update_surf();
        w
    }

    // ---------- streaming ----------

    fn blit(&mut self, i: i32, j: i32, cells: &[Cell], wall: &[u8]) {
        for r in 0..CU {
            let dst = (j as usize * CU + r) * WW + i as usize * CU;
            self.cells[dst..dst + CU].copy_from_slice(&cells[r * CU..(r + 1) * CU]);
            self.wall[dst..dst + CU].copy_from_slice(&wall[r * CU..(r + 1) * CU]);
        }
    }

    fn update_surf(&mut self) {
        for x in 0..WW {
            self.surf[x] = self.gen.surface(self.ox + x as i32);
        }
    }

    /// Surface height at a global column (cached inside the window).
    pub fn surface_at(&self, gx: i32) -> i32 {
        let lx = gx - self.ox;
        if lx >= 0 && (lx as usize) < WW {
            self.surf[lx as usize]
        } else {
            self.gen.surface(gx)
        }
    }

    /// Keep the window centred on (gx, gy), shifting by whole chunks with some hysteresis.
    pub fn recenter(&mut self, gx: f32, gy: f32) {
        let pcx = (gx.floor() as i32).div_euclid(CHUNK);
        let pcy = (gy.floor() as i32).div_euclid(CHUNK);
        let (cx0, cy0) = (self.ox.div_euclid(CHUNK), self.oy.div_euclid(CHUNK));
        let (ix, iy) = (pcx - cx0, pcy - cy0);
        let ncx0 = if ix < 4 { pcx - 4 } else if ix > 5 { pcx - 5 } else { cx0 };
        let ncy0 = if iy < 3 { pcy - 3 } else if iy > 4 { pcy - 4 } else { cy0 };
        if (ncx0, ncy0) != (cx0, cy0) {
            self.shift_to(ncx0, ncy0);
        }
    }

    fn shift_to(&mut self, ncx0: i32, ncy0: i32) {
        let (ocx0, ocy0) = (self.ox.div_euclid(CHUNK), self.oy.div_euclid(CHUNK));
        let old_cells = std::mem::replace(&mut self.cells, vec![Cell::EMPTY; WW * WH]);
        let old_wall = std::mem::replace(&mut self.wall, vec![0; WW * WH]);
        let old_next = std::mem::replace(&mut self.next, vec![false; SCW * SCH]);
        let old_dirty = std::mem::replace(&mut self.dirty, vec![false; (WCX * WCY) as usize]);
        let inside = |i: i32, j: i32| i >= 0 && j >= 0 && i < WCX && j < WCY;

        // Store modified chunks that leave the window; clean ones can be regenerated.
        for j in 0..WCY {
            for i in 0..WCX {
                let (gcx, gcy) = (ocx0 + i, ocy0 + j);
                if inside(gcx - ncx0, gcy - ncy0) || !old_dirty[(j * WCX + i) as usize] {
                    continue;
                }
                let mut cells = Vec::with_capacity(CU * CU);
                let mut wall = Vec::with_capacity(CU * CU);
                for r in 0..CU {
                    let src = (j as usize * CU + r) * WW + i as usize * CU;
                    cells.extend_from_slice(&old_cells[src..src + CU]);
                    wall.extend_from_slice(&old_wall[src..src + CU]);
                }
                self.store.insert((gcx, gcy), Stored { cells, wall, dirty: true });
            }
        }

        self.ox = ncx0 * CHUNK;
        self.oy = ncy0 * CHUNK;
        for j in 0..WCY {
            for i in 0..WCX {
                let (gcx, gcy) = (ncx0 + i, ncy0 + j);
                let (oi, oj) = (gcx - ocx0, gcy - ocy0);
                if inside(oi, oj) {
                    for r in 0..CU {
                        let src = (oj as usize * CU + r) * WW + oi as usize * CU;
                        let dst = (j as usize * CU + r) * WW + i as usize * CU;
                        self.cells[dst..dst + CU].copy_from_slice(&old_cells[src..src + CU]);
                        self.wall[dst..dst + CU].copy_from_slice(&old_wall[src..src + CU]);
                    }
                    for sy in 0..SPC {
                        for sx in 0..SPC {
                            let o = (oj as usize * SPC + sy) * SCW + oi as usize * SPC + sx;
                            let n = (j as usize * SPC + sy) * SCW + i as usize * SPC + sx;
                            self.next[n] = old_next[o];
                        }
                    }
                    self.dirty[(j * WCX + i) as usize] = old_dirty[(oj * WCX + oi) as usize];
                } else if let Some(s) = self.store.remove(&(gcx, gcy)) {
                    self.blit(i, j, &s.cells, &s.wall);
                    self.dirty[(j * WCX + i) as usize] = s.dirty;
                    if s.dirty {
                        for sy in 0..SPC {
                            for sx in 0..SPC {
                                self.next[(j as usize * SPC + sy) * SCW + i as usize * SPC + sx] = true;
                            }
                        }
                    }
                } else {
                    let (c, w) = self.gen.chunk(gcx, gcy);
                    self.blit(i, j, &c, &w);
                }
            }
        }
        self.update_surf();
        self.store.retain(|&(cx, cy), s| {
            s.dirty || (cx >= ncx0 - 3 && cx < ncx0 + WCX + 3 && cy >= ncy0 - 3 && cy < ncy0 + WCY + 3)
        });
    }

    /// Generate a few chunks around the window ahead of time so shifting is instant.
    pub fn prefetch(&mut self, budget: usize) {
        let (cx0, cy0) = (self.ox.div_euclid(CHUNK), self.oy.div_euclid(CHUNK));
        let mut done = 0;
        for j in -1..=WCY {
            for i in -1..=WCX {
                if i >= 0 && j >= 0 && i < WCX && j < WCY {
                    continue;
                }
                let k = (cx0 + i, cy0 + j);
                if self.store.contains_key(&k) {
                    continue;
                }
                let (cells, wall) = self.gen.chunk(k.0, k.1);
                self.store.insert(k, Stored { cells, wall, dirty: false });
                done += 1;
                if done >= budget {
                    return;
                }
            }
        }
    }

    // ---------- local helpers ----------

    #[inline]
    fn inb(x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < WW && (y as usize) < WH
    }

    #[inline]
    fn li(x: i32, y: i32) -> usize {
        y as usize * WW + x as usize
    }

    #[inline]
    fn lm(&self, x: i32, y: i32) -> M {
        if Self::inb(x, y) {
            self.cells[Self::li(x, y)].mat
        } else {
            M::Stone
        }
    }

    #[inline]
    fn wake(&mut self, x: i32, y: i32) {
        if !Self::inb(x, y) {
            return;
        }
        self.dirty[(y as usize / CU) * WCX as usize + x as usize / CU] = true;
        let cx0 = (x - 1).max(0) as usize / SC;
        let cx1 = (x + 1).min(WW as i32 - 1) as usize / SC;
        let cy0 = (y - 1).max(0) as usize / SC;
        let cy1 = (y + 1).min(WH as i32 - 1) as usize / SC;
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                self.next[cy * SCW + cx] = true;
            }
        }
    }

    #[inline]
    fn lset(&mut self, x: i32, y: i32, mut c: Cell) {
        if !Self::inb(x, y) {
            return;
        }
        c.clock = self.clock;
        self.cells[Self::li(x, y)] = c;
        self.wake(x, y);
    }

    #[inline]
    fn swap(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let a = Self::li(x0, y0);
        let b = Self::li(x1, y1);
        self.cells.swap(a, b);
        self.cells[a].clock = self.clock;
        self.cells[b].clock = self.clock;
        self.wake(x0, y0);
        self.wake(x1, y1);
    }

    // ---------- public, global coordinates ----------

    #[inline]
    pub fn mat(&self, gx: i32, gy: i32) -> M {
        self.lm(gx - self.ox, gy - self.oy)
    }

    #[inline]
    pub fn get(&self, gx: i32, gy: i32) -> Option<Cell> {
        let (x, y) = (gx - self.ox, gy - self.oy);
        Self::inb(x, y).then(|| self.cells[Self::li(x, y)])
    }

    pub fn in_window(&self, gx: i32, gy: i32) -> bool {
        Self::inb(gx - self.ox, gy - self.oy)
    }

    pub fn set(&mut self, gx: i32, gy: i32, c: Cell) {
        self.lset(gx - self.ox, gy - self.oy, c);
    }

    pub fn place(&mut self, gx: i32, gy: i32, m: M) {
        let c = Cell::new(m, &mut self.rng);
        self.set(gx, gy, c);
    }

    /// Set fire to (gx, gy) if what's there can burn.
    pub fn ignite_at(&mut self, gx: i32, gy: i32) {
        let (x, y) = (gx - self.ox, gy - self.oy);
        let m = self.lm(x, y);
        if m.flammable() && Self::inb(x, y) {
            self.ignite(x, y, m);
        }
    }

    fn ignite(&mut self, x: i32, y: i32, m: M) {
        match m {
            M::Gunpowder => {
                self.lset(x, y, Cell::EMPTY);
                if self.explosions.len() < 200 {
                    self.explosions.push((x + self.ox, y + self.oy, 5));
                }
            }
            M::Gas => {
                let f = Cell::fire(&mut self.rng);
                self.lset(x, y, f);
                if self.rng.chance(0.04) && self.explosions.len() < 200 {
                    self.explosions.push((x + self.ox, y + self.oy, 4));
                }
            }
            M::Grass => {
                let mut d = Cell::new(M::Dirt, &mut self.rng);
                d.shade = self.cells[Self::li(x, y)].shade;
                self.lset(x, y, d);
                if self.lm(x, y - 1) == M::Empty {
                    let f = Cell::fire(&mut self.rng);
                    self.lset(x, y - 1, f);
                }
            }
            _ => {
                let f = Cell::burning(m, &mut self.rng);
                self.lset(x, y, f);
            }
        }
    }

    // ---------- simulation ----------

    pub fn step(&mut self) {
        self.clock = self.clock.wrapping_add(1);
        self.frame += 1;
        std::mem::swap(&mut self.active, &mut self.next);
        self.next.fill(false);
        self.fire_count = 0;
        self.active_chunks = self.active.iter().filter(|a| **a).count();

        let ltr = self.frame % 2 == 0;
        for y in (0..WH).rev() {
            let cy = y / SC;
            for k in 0..SCW {
                let cx = if ltr { k } else { SCW - 1 - k };
                if !self.active[cy * SCW + cx] {
                    continue;
                }
                let x0 = cx * SC;
                if ltr {
                    for x in x0..x0 + SC {
                        self.update_cell(x as i32, y as i32);
                    }
                } else {
                    for x in (x0..x0 + SC).rev() {
                        self.update_cell(x as i32, y as i32);
                    }
                }
            }
        }
        self.random_ticks(500);
    }

    #[inline]
    fn update_cell(&mut self, x: i32, y: i32) {
        let i = Self::li(x, y);
        let c = self.cells[i];
        if c.mat == M::Empty || c.clock == self.clock {
            return;
        }
        match c.mat.kind() {
            Kind::Empty | Kind::Static => {}
            Kind::Powder => {
                if c.mat == M::Seed && self.update_seed(x, y) {
                    return;
                }
                self.update_powder(x, y, c);
            }
            Kind::Liquid => {
                match c.mat {
                    M::Acid => {
                        if self.update_acid(x, y) {
                            return;
                        }
                    }
                    M::Lava => {
                        if self.update_lava(x, y) || !self.rng.chance(0.4) {
                            return;
                        }
                    }
                    _ => {}
                }
                let c = self.cells[i];
                self.update_liquid(x, y, c);
            }
            Kind::Gas => self.update_gas(x, y, c),
            Kind::Fire => self.update_fire(x, y, c),
            Kind::Life => self.update_sprout(x, y, c),
        }
    }

    #[inline]
    fn can_enter(&mut self, x: i32, y: i32, d: u8) -> bool {
        if !Self::inb(x, y) {
            return false;
        }
        let m = self.cells[Self::li(x, y)].mat;
        match m.kind() {
            Kind::Empty | Kind::Gas | Kind::Fire => true,
            Kind::Liquid => m.density() < d && self.rng.chance(0.35),
            _ => false,
        }
    }

    fn fall(&mut self, x: i32, y: i32, c: Cell, d: u8) -> bool {
        let vel = (c.vel as i32 + 1).min(20);
        let steps = 1 + vel / 5;
        let mut cy = y;
        for _ in 0..steps {
            if self.can_enter(x, cy + 1, d) {
                self.swap(x, cy, x, cy + 1);
                cy += 1;
            } else {
                break;
            }
        }
        if cy != y {
            self.cells[Self::li(x, cy)].vel = vel as u8;
            true
        } else {
            self.cells[Self::li(x, y)].vel = 0;
            false
        }
    }

    fn update_powder(&mut self, x: i32, y: i32, c: Cell) {
        let d = c.mat.density();
        if self.fall(x, y, c, d) {
            return;
        }
        if self.rng.chance(0.9) {
            let dir = self.rng.sign();
            for dx in [dir, -dir] {
                if self.can_enter(x + dx, y + 1, d) {
                    self.swap(x, y, x + dx, y + 1);
                    return;
                }
            }
        }
    }

    /// Seeds sprout when resting on soil. Returns true if it sprouted.
    fn update_seed(&mut self, x: i32, y: i32) -> bool {
        let below = self.lm(x, y + 1);
        if matches!(below, M::Dirt | M::Grass) {
            self.wake(x, y);
            if self.lm(x, y - 1) == M::Empty && self.rng.chance(0.04) {
                let s = Cell::new(M::Sprout, &mut self.rng);
                self.lset(x, y, s);
                return true;
            }
        }
        false
    }

    fn update_sprout(&mut self, x: i32, y: i32, c: Cell) {
        self.wake(x, y);
        if !self.rng.chance(0.15) {
            return;
        }
        if c.life == 0 {
            // Bloom: a little crown of leaves.
            for dy in -2i32..=1 {
                for dx in -2i32..=2 {
                    if dx * dx + dy * dy <= 5 && self.lm(x + dx, y + dy) == M::Empty && self.rng.chance(0.8) {
                        let l = Cell::new(M::Leaves, &mut self.rng);
                        self.lset(x + dx, y + dy, l);
                    }
                }
            }
            let p = Cell::new(M::Plant, &mut self.rng);
            self.lset(x, y, p);
            return;
        }
        let r = self.rng.f32();
        let dx = if r < 0.6 { 0 } else if r < 0.8 { -1 } else { 1 };
        if self.lm(x + dx, y - 1) == M::Empty {
            let mut stem = Cell::new(M::Plant, &mut self.rng);
            stem.shade = c.shade;
            self.lset(x, y, stem);
            let mut tip = c;
            tip.life = c.life - 1;
            tip.temp = 0;
            self.lset(x + dx, y - 1, tip);
            if c.life > 6 && self.rng.chance(0.1) {
                let side = self.rng.sign();
                if self.lm(x + side, y - 1) == M::Empty && side != dx {
                    let mut b = c;
                    b.life = c.life / 2;
                    b.temp = 0;
                    self.lset(x + side, y - 1, b);
                }
            }
        } else {
            let t = c.temp.saturating_add(1);
            if t > 30 {
                let p = Cell::new(M::Plant, &mut self.rng);
                self.lset(x, y, p);
            } else {
                self.cells[Self::li(x, y)].temp = t;
            }
        }
    }

    fn update_liquid(&mut self, x: i32, y: i32, c: Cell) {
        let d = c.mat.density();
        if c.temp > 0 {
            self.cells[Self::li(x, y)].temp = c.temp.saturating_sub(1);
        }
        if self.fall(x, y, c, d) {
            return;
        }
        let dir = self.rng.sign();
        for dx in [dir, -dir] {
            if self.can_enter(x + dx, y + 1, d) {
                self.swap(x, y, x + dx, y + 1);
                return;
            }
        }
        let disp = c.mat.dispersion();
        for dx in [dir, -dir] {
            let mut tx = x;
            for s in 1..=disp {
                let nx = x + dx * s;
                if !Self::inb(nx, y) {
                    break;
                }
                let k = self.cells[Self::li(nx, y)].mat.kind();
                if k != Kind::Empty && k != Kind::Gas {
                    break;
                }
                tx = nx;
                if self.lm(nx, y + 1) == M::Empty {
                    break;
                }
            }
            if tx != x {
                self.swap(x, y, tx, y);
                return;
            }
        }
    }

    fn update_gas(&mut self, x: i32, y: i32, c: Cell) {
        if c.mat == M::Gas {
            // Cave gas lingers and drifts slowly; it only goes away by burning.
            let r = self.rng.f32();
            let (dx, dy) = if r < 0.25 { (0, -1) } else if r < 0.5 { (self.rng.sign(), 0) } else if r < 0.6 { (0, 1) } else { return };
            if self.lm(x + dx, y + dy) == M::Empty {
                self.swap(x, y, x + dx, y + dy);
            }
            return;
        }
        let steam = c.mat == M::Steam;
        let decay = if self.rng.chance(if steam { 0.8 } else { 0.6 }) { 1 } else { 0 };
        let life = c.life.saturating_sub(decay);
        if life == 0 {
            if steam && self.rng.chance(0.25) {
                let w = Cell::new(M::Water, &mut self.rng);
                self.lset(x, y, w);
            } else {
                self.lset(x, y, Cell::EMPTY);
            }
            return;
        }
        self.cells[Self::li(x, y)].life = life;
        self.wake(x, y);

        if self.rng.chance(if steam { 0.9 } else { 0.75 }) {
            let up = self.lm(x, y - 1);
            if up == M::Empty || (up.kind() == Kind::Liquid && self.rng.chance(0.3)) {
                self.swap(x, y, x, y - 1);
                return;
            }
        }
        let dir = self.rng.sign();
        for dx in [dir, -dir] {
            if self.lm(x + dx, y - 1) == M::Empty {
                self.swap(x, y, x + dx, y - 1);
                return;
            }
        }
        if self.rng.chance(0.5) && self.lm(x + dir, y) == M::Empty {
            self.swap(x, y, x + dir, y);
        }
    }

    fn update_fire(&mut self, x: i32, y: i32, c: Cell) {
        self.fire_count += 1;
        self.wake(x, y);
        let falls = c.temp == FIRE_FLOWING;
        let fuel = c.temp >= FIRE_FUEL;

        for (dx, dy) in N4 {
            if falls && dy == 1 {
                continue; // burning oil floats on water
            }
            if self.lm(x + dx, y + dy) == M::Water && self.rng.chance(if fuel { 0.3 } else { 0.8 }) {
                let s = Cell::gas(M::Steam, &mut self.rng, 30, 70);
                self.lset(x, y, s);
                if self.rng.chance(0.15) {
                    let s = Cell::gas(M::Steam, &mut self.rng, 30, 70);
                    self.lset(x + dx, y + dy, s);
                }
                return;
            }
        }

        let (hmin, hmax) = if fuel { (10, 45) } else { (25, 70) };
        for (dx, dy) in N8 {
            let (nx, ny) = (x + dx, y + dy);
            if !Self::inb(nx, ny) {
                continue;
            }
            let ni = Self::li(nx, ny);
            let m = self.cells[ni].mat;
            match m {
                M::Ice | M::Snow => {
                    if self.rng.chance(0.03) {
                        let w = Cell::new(M::Water, &mut self.rng);
                        self.lset(nx, ny, w);
                    }
                }
                _ => {
                    if let Some(b) = m.burn() {
                        let t = self.cells[ni].temp.saturating_add(self.rng.range(hmin, hmax) as u8);
                        if t >= b.ignite {
                            self.ignite(nx, ny, m);
                        } else {
                            self.cells[ni].temp = t;
                        }
                    }
                }
            }
        }

        let mut life = c.life;
        if self.rng.chance(0.75) {
            life = life.saturating_sub(1);
        }
        if life == 0 {
            if self.rng.chance(if fuel { 0.6 } else { 0.2 }) {
                let s = Cell::gas(M::Smoke, &mut self.rng, 60, 140);
                self.lset(x, y, s);
            } else {
                self.lset(x, y, Cell::EMPTY);
            }
            return;
        }
        self.cells[Self::li(x, y)].life = life;

        if fuel {
            if self.lm(x, y - 1) == M::Empty {
                let r = self.rng.f32();
                if r < 0.15 {
                    let s = Cell::gas(M::Smoke, &mut self.rng, 80, 160);
                    self.lset(x, y - 1, s);
                } else if r < 0.45 {
                    let f = Cell::fire(&mut self.rng);
                    self.lset(x, y - 1, f);
                }
            }
            if falls {
                let below = self.lm(x, y + 1);
                if below == M::Empty || below.kind() == Kind::Gas {
                    self.swap(x, y, x, y + 1);
                }
            }
        } else if self.rng.chance(0.55) {
            let dx = self.rng.range(-1, 2);
            if self.lm(x + dx, y - 1) == M::Empty {
                self.swap(x, y, x + dx, y - 1);
            }
        }
    }

    /// Lava heats, ignites, melts and is quenched by water. Returns true if it solidified.
    fn update_lava(&mut self, x: i32, y: i32) -> bool {
        let mut busy = false;
        for (dx, dy) in N8 {
            let (nx, ny) = (x + dx, y + dy);
            if !Self::inb(nx, ny) {
                continue;
            }
            let ni = Self::li(nx, ny);
            let m = self.cells[ni].mat;
            match m {
                M::Water => {
                    let s = Cell::gas(M::Steam, &mut self.rng, 40, 90);
                    self.lset(nx, ny, s);
                    if self.rng.chance(0.5) {
                        let b = Cell::new(M::Basalt, &mut self.rng);
                        self.lset(x, y, b);
                        return true;
                    }
                }
                M::Ice | M::Snow => {
                    let w = Cell::new(M::Water, &mut self.rng);
                    self.lset(nx, ny, w);
                }
                M::Sand => {
                    busy = true;
                    if self.rng.chance(0.004) {
                        let g = Cell::new(M::Glass, &mut self.rng);
                        self.lset(nx, ny, g);
                    }
                }
                _ => {
                    if let Some(b) = m.burn() {
                        busy = true;
                        let t = self.cells[ni].temp.saturating_add(self.rng.range(30, 80) as u8);
                        if t >= b.ignite {
                            self.ignite(nx, ny, m);
                        } else {
                            self.cells[ni].temp = t;
                        }
                    }
                }
            }
        }
        if busy {
            self.wake(x, y);
        }
        if self.rng.chance(0.002) && self.lm(x, y - 1) == M::Empty {
            let f = Cell::fire(&mut self.rng);
            self.lset(x, y - 1, f);
        }
        false
    }

    /// Returns true if the acid cell was used up.
    fn update_acid(&mut self, x: i32, y: i32) -> bool {
        let i = Self::li(x, y);
        let mut found = false;
        for (dx, dy) in N4 {
            let (nx, ny) = (x + dx, y + dy);
            if !Self::inb(nx, ny) {
                continue;
            }
            let rate = self.cells[Self::li(nx, ny)].mat.acid_rate();
            if rate <= 0.0 {
                continue;
            }
            found = true;
            if self.rng.chance(0.025 * rate) {
                if self.rng.chance(0.3) {
                    let s = Cell::gas(M::Smoke, &mut self.rng, 20, 50);
                    self.lset(nx, ny, s);
                } else {
                    self.lset(nx, ny, Cell::EMPTY);
                }
                let life = self.cells[i].life.saturating_sub(1);
                if life == 0 {
                    self.lset(x, y, Cell::EMPTY);
                    return true;
                }
                self.cells[i].life = life;
                break;
            }
        }
        if found {
            self.wake(x, y);
        }
        false
    }

    /// Slow background processes on random cells, like Minecraft's random ticks.
    fn random_ticks(&mut self, n: usize) {
        for _ in 0..n {
            let x = self.rng.range(0, WW as i32);
            let y = self.rng.range(1, WH as i32 - 1);
            let c = self.cells[Self::li(x, y)];
            match c.mat {
                M::Dirt => {
                    if self.lm(x, y - 1) == M::Empty
                        && [(-1, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (1, 1)]
                            .iter()
                            .any(|(dx, dy)| self.lm(x + dx, y + dy) == M::Grass)
                    {
                        let mut g = Cell::new(M::Grass, &mut self.rng);
                        g.shade = c.shade;
                        self.lset(x, y, g);
                    }
                }
                M::Grass => {
                    if self.lm(x, y - 1).solid() {
                        let mut d = Cell::new(M::Dirt, &mut self.rng);
                        d.shade = c.shade;
                        self.lset(x, y, d);
                    }
                }
                _ => {}
            }
        }
    }
}
