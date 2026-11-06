//! The wizard's spells: projectiles, beams and sprays.

use crate::audio::Sfx;
use crate::cell::Cell;
use crate::fx::{Fx, FROST, MAGIC, SPARK};
use crate::materials::Material as M;
use crate::render::Canvas;
use crate::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spell {
    Spark,
    Fireball,
    Bomb,
    DigBeam,
    Torrent,
    Frost,
    Teleport,
    Seeds,
}

pub const SPELLS: [Spell; 8] =
    [Spell::Spark, Spell::Fireball, Spell::Bomb, Spell::DigBeam, Spell::Torrent, Spell::Frost, Spell::Teleport, Spell::Seeds];

impl Spell {
    pub fn name(self) -> &'static str {
        match self {
            Spell::Spark => "Spark Bolt",
            Spell::Fireball => "Fireball",
            Spell::Bomb => "Bomb",
            Spell::DigBeam => "Digging Beam",
            Spell::Torrent => "Torrent",
            Spell::Frost => "Frost",
            Spell::Teleport => "Teleport",
            Spell::Seeds => "Seeds of Life",
        }
    }

    /// Mana per cast, or per second for held spells.
    pub fn cost(self) -> f32 {
        match self {
            Spell::Spark => 4.0,
            Spell::Fireball => 14.0,
            Spell::Bomb => 30.0,
            Spell::DigBeam => 22.0,
            Spell::Torrent => 20.0,
            Spell::Frost => 12.0,
            Spell::Teleport => 25.0,
            Spell::Seeds => 10.0,
        }
    }

    pub fn continuous(self) -> bool {
        matches!(self, Spell::DigBeam | Spell::Torrent)
    }

    pub fn cooldown(self) -> f32 {
        match self {
            Spell::Spark => 0.15,
            Spell::Fireball => 0.45,
            Spell::Bomb => 0.8,
            Spell::Frost => 0.5,
            Spell::Teleport => 0.6,
            Spell::Seeds => 0.35,
            _ => 0.0,
        }
    }

    /// 7x7 icon: '#' main colour, '+' accent colour.
    pub fn icon(self) -> ([&'static str; 7], [u8; 3], [u8; 3]) {
        match self {
            Spell::Spark => (
                ["...+...", "..+#+..", ".+###+.", "+#####+", ".+###+.", "..+#+..", "...+..."],
                [255, 240, 170],
                [255, 170, 60],
            ),
            Spell::Fireball => (
                ["....+..", "..+#+..", ".+###+.", "+##+##+", "+#+++#+", ".+###+.", "..+++.."],
                [255, 130, 30],
                [255, 220, 90],
            ),
            Spell::Bomb => (
                ["....++.", "...+...", "..###..", ".#####.", ".##+##.", ".#####.", "..###.."],
                [90, 86, 100],
                [255, 190, 80],
            ),
            Spell::DigBeam => (
                [".......", "##.....", "####...", "###++++", "####...", "##.....", "......."],
                [120, 220, 240],
                [230, 255, 255],
            ),
            Spell::Torrent => (
                ["...#...", "..###..", ".##+##.", ".#+###.", ".#####.", "..###..", "......."],
                [60, 130, 230],
                [180, 220, 255],
            ),
            Spell::Frost => (
                ["#..#..#", ".#.#.#.", "..#+#..", "##+++##", "..#+#..", ".#.#.#.", "#..#..#"],
                [160, 220, 255],
                [255, 255, 255],
            ),
            Spell::Teleport => (
                ["..###..", ".#...#.", "#..+..#", "#.+++.#", "#..+..#", ".#...#.", "..###.."],
                [180, 120, 255],
                [240, 210, 255],
            ),
            Spell::Seeds => (
                ["...#...", ".##.##.", "..###..", "...#...", "...#...", "..+++..", ".+++++."],
                [110, 200, 80],
                [130, 92, 56],
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Spark,
    Fireball,
    Bomb,
    Frost,
    Teleport,
}

struct Proj {
    kind: Kind,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
}

pub struct Spells {
    projs: Vec<Proj>,
    /// Where a teleport bolt landed, consumed by the game.
    pub teleport: Option<(f32, f32)>,
    beam: Option<(f32, f32, f32, f32)>,
    pub sounds: Vec<Sfx>,
}

impl Spells {
    pub fn new() -> Self {
        Self { projs: Vec::new(), teleport: None, beam: None, sounds: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.projs.clear();
        self.beam = None;
        self.teleport = None;
    }

    /// Cast `spell` from (x, y) towards (tx, ty). Held spells call this every frame.
    /// Returns gold dug up by the digging beam.
    pub fn cast(&mut self, spell: Spell, (x, y): (f32, f32), (tx, ty): (f32, f32), world: &mut World, fx: &mut Fx) -> u32 {
        let (dx, dy) = (tx - x, ty - y);
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        let (ux, uy) = (dx / len, dy / len);
        let mut shoot = |kind: Kind, speed: f32, life: f32, lift: f32| {
            self.projs.push(Proj { kind, x, y, vx: ux * speed, vy: uy * speed - lift, life });
        };
        match spell {
            Spell::Spark => {
                shoot(Kind::Spark, 240.0, 2.0, 0.0);
                self.sounds.push(Sfx::Whoosh);
            }
            Spell::Fireball => {
                shoot(Kind::Fireball, 170.0, 3.0, 20.0);
                self.sounds.push(Sfx::Fireball);
            }
            Spell::Bomb => {
                shoot(Kind::Bomb, 150.0, 1.8, 40.0);
                self.sounds.push(Sfx::Throw);
            }
            Spell::Frost => {
                shoot(Kind::Frost, 210.0, 1.5, 0.0);
                self.sounds.push(Sfx::Whoosh);
            }
            Spell::Teleport => {
                shoot(Kind::Teleport, 320.0, 1.2, 0.0);
                self.sounds.push(Sfx::Zap);
            }
            Spell::Seeds => {
                for _ in 0..6 {
                    let a = fx.rng.rangef(-0.35, 0.35);
                    let s = fx.rng.rangef(80.0, 140.0);
                    let (c, si) = (a.cos(), a.sin());
                    let sh = fx.rng.next_u32() as u8;
                    fx.debris(x, y, (ux * c - uy * si) * s, (ux * si + uy * c) * s - 20.0, M::Seed, sh);
                }
                self.sounds.push(Sfx::Throw);
            }
            Spell::Torrent => {
                for _ in 0..3 {
                    let a = fx.rng.rangef(-0.12, 0.12);
                    let s = fx.rng.rangef(130.0, 170.0);
                    let (c, si) = (a.cos(), a.sin());
                    let sh = fx.rng.next_u32() as u8;
                    fx.debris(x, y, (ux * c - uy * si) * s, (ux * si + uy * c) * s, M::Water, sh);
                }
                self.sounds.push(Sfx::Splash);
            }
            Spell::DigBeam => {
                let (mut bx, mut by) = (x, y);
                let mut hit = false;
                for _ in 0..140 {
                    bx += ux * 0.5;
                    by += uy * 0.5;
                    let m = world.mat(bx.floor() as i32, by.floor() as i32);
                    if !m.passable() && m != M::Empty {
                        hit = true;
                        break;
                    }
                }
                self.beam = Some((x, y, bx, by));
                if hit {
                    self.sounds.push(Sfx::Dig);
                    return fx.dissolve(world, bx, by, 2, 0.45);
                }
            }
        }
        0
    }

    pub fn step(&mut self, world: &mut World, fx: &mut Fx, dt: f32) {
        let mut projs = std::mem::take(&mut self.projs);
        projs.retain_mut(|p| {
            let gravity = match p.kind {
                Kind::Spark => 30.0,
                Kind::Fireball => 140.0,
                Kind::Bomb => 260.0,
                Kind::Frost => 40.0,
                Kind::Teleport => 0.0,
            };
            p.vy += gravity * dt;
            p.life -= dt;
            let dist = (p.vx * p.vx + p.vy * p.vy).sqrt() * dt;
            let n = (dist / 0.5).ceil().max(1.0) as i32;
            let (sx, sy) = (p.vx * dt / n as f32, p.vy * dt / n as f32);
            for _ in 0..n {
                let (nx, ny) = (p.x + sx, p.y + sy);
                let (cx, cy) = (nx.floor() as i32, ny.floor() as i32);
                let m = world.mat(cx, cy);
                if !m.passable() || !world.in_window(cx, cy) {
                    if p.kind == Kind::Bomb && world.in_window(cx, cy) {
                        // Bounce off whichever axis is blocked.
                        if world.mat((p.x + sx).floor() as i32, p.y.floor() as i32).passable() {
                            p.vy = -p.vy * 0.45;
                            p.vx *= 0.75;
                        } else {
                            p.vx = -p.vx * 0.45;
                        }
                        self.sounds.push(Sfx::Tick);
                        break;
                    }
                    self.impact(p, world, fx);
                    return false;
                }
                p.x = nx;
                p.y = ny;
                match p.kind {
                    Kind::Fireball => {
                        if m.flammable() {
                            world.ignite_at(cx, cy);
                        } else if m == M::Empty && fx.rng.chance(0.12) {
                            let f = Cell::fire(&mut world.rng);
                            world.set(cx, cy, f);
                        }
                        if fx.rng.chance(0.4) {
                            let (vx, vy) = (fx.rng.rangef(-20.0, 20.0), fx.rng.rangef(-30.0, 5.0));
                            let life = fx.rng.rangef(0.15, 0.4);
                            fx.spark(p.x, p.y, vx, vy, life, [255, 140, 40]);
                        }
                    }
                    Kind::Frost => {
                        if m == M::Fire {
                            world.set(cx, cy, Cell::EMPTY);
                        }
                        if fx.rng.chance(0.25) {
                            let (vx, vy) = (fx.rng.rangef(-12.0, 12.0), fx.rng.rangef(-12.0, 12.0));
                            fx.spark(p.x, p.y, vx, vy, 0.3, FROST);
                        }
                    }
                    Kind::Teleport => {
                        if fx.rng.chance(0.4) {
                            fx.spark(p.x, p.y, 0.0, 0.0, 0.35, MAGIC);
                        }
                    }
                    Kind::Spark => {
                        if fx.rng.chance(0.3) {
                            let (vx, vy) = (fx.rng.rangef(-15.0, 15.0), fx.rng.rangef(-15.0, 15.0));
                            let life = fx.rng.rangef(0.1, 0.35);
                            fx.spark(p.x, p.y, vx - p.vx * 0.05, vy - p.vy * 0.05, life, SPARK);
                        }
                    }
                    Kind::Bomb => {}
                }
            }
            if p.life <= 0.0 {
                self.impact(p, world, fx);
                return false;
            }
            true
        });
        projs.append(&mut self.projs);
        self.projs = projs;
    }

    fn impact(&mut self, p: &Proj, world: &mut World, fx: &mut Fx) {
        match p.kind {
            Kind::Spark => fx.explode(world, p.x, p.y, 5, 0.3),
            Kind::Fireball => fx.explode(world, p.x, p.y, 8, 1.0),
            Kind::Bomb => fx.explode(world, p.x, p.y, 14, 0.5),
            Kind::Frost => {
                fx.freeze(world, p.x, p.y, 8);
                self.sounds.push(Sfx::Freeze);
            }
            Kind::Teleport => {
                fx.burst(p.x, p.y, 30, 80.0, MAGIC);
                self.teleport = Some((p.x, p.y));
                self.sounds.push(Sfx::Zap);
            }
        }
    }

    pub fn draw(&mut self, cv: &mut Canvas, time: f32) {
        for p in &self.projs {
            let (x, y) = (p.x as i32, p.y as i32);
            let (core, halo) = match p.kind {
                Kind::Spark => ([255, 250, 230], [255, 190, 90]),
                Kind::Fireball => ([255, 230, 120], [255, 110, 20]),
                Kind::Bomb => {
                    let blink = (p.life * if p.life < 0.6 { 16.0 } else { 6.0 }) as i32 % 2 == 0;
                    ([70, 66, 80], if blink { [255, 80, 50] } else { [40, 38, 46] })
                }
                Kind::Frost => ([255, 255, 255], [150, 210, 255]),
                Kind::Teleport => ([250, 230, 255], [170, 110, 255]),
            };
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                cv.put(x + dx, y + dy, halo);
            }
            cv.put(x, y, core);
        }
        if let Some((x0, y0, x1, y1)) = self.beam {
            let len = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
            let n = (len * 2.0) as i32;
            for k in 0..n {
                let t = k as f32 / n.max(1) as f32;
                let wob = ((k as f32 * 0.7 + time * 40.0).sin() * 0.6) as i32;
                let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
                cv.put(x as i32, y as i32 + wob, [140, 240, 255]);
                cv.put(x as i32, y as i32, [230, 255, 255]);
            }
        }
    }

    /// Clear per-frame beam state after drawing (beam is re-cast each frame it's held).
    pub fn end_frame(&mut self) {
        self.beam = None;
    }

    pub fn lights(&self, add: &mut impl FnMut(f32, f32, [f32; 3])) {
        for p in &self.projs {
            let c = match p.kind {
                Kind::Spark => [3.0, 2.2, 1.0],
                Kind::Fireball => [5.0, 2.4, 0.6],
                Kind::Bomb => [0.6, 0.2, 0.1],
                Kind::Frost => [1.5, 2.5, 3.5],
                Kind::Teleport => [2.5, 1.5, 4.0],
            };
            add(p.x, p.y, c);
        }
        if let Some((x0, y0, x1, y1)) = self.beam {
            for k in 0..8 {
                let t = k as f32 / 7.0;
                add(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, [0.6, 1.4, 1.8]);
            }
        }
    }
}
