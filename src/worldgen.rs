//! Deterministic, chunk-by-chunk world generation.
//!
//! Every cell is a pure function of (seed, x, y), so any chunk can be generated in
//! isolation. Multi-cell structures (trees, pools) are anchored on coarse grids and
//! stamped into every chunk they overlap.
//!
//! Layers, top to bottom: sky, surface (grass/dirt, sand or snow by biome), stone with
//! caves that widen with depth, then basalt and huge caverns with lava lakes.

use crate::cell::Cell;
use crate::materials::Material as M;
use crate::noise::{fbm, hash3, value_noise, Rng};
use crate::world::CHUNK;

const CU: usize = CHUNK as usize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Forest,
    Desert,
    Snow,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Forest => "Forest",
            Biome::Desert => "Desert",
            Biome::Snow => "Tundra",
        }
    }
}

pub struct Generator {
    s: u32,
}

impl Generator {
    pub fn new(seed: u64) -> Self {
        Self { s: (seed ^ (seed >> 32)) as u32 }
    }

    fn biome_value(&self, x: i32) -> f32 {
        fbm(x as f32 * 0.0021, 7.3, 2, self.s.wrapping_add(11))
    }

    pub fn biome(&self, x: i32) -> Biome {
        let b = self.biome_value(x);
        if b < 0.38 {
            Biome::Desert
        } else if b > 0.62 {
            Biome::Snow
        } else {
            Biome::Forest
        }
    }

    /// Y of the topmost ground cell in column x (y grows downwards).
    pub fn surface(&self, x: i32) -> i32 {
        let fx = x as f32;
        let b = self.biome_value(x);
        // Deserts are flatter, tundra rises into mountains; blend smoothly between them.
        let rough = 0.45 + smooth(0.3, 0.45, b) * 0.55;
        let mount = smooth(0.52, 0.72, b);
        let hills = (fbm(fx * 0.011, 0.5, 4, self.s) - 0.5) * 70.0 * rough;
        let peaks = (fbm(fx * 0.006, 3.1, 3, self.s.wrapping_add(5)) - 0.42).max(0.0) * 320.0 * mount;
        (hills - peaks) as i32
    }

    pub fn spawn(&self) -> (f32, f32) {
        (1.0, (self.surface(1).min(self.surface(2)).min(self.surface(3)) - 10) as f32)
    }

    pub fn chunk(&self, cx: i32, cy: i32) -> (Vec<Cell>, Vec<u8>) {
        let mut cells = vec![Cell::EMPTY; CU * CU];
        let mut wall = vec![0u8; CU * CU];
        let mut rng = Rng::new(hash3(cx, cy, self.s) as u64 | 1);
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        for lx in 0..CHUNK {
            let x = x0 + lx;
            let surf = self.surface(x);
            if y0 + CHUNK < surf - 2 {
                continue; // all sky
            }
            let biome = self.biome(x);
            let topsoil = 5 + (value_noise(x as f32 * 0.05, 0.0, self.s.wrapping_add(12)) * 8.0) as i32;
            for ly in 0..CHUNK {
                let y = y0 + ly;
                let i = ly as usize * CU + lx as usize;
                let (m, w, shade) = self.base(x, y, surf, biome, topsoil, &mut rng);
                if m != M::Empty {
                    let mut c = Cell::new(m, &mut rng);
                    if let Some(s) = shade {
                        c.shade = s;
                    }
                    cells[i] = c;
                }
                wall[i] = w;
            }
        }
        self.stamp_pools(cx, cy, &mut cells, &mut rng);
        self.stamp_ruins(cx, cy, &mut cells, &mut rng);
        self.stamp_trees(cx, cy, &mut cells, &mut rng);
        if y0 < -100 {
            self.stamp_sky(cx, cy, &mut cells, &mut rng);
        }
        (cells, wall)
    }

    /// Material, wall shade and optional cell shade for one terrain cell.
    fn base(&self, x: i32, y: i32, surf: i32, biome: Biome, topsoil: i32, rng: &mut Rng) -> (M, u8, Option<u8>) {
        let d = y - surf;
        if d < 0 {
            // Grass tufts and flowers just above the forest floor.
            if biome == Biome::Forest && d >= -2 {
                let h = hash3(x, 5, self.s);
                let tall = h % 9 == 0;
                if (d == -1 && h % 3 == 0) || (d == -2 && tall) {
                    return (M::Plant, 0, None);
                }
            }
            return (M::Empty, 0, None);
        }
        let (fx, fy) = (x as f32, y as f32);
        let s = self.s;

        // Caves: none right under the surface, wider and more frequent with depth.
        let n = fbm(fx * 0.024, fy * 0.05, 4, s.wrapping_add(1));
        let wall = 1 + (n * 200.0) as u8;
        let cave_thr = 0.71 - ((d - 12) as f32 / 500.0).clamp(0.0, 0.11);
        let mut open = d > 12 && n > cave_thr;
        if !open && d > 6 {
            let worm = (fbm(fx * 0.012, fy * 0.022, 3, s.wrapping_add(2)) - 0.5).abs();
            open = worm < 0.02 + (d as f32 / 3000.0).min(0.012);
        }
        if !open && d > 170 {
            open = fbm(fx * 0.007, fy * 0.012, 3, s.wrapping_add(3)) > 0.62;
        }
        if open {
            if d > 60 && d < 280 && fbm(fx * 0.02, fy * 0.03, 2, s.wrapping_add(20)) > 0.69 {
                return (M::Gas, wall, None);
            }
            return (M::Empty, wall, None);
        }

        // Surface layers by biome.
        match biome {
            Biome::Forest => {
                if d == 0 {
                    return (M::Grass, wall, None);
                }
                if d < topsoil {
                    return (M::Dirt, wall, None);
                }
            }
            Biome::Desert => {
                if d > 1 && d < 14 && fbm(fx * 0.04, fy * 0.08, 2, s.wrapping_add(21)) > 0.64 {
                    return (M::Quicksand, wall, None);
                }
                if d < topsoil * 3 {
                    return (M::Sand, wall, None);
                }
            }
            Biome::Snow => {
                if d < 4 {
                    return (M::Snow, wall, None);
                }
                if d < topsoil + 4 {
                    return (M::Dirt, wall, None);
                }
                if d < 40 && fbm(fx * 0.04, fy * 0.06, 2, s.wrapping_add(13)) > 0.66 {
                    return (M::Ice, wall, None);
                }
            }
        }

        // Pockets and ores.
        if d < 70 && fbm(fx * 0.045, fy * 0.06, 2, s.wrapping_add(14)) > 0.7 {
            return (M::Dirt, wall, None);
        }
        if fbm(fx * 0.05, fy * 0.05, 2, s.wrapping_add(4)) > 0.71 {
            return (M::Sand, wall, None);
        }
        if d > 20 && d < 260 && fbm(fx * 0.06, fy * 0.09, 2, s.wrapping_add(22)) > 0.72 {
            return (M::Gravel, wall, None);
        }
        if d > 15 && value_noise(fx * 0.13, fy * 0.13, s.wrapping_add(7)) > 0.87 {
            return (M::Coal, wall, None);
        }
        if d > 70 && value_noise(fx * 0.16, fy * 0.16, s.wrapping_add(8)) > 0.9 {
            return (M::Gold, wall, None);
        }
        if d > 50 && value_noise(fx * 0.08, fy * 0.08, s.wrapping_add(9)) > 0.94 {
            return (M::Gunpowder, wall, None);
        }

        // Strata: wavy bands reuse the cave noise for free.
        let band = ((fy * 0.55 + n * 9.0).sin() * 0.5 + 0.5) * 120.0;
        let shade = Some((band + rng.f32() * 80.0) as u8);
        let deep = 230.0 + (value_noise(fx * 0.02, 0.0, s.wrapping_add(10)) - 0.5) * 60.0;
        if d as f32 > deep {
            (M::Basalt, wall, shade)
        } else {
            (M::Stone, wall, shade)
        }
    }

    /// Liquid pools and sealed pockets on a coarse grid.
    fn stamp_pools(&self, cx: i32, cy: i32, cells: &mut [Cell], rng: &mut Rng) {
        const FG: i32 = 90;
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        for gy in (y0 - 30).div_euclid(FG)..=(y0 + CHUNK + 30).div_euclid(FG) {
            for gx in (x0 - 50).div_euclid(FG)..=(x0 + CHUNK + 50).div_euclid(FG) {
                let h = hash3(gx, gy, self.s ^ 0xABCD);
                let fx = gx * FG + 25 + (h % (FG as u32 - 50)) as i32;
                let fy = gy * FG + 15 + ((h >> 8) % (FG as u32 - 30)) as i32;
                let d = fy - self.surface(fx);
                if d < 25 {
                    continue;
                }
                let roll = (h >> 16) % 100;
                let fill = if d < 140 {
                    match roll {
                        0..=34 => M::Water,
                        35..=47 => M::Oil,
                        48..=55 => M::Acid,
                        56..=62 => M::Gunpowder,
                        _ => continue,
                    }
                } else if d < 320 {
                    match roll {
                        0..=29 => M::Lava,
                        30..=44 => M::Water,
                        45..=52 => M::Acid,
                        53..=60 => M::Oil,
                        61..=68 => M::Gunpowder,
                        _ => continue,
                    }
                } else {
                    match roll {
                        0..=54 => M::Lava,
                        55..=62 => M::Acid,
                        _ => continue,
                    }
                };
                let rx = 8 + ((h >> 24) % 18) as i32 + if fill == M::Lava { 6 } else { 0 };
                let ry = 4 + ((h >> 28) % 6) as i32;
                let sealed = matches!(fill, M::Oil | M::Gunpowder);
                let shell = if d > 220 { M::Basalt } else { M::Stone };
                self.bowl(cx, cy, cells, rng, (fx, fy, rx, ry), fill, shell, sealed);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn bowl(&self, cx: i32, cy: i32, cells: &mut [Cell], rng: &mut Rng, (bx, by, rx, ry): (i32, i32, i32, i32), fill: M, shell: M, sealed: bool) {
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        let fill_from = if sealed { by - ry + 2 } else { by - ry / 3 };
        let xa = (bx - rx - 4).max(x0);
        let xb = (bx + rx + 4).min(x0 + CHUNK - 1);
        let ya = (by - ry - 4).max(y0);
        let yb = (by + ry + 4).min(y0 + CHUNK - 1);
        for y in ya..=yb {
            for x in xa..=xb {
                let (dx, dy) = ((x - bx) as f32 / rx as f32, (y - by) as f32 / ry as f32);
                let e = dx * dx + dy * dy;
                let i = ((y - y0) as usize) * CU + (x - x0) as usize;
                if e <= 1.0 {
                    cells[i] = if y >= fill_from { Cell::new(fill, rng) } else { Cell::EMPTY };
                } else if e <= 1.7 && (sealed || y >= by) {
                    let mut c = Cell::new(shell, rng);
                    c.shade = 50 + (rng.f32() * 60.0) as u8;
                    cells[i] = c;
                }
            }
        }
    }

    /// Small brick ruins underground holding a chest or a wand altar.
    fn stamp_ruins(&self, cx: i32, cy: i32, cells: &mut [Cell], rng: &mut Rng) {
        const RG: i32 = 130;
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        for gy in (y0 - 20).div_euclid(RG)..=(y0 + CHUNK + 20).div_euclid(RG) {
            for gx in (x0 - 30).div_euclid(RG)..=(x0 + CHUNK + 30).div_euclid(RG) {
                let h = hash3(gx, gy, self.s ^ 0x7777);
                if h % 100 >= 50 {
                    continue;
                }
                let rx = gx * RG + 20 + (h >> 8) as i32 % (RG - 40);
                let ry = gy * RG + 20 + (h >> 16) as i32 % (RG - 40);
                if ry - self.surface(rx) < 22 {
                    continue;
                }
                let (w, hgt) = (14 + (h >> 4) as i32 % 8, 8);
                let altar = (h >> 24) % 4 == 0;
                for y in ry - hgt..=ry {
                    for x in rx - w / 2..=rx + w / 2 {
                        if x < x0 || y < y0 || x >= x0 + CHUNK || y >= y0 + CHUNK {
                            continue;
                        }
                        let i = ((y - y0) as usize) * CU + (x - x0) as usize;
                        let edge = y == ry - hgt || y == ry || x == rx - w / 2 || x == rx + w / 2;
                        let door = (x == rx - w / 2 || x == rx + w / 2) && y > ry - 4 && y < ry;
                        let item = if altar { (x - rx).abs() <= 1 && y >= ry - 4 && y < ry } else { (x - rx).abs() <= 2 && y >= ry - 3 && y < ry };
                        let m = if item {
                            if altar { M::Altar } else { M::Chest }
                        } else if edge && !door {
                            M::Brick
                        } else {
                            M::Empty
                        };
                        let mut c = Cell::new(m, rng);
                        if m == M::Brick {
                            c.shade = if (y + (x / 3) * 2) % 3 == 0 { 70 } else { 150 + (hash3(x, y, 9) % 60) as u8 };
                        }
                        if m == M::Chest {
                            c.shade = if y == ry - 2 || (x - rx).abs() == 2 { 60 } else { 170 };
                        }
                        cells[i] = c;
                    }
                }
            }
        }
    }

    /// Clouds and floating islands; the higher you go, the stranger they get.
    fn stamp_sky(&self, cx: i32, cy: i32, cells: &mut [Cell], rng: &mut Rng) {
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        let put = |x: i32, y: i32, m: M, shade: Option<u8>, cells: &mut [Cell], rng: &mut Rng| {
            if x < x0 || y < y0 || x >= x0 + CHUNK || y >= y0 + CHUNK {
                return;
            }
            let i = ((y - y0) as usize) * CU + (x - x0) as usize;
            if cells[i].mat == M::Empty || m != M::Empty {
                let mut c = Cell::new(m, rng);
                if let Some(s) = shade {
                    c.shade = s;
                }
                cells[i] = c;
            }
        };
        // Cloud platforms.
        const CG: i32 = 90;
        for gy in (y0 - 10).div_euclid(CG)..=(y0 + CHUNK + 10).div_euclid(CG) {
            for gx in (x0 - 30).div_euclid(CG)..=(x0 + CHUNK + 30).div_euclid(CG) {
                let h = hash3(gx, gy, self.s ^ 0xC10D);
                let (bx, by) = (gx * CG + (h % CG as u32) as i32, gy * CG + ((h >> 8) % CG as u32) as i32);
                if by > self.surface(bx) - 110 || by < -520 || h >> 28 < 7 {
                    continue;
                }
                let (rx, ry) = (10 + (h >> 16) as i32 % 14, 3 + (h >> 20) as i32 % 3);
                for y in by - ry..=by + ry {
                    for x in bx - rx..=bx + rx {
                        let e = ((x - bx) as f32 / rx as f32).powi(2) + ((y - by) as f32 / ry as f32).powi(2);
                        if e < 1.0 - value_noise(x as f32 * 0.3, y as f32 * 0.3, 5) * 0.4 {
                            put(x, y, M::Cloud, None, cells, rng);
                        }
                    }
                }
            }
        }
        // Floating islands.
        const IG: i32 = 150;
        for gy in (y0 - 40).div_euclid(IG)..=(y0 + CHUNK + 40).div_euclid(IG) {
            for gx in (x0 - 50).div_euclid(IG)..=(x0 + CHUNK + 50).div_euclid(IG) {
                let h = hash3(gx, gy, self.s ^ 0x15EE);
                let (ix, iy) = (gx * IG + 30 + (h % (IG as u32 - 60)) as i32, gy * IG + ((h >> 8) % (IG as u32 - 40)) as i32);
                if iy > -260 || h >> 29 < 2 {
                    continue;
                }
                let astral = iy < -520;
                let (rx, depth) = (16 + (h >> 16) as i32 % 26, 10 + (h >> 22) as i32 % 16);
                for x in ix - rx..=ix + rx {
                    let t = ((x - ix) as f32 / rx as f32).abs();
                    let top = iy - (value_noise(x as f32 * 0.15, 1.0, h) * 3.0) as i32;
                    let bottom = iy + ((1.0 - t * t).max(0.0).powf(0.7) * depth as f32) as i32 - (value_noise(x as f32 * 0.4, 2.0, h) * 4.0) as i32;
                    for y in top..=bottom {
                        let k = y - top;
                        let m = if astral {
                            if k == 0 {
                                M::AlienGrass
                            } else if value_noise(x as f32 * 0.25, y as f32 * 0.25, h ^ 3) > 0.78 {
                                M::Crystal
                            } else {
                                M::MoonRock
                            }
                        } else if k == 0 {
                            M::Grass
                        } else if k < 4 {
                            M::Dirt
                        } else {
                            M::Stone
                        };
                        put(x, y, m, None, cells, rng);
                    }
                    let deco = hash3(x, top, h);
                    if astral {
                        if deco % 5 == 0 {
                            for k in 1..=(1 + deco as i32 % 3) {
                                put(x, top - k, M::Mushroom, None, cells, rng);
                            }
                        } else if deco % 11 == 0 {
                            for k in 1..=(3 + deco as i32 % 5) {
                                put(x, top - k, M::Crystal, None, cells, rng);
                            }
                        }
                    } else if deco % 3 == 0 {
                        put(x, top - 1, M::Plant, None, cells, rng);
                    }
                }
                // A ruin with a chest on some islands.
                if h % 3 == 0 {
                    for x in ix - 2..=ix + 2 {
                        for y in iy - 4..iy {
                            put(x, y, M::Chest, Some(if y == iy - 2 || (x - ix).abs() == 2 { 60 } else { 170 }), cells, rng);
                        }
                    }
                }
            }
        }
    }

    fn stamp_trees(&self, cx: i32, cy: i32, cells: &mut [Cell], rng: &mut Rng) {
        const SLOT: i32 = 11;
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        let mut put = |x: i32, y: i32, m: M, shade: u8, over: bool, cells: &mut [Cell]| {
            if x < x0 || y < y0 || x >= x0 + CHUNK || y >= y0 + CHUNK {
                return;
            }
            let i = ((y - y0) as usize) * CU + (x - x0) as usize;
            if over || cells[i].mat == M::Empty || cells[i].mat == M::Plant {
                let mut c = Cell::new(m, rng);
                c.shade = shade;
                cells[i] = c;
            }
        };
        for slot in (x0 - 12).div_euclid(SLOT)..=(x0 + CHUNK + 12).div_euclid(SLOT) {
            let h = hash3(slot, 77, self.s);
            if h % 100 >= 55 {
                continue;
            }
            let tx = slot * SLOT + ((h >> 8) % (SLOT as u32 - 3)) as i32;
            if tx.abs() < 8 {
                continue; // keep spawn clear
            }
            let (s0, s1) = (self.surface(tx), self.surface(tx + 1));
            if (s0 - s1).abs() > 2 || (self.surface(tx - 3) - self.surface(tx + 4)).abs() > 6 {
                continue;
            }
            let base = s0.max(s1);
            let height = 9 + ((h >> 16) % 11) as i32;
            let top = base - height;
            if top - 10 > y0 + CHUNK || base < y0 {
                continue;
            }
            let hv = |a: i32, b: i32| hash3(a, b, h);
            match self.biome(tx) {
                Biome::Forest => {
                    for y in top..base {
                        put(tx, y, M::Wood, (hv(tx, y) % 90) as u8 + 60, true, cells);
                        put(tx + 1, y, M::Wood, (hv(tx + 1, y) % 90) as u8 + 20, true, cells);
                    }
                    let (rx, ry) = (5 + (h >> 20) as i32 % 3, 4 + (h >> 22) as i32 % 2);
                    for dy in -ry - 1..=ry {
                        for dx in -rx - 1..=rx + 1 {
                            let e = (dx as f32 / rx as f32).powi(2) + (dy as f32 / ry as f32).powi(2);
                            let jag = (hv(dx, dy) % 100) as f32 / 100.0 * 0.35;
                            if e <= 1.0 + jag - 0.15 {
                                put(tx + dx, top + dy, M::Leaves, (hv(dx * 3, dy) % 256) as u8, false, cells);
                            }
                        }
                    }
                }
                Biome::Snow => {
                    for y in top..base {
                        put(tx, y, M::Wood, (hv(tx, y) % 60) as u8 + 30, true, cells);
                    }
                    for k in 0..height - 2 {
                        let w = 1 + (k % 4 + k / 3) / 2;
                        for dx in -w..=w {
                            put(tx + dx, top - 2 + k, M::Leaves, (hv(dx, k) % 70) as u8, false, cells);
                        }
                    }
                }
                Biome::Desert => {
                    if h % 3 != 0 {
                        continue;
                    }
                    let ch = 4 + (h >> 12) as i32 % 5;
                    for y in base - ch..base {
                        put(tx, y, M::Plant, 120, true, cells);
                        put(tx + 1, y, M::Plant, 70, true, cells);
                    }
                    let side = if h & 1 == 0 { -1 } else { 2 };
                    let ay = base - ch / 2 - 1;
                    put(tx + side, ay, M::Plant, 100, true, cells);
                    put(tx + side + side.signum(), ay, M::Plant, 100, true, cells);
                    put(tx + side + side.signum(), ay - 1, M::Plant, 100, true, cells);
                    put(tx + side + side.signum(), ay - 2, M::Plant, 100, true, cells);
                }
            }
        }
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
