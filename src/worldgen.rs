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
use crate::planets::{Flora, Kind as PKind, Planet};
use crate::world::CHUNK;

const CU: usize = CHUNK as usize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Forest,
    Desert,
    Snow,
    Ocean,
    Plains,
    Jungle,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Forest => "Forest",
            Biome::Desert => "Desert",
            Biome::Snow => "Tundra",
            Biome::Ocean => "Ocean",
            Biome::Plains => "Plains",
            Biome::Jungle => "Jungle",
        }
    }

    /// Grassy biomes share topsoil, tufts and night creatures.
    pub fn green(self) -> bool {
        matches!(self, Biome::Forest | Biome::Plains | Biome::Jungle)
    }
}

/// Sea level: columns whose ground lies below it are flooded.
pub const SEA: i32 = 14;

const VG: i32 = 700;
const GG: i32 = 260;
const VOLC_H: i32 = 48;
const VOLC_W: i32 = 64;

pub struct Generator {
    s: u32,
    pub planet: Planet,
}

/// Where every planet's arrival portal stands.
pub const PORTAL_HOME: i32 = 70;
const PG: i32 = 520;

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Oak,
    Jungle,
    Pine,
    Cactus,
    Shroom,
    Crystal,
    /// Cherry blossom: a crooked trunk under a pink cloud.
    Blossom,
    /// A rare ancient giant with flared roots and a layered canopy.
    Great,
    None,
}

impl Generator {
    pub fn new(seed: u64) -> Self {
        Self::with_planet(seed, Planet::earth())
    }

    pub fn with_planet(seed: u64, planet: Planet) -> Self {
        let seed = planet.world_seed(seed);
        Self { s: (seed ^ (seed >> 32)) as u32, planet }
    }

    pub fn seed_u32(&self) -> u32 {
        self.s
    }

    fn alien(&self) -> bool {
        !self.planet.is_earth()
    }

    pub fn sea_level(&self) -> i32 {
        if self.alien() {
            if self.planet.sea.is_some() { self.planet.sea_level } else { i32::MAX }
        } else {
            SEA
        }
    }

    /// Portal structures with x in [x0, x1]: (centre x, platform y, style).
    pub fn portals_near(&self, x0: i32, x1: i32) -> Vec<(i32, i32, u32)> {
        let mut out = Vec::new();
        if (x0..=x1).contains(&PORTAL_HOME) {
            out.push(self.portal_site(PORTAL_HOME, hash3(7, 7, self.s)));
        }
        for g in x0.div_euclid(PG) - 1..=x1.div_euclid(PG) + 1 {
            let h = hash3(g, 61, self.s);
            if h % 100 >= 60 {
                continue;
            }
            let px = g * PG + 120 + (h >> 8) as i32 % (PG - 240);
            if px < x0 || px > x1 || (px - PORTAL_HOME).abs() < 200 || self.volcano_near(px).is_some() || (!self.alien() && self.ocean(px) > 0.3) {
                continue;
            }
            out.push(self.portal_site(px, h));
        }
        out
    }

    fn portal_site(&self, px: i32, h: u32) -> (i32, i32, u32) {
        let top = (px - 12..=px + 12).map(|x| self.surface(x)).min().unwrap_or(0);
        let base = top.min(self.sea_level() - 1);
        (px, base, (h >> 12) % 4)
    }

    /// Bounding box around a structure's opening: (x0, y0, x1, y1), inclusive.
    pub fn portal_interior(px: i32, base: i32) -> (i32, i32, i32, i32) {
        (px - 8, base - 30, px + 8, base - 2)
    }

    /// Whether (dx, u) lies inside a structure's opening, where the portal surface goes.
    pub fn portal_fill(dx: i32, u: i32, style: u32) -> bool {
        let ax = dx.abs();
        if u > -2 {
            return false;
        }
        match style {
            0 => {
                (ax <= 4 && u >= -18) || (u < -18 && u >= -28 && ((dx * dx) as f32 + ((u + 18) as f32 * 1.15).powi(2)).sqrt() < 5.0)
            }
            1 => (dx as f32 / 7.5).powi(2) + ((u + 10) as f32 / 10.5).powi(2) <= 0.78,
            2 => ax <= 4 && u >= -24,
            _ => ax <= 4 && u >= -18,
        }
    }

    /// What a portal structure puts at offset (dx, u) from its platform centre (u < 0 is up).
    fn portal_cell(dx: i32, u: i32, style: u32) -> Option<M> {
        let ax = dx.abs();
        if ax <= 12 && (-1..=1).contains(&u) {
            if u == -1 && ax > 10 {
                return Some(M::Empty);
            }
            if u == -1 && ax == 10 {
                return Some(M::Rune);
            }
            return Some(M::PortalStone);
        }
        let frame = match style {
            0 => {
                // Stone arch with gold trim and rune-lit pillars.
                if (5..=7).contains(&ax) && (-19..=-2).contains(&u) {
                    Some(if ax == 6 && u % 3 == 0 { M::Rune } else { M::PortalStone })
                } else if (-28..=-18).contains(&u) {
                    let d = ((dx * dx) as f32 + ((u + 18) as f32 * 1.15).powi(2)).sqrt();
                    if d >= 5.0 && d <= 7.4 {
                        Some(M::PortalStone)
                    } else if d > 7.4 && d <= 8.4 {
                        Some(M::Gold)
                    } else if dx == 0 && (d - 8.0).abs() < 2.0 {
                        Some(M::Rune)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            1 => {
                // Crystal ring held by rune nodes.
                let e = (dx as f32 / 7.5).powi(2) + ((u + 10) as f32 / 10.5).powi(2);
                if e > 0.78 && e <= 1.08 {
                    let node = (dx == 0 && (u == -21 || u == 0)) || (ax >= 7 && (u + 10).abs() <= 1);
                    Some(if node { M::Rune } else { M::Crystal })
                } else if ax <= 3 && (u == -2) && e > 0.6 {
                    Some(M::Brick)
                } else {
                    None
                }
            }
            2 => {
                // Twin basalt monoliths under a lintel, with glyphs.
                if (5..=8).contains(&ax) && (-24..=-2).contains(&u) {
                    let glyph = ax == 6 && (u % 4 == 0 || u % 4 == -1) && hash3(dx, u, 3) % 2 == 0;
                    Some(if glyph { M::Rune } else { M::Basalt })
                } else if ax <= 10 && (-27..=-25).contains(&u) {
                    Some(if u == -26 && ax % 3 == 0 { M::Rune } else { M::Basalt })
                } else if ax <= 1 && u == -30 {
                    Some(M::Rune)
                } else {
                    None
                }
            }
            _ => {
                // Brick temple with a stepped roof and a rune eye.
                if (5..=6).contains(&ax) && (-18..=-2).contains(&u) {
                    Some(if u % 5 == 0 { M::Gold } else { M::Brick })
                } else if (-27..=-19).contains(&u) && ax <= 10 - (-19 - u) {
                    if u == -19 {
                        Some(M::Gold)
                    } else if u == -23 && ax <= 1 {
                        Some(M::Rune)
                    } else {
                        Some(M::Brick)
                    }
                } else {
                    None
                }
            }
        };
        if frame.is_some() {
            return frame;
        }
        if ax <= 12 && (-32..=-2).contains(&u) {
            return Some(M::Empty); // clear the plaza
        }
        None
    }

    fn stamp_portals(&self, cx: i32, cy: i32, cells: &mut [Cell], rng: &mut Rng) {
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        for (px, base, style) in self.portals_near(x0 - 14, x0 + CHUNK + 14) {
            for x in (px - 12).max(x0)..=(px + 12).min(x0 + CHUNK - 1) {
                let ground = self.surface(x);
                for y in (base - 32).max(y0)..=(ground.max(base + 1)).min(y0 + CHUNK - 1) {
                    let u = y - base;
                    let m = if u > 1 { Some(M::PortalStone) } else { Self::portal_cell(x - px, u, style) };
                    if let Some(m) = m {
                        let i = ((y - y0) as usize) * CU + (x - x0) as usize;
                        let floor = u >= -1 && m != M::Empty;
                        if floor {
                            // The platform and foundation are solid ground.
                            let mut c = Cell::new(m, rng);
                            if m == M::PortalStone {
                                c.shade = if (x + y * 3) % 7 == 0 { 70 } else { 140 + (hash3(x, y, 4) % 60) as u8 };
                            }
                            cells[i] = c;
                        } else {
                            // The arch itself stands in the background: you walk through it.
                            cells[i] = Cell::EMPTY;
                            cells[i].back = m;
                        }
                    }
                }
            }
        }
    }

    /// Terrain surface on another planet.
    fn alien_surface(&self, x: i32) -> i32 {
        let p = &self.planet;
        let fx = x as f32;
        let hills = (fbm(fx * 0.012, 0.5, 4, self.s) - 0.5) * 70.0 * p.amp;
        let peaks = (fbm(fx * 0.006, 3.1, 3, self.s.wrapping_add(5)) - 0.45).max(0.0) * 300.0 * p.peaks;
        let crater = if matches!(p.kind, PKind::Moon | PKind::Crystal) {
            let c = (value_noise(fx * 0.02, 9.0, self.s.wrapping_add(6)) - 0.62).max(0.0);
            c * c * 900.0
        } else {
            0.0
        };
        (hills - peaks + crater) as i32
    }

    fn alien_base(&self, x: i32, y: i32, surf: i32, topsoil: i32, rng: &mut Rng) -> (M, u8, Option<u8>) {
        let p = &self.planet;
        let d = y - surf;
        if d < 0 {
            if let Some(sea) = p.sea {
                if y >= p.sea_level {
                    return (sea, 0, None);
                }
            }
            if matches!(p.flora, Flora::Trees | Flora::Shrooms) && d == -1 && hash3(x, 5, self.s) % 3 == 0 {
                return (M::Plant, 0, None);
            }
            return (M::Empty, 0, None);
        }
        let (fx, fy) = (x as f32, y as f32);
        let s = self.s;
        let n = fbm(fx * 0.024, fy * 0.05, 4, s.wrapping_add(1));
        let wall = 1 + (n * 200.0) as u8;
        let mut open = d > 12 && n > 0.71 - ((d - 12) as f32 / 500.0).clamp(0.0, 0.11);
        if !open && d > 6 {
            open = (fbm(fx * 0.012, fy * 0.022, 3, s.wrapping_add(2)) - 0.5).abs() < 0.022;
        }
        if open {
            return (M::Empty, wall, None);
        }
        if d == 0 {
            return (p.top, wall, None);
        }
        if d < topsoil {
            return (p.soil, wall, None);
        }
        if p.kind == PKind::Crystal && value_noise(fx * 0.12, fy * 0.12, s.wrapping_add(7)) > 0.8 {
            return (M::Crystal, wall, None);
        }
        if d > 40 && value_noise(fx * 0.16, fy * 0.16, s.wrapping_add(8)) > 0.88 {
            return (M::Gold, wall, None);
        }
        if p.kind == PKind::Lava && d > 30 && value_noise(fx * 0.05, fy * 0.05, s.wrapping_add(9)) > 0.8 {
            return (M::Lava, wall, None);
        }
        let band = ((fy * 0.55 + n * 9.0).sin() * 0.5 + 0.5) * 120.0;
        (if d > 220 { M::Basalt } else { p.rock }, wall, Some((band + rng.f32() * 80.0) as u8))
    }

    fn biome_value(&self, x: i32) -> f32 {
        fbm(x as f32 * 0.0021, 7.3, 2, self.s.wrapping_add(11))
    }

    /// 1 in the open ocean, 0 inland.
    pub fn ocean(&self, x: i32) -> f32 {
        if self.alien() {
            return 0.0;
        }
        let c = fbm(x as f32 * 0.0011, 11.0, 3, self.s.wrapping_add(40));
        // Keep the area around spawn on land, fading out smoothly.
        let k = smooth(420.0, 260.0, x.abs() as f32);
        let c = c + (c.max(0.45) - c) * k;
        smooth(0.40, 0.31, c)
    }

    pub fn biome(&self, x: i32) -> Biome {
        if self.alien() {
            return match self.planet.kind {
                PKind::Dune => Biome::Desert,
                PKind::Ice => Biome::Snow,
                PKind::Jungle => Biome::Jungle,
                PKind::Ocean if self.surface(x) >= self.planet.sea_level => Biome::Ocean,
                _ => Biome::Plains,
            };
        }
        if self.ocean(x) > 0.5 {
            return Biome::Ocean;
        }
        let b = self.biome_value(x);
        if b < 0.38 {
            Biome::Desert
        } else if b > 0.62 {
            Biome::Snow
        } else {
            let h = fbm(x as f32 * 0.0017, 3.3, 2, self.s.wrapping_add(41));
            if h > 0.6 {
                Biome::Jungle
            } else if h < 0.4 {
                Biome::Plains
            } else {
                Biome::Forest
            }
        }
    }

    /// Y of the topmost ground cell in column x (y grows downwards).
    pub fn surface(&self, x: i32) -> i32 {
        if self.alien() {
            return self.alien_surface(x);
        }
        let fx = x as f32;
        let b = self.biome_value(x);
        // Deserts are flatter, tundra rises into mountains; blend smoothly between them.
        let rough = 0.45 + smooth(0.3, 0.45, b) * 0.55;
        let mount = smooth(0.52, 0.72, b);
        let hills = (fbm(fx * 0.011, 0.5, 4, self.s) - 0.5) * 70.0 * rough;
        let peaks = (fbm(fx * 0.006, 3.1, 3, self.s.wrapping_add(5)) - 0.42).max(0.0) * 320.0 * mount;
        let land = hills - peaks;
        // Ocean basins: blend down to a sandy sea floor.
        let o = self.ocean(x);
        let floor = SEA as f32 + 24.0 + fbm(fx * 0.02, 5.0, 2, self.s.wrapping_add(42)) * 22.0;
        (land + (floor - land) * o) as i32
    }

    /// Ruin in grid cell (gx, gy): (centre x, floor y, half-width, has altar).
    pub fn ruin_at(&self, gx: i32, gy: i32) -> Option<(i32, i32, i32, bool)> {
        const RG: i32 = 130;
        let h = hash3(gx, gy, self.s ^ 0x7777);
        if h % 100 >= 50 {
            return None;
        }
        let rx = gx * RG + 20 + (h >> 8) as i32 % (RG - 40);
        let ry = gy * RG + 20 + (h >> 16) as i32 % (RG - 40);
        if ry - self.surface(rx) < 22 {
            return None;
        }
        Some((rx, ry, (14 + (h >> 4) as i32 % 8) / 2, (h >> 24) % 4 == 0))
    }

    /// Floating island in grid cell (gx, gy): (x, top y, half-width, depth, astral, hash).
    pub fn island_at(&self, gx: i32, gy: i32) -> Option<(i32, i32, i32, i32, bool, u32)> {
        const IG: i32 = 150;
        let h = hash3(gx, gy, self.s ^ 0x15EE);
        let (ix, iy) = (gx * IG + 30 + (h % (IG as u32 - 60)) as i32, gy * IG + ((h >> 8) % (IG as u32 - 40)) as i32);
        if iy > -260 || h >> 29 < 2 {
            return None;
        }
        Some((ix, iy, 16 + (h >> 16) as i32 % 26, 10 + (h >> 22) as i32 % 16, iy < -520, h))
    }

    /// Volcanoes and geysers whose x lies in [x0, x1]: (is_volcano, x, vent top y).
    pub fn features_near(&self, x0: i32, x1: i32) -> Vec<(bool, i32, i32)> {
        let mut out = Vec::new();
        for g in x0.div_euclid(VG) - 1..=x1.div_euclid(VG) + 1 {
            if let Some((vx, base)) = self.volcano_cell(g) {
                if vx >= x0 && vx <= x1 {
                    out.push((true, vx, base - VOLC_H + 6));
                }
            }
        }
        for g in x0.div_euclid(GG) - 1..=x1.div_euclid(GG) + 1 {
            let h = hash3(g, 92, self.s);
            if h % 100 >= 45 {
                continue;
            }
            let gx = g * GG + 40 + (h >> 8) as i32 % (GG - 80);
            if gx >= x0 && gx <= x1 && self.volcano_near(gx).is_none() {
                out.push((false, gx, self.surface(gx)));
            }
        }
        out
    }

    fn volcano_cell(&self, g: i32) -> Option<(i32, i32)> {
        let h = hash3(g, 91, self.s);
        if h % 100 >= 55 {
            return None;
        }
        let vx = g * VG + 150 + (h >> 8) as i32 % (VG - 300);
        if vx.abs() < 200 {
            return None; // not on top of the spawn
        }
        Some((vx, self.surface(vx)))
    }

    /// The volcano whose slopes cover column x, if any: (centre x, base y).
    fn volcano_near(&self, x: i32) -> Option<(i32, i32)> {
        let g = x.div_euclid(VG);
        (g - 1..=g + 1).filter_map(|g| self.volcano_cell(g)).find(|(vx, _)| (x - vx).abs() <= VOLC_W + 2)
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
            if y0 + CHUNK < surf.min(SEA) - 30 {
                continue; // all sky
            }
            let biome = self.biome(x);
            let topsoil = 5 + (value_noise(x as f32 * 0.05, 0.0, self.s.wrapping_add(12)) * 8.0) as i32;
            for ly in 0..CHUNK {
                let y = y0 + ly;
                let i = ly as usize * CU + lx as usize;
                let (m, w, shade) = self.base(x, y, surf, biome, topsoil, &mut rng);
                if m == M::Mushroom || m == M::Flower {
                    cells[i].back = m;
                } else if m == M::Plant {
                    cells[i].back = M::Plant;
                    if y >= self.sea_level() && y < surf {
                        cells[i].mat = self.planet.sea.unwrap_or(M::Water);
                    }
                } else if m != M::Empty {
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
        self.stamp_vents(cx, cy, &mut cells, &mut rng);
        self.stamp_portals(cx, cy, &mut cells, &mut rng);
        self.stamp_ruins(cx, cy, &mut cells, &mut rng);
        self.stamp_landmarks(cx, cy, &mut cells, &mut rng);
        self.stamp_trees(cx, cy, &mut cells, &mut rng);
        if y0 < -100 {
            self.stamp_sky(cx, cy, &mut cells, &mut rng);
        }
        (cells, wall)
    }

    /// Material, wall shade and optional cell shade for one terrain cell.
    fn base(&self, x: i32, y: i32, surf: i32, biome: Biome, topsoil: i32, rng: &mut Rng) -> (M, u8, Option<u8>) {
        if self.alien() {
            return self.alien_base(x, y, surf, topsoil, rng);
        }
        let d = y - surf;
        if d < 0 {
            let underwater = y >= SEA;
            let h = hash3(x, 5, self.s);
            if underwater {
                // Kelp sways on the sea floor.
                if biome == Biome::Ocean && h % 4 == 0 && -d <= 2 + (h >> 8) as i32 % 9 && y > SEA + 2 {
                    return (M::Plant, 0, None);
                }
                return (M::Water, 0, None);
            }
            // Grass tufts and flowers just above grassy ground; meadows burst with colour.
            if biome.green() && d >= -3 {
                let meadow = fbm(x as f32 * 0.008, 3.0, 2, self.s.wrapping_add(50));
                let bloom = meadow > 0.56 || biome == Biome::Plains && meadow > 0.48;
                let tall = h % if bloom { 4 } else { 9 } == 0;
                let dense = if biome == Biome::Plains || bloom { h % 5 != 0 } else { h % 3 == 0 };
                if d == -1 && bloom && h % 3 == 0 || d == -2 && bloom && h % 7 == 0 {
                    return (M::Flower, 0, None);
                }
                if (d == -1 && dense) || (d == -2 && tall) || (d == -3 && bloom && h % 13 == 0) {
                    return (M::Plant, 0, None);
                }
            }
            return (M::Empty, 0, None);
        }
        let (fx, fy) = (x as f32, y as f32);
        let s = self.s;

        let n = fbm(fx * 0.024, fy * 0.05, 4, s.wrapping_add(1));
        let wall = 1 + (n * 200.0) as u8;
        let open = self.cave_open(x, y, d, n);
        // Deep below, caverns get a character of their own: crystal grottos and glowing
        // mushroom groves, found by checking for open space just above or below.
        let zone = if d > 190 { fbm(fx * 0.005, fy * 0.007, 2, s.wrapping_add(34)) } else { 0.5 };
        let grotto = zone < 0.42;
        let grove = zone > 0.6;
        if open {
            if d > 60 && d < 280 && !grotto && !grove && fbm(fx * 0.02, fy * 0.03, 2, s.wrapping_add(20)) > 0.69 {
                return (M::Gas, wall, None);
            }
            if grotto || grove {
                let h = hash3(x, 61, s);
                let tall = 1 + (h >> 4) as i32 % if grotto { 6 } else { 2 };
                let floor = (1..=tall).find(|&k| !self.cave_open(x, y + k, d + k, fbm(fx * 0.024, (fy + k as f32) * 0.05, 4, s.wrapping_add(1))));
                if let Some(k) = floor {
                    if grotto && h % 3 == 0 && k <= tall {
                        return (M::Crystal, wall, None); // crystal spikes on the floor
                    }
                    if grove && k == 1 && h % 4 != 0 {
                        return (M::Mushroom, wall, None); // glowing tufts
                    }
                    if grove && h % 9 == 0 && k <= 2 {
                        return (M::Mushroom, wall, None);
                    }
                }
                if grotto && h % 5 == 1 {
                    // Crystals hanging from the roof.
                    let drop = 1 + (h >> 8) as i32 % 4;
                    if (1..=drop).any(|k| !self.cave_open(x, y - k, d - k, fbm(fx * 0.024, (fy - k as f32) * 0.05, 4, s.wrapping_add(1)))) {
                        return (M::Crystal, wall, None);
                    }
                }
            }
            return (M::Empty, wall, None);
        }
        if grove && self.cave_open(x, y - 1, d - 1, fbm(fx * 0.024, (fy - 1.0) * 0.05, 4, s.wrapping_add(1))) {
            return (M::AlienGrass, wall, None); // glowing moss carpets the grove floor
        }
        if grotto && value_noise(fx * 0.09, fy * 0.09, s.wrapping_add(35)) > 0.84 {
            return (M::Crystal, wall, None); // veins of crystal in the grotto walls
        }

        // Surface layers by biome.
        match biome {
            _ if surf >= SEA - 2 && d < 5 && self.ocean(x) > 0.02 || biome == Biome::Ocean && d < 7 => {
                return (M::Sand, wall, None); // beaches, lake beds and the sea floor
            }
            Biome::Forest | Biome::Plains | Biome::Jungle => {
                if d == 0 {
                    return (M::Grass, wall, None);
                }
                if d < topsoil + if biome == Biome::Jungle { 4 } else { 0 } {
                    return (M::Dirt, wall, None);
                }
            }
            Biome::Ocean => {}
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

    /// Whether the cell at depth d below the surface is cave. Near the surface, caves come
    /// in regions (some country is riddled, most is solid ground); deeper down every
    /// region opens up, and far below there are always great halls.
    fn cave_open(&self, x: i32, y: i32, d: i32, n: f32) -> bool {
        if d <= 6 {
            return false;
        }
        let (fx, fy) = (x as f32, y as f32);
        let s = self.s;
        let region = smooth(0.46, 0.62, fbm(fx * 0.0022, fy * 0.003 + 9.0, 2, s.wrapping_add(31)));
        let deep = smooth(110.0, 260.0, d as f32);
        let k = region.max(deep);
        let thr = 0.71 - ((d - 12) as f32 / 500.0).clamp(0.0, 0.11) + (1.0 - k) * 0.14;
        if d > 12 && n > thr {
            return true;
        }
        if k > 0.3 {
            let worm = (fbm(fx * 0.012, fy * 0.022, 3, s.wrapping_add(2)) - 0.5).abs();
            if worm < (0.02 + (d as f32 / 3000.0).min(0.012)) * k {
                return true;
            }
        }
        if d > 170 && fbm(fx * 0.007, fy * 0.012, 3, s.wrapping_add(3)) > 0.62 - deep * 0.05 {
            return true;
        }
        // Great halls far below.
        d > 300 && fbm(fx * 0.004, fy * 0.009, 3, s.wrapping_add(33)) > 0.53
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
                let Some((rx, ry, half, altar)) = self.ruin_at(gx, gy) else { continue };
                let (w, hgt) = (half * 2, 8);
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

    /// A landmark in surface grid cell g: (x, ground y, kind, hash). Kinds: 0 ruined
    /// watchtower, 1 stone arch, 2 hilltop shrine, 3 ring of standing stones.
    pub fn landmark(&self, g: i32) -> Option<(i32, i32, u32, u32)> {
        const LG: i32 = 340;
        let h = hash3(g, 404, self.s);
        if h % 100 >= 70 || self.planet.kind == PKind::Moon {
            return None;
        }
        let x = g * LG + 60 + (h >> 8) as i32 % (LG - 120);
        if x.abs() < 150 || self.volcano_near(x).is_some() || !self.portals_near(x - 30, x + 30).is_empty() || self.ocean(x) > 0.1 {
            return None;
        }
        let ground = (x - 12..=x + 12).map(|c| self.surface(c)).max().unwrap_or(0);
        if ground >= self.sea_level() {
            return None;
        }
        Some((x, ground, (h >> 16) % 4, h))
    }

    /// Landmarks to find on the surface: they stand on foundations dug into the ground.
    fn stamp_landmarks(&self, cx: i32, cy: i32, cells: &mut [Cell], rng: &mut Rng) {
        const LG: i32 = 340;
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        let put = |x: i32, y: i32, m: M, shade: u8, back: bool, cells: &mut [Cell], rng: &mut Rng| {
            if x < x0 || y < y0 || x >= x0 + CHUNK || y >= y0 + CHUNK {
                return;
            }
            let i = ((y - y0) as usize) * CU + (x - x0) as usize;
            if back {
                cells[i].back = m;
                if cells[i].mat.foliage() || cells[i].mat == M::Plant {
                    cells[i].mat = M::Empty;
                }
            } else {
                let mut c = Cell::new(m, rng);
                c.shade = shade;
                c.back = cells[i].back;
                cells[i] = c;
            }
        };
        for g in (x0 - 40).div_euclid(LG)..=(x0 + CHUNK + 40).div_euclid(LG) {
            let Some((lx, ground, kind, h)) = self.landmark(g) else { continue };
            if ground - 60 > y0 + CHUNK || ground + 30 < y0 {
                continue;
            }
            let brick = |x: i32, y: i32| if (y + (x.div_euclid(3)) * 2).rem_euclid(3) == 0 { 70 } else { 140 + (hash3(x, y, 9) % 70) as u8 };
            // Foundations: fill down to the real ground under the footprint.
            let half = match kind {
                0 => 5,
                1 => 13,
                2 => 9,
                _ => 12,
            };
            for x in lx - half..=lx + half {
                for y in ground..self.surface(x).max(ground) + 3 {
                    put(x, y, M::Stone, 90, false, cells, rng);
                }
                for y in self.surface(x)..ground {
                    put(x, y, M::Stone, 90, false, cells, rng);
                }
            }
            match kind {
                0 => {
                    // Ruined watchtower: hollow brick shaft, floors, windows, a broken crown.
                    let hgt = 24 + (h >> 20) as i32 % 12;
                    for y in ground - hgt..ground {
                        for x in lx - 5..=lx + 5 {
                            let edge = (x - lx).abs() == 5;
                            let floor = (ground - y) % 9 == 0 && (x - lx).abs() < 5 && (ground - y) > 0 && (x - lx) != 3 && (x - lx) != 2;
                            let window = edge && (ground - y) % 9 == 5 || edge && (ground - y) % 9 == 4;
                            let crumble = y < ground - hgt + 4 && hash3(x, y, h) % 3 == 0;
                            if (edge || floor) && !window && !crumble {
                                put(x, y, M::Brick, brick(x, y), false, cells, rng);
                            } else if !crumble {
                                put(x, y, M::Brick, 0, true, cells, rng);
                            }
                        }
                    }
                    // A doorway, and a chest on the top floor.
                    for y in ground - 4..ground {
                        put(lx - 5, y, M::Empty, 0, false, cells, rng);
                    }
                    let top_floor = ground - ((hgt - 4) / 9) * 9;
                    for dx in -1..=1 {
                        for dy in 1..=2 {
                            put(lx + dx, top_floor - dy, M::Chest, if dy == 2 || dx != 0 { 170 } else { 60 }, false, cells, rng);
                        }
                    }
                }
                1 => {
                    // A natural stone arch, mossy on top.
                    for x in lx - 13..=lx + 13 {
                        let t = (x - lx) as f32 / 13.0;
                        let inner = (1.0 - t * t).max(0.0).sqrt() * 16.0;
                        let outer = inner + 5.0 + (hash3(x, 1, h) % 3) as f32;
                        let leg = t.abs() > 0.72;
                        let (from, to) = if leg { (ground - outer as i32, ground) } else { (ground - outer as i32, ground - inner as i32) };
                        for y in from..to {
                            let top = y == from;
                            put(x, y, if top { M::Grass } else { M::Stone }, 60 + (hash3(x, y, 3) % 120) as u8, false, cells, rng);
                        }
                        if !leg && hash3(x, 2, h) % 3 == 0 {
                            for k in 0..(2 + hash3(x, 3, h) % 6) as i32 {
                                put(x, ground - inner as i32 + k, M::Plant, 0, true, cells, rng);
                            }
                        }
                    }
                }
                2 => {
                    // Shrine: a raised platform, pillars, a roof, an altar and glowing runes.
                    for x in lx - 9..=lx + 9 {
                        put(x, ground - 1, M::Brick, brick(x, ground), false, cells, rng);
                    }
                    for px in [lx - 7, lx - 3, lx + 3, lx + 7] {
                        for y in ground - 12..ground - 1 {
                            put(px, y, M::Stone, 170, false, cells, rng);
                        }
                    }
                    for k in 0..4 {
                        for x in lx - 9 + k..=lx + 9 - k {
                            put(x, ground - 13 - k, M::Brick, brick(x, k), false, cells, rng);
                        }
                    }
                    for x in lx - 6..=lx + 6 {
                        for y in ground - 12..ground - 1 {
                            if (x - lx).abs() != 3 {
                                let rune = (x - lx).abs() == 5 && (y - ground) % 3 == 0;
                                put(x, y, if rune { M::Rune } else { M::Brick }, 0, true, cells, rng);
                            }
                        }
                    }
                    for dx in -1..=1 {
                        for y in ground - 4..ground - 1 {
                            put(lx + dx, y, M::Altar, 0, false, cells, rng);
                        }
                    }
                }
                _ => {
                    // A ring of standing stones, the tallest in the middle, runes carved in.
                    for k in -2..=2i32 {
                        let sx = lx + k * 5;
                        let hgt = 8 + (2 - k.abs()) * 3 + (hash3(k, 7, h) % 3) as i32;
                        for y in ground - hgt..ground {
                            for dx in 0..2 {
                                let rune = dx == 0 && (y - ground + hgt) % 4 == 2;
                                put(sx + dx, y, if rune { M::Rune } else { M::Stone }, 120 + (hash3(sx + dx, y, h) % 80) as u8, false, cells, rng);
                            }
                        }
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
            if matches!(m, M::Mushroom | M::Plant) {
                cells[i].back = m;
                return;
            }
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
                let Some((ix, iy, rx, depth, astral, h)) = self.island_at(gx, gy) else { continue };
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

    /// Volcano cones with a lava chamber, and geyser shafts over a water pocket.
    fn stamp_vents(&self, cx: i32, cy: i32, cells: &mut [Cell], rng: &mut Rng) {
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        let mut set = |x: i32, y: i32, m: M, cells: &mut [Cell]| {
            if x >= x0 && y >= y0 && x < x0 + CHUNK && y < y0 + CHUNK {
                let i = ((y - y0) as usize) * CU + (x - x0) as usize;
                cells[i] = Cell::new(m, rng);
            }
        };
        for x in x0..x0 + CHUNK {
            if let Some((vx, base)) = self.volcano_near(x) {
                let dx = (x - vx).abs();
                let t = (1.0 - dx as f32 / VOLC_W as f32).max(0.0);
                let mut top = base - (VOLC_H as f32 * t.powf(1.3)) as i32;
                if dx < 6 {
                    top = base - VOLC_H + 6; // crater
                }
                let ground = self.surface(x);
                for y in top.max(y0)..ground.min(y0 + CHUNK) {
                    let m = if value_noise(x as f32 * 0.2, y as f32 * 0.2, 13) > 0.7 { M::Stone } else { M::Basalt };
                    set(x, y, m, cells);
                }
                if dx <= 2 {
                    for y in (base - VOLC_H + 6).max(y0)..(base + 34).min(y0 + CHUNK) {
                        set(x, y, M::Empty, cells);
                    }
                }
                // Magma chamber.
                for y in (base + 28).max(y0)..(base + 50).min(y0 + CHUNK) {
                    let e = ((x - vx) as f32 / 18.0).powi(2) + ((y - base - 40) as f32 / 9.0).powi(2);
                    if e <= 1.0 {
                        set(x, y, M::Lava, cells);
                    } else if e <= 1.5 {
                        set(x, y, M::Basalt, cells);
                    }
                }
            }
        }
        for (volcano, gx, top) in self.features_near(x0 - 4, x0 + CHUNK + 4) {
            if volcano {
                continue;
            }
            for y in top - 2..top + 16 {
                for x in gx - 4..=gx + 4 {
                    let dx = (x - gx).abs();
                    if y >= top + 12 && dx <= 3 {
                        set(x, y, if y >= top + 15 { M::Stone } else { M::Water }, cells);
                    } else if dx <= 1 && y >= top - 2 {
                        set(x, y, M::Empty, cells);
                    } else if dx == 2 && y >= top - 1 || (dx <= 4 && y == top - 1 && dx >= 2) {
                        set(x, y, M::Stone, cells);
                    }
                }
            }
        }
    }

    fn stamp_trees(&self, cx: i32, cy: i32, cells: &mut [Cell], rng: &mut Rng) {
        const SLOT: i32 = 11;
        let (x0, y0) = (cx * CHUNK, cy * CHUNK);
        let put = |x: i32, y: i32, m: M, shade: u8, over: bool, cells: &mut [Cell]| {
            if x < x0 || y < y0 || x >= x0 + CHUNK || y >= y0 + CHUNK {
                return;
            }
            let i = ((y - y0) as usize) * CU + (x - x0) as usize;
            let _ = shade;
            if over || cells[i].back == M::Empty {
                cells[i].back = m;
            }
        };
        for slot in (x0 - 26).div_euclid(SLOT)..=(x0 + CHUNK + 26).div_euclid(SLOT) {
            let h = hash3(slot, 77, self.s);
            let tx = slot * SLOT + ((h >> 8) % (SLOT as u32 - 3)) as i32;
            let biome = self.biome(tx);
            let style = if self.alien() {
                match self.planet.flora {
                    Flora::Trees => Style::Oak,
                    Flora::Pines => Style::Pine,
                    Flora::Cacti => Style::Cactus,
                    Flora::Shrooms => Style::Shroom,
                    Flora::Crystals => Style::Crystal,
                    Flora::None => Style::None,
                }
            } else {
                let grove = fbm(tx as f32 * 0.004, 8.0, 2, self.s.wrapping_add(51));
                match biome {
                    Biome::Jungle if h % 29 == 0 => Style::Great,
                    Biome::Jungle => Style::Jungle,
                    Biome::Snow => Style::Pine,
                    Biome::Desert => Style::Cactus,
                    Biome::Ocean => Style::None,
                    _ if h % 31 == 0 => Style::Great,
                    Biome::Forest | Biome::Plains if grove > 0.6 => Style::Blossom,
                    _ => Style::Oak,
                }
            };
            let chance = match (style, biome) {
                (Style::Jungle, _) => 92,
                (Style::Oak, Biome::Plains) if !self.alien() => 12,
                (Style::None, _) => 0,
                (Style::Crystal, _) => 35,
                (Style::Great, _) => 100,
                (Style::Blossom, _) => 60,
                _ => 55,
            };
            if h % 100 >= chance || self.surface(tx) >= self.sea_level() || !self.portals_near(tx - 16, tx + 16).is_empty() {
                continue;
            }
            if tx.abs() < 8 || self.volcano_near(tx).is_some() {
                continue; // keep spawn clear
            }
            let (s0, s1) = (self.surface(tx), self.surface(tx + 1));
            if (s0 - s1).abs() > 2 || (self.surface(tx - 3) - self.surface(tx + 4)).abs() > 6 {
                continue;
            }
            let base = s0.max(s1);
            let height = match style {
                Style::Jungle => 16 + ((h >> 16) % 14) as i32,
                Style::Great => 30 + ((h >> 16) % 12) as i32,
                _ => 9 + ((h >> 16) % 11) as i32,
            };
            let top = base - height;
            if top - 22 > y0 + CHUNK || base + 4 < y0 {
                continue;
            }
            let hv = |a: i32, b: i32| hash3(a, b, h);
            match style {
                Style::Shroom => {
                    // Giant glowing mushroom.
                    let stem = 5 + (h >> 18) as i32 % 9;
                    for y in base - stem..base {
                        put(tx, y, M::Plant, 0, true, cells);
                        put(tx + 1, y, M::Plant, 0, true, cells);
                    }
                    let rx = 4 + (h >> 22) as i32 % 4;
                    for dy in -3..=0 {
                        for dx in -rx..=rx + 1 {
                            if (dx as f32 - 0.5).abs() / rx as f32 + (-dy) as f32 / 3.5 <= 1.1 {
                                put(tx + dx, base - stem + dy, M::Mushroom, 0, true, cells);
                            }
                        }
                    }
                }
                Style::Crystal => {
                    // Crystal spire (solid, glowing).
                    let hgt = 5 + (h >> 18) as i32 % 12;
                    for k in 0..hgt {
                        let w = ((hgt - k) / 4).min(2);
                        for dx in -w..=w {
                            let (x, y) = (tx + dx, base - 1 - k);
                            if x >= x0 && y >= y0 && x < x0 + CHUNK && y < y0 + CHUNK {
                                let i = ((y - y0) as usize) * CU + (x - x0) as usize;
                                cells[i] = Cell::new(M::Crystal, rng);
                            }
                        }
                    }
                }
                Style::Oak | Style::Jungle => {
                    for y in top..base {
                        put(tx, y, M::Wood, (hv(tx, y) % 90) as u8 + 60, true, cells);
                        put(tx + 1, y, M::Wood, (hv(tx + 1, y) % 90) as u8 + 20, true, cells);
                    }
                    let jungle = style == Style::Jungle;
                    let (rx, ry) = if jungle { (8 + (h >> 20) as i32 % 3, 5) } else { (5 + (h >> 20) as i32 % 3, 4 + (h >> 22) as i32 % 2) };
                    if jungle {
                        // Hanging vines.
                        for k in 0..6 {
                            let vx = tx - rx + 1 + (hv(k, 9) % (2 * rx as u32)) as i32;
                            let len = 4 + (hv(k, 10) % 9) as i32;
                            for vy in 0..len {
                                put(vx, top + ry - 1 + vy, M::Plant, 0, false, cells);
                            }
                        }
                    }
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
                Style::Pine => {
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
                Style::Great => {
                    // Trunk four wide, flaring into roots, leaning slightly.
                    for y in top..base + 2 {
                        let t = (base - y) as f32 / height as f32;
                        let lean = (t * t * 3.0 * if h & 2 == 0 { 1.0 } else { -1.0 }) as i32;
                        let flare = if y >= base - 4 { (base - y - 4).abs().min(4) / 2 + (4 - (base - y).min(4)) } else { 0 };
                        for dx in -1 - flare..=2 + flare {
                            put(tx + dx + lean, y, M::Wood, 0, true, cells);
                        }
                    }
                    // Big boughs and a layered canopy of overlapping clouds of leaves.
                    let lean_top = (3.0 * if h & 2 == 0 { 1.0 } else { -1.0 }) as i32;
                    let lobes = [(0, -2, 13, 6), (-10, 4, 8, 5), (11, 3, 9, 5), (-4, -8, 9, 5), (6, -7, 8, 5)];
                    for (k, (lx, ly, rx, ry)) in lobes.iter().enumerate() {
                        let (cxl, cyl) = (tx + lean_top + lx, top + ly);
                        // Bough from the trunk to the lobe.
                        let steps = lx.abs().max(ly.abs()).max(1);
                        for j in 0..=steps {
                            let bx = tx + lean_top + lx * j / steps;
                            let by = top + 6 + (ly - 6) * j / steps;
                            put(bx, by, M::Wood, 0, true, cells);
                        }
                        for dy in -ry - 1..=*ry {
                            for dx in -rx - 1..=*rx + 1 {
                                let e = (dx as f32 / *rx as f32).powi(2) + (dy as f32 / *ry as f32).powi(2);
                                let jag = (hv(dx + k as i32 * 40, dy) % 100) as f32 / 100.0 * 0.3;
                                if e <= 1.0 + jag - 0.12 {
                                    put(cxl + dx, cyl + dy, M::Leaves, 0, false, cells);
                                }
                            }
                        }
                    }
                }
                Style::Blossom => {
                    // A crooked trunk that wanders as it climbs, under a pink cloud.
                    let mut wx = tx;
                    for y in (top..base).rev() {
                        if hv(1, y) % 5 == 0 {
                            wx += if hv(2, y) % 2 == 0 { 1 } else { -1 };
                        }
                        put(wx, y, M::Wood, 0, true, cells);
                        put(wx + 1, y, M::Wood, 0, true, cells);
                    }
                    let (rx, ry) = (6 + (h >> 20) as i32 % 3, 4);
                    for (lx, ly, sx, sy) in [(0, 0, rx, ry), (-rx + 2, 2, rx / 2 + 1, ry - 1), (rx - 1, 2, rx / 2 + 1, ry - 1)] {
                        for dy in -sy - 1..=sy {
                            for dx in -sx - 1..=sx + 1 {
                                let e = (dx as f32 / sx as f32).powi(2) + (dy as f32 / sy as f32).powi(2);
                                let jag = (hv(dx + lx * 7, dy) % 100) as f32 / 100.0 * 0.35;
                                if e <= 1.0 + jag - 0.15 {
                                    put(wx + lx + dx, top + ly + dy, M::Blossom, 0, false, cells);
                                }
                            }
                        }
                    }
                    // A carpet of fallen petals.
                    for dx in -rx..=rx {
                        if hv(dx, 99) % 3 == 0 {
                            put(tx + dx, self.surface(tx + dx) - 1, M::Blossom, 0, false, cells);
                        }
                    }
                }
                Style::None => {}
                Style::Cactus => {
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
