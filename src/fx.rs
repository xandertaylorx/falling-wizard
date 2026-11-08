//! Particles (sparks and flying debris), explosions, freezing and dissolving.

use crate::cell::Cell;
use crate::materials::{Kind, Material as M};
use crate::noise::Rng;
use crate::render::{shade, Canvas};
use crate::world::World;

const MAX_PARTICLES: usize = 8000;

pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// `Empty` means a purely visual spark.
    pub mat: M,
    pub shade: u8,
    pub life: f32,
    pub col: [u8; 3],
    pub glow: f32,
    pub gravity: f32,
    /// Side-to-side drift (leaves, snow).
    pub flutter: f32,
    /// Drawn finer than a world pixel: 0 pixel, 1 rain streak, 2 soft ember, 3 wind stream.
    pub fine: u8,
    /// Starting life, for fading in and out.
    pub max: f32,
    /// How hard a wind stream curls.
    pub curl: f32,
}

pub const FINE_RAIN: u8 = 1;
pub const FINE_EMBER: u8 = 2;
pub const FINE_WIND: u8 = 3;

pub struct Fx {
    particles: Vec<Particle>,
    pub shake: f32,
    /// Full-screen flash (lightning), decays quickly.
    pub flash: f32,
    /// Explosions this step as (x, y, radius), consumed by the game for sound/damage.
    pub explosions: Vec<(f32, f32, i32, crate::mobs::Dmg)>,
    /// Requested freeze-frame and zoom punch, consumed by the game.
    pub hitstop: f32,
    pub kick: f32,
    /// Black holes pulling particles in.
    pub attractors: Vec<(f32, f32)>,
    /// Tornadoes: (x, ground y, height).
    pub tornadoes: Vec<(f32, f32, f32)>,
    /// Ambient wind, drifts light particles.
    pub wind: f32,
    time: f32,
    pub rng: Rng,
}

pub const SPARK: [u8; 3] = [255, 200, 90];
pub const FROST: [u8; 3] = [170, 230, 255];
pub const MAGIC: [u8; 3] = [200, 140, 255];
pub const RAIN: [u8; 3] = [150, 180, 230];
pub const SNOWF: [u8; 3] = [244, 248, 255];

impl Fx {
    pub fn new(seed: u64) -> Self {
        Self { particles: Vec::with_capacity(MAX_PARTICLES), shake: 0.0, flash: 0.0, explosions: Vec::new(), hitstop: 0.0, kick: 0.0, attractors: Vec::new(), tornadoes: Vec::new(), wind: 0.0, time: 0.0, rng: Rng::new(seed ^ 0xF00D) }
    }

    pub fn spark(&mut self, x: f32, y: f32, vx: f32, vy: f32, life: f32, col: [u8; 3]) {
        if self.particles.len() < MAX_PARTICLES {
            let fiery = col[0] >= 200 && col[2] <= 140 && col[1] <= 220;
            let fine = if fiery && self.rng.chance(0.5) { FINE_EMBER } else { 0 };
            self.particles.push(Particle { x, y, vx, vy, mat: M::Empty, shade: 0, life, col, glow: 0.6, gravity: 220.0, flutter: 0.0, fine, max: life, curl: 0.0 });
        }
    }

    pub fn burst(&mut self, x: f32, y: f32, n: usize, speed: f32, col: [u8; 3]) {
        for _ in 0..n {
            let a = self.rng.rangef(0.0, std::f32::consts::TAU);
            let s = self.rng.rangef(speed * 0.3, speed);
            let life = self.rng.rangef(0.2, 0.6);
            self.spark(x, y, a.cos() * s, a.sin() * s - 20.0, life, col);
        }
    }

    /// A material particle that becomes a cell where it lands.
    pub fn debris(&mut self, x: f32, y: f32, vx: f32, vy: f32, mat: M, shade: u8) {
        if self.particles.len() < MAX_PARTICLES {
            self.particles.push(Particle { x, y, vx, vy, mat, shade, life: 4.0, col: [0; 3], glow: 0.0, gravity: 220.0, flutter: 0.0, fine: 0, max: 4.0, curl: 0.0 });
        }
    }

    /// A light ambient particle (dust, leaf, snowflake, firefly).
    #[allow(clippy::too_many_arguments)]
    pub fn mote(&mut self, x: f32, y: f32, vx: f32, vy: f32, life: f32, col: [u8; 3], glow: f32, gravity: f32, flutter: f32) {
        if self.particles.len() < MAX_PARTICLES - 500 {
            self.particles.push(Particle { x, y, vx, vy, mat: M::Empty, shade: 0, life, col, glow, gravity, flutter, fine: 0, max: life, curl: 0.0 });
        }
    }

    /// Shove particles inside a cone (wind spells).
    pub fn push_cone(&mut self, x: f32, y: f32, ux: f32, uy: f32, len: f32, half: f32, force: f32) {
        for p in self.particles.iter_mut() {
            let (dx, dy) = (p.x - x, p.y - y);
            let along = dx * ux + dy * uy;
            if along < 0.0 || along > len {
                continue;
            }
            let side = (dx * -uy + dy * ux).abs();
            if side < along * half + 3.0 {
                p.vx += ux * force;
                p.vy += uy * force;
            }
        }
    }

    pub fn clear(&mut self) {
        self.particles.clear();
        self.explosions.clear();
        self.attractors.clear();
        self.tornadoes.clear();
    }

    pub fn count(&self) -> usize {
        self.particles.len()
    }

    pub fn step(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        let mut ps = std::mem::take(&mut self.particles);
        let attractors = std::mem::take(&mut self.attractors);
        let time = self.time;
        ps.retain_mut(|p| {
            p.life -= dt;
            if p.fine == FINE_WIND {
                // Swirl: the curl swings back and forth, so streams draw S-curves.
                let age = p.max - p.life;
                let turn = p.curl * (age * 5.0 + p.y * 0.05 + time).sin() * dt;
                let (sn, cs) = turn.sin_cos();
                let (vx, vy) = (p.vx * cs - p.vy * sn, p.vx * sn + p.vy * cs);
                p.vx = vx;
                p.vy = vy;
                p.x += p.vx * dt;
                p.y += p.vy * dt;
                return p.life > 0.0;
            }
            let mut pulled = false;
            for &(ax, ay) in &attractors {
                let (dx, dy) = (ax - p.x, ay - p.y);
                let d = (dx * dx + dy * dy).sqrt();
                if d < 2.5 {
                    return false;
                }
                if d < 90.0 {
                    pulled = true;
                    let k = 1400.0 / (d + 6.0);
                    p.vx += (dx / d * k + dy / d * k * 0.35) * dt * 8.0;
                    p.vy += (dy / d * k - dx / d * k * 0.35) * dt * 8.0;
                    let sp = (p.vx * p.vx + p.vy * p.vy).sqrt();
                    if sp > 320.0 {
                        p.vx *= 320.0 / sp;
                        p.vy *= 320.0 / sp;
                    }
                    p.life = p.life.max(0.5);
                }
            }
            for &(tx, ty, th) in &self.tornadoes {
                let (dx, h) = (tx - p.x, ty - p.y);
                if dx.abs() < 7.0 + h.max(0.0) * 0.25 && h > -4.0 && h < th {
                    pulled = true;
                    // Spin around the funnel and climb; fling out at the top.
                    let spin = (self.time * 9.0 + p.y * 0.4).sin();
                    p.vx += (dx * 9.0 + spin * 260.0) * dt;
                    p.vy -= 330.0 * dt;
                    p.vy = p.vy.max(-150.0);
                    if h > th - 6.0 {
                        p.vx += -dx.signum() * 500.0 * dt;
                    }
                    p.life = p.life.max(0.6);
                }
            }
            if !pulled {
                p.vy += p.gravity * dt;
            }
            if p.mat == M::Empty && p.gravity < 100.0 {
                p.vx += (self.wind * 0.8 - p.vx) * dt * 0.6;
            }
            if p.flutter > 0.0 {
                p.vx += (self.time * 3.0 + p.y * 0.2).sin() * p.flutter * dt;
            }
            p.vx *= 1.0 - 0.6 * dt;
            let n = (p.vx.abs().max(p.vy.abs()) * dt).ceil().max(1.0) as i32;
            let (sx, sy) = (p.vx * dt / n as f32, p.vy * dt / n as f32);
            for _ in 0..n {
                let (nx, ny) = (p.x + sx, p.y + sy);
                let (cx, cy) = (nx.floor() as i32, ny.floor() as i32);
                if !world.in_window(cx, cy) {
                    return false;
                }
                let m = world.mat(cx, cy);
                if p.col == SNOWF && p.mat == M::Empty {
                    if m == M::Fire && self.rng.chance(0.3) {
                        world.set(cx, cy, Cell::EMPTY);
                        return false;
                    }
                    if !m.passable() {
                        // Snow settles and builds up.
                        if m.kind() != Kind::Liquid && self.rng.chance(0.05) && world.mat(cx, cy - 1) == M::Empty {
                            world.place(cx, cy - 1, M::Snow);
                        }
                        return false;
                    }
                    p.x = nx;
                    p.y = ny;
                    continue;
                }
                if p.col == RAIN && p.mat == M::Empty {
                    if m == M::Fire {
                        world.set(cx, cy, Cell::EMPTY);
                        return false;
                    }
                    if !m.passable() || m.kind() == Kind::Liquid {
                        if self.rng.chance(0.02) && world.mat(cx, cy - 1) == M::Empty {
                            world.place(cx, cy - 1, M::Water);
                        }
                        return false;
                    }
                    p.x = nx;
                    p.y = ny;
                    continue;
                }
                let blocked = if p.mat == M::Empty { !m.passable() || m.flammable() } else { !m.passable() };
                if blocked {
                    if p.mat == M::Empty {
                        if m.flammable() && p.col == SPARK && self.rng.chance(0.3) {
                            world.ignite_at(cx, cy);
                        }
                        if !m.passable() {
                            return false;
                        }
                    } else if pulled {
                        world.set(cx, cy, Cell::EMPTY);
                    } else {
                        deposit(world, p);
                        return false;
                    }
                }
                p.x = nx;
                p.y = ny;
            }
            if p.life <= 0.0 {
                if p.mat != M::Empty {
                    deposit(world, p);
                }
                return false;
            }
            true
        });
        ps.append(&mut self.particles);
        self.particles = ps;
        self.attractors = attractors;
        self.shake *= (1.0 - 7.0 * dt).max(0.0);
        self.flash = (self.flash - dt * 3.0).max(0.0);
    }

    pub fn snow_flake(&mut self, x: f32, y: f32, wind: f32) {
        if self.particles.len() < MAX_PARTICLES - 800 {
            self.particles.push(Particle { x, y, vx: wind * 0.5, vy: 28.0, mat: M::Empty, shade: 0, life: 9.0, col: SNOWF, glow: 0.0, gravity: 0.0, flutter: 35.0, fine: 0, max: 9.0, curl: 0.0 });
        }
    }

    /// A curling streak of wind, drawn as a thin flowing line.
    pub fn wind_stream(&mut self, x: f32, y: f32, vx: f32, vy: f32, life: f32, curl: f32) {
        if self.particles.len() < MAX_PARTICLES - 300 {
            self.particles.push(Particle { x, y, vx, vy, mat: M::Empty, shade: 0, life, col: [236, 244, 255], glow: 0.0, gravity: 0.0, flutter: 0.0, fine: FINE_WIND, max: life, curl });
        }
    }

    pub fn rain_drop(&mut self, x: f32, y: f32, wind: f32) {
        if self.particles.len() < MAX_PARTICLES {
            self.particles.push(Particle { x, y, vx: wind, vy: 260.0, mat: M::Empty, shade: 0, life: 2.0, col: RAIN, glow: 0.0, gravity: 0.0, flutter: 0.0, fine: FINE_RAIN, max: 2.0, curl: 0.0 });
        }
    }

    /// Blast a circle out of the world, flinging debris and lighting fires.
    /// `fire` scales how much flame is left behind (0..1).
    pub fn explode(&mut self, world: &mut World, x: f32, y: f32, r: i32, fire: f32) {
        self.explosions.push((x, y, r, if fire >= 0.9 { crate::mobs::Dmg::Fire } else { crate::mobs::Dmg::Blast }));
        if r >= 6 {
            world.scorch(x as i32, y as i32, r + 2, 0.55);
        }
        self.shake = (self.shake + 1.5 + r as f32 * 0.6).min(14.0);
        let (cx, cy) = (x.floor() as i32, y.floor() as i32);
        let rf = r as f32;
        for dy in -(r + 2)..=(r + 2) {
            for dx in -(r + 2)..=(r + 2) {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                let (px, py) = (cx + dx, cy + dy);
                if d > rf + 2.0 {
                    continue;
                }
                let Some(c) = world.get(px, py) else { continue };
                let m = c.mat;
                if c.back.foliage() && d <= rf {
                    world.set_back(px, py, M::Empty);
                    if c.back.flammable() && self.rng.chance(0.3 * fire) {
                        world.ignite_at(px, py);
                    } else if self.rng.chance(0.4) {
                        let col = shade(c.back.base_color(), self.rng.next_u32() as u8, 0.4);
                        let (vx, vy) = (dx as f32 * 12.0 + self.rng.rangef(-30.0, 30.0), dy as f32 * 12.0 - self.rng.rangef(40.0, 90.0));
                        let life = self.rng.rangef(0.8, 1.6);
                        self.mote(px as f32, py as f32, vx, vy, life, col, 0.0, 120.0, 30.0);
                    }
                }
                if d > rf {
                    if m.flammable() && m != M::Gunpowder {
                        world.ignite_at(px, py);
                    } else if m == M::Empty && self.rng.chance(0.25 * fire) {
                        let f = Cell::fire(&mut world.rng);
                        world.set(px, py, f);
                    }
                    continue;
                }
                match m {
                    M::Empty | M::Smoke | M::Steam => {
                        if self.rng.chance(0.15 * fire) {
                            let f = Cell::fire(&mut world.rng);
                            world.set(px, py, f);
                        }
                        continue;
                    }
                    M::Fire => continue,
                    m if m.indestructible() => continue,
                    M::Gunpowder => {
                        world.ignite_at(px, py);
                        continue;
                    }
                    M::Basalt if d > rf * 0.6 => continue,
                    M::Leaves | M::Plant | M::Sprout => {
                        if self.rng.chance(0.5) {
                            world.ignite_at(px, py);
                        } else {
                            world.set(px, py, Cell::EMPTY);
                        }
                        continue;
                    }
                    _ => {}
                }
                if m.flammable() && self.rng.chance(0.6) {
                    world.ignite_at(px, py);
                    continue;
                }
                let throw = match m.kind() {
                    Kind::Static => 0.3,
                    Kind::Powder => 0.6,
                    Kind::Liquid => 0.5,
                    _ => 0.0,
                };
                if throw > 0.0 && self.rng.chance(throw) {
                    let dl = d.max(0.5);
                    let speed = self.rng.rangef(60.0, 170.0) * (1.2 - d / rf * 0.6);
                    let mat = match m {
                        M::Stone | M::Basalt | M::Glass => M::Rubble,
                        M::Grass => M::Dirt,
                        M::Ice => M::Snow,
                        other => other,
                    };
                    let (vx, vy) = (dx as f32 / dl * speed + self.rng.rangef(-20.0, 20.0), dy as f32 / dl * speed - self.rng.rangef(30.0, 80.0));
                    self.debris(px as f32 + 0.5, py as f32 + 0.5, vx, vy, mat, c.shade);
                }
                if self.rng.chance(0.1 * fire) {
                    let f = Cell::fire(&mut world.rng);
                    world.set(px, py, f);
                } else {
                    world.set(px, py, Cell::EMPTY);
                }
            }
        }
        self.burst(x, y, 20 + r as usize * 5, 60.0 + r as f32 * 14.0, SPARK);
    }

    /// Frost: water to ice, lava to basalt, fire out, and a dusting of snow.
    pub fn freeze(&mut self, world: &mut World, x: f32, y: f32, r: i32) {
        let (cx, cy) = (x.floor() as i32, y.floor() as i32);
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (px, py) = (cx + dx, cy + dy);
                match world.mat(px, py) {
                    M::Water => world.place(px, py, M::Ice),
                    M::Lava => world.place(px, py, M::Basalt),
                    M::Fire | M::Steam => world.set(px, py, Cell::EMPTY),
                    M::Empty => {
                        if world.mat(px, py + 1).solid() && self.rng.chance(0.5) {
                            world.place(px, py, M::Snow);
                        }
                    }
                    _ => {}
                }
            }
        }
        self.burst(x, y, 40, 120.0, FROST);
    }

    /// Remove solid cells in a disc with probability `chance`. Returns gold dug up.
    pub fn dissolve(&mut self, world: &mut World, x: f32, y: f32, r: i32, chance: f32) -> u32 {
        let (cx, cy) = (x.floor() as i32, y.floor() as i32);
        let mut gold = 0;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (px, py) = (cx + dx, cy + dy);
                let m = world.mat(px, py);
                if m == M::Empty && world.back(px, py).foliage() && self.rng.chance(chance) {
                    world.set_back(px, py, M::Empty);
                    continue;
                }
                if m == M::Empty || !world.in_window(px, py) || m.kind() == Kind::Gas || m == M::Fire {
                    continue;
                }
                if m.indestructible() {
                    continue;
                }
                let p = if m == M::Basalt { chance * 0.3 } else { chance };
                if self.rng.chance(p) {
                    if m == M::Gold {
                        gold += 1;
                    }
                    world.set(px, py, Cell::EMPTY);
                    if self.rng.chance(0.08) {
                        let (vx, vy) = (self.rng.rangef(-30.0, 30.0), self.rng.rangef(-40.0, 0.0));
                        let col = shade(m.base_color(), 200, 0.3);
                        self.spark(px as f32 + 0.5, py as f32 + 0.5, vx, vy, 0.3, col);
                    }
                }
            }
        }
        gold
    }

    pub fn draw(&self, cv: &mut Canvas) {
        for p in self.particles.iter().filter(|p| p.fine == 0) {
            let rgb = if p.mat == M::Empty {
                let t = (p.life * 3.0).min(1.0);
                [
                    (p.col[0] as f32 * (0.6 + 0.4 * t)) as u8,
                    (p.col[1] as f32 * (0.6 + 0.4 * t)) as u8,
                    (p.col[2] as f32 * (0.6 + 0.4 * t)) as u8,
                ]
            } else {
                shade(p.mat.base_color(), p.shade, p.mat.variation())
            };
            cv.put(p.x as i32, p.y as i32, rgb);
        }
    }

    /// Finer-than-pixel effects drawn over the finished frame: hair-thin rain, soft
    /// glowing embers, and flowing wind lines. `left`/`top` is the view's corner in
    /// world cells and `s` its screen pixels per cell.
    pub fn draw_fine(&self, left: f32, top: f32, s: f32) {
        use macroquad::prelude::{draw_circle, draw_line, Color};
        let to = |x: f32, y: f32| ((x - left) * s, (y - top) * s);
        let col = |c: [u8; 3], a: f32| Color::new(c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, a.clamp(0.0, 1.0));
        for p in self.particles.iter().filter(|p| p.fine != 0) {
            let (x, y) = to(p.x, p.y);
            match p.fine {
                FINE_RAIN => {
                    let (tx, ty) = to(p.x - p.vx * 0.011, p.y - p.vy * 0.011);
                    draw_line(tx, ty, x, y, 1.0, col(p.col, 0.3));
                }
                FINE_EMBER => {
                    let f = (p.life / p.max.max(0.01)).clamp(0.0, 1.0);
                    let r = (0.28 + 0.2 * f) * s;
                    let warm = [p.col[0], (p.col[1] as f32 * (0.55 + 0.45 * f)) as u8, (p.col[2] as f32 * f) as u8];
                    draw_circle(x, y, r * 3.0, col(warm, 0.12 * f));
                    let (tx, ty) = to(p.x - p.vx * 0.025, p.y - p.vy * 0.025);
                    draw_line(tx, ty, x, y, r * 1.1, col(warm, 0.45 * f));
                    draw_circle(x, y, r, col([255, 240, 200], 0.95 * f + 0.05));
                }
                _ => {
                    // Walk backwards along the curl to draw the stream's tail.
                    let env = (std::f32::consts::PI * (1.0 - p.life / p.max.max(0.01))).sin();
                    let (mut px, mut py, mut vx, mut vy) = (p.x, p.y, p.vx, p.vy);
                    let age = p.max - p.life;
                    let n = 12;
                    for k in 0..n {
                        let h = 0.014;
                        let turn = -p.curl * ((age - k as f32 * h) * 5.0 + py * 0.05 + self.time).sin() * h;
                        let (sn, cs) = turn.sin_cos();
                        let (nvx, nvy) = (vx * cs - vy * sn, vx * sn + vy * cs);
                        vx = nvx;
                        vy = nvy;
                        let (qx, qy) = (px - vx * h, py - vy * h);
                        let (a, b) = (to(px, py), to(qx, qy));
                        let fade = 1.0 - k as f32 / n as f32;
                        draw_line(a.0, a.1, b.0, b.1, 1.0 + fade * 0.8, col(p.col, 0.42 * env * fade));
                        px = qx;
                        py = qy;
                    }
                }
            }
        }
    }

    pub fn lights(&self, add: &mut impl FnMut(f32, f32, [f32; 3])) {
        for p in &self.particles {
            if p.glow > 0.0 {
                let k = p.glow * (p.life * 3.0).min(1.0) / 255.0;
                add(p.x, p.y, [p.col[0] as f32 * k, p.col[1] as f32 * k, p.col[2] as f32 * k]);
            }
        }
    }
}

fn deposit(world: &mut World, p: &Particle) {
    let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
    for yy in [y, y - 1] {
        let m = world.mat(x, yy);
        if world.in_window(x, yy) && m.passable() && m != M::Leaves && m != M::Plant {
            let mut c = Cell::new(p.mat, &mut world.rng);
            c.shade = p.shade;
            world.set(x, yy, c);
            return;
        }
    }
}
