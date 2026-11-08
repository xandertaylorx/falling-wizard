//! The galaxy: an infinite grid of procedurally generated planets. Earth sits at (0, 0).
//! Each planet's look, terrain, seas, plant life, gravity and creatures come from a hash
//! of its galaxy coordinates, so the same planet is always the same.

use crate::materials::Material as M;
use crate::mobs::MobKind;
use crate::noise::hash3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Earth,
    Dune,
    Ice,
    Lava,
    Jungle,
    Toxic,
    Crystal,
    Moon,
    Ocean,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Earth => "HOMEWORLD",
            Kind::Dune => "DUNE WORLD",
            Kind::Ice => "ICE WORLD",
            Kind::Lava => "LAVA WORLD",
            Kind::Jungle => "JUNGLE WORLD",
            Kind::Toxic => "TOXIC WORLD",
            Kind::Crystal => "CRYSTAL WORLD",
            Kind::Moon => "BARREN MOON",
            Kind::Ocean => "OCEAN WORLD",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flora {
    Trees,
    Pines,
    Cacti,
    Shrooms,
    Crystals,
    None,
}

#[derive(Clone, Debug)]
pub struct Planet {
    pub gx: i32,
    pub gy: i32,
    pub name: String,
    pub kind: Kind,
    pub gravity: f32,
    pub sky_top: [u8; 3],
    pub sky_horizon: [u8; 3],
    /// Colour multiplier applied to terrain.
    pub tint: [f32; 3],
    pub top: M,
    pub soil: M,
    pub rock: M,
    pub sea: Option<M>,
    pub sea_level: i32,
    pub flora: Flora,
    pub amp: f32,
    pub peaks: f32,
    pub creatures: &'static [MobKind],
    /// Portal colour (0..1).
    pub hue: f32,
    pub weather: bool,
}

impl PartialEq for Planet {
    fn eq(&self, o: &Self) -> bool {
        self.gx == o.gx && self.gy == o.gy
    }
}

const SYL: [&str; 24] = [
    "ka", "zor", "ve", "lu", "tha", "rix", "om", "nae", "qua", "dra", "sel", "un", "pho", "xi", "gar", "mo", "tal", "ry", "che", "bo", "ish", "vel", "ar", "esh",
];

impl Planet {
    pub fn earth() -> Planet {
        Planet {
            gx: 0,
            gy: 0,
            name: "EARTH".into(),
            kind: Kind::Earth,
            gravity: 1.0,
            sky_top: [60, 112, 196],
            sky_horizon: [184, 212, 234],
            tint: [1.0, 1.0, 1.0],
            top: M::Grass,
            soil: M::Dirt,
            rock: M::Stone,
            sea: Some(M::Water),
            sea_level: crate::worldgen::SEA,
            flora: Flora::Trees,
            amp: 1.0,
            peaks: 1.0,
            creatures: &[],
            hue: 0.58,
            weather: true,
        }
    }

    pub fn is_earth(&self) -> bool {
        self.gx == 0 && self.gy == 0
    }

    /// The planet at galaxy cell (gx, gy), if there is one.
    pub fn at(gx: i32, gy: i32) -> Option<Planet> {
        if gx == 0 && gy == 0 {
            return Some(Planet::earth());
        }
        let h = hash3(gx, gy, 0x9A1A_7E11);
        if h % 100 >= 55 {
            return None;
        }
        use MobKind::*;
        let kinds = [Kind::Dune, Kind::Ice, Kind::Lava, Kind::Jungle, Kind::Toxic, Kind::Crystal, Kind::Moon, Kind::Ocean];
        let kind = kinds[((h >> 8) % kinds.len() as u32) as usize];
        let v = |shift: u32| ((h >> shift) & 0xFF) as f32 / 255.0;
        let jig = |c: [u8; 3], s: u32| -> [u8; 3] {
            let k = 0.8 + v(s) * 0.4;
            [(c[0] as f32 * k).min(255.0) as u8, (c[1] as f32 * (1.6 - k)).min(255.0) as u8, (c[2] as f32 * k).min(255.0) as u8]
        };
        let name = {
            let n = 2 + (h >> 20) % 2;
            let mut s = String::new();
            for i in 0..n {
                s.push_str(SYL[((hash3(gx, gy, i + 7) >> 4) % SYL.len() as u32) as usize]);
            }
            let mut s = s.to_uppercase();
            if (h >> 22) % 3 == 0 {
                s.push_str(&format!(" {}", ["PRIME", "II", "III", "IV", "MAJOR", "MINOR", "B", "X"][((h >> 24) % 8) as usize]));
            }
            s
        };
        let (sky_top, sky_horizon, tint, top, soil, rock, sea, sea_level, flora, amp, peaks, creatures, gravity, hue, weather): (
            [u8; 3],
            [u8; 3],
            [f32; 3],
            M,
            M,
            M,
            Option<M>,
            i32,
            Flora,
            f32,
            f32,
            &'static [MobKind],
            f32,
            f32,
            bool,
        ) = match kind {
            Kind::Dune => ([210, 120, 80], [250, 200, 150], [1.15, 0.78, 0.6], M::Sand, M::Sand, M::Stone, None, 999, Flora::Cacti, 1.3, 0.6, &[Scorpion, Lion, Salamander, Wisp], 0.8, 0.07, true),
            Kind::Ice => ([120, 170, 230], [220, 236, 250], [0.85, 0.95, 1.2], M::Snow, M::Ice, M::Stone, Some(M::Water), 20, Flora::Pines, 1.1, 1.4, &[SnowWolf, Yeti, Wisp, Ghost], 0.9, 0.55, true),
            Kind::Lava => ([60, 20, 24], [170, 70, 40], [0.8, 0.62, 0.6], M::Basalt, M::Basalt, M::Basalt, Some(M::Lava), 22, Flora::None, 1.2, 1.2, &[FireImp, Salamander, Golem, Dragon], 1.1, 0.02, false),
            Kind::Jungle => ([70, 170, 150], [190, 236, 210], [0.8, 1.15, 0.85], M::AlienGrass, M::Dirt, M::Stone, Some(M::Water), 16, Flora::Shrooms, 1.0, 0.7, &[Tiger, Slime, Mushling, Bat], 1.0, 0.36, true),
            Kind::Toxic => ([150, 170, 60], [220, 230, 140], [0.95, 1.05, 0.7], M::AlienGrass, M::Dirt, M::Stone, Some(M::Acid), 18, Flora::Shrooms, 0.9, 0.5, &[Slime, VoidEye, Ghost, Mushling], 0.9, 0.25, true),
            Kind::Crystal => ([40, 20, 76], [120, 80, 170], [0.9, 0.8, 1.2], M::MoonRock, M::MoonRock, M::MoonRock, None, 999, Flora::Crystals, 1.2, 1.0, &[StarJelly, Wisp, VoidEye, MoonHopper], 0.5, 0.78, false),
            Kind::Moon => ([4, 4, 12], [20, 20, 36], [1.0, 1.0, 1.05], M::MoonRock, M::MoonRock, M::MoonRock, None, 999, Flora::None, 0.7, 0.3, &[MoonHopper, StarJelly, Wisp], 0.3, 0.12, false),
            Kind::Ocean => ([70, 150, 220], [190, 225, 245], [1.0, 1.0, 1.0], M::Sand, M::Sand, M::Stone, Some(M::Water), -6, Flora::Trees, 1.3, 0.2, &[Fish, Shark, Slime, CloudSprite], 1.0, 0.5, true),
            Kind::Earth => unreachable!(),
        };
        let tint = [tint[0] * (0.92 + v(4) * 0.16), tint[1] * (0.92 + v(12) * 0.16), tint[2] * (0.92 + v(16) * 0.16)];
        Some(Planet {
            gx,
            gy,
            name,
            kind,
            gravity: gravity * (0.85 + v(26) * 0.3),
            sky_top: jig(sky_top, 3),
            sky_horizon: jig(sky_horizon, 11),
            tint,
            top,
            soil,
            rock,
            sea,
            sea_level,
            flora,
            amp,
            peaks,
            creatures,
            hue: (hue + (v(18) - 0.5) * 0.08).rem_euclid(1.0),
            weather,
        })
    }

    /// Up to `n` planets nearest to galaxy cell (cx, cy), nearest first. Earth is always included.
    pub fn nearest(cx: i32, cy: i32, n: usize) -> Vec<Planet> {
        let mut found: Vec<(i32, Planet)> = Vec::new();
        let mut r = 0;
        while found.len() < n + 1 && r < 60 {
            for y in cy - r..=cy + r {
                for x in cx - r..=cx + r {
                    if (x - cx).abs().max((y - cy).abs()) != r {
                        continue;
                    }
                    if let Some(p) = Planet::at(x, y) {
                        found.push(((x - cx).pow(2) + (y - cy).pow(2), p));
                    }
                }
            }
            r += 1;
        }
        found.sort_by_key(|f| f.0);
        let mut out: Vec<Planet> = found.into_iter().map(|f| f.1).take(n).collect();
        if !out.iter().any(|p| p.is_earth()) {
            out.insert(0, Planet::earth());
            out.truncate(n.max(1));
        } else if let Some(i) = out.iter().position(|p| p.is_earth()) {
            let e = out.remove(i);
            out.insert(0, e);
        }
        out
    }

    /// Distance in light years between two planets.
    pub fn light_years(&self, o: &Planet) -> f32 {
        (((self.gx - o.gx).pow(2) + (self.gy - o.gy).pow(2)) as f32).sqrt() * 4.3
    }

    pub fn color(&self) -> [u8; 3] {
        hue_rgb(self.hue, 0.8, 1.0)
    }

    pub fn world_seed(&self, base: u64) -> u64 {
        if self.is_earth() {
            base
        } else {
            base ^ (hash3(self.gx, self.gy, 0x51ED) as u64) << 13 ^ 0xA5A5
        }
    }
}

/// HSV to RGB, all in 0..1.
pub fn hue_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

/// What the air is like: how much there is to burn, how hot or cold it runs, and
/// how high the atmosphere reaches (0 = none at all).
#[derive(Clone, Copy, Debug)]
pub struct Climate {
    /// Oxygen: scales how readily fire spreads. 0 means vacuum; fire chokes at once.
    pub o2: f32,
    /// -1 cold (open water freezes, lava crusts), 0 temperate, 1 hot (ice melts, water boils off).
    pub heat: i8,
    /// Height of the atmosphere's edge above sea level, in cells.
    pub atmo: i32,
}

/// Something hanging in a planet's sky, seen best from orbit.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    /// Position: fraction of the view width, and pixels below the top.
    pub fx: f32,
    pub y: f32,
    pub r: f32,
    pub a: [u8; 3],
    pub b: [u8; 3],
    /// 0 banded gas giant, 1 cratered moon, 2 ocean world with clouds, 3 molten.
    pub style: u8,
    pub ring: Option<[u8; 3]>,
    /// Parallax strength.
    pub depth: f32,
}

/// The sky scene of a planet: its suns, the bodies you can see from orbit, and the
/// colour of the nebula behind the stars.
#[derive(Clone, Debug)]
pub struct SkyScene {
    pub sun: [u8; 3],
    pub sun_r: f32,
    /// A second, smaller sun (binary systems).
    pub twin: Option<[u8; 3]>,
    pub bodies: Vec<Body>,
    pub nebula: [u8; 3],
    pub nebula_amt: f32,
    /// Glow colours of the atmosphere's edge.
    pub rim: [[u8; 3]; 2],
}

impl Planet {
    pub fn climate(&self) -> Climate {
        let h = hash3(self.gx, self.gy, 0xC11A);
        let jig = 0.85 + ((h & 0xFF) as f32 / 255.0) * 0.3;
        let (o2, heat, atmo) = match self.kind {
            Kind::Earth => (1.0, 0, 480.0),
            Kind::Dune => (0.8, 1, 420.0),
            Kind::Ice => (1.0, -1, 520.0),
            Kind::Lava => (0.6, 1, 560.0),
            Kind::Jungle => (1.35, 0, 600.0),
            Kind::Toxic => (0.5, 0, 640.0),
            Kind::Crystal => (0.25, -1, 300.0),
            Kind::Moon => (0.0, -1, 0.0),
            Kind::Ocean => (1.0, 0, 500.0),
        };
        let atmo = if self.is_earth() { atmo } else { atmo * jig };
        Climate { o2, heat, atmo: atmo as i32 }
    }

    pub fn sky_scene(&self) -> SkyScene {
        if self.is_earth() {
            return SkyScene {
                sun: [255, 244, 210],
                sun_r: 7.0,
                twin: None,
                bodies: vec![
                    Body { fx: 0.72, y: 10.0, r: 18.0, a: [196, 150, 110], b: [170, 120, 96], style: 0, ring: Some([210, 190, 160]), depth: 0.03 },
                    Body { fx: 0.2, y: 34.0, r: 7.0, a: [200, 200, 206], b: [150, 150, 160], style: 1, ring: None, depth: 0.05 },
                ],
                nebula: [70, 60, 140],
                nebula_amt: 0.18,
                rim: [[90, 220, 200], [140, 120, 255]],
            };
        }
        let h = |k: u32| hash3(self.gx, self.gy, 0x5C1E + k);
        let f = |k: u32| (h(k) & 0xFFFF) as f32 / 65535.0;
        let (sun, sun_r) = match self.kind {
            Kind::Lava => ([255, 120, 70], 13.0),
            Kind::Crystal => ([200, 220, 255], 4.0),
            Kind::Dune => ([255, 220, 160], 9.0),
            Kind::Ice => ([230, 240, 255], 5.0),
            Kind::Toxic => ([240, 255, 170], 8.0),
            _ => ([255, 240, 205], 6.0 + f(1) * 3.0),
        };
        let twin = (matches!(self.kind, Kind::Dune) || h(2) % 5 == 0).then(|| hue_rgb(f(3) * 0.15, 0.45, 1.0));
        let mut bodies = Vec::new();
        let n = 1 + h(4) % 3;
        for k in 0..n {
            let style = (h(10 + k) % 4) as u8;
            let hue = f(20 + k);
            let (a, b) = match style {
                0 => (hue_rgb(hue, 0.45, 0.85), hue_rgb(hue + 0.05, 0.55, 0.65)),
                1 => (hue_rgb(hue, 0.08, 0.8), hue_rgb(hue, 0.1, 0.55)),
                2 => ([60, 120, 220], [240, 245, 255]),
                _ => ([60, 30, 30], [255, 120, 40]),
            };
            let r = match style {
                0 => 14.0 + f(30 + k) * 22.0,
                _ => 4.0 + f(30 + k) * 10.0,
            };
            let ring = (style == 0 && h(40 + k) % 2 == 0).then(|| hue_rgb(hue + 0.1, 0.25, 0.9));
            bodies.push(Body { fx: 0.12 + (k as f32 + f(50 + k) * 0.7) / n as f32 * 0.8, y: 6.0 + f(60 + k) * 60.0, r, a, b, style, ring, depth: 0.02 + f(70 + k) * 0.04 });
        }
        let rim_a = hue_rgb(self.hue + 0.05, 0.6, 1.0);
        SkyScene {
            sun,
            sun_r,
            twin,
            bodies,
            nebula: hue_rgb(f(80), 0.7, 0.55),
            nebula_amt: 0.12 + f(81) * 0.3,
            rim: [rim_a, [self.sky_horizon[0], self.sky_horizon[1], self.sky_horizon[2]]],
        }
    }
}
