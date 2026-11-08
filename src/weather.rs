//! Day/night, a day counter, and a weather system that drifts between states.
//!
//! Each state sets targets for rain, cloud cover, fog and wind; the live values ease
//! towards them, so weather rolls in and clears up gradually. Precipitation depends on
//! where you are: rain in most places, snow in the tundra, a light shower in deserts,
//! and windy weather turns into a sandstorm over the dunes.

use crate::audio::Sfx;
use crate::fx::Fx;
use crate::materials::Material as M;
use crate::noise::Rng;
use crate::render::Canvas;
use crate::world::World;
use crate::worldgen::Biome;

/// Seconds for a full day.
pub const DAY_LEN: f32 = 600.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sky {
    Clear,
    Cloudy,
    Rain,
    Storm,
    Fog,
    Windy,
}

impl Sky {
    pub fn name(self) -> &'static str {
        match self {
            Sky::Clear => "CLEAR",
            Sky::Cloudy => "CLOUDY",
            Sky::Rain => "RAIN",
            Sky::Storm => "THUNDERSTORM",
            Sky::Fog => "FOG",
            Sky::Windy => "WINDY",
        }
    }
    /// (rain, clouds, fog, wind)
    fn targets(self) -> (f32, f32, f32, f32) {
        match self {
            Sky::Clear => (0.0, 0.12, 0.0, 8.0),
            Sky::Cloudy => (0.0, 0.7, 0.0, 16.0),
            Sky::Rain => (0.6, 0.85, 0.08, 20.0),
            Sky::Storm => (1.0, 1.0, 0.12, 42.0),
            Sky::Fog => (0.0, 0.35, 0.85, 3.0),
            Sky::Windy => (0.0, 0.4, 0.0, 55.0),
        }
    }
}

pub struct Weather {
    /// 0 = midnight, 0.5 = noon.
    pub time: f32,
    pub day: u32,
    pub kind: Sky,
    pub rain: f32,
    pub clouds: f32,
    pub fog: f32,
    gust: f32,
    dir: f32,
    change_t: f32,
    bolt_t: f32,
    bolts: Vec<(Vec<(f32, f32)>, f32)>,
    /// Lightning strikes this frame, for the game to apply damage.
    pub strikes: Vec<(f32, f32)>,
    /// How sandy the air is where the wizard stands (desert + wind).
    pub sand: f32,
    rng: Rng,
    clock: f32,
    /// Weather switched off in world options: always clear.
    pub locked: bool,
}

impl Weather {
    pub fn new(seed: u64) -> Self {
        Self {
            time: 0.3,
            day: 1,
            kind: Sky::Clear,
            rain: 0.0,
            clouds: 0.12,
            fog: 0.0,
            gust: 8.0,
            dir: -1.0,
            change_t: 120.0,
            bolt_t: 6.0,
            bolts: Vec::new(),
            strikes: Vec::new(),
            sand: 0.0,
            rng: Rng::new(seed ^ 0x5A1A),
            clock: 0.0,
            locked: false,
        }
    }

    /// 0 at night .. 1 at midday, with a gentle dawn and dusk.
    pub fn daylight(&self) -> f32 {
        let d = 0.5 - (self.time * std::f32::consts::TAU).cos() * 0.5;
        (d * 1.7 - 0.35).clamp(0.0, 1.0)
    }

    pub fn night(&self) -> bool {
        self.daylight() < 0.25
    }

    pub fn storm(&self) -> bool {
        self.kind == Sky::Storm && self.rain > 0.5
    }

    /// "DAY 3  06:40" style clock.
    pub fn clock_text(&self) -> String {
        let mins = (self.time * 24.0 * 60.0) as u32;
        format!("DAY {}  {:02}:{:02}", self.day, mins / 60, mins % 60)
    }

    pub fn label(&self) -> &'static str {
        self.kind.name()
    }

    pub fn set(&mut self, kind: Sky) {
        self.kind = kind;
        let (r, c, f, w) = kind.targets();
        self.rain = r;
        self.clouds = c;
        self.fog = f;
        self.gust = w;
        self.change_t = 240.0;
    }

    /// Wind right now, with slow swells and quick gusts.
    pub fn wind_now(&self) -> f32 {
        let c = self.clock;
        self.dir * self.gust * (0.6 + 0.4 * (c * 0.23).sin()) + (c * 1.7).sin() * (c * 0.31).sin() * self.gust * 0.4
    }

    fn pick_next(&mut self) {
        let dawn = (0.2..0.33).contains(&self.time);
        let weights = [(Sky::Clear, 4.0), (Sky::Cloudy, 3.0), (Sky::Rain, 2.2), (Sky::Storm, 1.0), (Sky::Fog, if dawn { 3.0 } else { 0.2 }), (Sky::Windy, 1.2)];
        let total: f32 = weights.iter().map(|w| w.1).sum();
        let mut r = self.rng.f32() * total;
        for (k, w) in weights {
            if r < w {
                self.kind = k;
                break;
            }
            r -= w;
        }
        self.dir = self.rng.sign() as f32;
        self.change_t = self.rng.rangef(70.0, 180.0);
    }

    pub fn update(&mut self, dt: f32, world: &mut World, fx: &mut Fx, (px, py): (f32, f32), sounds: &mut Vec<Sfx>) {
        let before = self.time;
        self.time = (self.time + dt / DAY_LEN).fract();
        if self.time < before {
            self.day += 1;
        }
        self.clock += dt;
        self.change_t -= dt;
        if self.change_t <= 0.0 && !self.locked {
            self.pick_next();
        }
        let (tr, tc, tf, tw) = self.kind.targets();
        let k = (dt * 0.06).min(1.0);
        self.rain += (tr - self.rain) * k;
        self.clouds += (tc - self.clouds) * k;
        self.fog += (tf - self.fog) * k;
        self.gust += (tw - self.gust) * k;
        for b in self.bolts.iter_mut() {
            b.1 -= dt;
        }
        self.bolts.retain(|b| b.1 > 0.0);

        let biome = world.gen.biome(px as i32);
        let surf = world.surface_at(px as i32) as f32;
        let outdoors = py < surf + 60.0 && py > -300.0;
        let target_sand = if biome == Biome::Desert && self.gust > 30.0 && outdoors { ((self.gust - 30.0) / 25.0).min(1.0) } else { 0.0 };
        self.sand += (target_sand - self.sand) * (dt * 0.5).min(1.0);

        // Precipitation falls only where there's open sky.
        if outdoors && self.rain > 0.05 {
            let (amount, snow) = match biome {
                Biome::Snow => (self.rain, true),
                Biome::Desert => (self.rain * 0.25, false),
                _ => (self.rain, false),
            };
            let wind = self.wind_now();
            for _ in 0..(amount * if snow { 10.0 } else { 16.0 }) as i32 {
                let x = px + self.rng.rangef(-220.0, 220.0);
                let y = py - 120.0 + self.rng.rangef(-20.0, 10.0);
                if y < world.surface_at(x as i32) as f32 && world.mat(x as i32, y as i32).passable() {
                    if snow {
                        fx.snow_flake(x, y, wind);
                    } else {
                        fx.rain_drop(x, y, wind);
                    }
                }
            }
        }
        // Wind you can see: long thin streams curling across the sky.
        let wind = self.wind_now();
        if wind.abs() > 18.0 && self.rng.chance(((wind.abs() - 18.0) / 40.0).min(1.0) * 0.6) {
            let x = px - wind.signum() * self.rng.rangef(120.0, 220.0) + self.rng.rangef(-60.0, 60.0);
            let y = py + self.rng.rangef(-90.0, 40.0);
            if y < world.surface_at(x as i32) as f32 {
                let curl = self.rng.rangef(1.0, 3.5) * if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                let life = self.rng.rangef(1.2, 2.2);
                fx.wind_stream(x, y, wind * self.rng.rangef(3.0, 4.5), self.rng.rangef(-8.0, 8.0), life, curl);
            }
        }
        // Sandstorm: sand streams across the dunes and drifts pile up.
        if self.sand > 0.05 {
            let wind = self.wind_now();
            for _ in 0..(self.sand * 18.0) as i32 {
                let x = px - wind.signum() * 200.0 + self.rng.rangef(-60.0, 260.0) * wind.signum();
                let y = py + self.rng.rangef(-90.0, 50.0);
                if world.mat(x as i32, y as i32) == M::Empty {
                    let v = wind * self.rng.rangef(3.0, 5.0);
                    let col = [214, 182 + self.rng.range(0, 20) as u8, 118];
                    fx.mote(x, y, v, self.rng.rangef(-6.0, 6.0), 1.2, col, 0.0, 0.0, 0.0);
                }
            }
            if self.rng.chance(self.sand * 0.4) {
                let x = px + self.rng.rangef(-150.0, 150.0);
                let y = world.surface_at(x as i32) as f32 - 20.0;
                let sh = self.rng.next_u32() as u8;
                fx.debris(x, y, wind * 2.0, 0.0, M::Sand, sh);
            }
        }

        // Thunderstorms: lightning prefers tall things like trees.
        if self.storm() && outdoors {
            self.bolt_t -= dt;
            if self.bolt_t <= 0.0 {
                self.bolt_t = self.rng.rangef(3.0, 9.0);
                let mut x = px + self.rng.rangef(-160.0, 160.0);
                for _ in 0..8 {
                    let cx = px + self.rng.rangef(-160.0, 160.0);
                    let top = world.surface_at(cx as i32);
                    if (2..20).any(|k| world.back(cx as i32, top - k) == M::Wood) {
                        x = cx;
                        break;
                    }
                }
                let mut y = py - 120.0;
                let mut pts = vec![(x, y)];
                let mut bx = x;
                while y < py + 140.0 {
                    y += 1.0;
                    if (y as i32) % 6 == 0 {
                        bx += self.rng.rangef(-3.0, 3.0);
                        pts.push((bx, y));
                    }
                    let (gx, gy) = (bx as i32, y as i32);
                    if !world.mat(gx, gy).passable() || world.back(gx, gy) == M::Wood {
                        break;
                    }
                }
                pts.push((bx, y));
                // A couple of forks.
                for _ in 0..3 {
                    let k = self.rng.range(1, pts.len() as i32 - 1).max(1) as usize;
                    let (mut fx_, mut fy_) = pts[k];
                    let dir = self.rng.sign() as f32;
                    let mut fork = vec![(fx_, fy_)];
                    for _ in 0..self.rng.range(3, 6) {
                        fx_ += dir * self.rng.rangef(2.0, 5.0);
                        fy_ += self.rng.rangef(3.0, 6.0);
                        fork.push((fx_, fy_));
                    }
                    self.bolts.push((fork, 0.3));
                }
                for k in -3..=3 {
                    world.ignite_at(bx as i32 + k, y as i32 - 2);
                    world.ignite_at(bx as i32 + k, y as i32 - 6);
                }
                fx.explode(world, bx, y, 4, 1.0);
                world.scorch(bx as i32, y as i32, 6, 0.9);
                fx.burst(bx, y, 40, 140.0, [210, 235, 255]);
                fx.flash = 0.8;
                fx.shake = (fx.shake + 6.0).min(14.0);
                fx.kick += 2.0;
                sounds.push(Sfx::Thunder);
                self.strikes.push((bx, y));
                self.bolts.push((pts, 0.35));
            } else if self.rng.chance(dt * 0.15) {
                // Distant lightning: a flicker and a far-off rumble.
                fx.flash = fx.flash.max(0.18);
            }
        }
    }

    /// Subtle life in the air: falling leaves, dust, fireflies, blowing sand, cave drips.
    pub fn ambience(&mut self, world: &World, fx: &mut Fx, (px, py): (f32, f32)) {
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
                if r < 0.3 + self.gust / 100.0 {
                    let col = if self.rng.chance(0.3) { [222, 146, 56] } else if biome == Biome::Snow { [46, 96, 56] } else { [96, 156, 66] };
                    fx.mote(x, y, wind * 0.4, 8.0, 7.0, col, 0.0, 10.0, 70.0);
                }
            } else if depth < -1 && gy > -240 {
                if biome == Biome::Snow && self.rain < 0.05 && r < 0.12 {
                    fx.mote(x, y, wind * 0.4, 14.0, 7.0, [240, 244, 252], 0.0, 4.0, 30.0);
                } else if self.night() && biome.green() && depth > -30 && r < 0.1 {
                    let (vx, vy) = (self.rng.rangef(-8.0, 8.0), self.rng.rangef(-6.0, 6.0));
                    fx.mote(x, y, vx, vy, 4.0, [210, 255, 120], 2.0, 0.0, 25.0);
                } else if !self.night() && self.rain < 0.1 && r < 0.05 {
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

    pub fn draw(&self, cv: &mut Canvas) {
        for (pts, life) in &self.bolts {
            if *life < 0.22 && ((life * 40.0) as i32) % 2 == 1 {
                continue;
            }
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
