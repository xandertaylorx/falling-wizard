//! The cellular grid, its update rules and its rendering into an RGBA buffer.
//!
//! The grid is split into CS x CS chunks. A chunk is only simulated when something
//! in or next to it changed during the previous step, so settled areas cost nothing.

use crate::cell::Cell;
use crate::materials::{Kind, Material};
use crate::noise::{fbm, hash3, Rng};

pub const W: usize = 320;
pub const H: usize = 180;
pub const CS: usize = 16;
pub const CW: usize = (W + CS - 1) / CS;
pub const CH: usize = (H + CS - 1) / CS;

const N4: [(i32, i32); 4] = [(0, 1), (-1, 0), (1, 0), (0, -1)];
const N8: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

const FIRE_PAL: [[u8; 3]; 5] = [
    [150, 24, 8],
    [222, 58, 12],
    [255, 122, 20],
    [255, 188, 48],
    [255, 240, 150],
];

pub struct World {
    pub cells: Vec<Cell>,
    bg: Vec<[u8; 3]>,
    pub clock: u8,
    pub frame: u64,
    active: Vec<bool>,
    next: Vec<bool>,
    pub rng: Rng,
    pub fire_count: usize,
    pub active_chunks: usize,
}

impl World {
    pub fn new(seed: u64) -> Self {
        let mut bg = vec![[0u8; 3]; W * H];
        for y in 0..H {
            for x in 0..W {
                let t = y as f32 / H as f32;
                let n = fbm(x as f32 * 0.07, y as f32 * 0.07, 3, 4242);
                let f = 0.7 + n * 0.6;
                let base = [16.0 + 12.0 * t, 13.0 + 6.0 * t, 26.0 - 4.0 * t];
                bg[y * W + x] = [
                    (base[0] * f) as u8,
                    (base[1] * f) as u8,
                    (base[2] * f) as u8,
                ];
            }
        }
        Self {
            cells: vec![Cell::EMPTY; W * H],
            bg,
            clock: 0,
            frame: 0,
            active: vec![true; CW * CH],
            next: vec![true; CW * CH],
            rng: Rng::new(seed),
            fire_count: 0,
            active_chunks: CW * CH,
        }
    }

    #[inline]
    pub fn in_bounds(x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H
    }

    #[inline]
    pub fn idx(x: i32, y: i32) -> usize {
        y as usize * W + x as usize
    }

    /// Material at (x, y); out of bounds behaves like solid stone.
    #[inline]
    pub fn mat(&self, x: i32, y: i32) -> Material {
        if Self::in_bounds(x, y) {
            self.cells[Self::idx(x, y)].mat
        } else {
            Material::Stone
        }
    }

    /// Mark the chunks around (x, y) for simulation next step.
    #[inline]
    pub fn wake(&mut self, x: i32, y: i32) {
        let cx0 = (x - 1).clamp(0, W as i32 - 1) as usize / CS;
        let cx1 = (x + 1).clamp(0, W as i32 - 1) as usize / CS;
        let cy0 = (y - 1).clamp(0, H as i32 - 1) as usize / CS;
        let cy1 = (y + 1).clamp(0, H as i32 - 1) as usize / CS;
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                self.next[cy * CW + cx] = true;
            }
        }
    }

    pub fn wake_all(&mut self) {
        self.next.fill(true);
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, mut c: Cell) {
        if !Self::in_bounds(x, y) {
            return;
        }
        c.clock = self.clock;
        self.cells[Self::idx(x, y)] = c;
        self.wake(x, y);
    }

    pub fn place(&mut self, x: i32, y: i32, m: Material) {
        let c = Cell::new(m, &mut self.rng);
        self.set(x, y, c);
    }

    #[inline]
    fn swap(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let a = Self::idx(x0, y0);
        let b = Self::idx(x1, y1);
        self.cells.swap(a, b);
        self.cells[a].clock = self.clock;
        self.cells[b].clock = self.clock;
        self.wake(x0, y0);
        self.wake(x1, y1);
    }

    /// One fixed simulation step: bottom-to-top, alternating horizontal scan direction.
    pub fn step(&mut self) {
        self.clock = self.clock.wrapping_add(1);
        self.frame += 1;
        std::mem::swap(&mut self.active, &mut self.next);
        self.next.fill(false);
        self.fire_count = 0;
        self.active_chunks = self.active.iter().filter(|a| **a).count();

        let ltr = self.frame % 2 == 0;
        for y in (0..H).rev() {
            let cy = y / CS;
            for k in 0..CW {
                let cx = if ltr { k } else { CW - 1 - k };
                if !self.active[cy * CW + cx] {
                    continue;
                }
                let x0 = cx * CS;
                let x1 = (x0 + CS).min(W);
                if ltr {
                    for x in x0..x1 {
                        self.update_cell(x as i32, y as i32);
                    }
                } else {
                    for x in (x0..x1).rev() {
                        self.update_cell(x as i32, y as i32);
                    }
                }
            }
        }
    }

    #[inline]
    fn update_cell(&mut self, x: i32, y: i32) {
        let i = Self::idx(x, y);
        let c = self.cells[i];
        if c.mat == Material::Empty || c.clock == self.clock {
            return;
        }
        match c.mat.kind() {
            Kind::Empty | Kind::Static => {}
            Kind::Powder => self.update_powder(x, y, c),
            Kind::Liquid => {
                if c.mat == Material::Acid && self.update_acid(x, y) {
                    return;
                }
                let c = self.cells[i];
                self.update_liquid(x, y, c);
            }
            Kind::Gas => self.update_smoke(x, y, c),
            Kind::Fire => self.update_fire(x, y, c),
        }
    }

    /// Can a cell of density `d` move into (x, y)?
    #[inline]
    fn can_enter(&mut self, x: i32, y: i32, d: u8) -> bool {
        if !Self::in_bounds(x, y) {
            return false;
        }
        let m = self.cells[Self::idx(x, y)].mat;
        match m.kind() {
            Kind::Empty | Kind::Gas | Kind::Fire => true,
            Kind::Liquid => m.density() < d && self.rng.chance(0.35),
            _ => false,
        }
    }

    /// Accelerating fall. Returns true if the cell moved.
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
            self.cells[Self::idx(x, cy)].vel = vel as u8;
            true
        } else {
            self.cells[Self::idx(x, y)].vel = 0;
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

    fn update_liquid(&mut self, x: i32, y: i32, c: Cell) {
        let d = c.mat.density();
        if c.temp > 0 {
            self.cells[Self::idx(x, y)].temp = c.temp.saturating_sub(1);
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
        // Spread sideways, stopping early at a drop so it pours over ledges.
        let disp = c.mat.dispersion();
        for dx in [dir, -dir] {
            let mut tx = x;
            for s in 1..=disp {
                let nx = x + dx * s;
                if !Self::in_bounds(nx, y) {
                    break;
                }
                let k = self.cells[Self::idx(nx, y)].mat.kind();
                if k != Kind::Empty && k != Kind::Gas {
                    break;
                }
                tx = nx;
                if self.mat(nx, y + 1) == Material::Empty {
                    break;
                }
            }
            if tx != x {
                self.swap(x, y, tx, y);
                return;
            }
        }
    }

    fn update_smoke(&mut self, x: i32, y: i32, c: Cell) {
        let decay = if y < 2 {
            4
        } else if self.rng.chance(0.6) {
            1
        } else {
            0
        };
        let life = c.life.saturating_sub(decay);
        if life == 0 {
            self.set(x, y, Cell::EMPTY);
            return;
        }
        self.cells[Self::idx(x, y)].life = life;
        self.wake(x, y);

        if self.rng.chance(0.75) {
            let up = self.mat(x, y - 1);
            if up == Material::Empty || (up.kind() == Kind::Liquid && self.rng.chance(0.3)) {
                self.swap(x, y, x, y - 1);
                return;
            }
        }
        let dir = self.rng.sign();
        for dx in [dir, -dir] {
            if self.mat(x + dx, y - 1) == Material::Empty {
                self.swap(x, y, x + dx, y - 1);
                return;
            }
        }
        if self.rng.chance(0.5) && self.mat(x + dir, y) == Material::Empty {
            self.swap(x, y, x + dir, y);
        }
    }

    fn update_fire(&mut self, x: i32, y: i32, c: Cell) {
        self.fire_count += 1;
        self.wake(x, y);
        let fuel = c.temp > 200;

        // Water puts fire out, sometimes boiling off as steam.
        for (dx, dy) in N4 {
            // Burning oil floats on water, so water below it doesn't douse it.
            if fuel && dy == 1 {
                continue;
            }
            if self.mat(x + dx, y + dy) == Material::Water && self.rng.chance(if fuel { 0.3 } else { 0.8 }) {
                let s = Cell::smoke(&mut self.rng, 30, 70);
                self.set(x, y, s);
                if self.rng.chance(0.12) {
                    let s = Cell::smoke(&mut self.rng, 30, 70);
                    self.set(x + dx, y + dy, s);
                }
                return;
            }
        }

        // Heat flammable neighbours; they ignite once hot enough.
        let (heat_min, heat_max) = if fuel { (10, 45) } else { (25, 70) };
        for (dx, dy) in N8 {
            let (nx, ny) = (x + dx, y + dy);
            if !Self::in_bounds(nx, ny) {
                continue;
            }
            let ni = Self::idx(nx, ny);
            if self.cells[ni].mat.flammable() {
                let h = self.rng.range(heat_min, heat_max) as u8;
                let t = self.cells[ni].temp.saturating_add(h);
                if t >= 100 {
                    let f = Cell::fire(true, &mut self.rng);
                    self.set(nx, ny, f);
                } else {
                    self.cells[ni].temp = t;
                }
            }
        }

        let mut life = c.life;
        if self.rng.chance(0.75) {
            life = life.saturating_sub(1);
        }
        if life == 0 {
            if self.rng.chance(if fuel { 0.65 } else { 0.2 }) {
                let s = Cell::smoke(&mut self.rng, 60, 140);
                self.set(x, y, s);
            } else {
                self.set(x, y, Cell::EMPTY);
            }
            return;
        }
        self.cells[Self::idx(x, y)].life = life;

        if fuel {
            // Burning oil throws up flames and lots of smoke.
            if self.mat(x, y - 1) == Material::Empty {
                let r = self.rng.f32();
                if r < 0.2 {
                    let s = Cell::smoke(&mut self.rng, 90, 180);
                    self.set(x, y - 1, s);
                } else if r < 0.5 {
                    let f = Cell::fire(false, &mut self.rng);
                    self.set(x, y - 1, f);
                }
            }
            let below = self.mat(x, y + 1);
            if below == Material::Empty || below.kind() == Kind::Gas {
                self.swap(x, y, x, y + 1);
            }
        } else if self.rng.chance(0.55) {
            let dx = self.rng.range(-1, 2);
            if self.mat(x + dx, y - 1) == Material::Empty {
                self.swap(x, y, x + dx, y - 1);
            }
        }
    }

    /// Returns true if the acid cell was used up.
    fn update_acid(&mut self, x: i32, y: i32) -> bool {
        let i = Self::idx(x, y);
        let mut found = false;
        for (dx, dy) in N4 {
            let (nx, ny) = (x + dx, y + dy);
            if !Self::in_bounds(nx, ny) || !self.cells[Self::idx(nx, ny)].mat.dissolvable() {
                continue;
            }
            found = true;
            if self.rng.chance(0.025) {
                if self.rng.chance(0.3) {
                    let s = Cell::smoke(&mut self.rng, 20, 50);
                    self.set(nx, ny, s);
                } else {
                    self.set(nx, ny, Cell::EMPTY);
                }
                let life = self.cells[i].life.saturating_sub(1);
                if life == 0 {
                    if self.rng.chance(0.4) {
                        let s = Cell::smoke(&mut self.rng, 20, 50);
                        self.set(x, y, s);
                    } else {
                        self.set(x, y, Cell::EMPTY);
                    }
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

    /// Write every cell's colour into an RGBA8 buffer of W*H pixels.
    pub fn render(&self, px: &mut [u8]) {
        let frame = self.frame;
        for y in 0..H {
            for x in 0..W {
                let i = y * W + x;
                let c = self.cells[i];
                let rgb = match c.mat {
                    Material::Empty => self.bg[i],
                    Material::Smoke => {
                        let a = (c.life as f32 / 90.0).min(1.0) * 0.85;
                        lerp3(self.bg[i], Material::Smoke.base_color(), a)
                    }
                    Material::Fire => {
                        let h = hash3(x as i32, y as i32, (frame / 2) as u32 ^ ((c.life as u32) << 8));
                        let heat = if c.temp > 200 { 0.3 } else { 0.0 }
                            + (c.life as f32 / 25.0).min(1.0) * 0.35;
                        let v = (h & 255) as f32 / 255.0 * 0.6 + heat;
                        FIRE_PAL[((v * 5.0) as usize).min(4)]
                    }
                    Material::Water => {
                        let tw = (hash3(x as i32, y as i32, (frame / 8) as u32) & 15) as u8;
                        shade(c.mat.base_color(), c.shade.saturating_add(tw), c.mat.variation())
                    }
                    Material::Oil if c.temp > 20 => {
                        let base = lerp3(c.mat.base_color(), [150, 60, 30], c.temp as f32 / 120.0);
                        shade(base, c.shade, c.mat.variation())
                    }
                    m => shade(m.base_color(), c.shade, m.variation()),
                };
                let o = i * 4;
                px[o] = rgb[0];
                px[o + 1] = rgb[1];
                px[o + 2] = rgb[2];
                px[o + 3] = 255;
            }
        }
    }
}

#[inline]
pub fn shade(base: [u8; 3], s: u8, amt: f32) -> [u8; 3] {
    let f = 1.0 + (s as f32 / 255.0 - 0.5) * amt;
    [
        (base[0] as f32 * f).min(255.0) as u8,
        (base[1] as f32 * f).min(255.0) as u8,
        (base[2] as f32 * f).min(255.0) as u8,
    ]
}

#[inline]
pub fn lerp3(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}
