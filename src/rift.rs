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
}

impl Rift {
    pub fn new(x: f32, y: f32, planet: (i32, i32), seed: u32) -> Rift {
        Rift { x, y, age: 0.0, life: RIFT_LIFE, planet, white: false, seed }
    }

    pub fn white_hole(x: f32, y: f32, planet: (i32, i32), seed: u32) -> Rift {
        Rift { x, y, age: 0.0, life: 4.5, planet, white: true, seed }
    }

    /// 0 closed .. 1 fully open: tears open fast, seals slowly at the end.
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
        let v = (y - self.y) / hh;
        if v.abs() >= 1.0 {
            return 1.0 + (v.abs() - 1.0) * 4.0 + ((x - self.x) / hw).abs() * 0.5;
        }
        // Lens-shaped profile that pinches to a point at both ends.
        let profile = (1.0 - v * v).powf(0.75);
        let ragged = 1.0 + 0.22 * (fbm(v * 5.0 + t * 0.35, self.seed as f32 * 0.01, 2, self.seed) - 0.5) * 2.0;
        let bend = (v * 2.7 + t * 0.6 + self.seed as f32).sin() * 1.6 * profile;
        let u = (x - self.x - bend) / hw;
        u.abs() / (profile * ragged).max(0.05)
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
