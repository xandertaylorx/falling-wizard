//! Day/night cycle, rain and lightning storms.

use crate::audio::Sfx;
use crate::fx::Fx;
use crate::noise::Rng;
use crate::render::Canvas;
use crate::world::World;

/// Seconds for a full day.
pub const DAY_LEN: f32 = 480.0;

pub struct Weather {
    /// 0 = midnight, 0.5 = noon.
    pub time: f32,
    pub rain: f32,
    target: f32,
    pub storm: bool,
    change_t: f32,
    bolt_t: f32,
    pub wind: f32,
    bolts: Vec<(Vec<(f32, f32)>, f32)>,
    rng: Rng,
    clock: f32,
}

impl Weather {
    pub fn new(seed: u64) -> Self {
        Self { time: 0.32, rain: 0.0, target: 0.0, storm: false, change_t: 90.0, bolt_t: 6.0, wind: -20.0, bolts: Vec::new(), rng: Rng::new(seed ^ 0x5A1A), clock: 0.0 }
    }

    /// 0 at night .. 1 at midday, with a gentle dawn and dusk.
    pub fn daylight(&self) -> f32 {
        let d = 0.5 - (self.time * std::f32::consts::TAU).cos() * 0.5;
        (d * 1.7 - 0.35).clamp(0.0, 1.0)
    }

    /// Wind right now, with slow swells and quick gusts.
    pub fn wind_now(&self) -> f32 {
        let c = self.clock;
        self.wind * (0.6 + 0.4 * (c * 0.23).sin()) + (c * 1.7).sin() * (c * 0.31).sin() * 14.0 * (0.4 + self.rain)
    }

    /// Subtle life in the air: falling leaves, dust, snow, fireflies, blowing sand, cave drips.
    pub fn ambience(&mut self, world: &World, fx: &mut Fx, (px, py): (f32, f32)) {
        use crate::materials::Material as M;
        use crate::worldgen::Biome;
        let wind = self.wind_now();
        for _ in 0..8 {
            let (x, y) = (px + self.rng.rangef(-180.0, 180.0), py + self.rng.rangef(-110.0, 110.0));
            let (gx, gy) = (x as i32, y as i32);
            let Some(c) = world.get(gx, gy) else { continue };
            if c.mat != M::Empty {
                continue;
            }
            let depth = gy - world.surface_at(gx);
            let biome = world.gen.biome(gx);
            let r = self.rng.f32();
            if c.back == M::Leaves {
                if r < 0.3 {
                    let col = if self.rng.chance(0.3) { [222, 146, 56] } else if biome == Biome::Snow { [46, 96, 56] } else { [96, 156, 66] };
                    fx.mote(x, y, wind * 0.4, 8.0, 7.0, col, 0.0, 10.0, 70.0);
                }
            } else if depth < -1 && gy > -240 {
                if biome == Biome::Snow && r < 0.45 {
                    fx.mote(x, y, wind * 0.4, 14.0, 7.0, [240, 244, 252], 0.0, 4.0, 30.0);
                } else if biome == Biome::Desert && wind.abs() > 14.0 && r < 0.5 {
                    let gy2 = world.surface_at(gx) as f32 - 1.0 - self.rng.rangef(0.0, 5.0);
                    fx.mote(x, gy2, wind * 2.4, -4.0, 0.9, [214, 184, 120], 0.0, 0.0, 0.0);
                } else if self.night() && biome == Biome::Forest && depth > -30 && r < 0.1 {
                    let (vx, vy) = (self.rng.rangef(-8.0, 8.0), self.rng.rangef(-6.0, 6.0));
                    fx.mote(x, y, vx, vy, 4.0, [210, 255, 120], 2.0, 0.0, 25.0);
                } else if !self.night() && r < 0.05 {
                    fx.mote(x, y, wind * 0.3, self.rng.rangef(-3.0, 3.0), 6.0, [232, 226, 206], 0.0, 0.0, 12.0);
                }
            } else if depth > 6 {
                if r < 0.05 {
                    fx.mote(x, y, self.rng.rangef(-3.0, 3.0), self.rng.rangef(-2.0, 2.0), 5.0, [130, 124, 120], 0.0, 0.0, 6.0);
                } else if r < 0.09 && world.mat(gx, gy - 1).solid() {
                    let sh = self.rng.next_u32() as u8;
                    fx.debris(x, y, 0.0, 5.0, M::Water, sh);
                }
            }
        }
    }

    pub fn night(&self) -> bool {
        self.daylight() < 0.25
    }

    pub fn set(&mut self, rain: f32, storm: bool) {
        self.target = rain;
        self.rain = rain;
        self.storm = storm;
        self.change_t = 180.0;
    }

    pub fn label(&self) -> &'static str {
        if self.storm {
            "STORM"
        } else if self.rain > 0.2 {
            "RAIN"
        } else if self.night() {
            "NIGHT"
        } else {
            "DAY"
        }
    }

    pub fn update(&mut self, dt: f32, world: &mut World, fx: &mut Fx, (px, py): (f32, f32), sounds: &mut Vec<Sfx>) {
        self.time = (self.time + dt / DAY_LEN).fract();
        self.clock += dt;
        self.change_t -= dt;
        if self.change_t <= 0.0 {
            self.change_t = self.rng.rangef(50.0, 140.0);
            let r = self.rng.f32();
            self.target = if r < 0.55 { 0.0 } else if r < 0.82 { 0.5 } else { 1.0 };
            self.storm = self.target >= 1.0;
            self.wind = self.rng.rangef(-40.0, 40.0);
        }
        self.rain += (self.target - self.rain) * (dt * 0.15).min(1.0);
        for b in self.bolts.iter_mut() {
            b.1 -= dt;
        }
        self.bolts.retain(|b| b.1 > 0.0);

        // Rain only falls where there's open sky, and not up in the clouds.
        let surf = world.surface_at(px as i32);
        if self.rain > 0.05 && py < surf as f32 + 60.0 && py > -260.0 {
            let n = (self.rain * 14.0) as i32;
            for _ in 0..n {
                let x = px + self.rng.rangef(-220.0, 220.0);
                let y = py - 120.0 + self.rng.rangef(-20.0, 10.0);
                if y < world.surface_at(x as i32) as f32 && world.mat(x as i32, y as i32).passable() {
                    fx.rain_drop(x, y, self.wind_now());
                }
            }
        }

        if self.storm && self.rain > 0.6 && py > -260.0 {
            self.bolt_t -= dt;
            if self.bolt_t <= 0.0 {
                self.bolt_t = self.rng.rangef(4.0, 11.0);
                let x = px + self.rng.rangef(-150.0, 150.0);
                let mut y = py - 110.0;
                let mut pts = vec![(x, y)];
                let mut bx = x;
                while y < py + 120.0 {
                    y += 1.0;
                    if (y as i32) % 7 == 0 {
                        bx += self.rng.rangef(-3.0, 3.0);
                        pts.push((bx, y));
                    }
                    if !world.mat(bx as i32, y as i32).passable() {
                        break;
                    }
                }
                pts.push((bx, y));
                fx.explode(world, bx, y, 4, 1.0);
                fx.flash = 0.7;
                sounds.push(Sfx::Thunder);
                self.bolts.push((pts, 0.25));
            }
        }
    }

    pub fn draw(&self, cv: &mut Canvas) {
        for (pts, _) in &self.bolts {
            for w in pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) as i32).max(1);
                for k in 0..=n {
                    let t = k as f32 / n as f32;
                    let (x, y) = ((a.0 + (b.0 - a.0) * t) as i32, (a.1 + (b.1 - a.1) * t) as i32);
                    cv.put(x, y, [255, 255, 240]);
                    cv.put(x + 1, y, [200, 210, 255]);
                }
            }
        }
    }

    pub fn lights(&self, add: &mut impl FnMut(f32, f32, [f32; 3])) {
        for (pts, t) in &self.bolts {
            for p in pts {
                add(p.0, p.1, [12.0 * t, 12.0 * t, 14.0 * t]);
            }
        }
    }
}
