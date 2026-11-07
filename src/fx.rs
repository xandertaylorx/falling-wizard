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
}

pub struct Fx {
    particles: Vec<Particle>,
    pub shake: f32,
    /// Full-screen flash (lightning), decays quickly.
    pub flash: f32,
    /// Explosions this step as (x, y, radius), consumed by the game for sound/damage.
    pub explosions: Vec<(f32, f32, i32)>,
    /// Black holes pulling particles in.
    pub attractors: Vec<(f32, f32)>,
    pub rng: Rng,
}

pub const SPARK: [u8; 3] = [255, 200, 90];
pub const FROST: [u8; 3] = [170, 230, 255];
pub const MAGIC: [u8; 3] = [200, 140, 255];
pub const RAIN: [u8; 3] = [150, 180, 230];

impl Fx {
    pub fn new(seed: u64) -> Self {
        Self { particles: Vec::with_capacity(MAX_PARTICLES), shake: 0.0, flash: 0.0, explosions: Vec::new(), attractors: Vec::new(), rng: Rng::new(seed ^ 0xF00D) }
    }

    pub fn spark(&mut self, x: f32, y: f32, vx: f32, vy: f32, life: f32, col: [u8; 3]) {
        if self.particles.len() < MAX_PARTICLES {
            self.particles.push(Particle { x, y, vx, vy, mat: M::Empty, shade: 0, life, col, glow: 0.6, gravity: 220.0 });
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
            self.particles.push(Particle { x, y, vx, vy, mat, shade, life: 4.0, col: [0; 3], glow: 0.0, gravity: 220.0 });
        }
    }

    pub fn count(&self) -> usize {
        self.particles.len()
    }

    pub fn step(&mut self, world: &mut World, dt: f32) {
        let mut ps = std::mem::take(&mut self.particles);
        let attractors = std::mem::take(&mut self.attractors);
        ps.retain_mut(|p| {
            p.life -= dt;
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
            if !pulled {
                p.vy += p.gravity * dt;
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

    pub fn rain_drop(&mut self, x: f32, y: f32, wind: f32) {
        if self.particles.len() < MAX_PARTICLES {
            self.particles.push(Particle { x, y, vx: wind, vy: 260.0, mat: M::Empty, shade: 0, life: 2.0, col: RAIN, glow: 0.0, gravity: 0.0 });
        }
    }

    /// Blast a circle out of the world, flinging debris and lighting fires.
    /// `fire` scales how much flame is left behind (0..1).
    pub fn explode(&mut self, world: &mut World, x: f32, y: f32, r: i32, fire: f32) {
        self.explosions.push((x, y, r));
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
                if m == M::Empty || !world.in_window(px, py) || m.kind() == Kind::Gas || m == M::Fire {
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
        for p in &self.particles {
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
