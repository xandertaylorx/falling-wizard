//! Rifts: when two black holes spiral into each other they tear a hole in the world.
//! The tear stays open for a long time, shows a galaxy inside, and leads anywhere.

use crate::noise::{fbm, hash3};

/// How long a rift stays open: half an hour.
pub const RIFT_LIFE: f32 = 1800.0;
/// The tear's half width and half height when fully open.
pub const HALF_W: f32 = 9.0;
pub const HALF_H: f32 = 24.0;

#[derive(Clone)]
pub struct Rift {
    pub x: f32,
    pub y: f32,
    pub age: f32,
    pub life: f32,
    /// Galaxy cell of the planet it was torn on.
    pub planet: (i32, i32),
    /// A white hole: the short-lived tear you come out of.
    pub white: bool,
    pub seed: u32,
    /// Seconds since a Portal Wand bolt started sealing it (negative: not sealing).
    pub closing: f32,
    /// Seconds since a black hole cracked it (negative: intact).
    pub shatter: f32,
}

/// Seconds for a rift to zip shut, and to crack apart before it blows.
pub const CLOSE_TIME: f32 = 2.6;
pub const SHATTER_TIME: f32 = 1.1;

/// A patch of void wasteland left by a shattered rift: violet rock and crystal under
/// a starry sky, spreading out to `r` cells.
#[derive(Clone)]
pub struct Blight {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    /// How far it has spread so far.
    pub grown: f32,
    pub planet: (i32, i32),
    pub seed: u32,
    /// The ragged edge, sampled around the circle.
    table: Vec<f32>,
}

/// How big a void wasteland gets.
pub const BLIGHT_RADIUS: f32 = 475.0;
/// Inside this fraction of the radius the blast left an open crater.
pub const CRATER: f32 = 0.12;
const EDGE_SAMPLES: usize = 512;

impl Blight {
    pub fn new(x: f32, y: f32, r: f32, grown: f32, planet: (i32, i32), seed: u32) -> Blight {
        let table = (0..EDGE_SAMPLES)
            .map(|k| {
                let a = k as f32 / EDGE_SAMPLES as f32 * std::f32::consts::TAU;
                0.72 + 0.4 * fbm(a.cos() * 2.4 + 5.0, a.sin() * 2.4 + 5.0, 4, seed)
            })
            .collect();
        Blight { x, y, r, grown, planet, seed, table }
    }

    /// Ragged edge: how far the void reaches in direction `a`, as a fraction of `r`.
    pub fn reach(&self, a: f32) -> f32 {
        let f = a.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * EDGE_SAMPLES as f32;
        let i = f as usize % EDGE_SAMPLES;
        let j = (i + 1) % EDGE_SAMPLES;
        let t = f.fract();
        self.table[i] * (1.0 - t) + self.table[j] * t
    }

    /// How deep into the wasteland (gx, gy) is: 0 at the blast, 1 at the edge, None outside.
    pub fn depth(&self, gx: f32, gy: f32) -> Option<f32> {
        let (dx, dy) = (gx - self.x, gy - self.y);
        let d2 = dx * dx + dy * dy;
        if d2 > (self.r * 1.13).powi(2) {
            return None;
        }
        let deep = d2.sqrt() / (self.reach(dy.atan2(dx)) * self.r);
        (deep < 1.0).then_some(deep)
    }

    /// 0 outside .. 1 deep inside (what's spread so far).
    pub fn cover(&self, x: f32, y: f32) -> f32 {
        let (dx, dy) = (x - self.x, y - self.y);
        let d = (dx * dx + dy * dy).sqrt();
        let lim = self.grown * self.reach(dy.atan2(dx));
        ((lim - d) / 14.0).clamp(0.0, 1.0)
    }
}

impl Rift {
    pub fn new(x: f32, y: f32, planet: (i32, i32), seed: u32) -> Rift {
        Rift { x, y, age: 0.0, life: RIFT_LIFE, planet, white: false, seed, closing: -1.0, shatter: -1.0 }
    }

    pub fn white_hole(x: f32, y: f32, planet: (i32, i32), seed: u32) -> Rift {
        Rift { x, y, age: 0.0, life: 4.5, planet, white: true, seed, closing: -1.0, shatter: -1.0 }
    }

    /// 0 closed .. 1 fully open: tears open fast, seals slowly at the end.
    /// Where the zipper has got to (0..1 of the half height), while sealing.
    pub fn zip(&self) -> f32 {
        let z = (self.closing.max(0.0) / CLOSE_TIME).min(1.0);
        z * z * (3.0 - 2.0 * z)
    }

    /// Can you fall into it right now?
    pub fn enterable(&self) -> bool {
        !self.white && self.closing < 0.0 && self.shatter < 0.0 && self.open() > 0.5
    }

    pub fn open(&self) -> f32 {
        let grow = if self.white { (self.age / 0.3).min(1.0) } else { (self.age / 2.2).min(1.0).powf(0.6) };
        let fade = ((self.life - self.age) / if self.white { 2.5 } else { 8.0 }).clamp(0.0, 1.0);
        grow * fade
    }

    pub fn size(&self) -> (f32, f32) {
        let o = self.open();
        let k = if self.white { 0.7 } else { 1.0 };
        (HALF_W * o * k, HALF_H * o.powf(0.7) * k)
    }

    /// Normalised distance from the tear's middle: below 1 is inside.
    /// The edge is ragged and slowly writhes, so it reads as a rip, not an oval.
    pub fn dist(&self, x: f32, y: f32, t: f32) -> f32 {
        let (hw, hh) = self.size();
        if hw < 0.3 || hh < 0.3 {
            return 9.0;
        }
        // Cracking: the whole tear shudders.
        let x = if self.shatter >= 0.0 { x + (t * 90.0 + y * 0.7).sin() * self.shatter * 2.0 } else { x };
        let v = (y - self.y) / hh;
        if v.abs() >= 1.0 {
            return 1.0 + (v.abs() - 1.0) * 4.0 + ((x - self.x) / hw).abs() * 0.5;
        }
        // Lens-shaped profile that pinches to a point at both ends.
        let profile = (1.0 - v * v).powf(0.75);
        let ragged = 1.0 + 0.22 * (fbm(v * 5.0 + t * 0.35, self.seed as f32 * 0.01, 2, self.seed) - 0.5) * 2.0;
        let bend = (v * 2.7 + t * 0.6 + self.seed as f32).sin() * 1.6 * profile;
        let u = (x - self.x - bend) / hw;
        // Sealing: zips shut from both ends towards the middle, then pinches thin.
        let zip = if self.closing >= 0.0 {
            let z = (self.closing / CLOSE_TIME).min(1.0);
            let z = z * z * (3.0 - 2.0 * z);
            ((1.0 - v.abs() - z * 1.02) / 0.1).clamp(0.0, 1.0) * (1.0 - ((z - 0.85) / 0.15).clamp(0.0, 1.0) * 0.8)
        } else {
            1.0
        };
        if zip <= 0.0 {
            return 9.0;
        }
        u.abs() / (profile * ragged * zip).max(0.05)
    }
}

/// The view through a rift: deep space with a Milky Way band, dust lanes, a glowing
/// bulge and twinkling stars. `u`, `v` are in cells relative to the rift's centre.
pub fn galaxy(u: f32, v: f32, t: f32, seed: u32) -> [f32; 3] {
    let (s, c) = (0.55f32).sin_cos();
    let (a, b) = (u * c - v * s, u * s + v * c);
    // Background nebulae.
    let neb = fbm(a * 0.05 + 3.0, b * 0.05, 3, seed ^ 0x55);
    let neb2 = fbm(a * 0.08, b * 0.08 + 9.0, 3, seed ^ 0x99);
    let mut col = [6.0 + neb * 40.0 + neb2 * 10.0, 4.0 + neb2 * 22.0, 18.0 + neb * 60.0];
    // The band, thick and clumpy.
    let thick = 5.0 + fbm(a * 0.07, 1.0, 3, seed) * 6.0;
    let band = (-(b / thick).powi(2)).exp();
    let clump = fbm(a * 0.18, b * 0.25, 3, seed ^ 0x13);
    let glow = band * (0.55 + clump * 0.8);
    col[0] += glow * 190.0;
    col[1] += glow * 170.0;
    col[2] += glow * 215.0;
    // Dark dust lanes running along it.
    let lane = (b - (a * 0.09).sin() * 1.8 - (fbm(a * 0.2, 4.0, 2, seed) - 0.5) * 3.0).abs();
    let dust = (1.0 - lane / 1.6).clamp(0.0, 1.0) * band * (0.4 + fbm(a * 0.3, b * 0.3, 2, seed ^ 7) * 0.6);
    for k in &mut col {
        *k *= 1.0 - dust * 0.75;
    }
    // Warm bulge.
    let bulge = (-(a * a + b * b * 3.0) / 70.0).exp();
    col[0] += bulge * 255.0;
    col[1] += bulge * 200.0;
    col[2] += bulge * 140.0;
    // Stars, more of them along the band, twinkling.
    let (ix, iy) = (u.floor() as i32, v.floor() as i32);
    let h = hash3(ix, iy, seed ^ 0x51A2);
    let thresh = 14 + (band * 70.0) as u32;
    if h % 1000 < thresh {
        let tw = 0.6 + 0.4 * (t * (1.5 + (h % 7) as f32) + (h % 100) as f32).sin();
        let tint = match (h >> 10) % 4 {
            0 => [255.0, 210.0, 170.0],
            1 => [170.0, 200.0, 255.0],
            _ => [255.0, 255.0, 255.0],
        };
        for k in 0..3 {
            col[k] = col[k].max(tint[k] * tw);
        }
    }
    col
}

/// The sky over the void wasteland: violet-black space, nebula wisps and stars.
pub fn void_sky(gx: i32, gy: i32, t: f32) -> [f32; 3] {
    let (x, y) = (gx as f32, gy as f32);
    let n = fbm(x * 0.012 + t * 0.01, y * 0.016, 3, 0xF0_1D);
    let n2 = fbm(x * 0.03, y * 0.03 - t * 0.02, 2, 0x5EED);
    let mut c = [10.0 + n * 50.0 + n2 * 20.0, 4.0 + n2 * 14.0, 22.0 + n * 70.0];
    let h = hash3(gx, gy, 0xB1_16);
    if h % 1000 < 7 {
        let tw = 0.6 + 0.4 * (t * 2.0 + (h % 50) as f32).sin();
        c = [230.0 * tw, 210.0 * tw, 255.0 * tw];
    }
    c
}

/// What a cell turns into inside the void, if anything. `deep` is 0 at the blast centre.
pub fn void_material(m: crate::materials::Material, x: i32, y: i32, deep: f32, seed: u32) -> Option<crate::materials::Material> {
    use crate::materials::{Kind, Material as M};
    if m.indestructible() {
        return None;
    }
    let new = if deep < CRATER {
        // The crater: open space with chunks of void rock floating in it.
        if m != M::Empty && fbm(x as f32 * 0.07, y as f32 * 0.07, 3, seed) > 0.6 { M::VoidRock } else { M::Empty }
    } else {
        match m.kind() {
            Kind::Static => {
                if fbm(x as f32 * 0.16, y as f32 * 0.16, 2, seed ^ 3) > 0.7 {
                    M::VoidCrystal
                } else {
                    M::VoidRock
                }
            }
            Kind::Powder => M::VoidDust,
            Kind::Liquid if m != M::Lava => M::Empty,
            Kind::Life => M::Empty,
            _ => return None,
        }
    };
    (new != m).then_some(new)
}

/// Height of the crystal spire growing from void ground at (x, y), if one grows there.
pub fn spire(x: i32, y: i32, seed: u32) -> Option<i32> {
    (hash3(x, y, seed) % 11 == 0).then(|| 2 + (hash3(x, y, seed ^ 9) % 6) as i32)
}
