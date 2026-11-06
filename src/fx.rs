//! Spells, flying debris, spark particles, screen shake and the fire glow buffer.

use crate::cell::Cell;
use crate::materials::{Kind, Material};
use crate::noise::Rng;
use crate::world::{shade, World, H, W};

const MAX_PARTICLES: usize = 6000;
const BOLT_SPEED: f32 = 230.0;

pub struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    /// `Empty` means a purely visual spark.
    mat: Material,
    shade: u8,
    life: f32,
}

pub struct Bolt {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
}

pub struct Fx {
    particles: Vec<Particle>,
    bolts: Vec<Bolt>,
    pub shake: f32,
    /// Explosion positions from the last step, consumed by the game loop for sound/knockback.
    pub explosions: Vec<(f32, f32)>,
    rng: Rng,
}

impl Fx {
    pub fn new(seed: u64) -> Self {
        Self {
            particles: Vec::with_capacity(MAX_PARTICLES),
            bolts: Vec::new(),
            shake: 0.0,
            explosions: Vec::new(),
            rng: Rng::new(seed ^ 0xF00D),
        }
    }

    pub fn clear(&mut self) {
        self.particles.clear();
        self.bolts.clear();
        self.explosions.clear();
    }

    pub fn cast(&mut self, x: f32, y: f32, tx: f32, ty: f32) {
        let (dx, dy) = (tx - x, ty - y);
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        let (vx, vy) = (dx / len * BOLT_SPEED, dy / len * BOLT_SPEED);
        self.bolts.push(Bolt { x, y, vx, vy, life: 2.0 });
        for _ in 0..6 {
            let a = self.rng.rangef(-0.6, 0.6);
            let s = self.rng.rangef(20.0, 60.0);
            let (c, si) = (a.cos(), a.sin());
            self.spark(x, y, (vx * c - vy * si) / BOLT_SPEED * s, (vx * si + vy * c) / BOLT_SPEED * s, 0.25);
        }
    }

    fn spark(&mut self, x: f32, y: f32, vx: f32, vy: f32, life: f32) {
        if self.particles.len() < MAX_PARTICLES {
            self.particles.push(Particle { x, y, vx, vy, mat: Material::Empty, shade: 0, life });
        }
    }

    pub fn step(&mut self, world: &mut World, dt: f32) {
        // Bolts: march in half-cell steps so they never tunnel through thin walls.
        let mut bolts = std::mem::take(&mut self.bolts);
        bolts.retain_mut(|b| {
            b.vy += 30.0 * dt;
            b.life -= dt;
            let dist = (b.vx * b.vx + b.vy * b.vy).sqrt() * dt;
            let n = (dist / 0.5).ceil().max(1.0) as i32;
            let (sx, sy) = (b.vx * dt / n as f32, b.vy * dt / n as f32);
            for _ in 0..n {
                let (nx, ny) = (b.x + sx, b.y + sy);
                let (cx, cy) = (nx.floor() as i32, ny.floor() as i32);
                if !World::in_bounds(cx, cy) || !world.mat(cx, cy).passable() {
                    self.explode(world, b.x, b.y, 7);
                    return false;
                }
                b.x = nx;
                b.y = ny;
                if self.rng.chance(0.3) {
                    let (vx, vy) = (self.rng.rangef(-15.0, 15.0), self.rng.rangef(-15.0, 15.0));
                    let life = self.rng.rangef(0.1, 0.35);
                    self.spark(b.x, b.y, vx - b.vx * 0.05, vy - b.vy * 0.05, life);
                }
            }
            if b.life <= 0.0 {
                self.explode(world, b.x, b.y, 7);
                return false;
            }
            true
        });
        self.bolts = bolts;

        let mut ps = std::mem::take(&mut self.particles);
        ps.retain_mut(|p| {
            p.life -= dt;
            p.vy += 220.0 * dt;
            p.vx *= 1.0 - 0.6 * dt;
            let n = (p.vx.abs().max(p.vy.abs()) * dt).ceil().max(1.0) as i32;
            let (sx, sy) = (p.vx * dt / n as f32, p.vy * dt / n as f32);
            for _ in 0..n {
                let (nx, ny) = (p.x + sx, p.y + sy);
                let (cx, cy) = (nx.floor() as i32, ny.floor() as i32);
                if !World::in_bounds(cx, cy) {
                    return false;
                }
                let m = world.mat(cx, cy);
                if !m.passable() {
                    if p.mat == Material::Empty {
                        if m.flammable() && self.rng.chance(0.3) {
                            let f = Cell::fire(true, &mut world.rng);
                            world.set(cx, cy, f);
                        }
                    } else {
                        deposit(world, p);
                    }
                    return false;
                }
                p.x = nx;
                p.y = ny;
            }
            if p.life <= 0.0 {
                if p.mat != Material::Empty {
                    deposit(world, p);
                }
                return false;
            }
            true
        });
        // Keep any particles spawned while we were iterating.
        ps.append(&mut self.particles);
        self.particles = ps;

        self.shake *= (1.0 - 7.0 * dt).max(0.0);
    }

    /// Blast a circle out of the world, flinging its contents as debris and lighting fires.
    pub fn explode(&mut self, world: &mut World, x: f32, y: f32, r: i32) {
        self.explosions.push((x, y));
        self.shake = (self.shake + 6.0).min(12.0);
        let (cx, cy) = (x.floor() as i32, y.floor() as i32);
        let rf = r as f32;
        for dy in -(r + 2)..=(r + 2) {
            for dx in -(r + 2)..=(r + 2) {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                let (px, py) = (cx + dx, cy + dy);
                if d > rf + 2.0 || !World::in_bounds(px, py) {
                    continue;
                }
                let c = world.cells[World::idx(px, py)];
                let m = c.mat;
                if d > rf {
                    // Scorched rim.
                    if m.flammable() {
                        let f = Cell::fire(true, &mut world.rng);
                        world.set(px, py, f);
                    } else if m == Material::Empty && self.rng.chance(0.25) {
                        let f = Cell::fire(false, &mut world.rng);
                        world.set(px, py, f);
                    }
                    continue;
                }
                if m.flammable() {
                    let f = Cell::fire(true, &mut world.rng);
                    world.set(px, py, f);
                    continue;
                }
                if m == Material::Fire {
                    continue;
                }
                let throw = match m.kind() {
                    Kind::Static => 0.3,
                    Kind::Powder => 0.6,
                    Kind::Liquid => 0.5,
                    _ => 0.0,
                };
                if throw > 0.0 && self.rng.chance(throw) && self.particles.len() < MAX_PARTICLES {
                    let dl = d.max(0.5);
                    let speed = self.rng.rangef(60.0, 170.0) * (1.2 - d / rf * 0.6);
                    self.particles.push(Particle {
                        x: px as f32 + 0.5,
                        y: py as f32 + 0.5,
                        vx: dx as f32 / dl * speed + self.rng.rangef(-20.0, 20.0),
                        vy: dy as f32 / dl * speed - self.rng.rangef(30.0, 80.0),
                        mat: if m == Material::Stone { Material::Rubble } else { m },
                        shade: c.shade,
                        life: 4.0,
                    });
                }
                if self.rng.chance(0.1) {
                    let f = Cell::fire(false, &mut world.rng);
                    world.set(px, py, f);
                } else {
                    world.set(px, py, Cell::EMPTY);
                }
            }
        }
        for _ in 0..50 {
            let a = self.rng.rangef(0.0, std::f32::consts::TAU);
            let s = self.rng.rangef(40.0, 200.0);
            let life = self.rng.rangef(0.2, 0.7);
            self.spark(x, y, a.cos() * s, a.sin() * s - 30.0, life);
        }
    }

    /// Draw particles and bolts on top of the world image.
    pub fn draw(&self, px: &mut [u8]) {
        for p in &self.particles {
            let rgb = if p.mat == Material::Empty {
                let t = (p.life * 3.0).min(1.0);
                [255, (150.0 + 100.0 * t) as u8, (40.0 + 160.0 * t) as u8]
            } else {
                shade(p.mat.base_color(), p.shade, p.mat.variation())
            };
            put(px, p.x as i32, p.y as i32, rgb);
        }
        for b in &self.bolts {
            let (x, y) = (b.x as i32, b.y as i32);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                put(px, x + dx, y + dy, [120, 200, 255]);
            }
            put(px, x, y, [240, 250, 255]);
        }
    }

    pub fn light_sources(&self, mut add: impl FnMut(f32, f32, [f32; 3])) {
        for p in &self.particles {
            if p.mat == Material::Empty {
                add(p.x, p.y, [0.9, 0.6, 0.2]);
            }
        }
        for b in &self.bolts {
            add(b.x, b.y, [3.0, 5.0, 8.0]);
        }
    }
}

fn deposit(world: &mut World, p: &Particle) {
    let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
    for yy in [y, y - 1] {
        if World::in_bounds(x, yy) && world.mat(x, yy).passable() {
            let mut c = Cell::new(p.mat, &mut world.rng);
            c.shade = p.shade;
            world.set(x, yy, c);
            return;
        }
    }
}

#[inline]
pub fn put(px: &mut [u8], x: i32, y: i32, rgb: [u8; 3]) {
    if x < 0 || y < 0 || x as usize >= W || y as usize >= H {
        return;
    }
    let o = (y as usize * W + x as usize) * 4;
    px[o] = rgb[0];
    px[o + 1] = rgb[1];
    px[o + 2] = rgb[2];
}

/// Low-res light map for the additive fire glow.
pub const GS: usize = 4;
pub const GW: usize = W / GS;
pub const GH: usize = H / GS;

pub struct Glow {
    rgb: Vec<[f32; 3]>,
    tmp: Vec<[f32; 3]>,
}

impl Glow {
    pub fn new() -> Self {
        Self { rgb: vec![[0.0; 3]; GW * GH], tmp: vec![[0.0; 3]; GW * GH] }
    }

    pub fn compute(&mut self, world: &World, fx: &Fx, out: &mut [u8]) {
        self.rgb.fill([0.0; 3]);
        let rgb = &mut self.rgb;
        let mut add = |x: f32, y: f32, c: [f32; 3]| {
            let (gx, gy) = ((x as usize / GS).min(GW - 1), (y as usize / GS).min(GH - 1));
            let v = &mut rgb[gy * GW + gx];
            v[0] += c[0];
            v[1] += c[1];
            v[2] += c[2];
        };
        for y in 0..H {
            for x in 0..W {
                let c = &world.cells[y * W + x];
                match c.mat {
                    Material::Fire => {
                        let k = if c.temp > 200 { 1.0 } else { 0.7 };
                        add(x as f32, y as f32, [1.0 * k, 0.48 * k, 0.12 * k]);
                    }
                    Material::Acid => add(x as f32, y as f32, [0.03, 0.12, 0.02]),
                    _ => {}
                }
            }
        }
        fx.light_sources(&mut add);

        for _ in 0..2 {
            blur(&self.rgb, &mut self.tmp, 1, 0);
            blur(&self.tmp, &mut self.rgb, 0, 1);
        }
        for (i, v) in self.rgb.iter().enumerate() {
            let o = i * 4;
            for k in 0..3 {
                out[o + k] = ((1.0 - (-v[k] * 0.09).exp()) * 235.0) as u8;
            }
            out[o + 3] = 255;
        }
    }
}

fn blur(src: &[[f32; 3]], dst: &mut [[f32; 3]], sx: i32, sy: i32) {
    for y in 0..GH as i32 {
        for x in 0..GW as i32 {
            let mut acc = [0.0f32; 3];
            for k in -2..=2 {
                let (xx, yy) = (x + k * sx, y + k * sy);
                if xx < 0 || yy < 0 || xx >= GW as i32 || yy >= GH as i32 {
                    continue;
                }
                let s = src[yy as usize * GW + xx as usize];
                acc[0] += s[0];
                acc[1] += s[1];
                acc[2] += s[2];
            }
            dst[y as usize * GW + x as usize] = [acc[0] * 0.24, acc[1] * 0.24, acc[2] * 0.24];
        }
    }
}
