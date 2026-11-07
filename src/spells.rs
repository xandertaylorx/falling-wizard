//! Spell cards, wands and everything they shoot.
//!
//! Each spell sits in its own numbered hotbar slot.

use crate::audio::Sfx;
use crate::cell::Cell;
use crate::fx::{Fx, FROST, MAGIC, SPARK};
use crate::materials::Material as M;
use crate::mobs::Mobs;
use crate::render::Canvas;
use crate::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Card {
    Spark,
    Fireball,
    Bomb,
    Frost,
    Teleport,
    AcidBall,
    Lightning,
    OilSpray,
    BlackHole,
    Seeds,
    Torrent,
    DigBeam,
}

pub const SPELL_BAR: [Card; 12] = [
    Card::Spark,
    Card::Fireball,
    Card::Bomb,
    Card::Lightning,
    Card::Frost,
    Card::AcidBall,
    Card::Torrent,
    Card::Seeds,
    Card::Teleport,
    Card::DigBeam,
    Card::OilSpray,
    Card::BlackHole,
];

impl Card {
    pub fn name(self) -> &'static str {
        match self {
            Card::Spark => "Spark Bolt",
            Card::Fireball => "Fireball",
            Card::Bomb => "Bomb",
            Card::Frost => "Frost",
            Card::Teleport => "Teleport",
            Card::AcidBall => "Acid Ball",
            Card::Lightning => "Lightning",
            Card::OilSpray => "Oil Spray",
            Card::BlackHole => "Black Hole",
            Card::Seeds => "Seeds of Life",
            Card::Torrent => "Torrent",
            Card::DigBeam => "Digging Beam",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Card::Spark => "Fast bolt, small blast",
            Card::Fireball => "Arcing ball of fire",
            Card::Bomb => "Bounces, then a huge blast",
            Card::Frost => "Freezes water and lava",
            Card::Teleport => "Blink to where it lands",
            Card::AcidBall => "Splashes corrosive acid",
            Card::Lightning => "Instant strike in a line",
            Card::OilSpray => "Spray of flammable oil",
            Card::BlackHole => "Swallows everything nearby",
            Card::Seeds => "Scatter seeds that grow",
            Card::Torrent => "Spray of water",
            Card::DigBeam => "Hold to bore through rock",
        }
    }

    pub fn mana(self) -> f32 {
        match self {
            Card::Spark => 3.0,
            Card::Fireball => 10.0,
            Card::Bomb => 25.0,
            Card::Frost => 9.0,
            Card::Teleport => 18.0,
            Card::AcidBall => 10.0,
            Card::Lightning => 20.0,
            Card::OilSpray => 0.5,
            Card::BlackHole => 45.0,
            Card::Seeds => 8.0,
            Card::Torrent => 0.5,
            Card::DigBeam => 0.5,
        }
    }

    pub fn delay(self) -> f32 {
        match self {
            Card::Spark => 0.12,
            Card::Fireball => 0.35,
            Card::Bomb => 0.7,
            Card::Frost => 0.3,
            Card::Teleport => 0.45,
            Card::AcidBall => 0.3,
            Card::Lightning => 0.5,
            Card::OilSpray => 0.02,
            Card::BlackHole => 1.2,
            Card::Seeds => 0.3,
            Card::Torrent => 0.02,
            Card::DigBeam => 0.0,
        }
    }

    /// 7x7 icon: '#' main colour, '+' accent colour.
    pub fn icon(self) -> ([&'static str; 7], [u8; 3], [u8; 3]) {
        match self {
            Card::Spark => (["...+...", "..+#+..", ".+###+.", "+#####+", ".+###+.", "..+#+..", "...+..."], [255, 240, 170], [255, 170, 60]),
            Card::Fireball => (["....+..", "..+#+..", ".+###+.", "+##+##+", "+#+++#+", ".+###+.", "..+++.."], [255, 130, 30], [255, 220, 90]),
            Card::Bomb => (["....++.", "...+...", "..###..", ".#####.", ".##+##.", ".#####.", "..###.."], [90, 86, 100], [255, 190, 80]),
            Card::Frost => (["#..#..#", ".#.#.#.", "..#+#..", "##+++##", "..#+#..", ".#.#.#.", "#..#..#"], [160, 220, 255], [255, 255, 255]),
            Card::Teleport => (["..###..", ".#...#.", "#..+..#", "#.+++.#", "#..+..#", ".#...#.", "..###.."], [180, 120, 255], [240, 210, 255]),
            Card::AcidBall => (["..###..", ".##+##.", "##+####", "#######", "#######", ".#####.", "#.#.#.."], [110, 232, 64], [220, 255, 180]),
            Card::Lightning => (["....##.", "...##..", "..##...", ".#####.", "...##..", "..##...", ".##...."], [255, 250, 140], [255, 255, 255]),
            Card::OilSpray => (["...#...", "..###..", ".##+##.", ".#+###.", ".#####.", "..###..", "......."], [70, 48, 52], [150, 110, 90]),
            Card::BlackHole => (["..+++..", ".+###+.", "+##.##+", "+#...#+", "+##.##+", ".+###+.", "..+++.."], [20, 10, 30], [190, 110, 255]),
            Card::Seeds => (["...#...", ".##.##.", "..###..", "...#...", "...#...", "..+++..", ".+++++."], [110, 200, 80], [130, 92, 56]),
            Card::Torrent => (["...#...", "..###..", ".##+##.", ".#+###.", ".#####.", "..###..", "......."], [60, 130, 230], [180, 220, 255]),
            Card::DigBeam => ([".......", "##.....", "####...", "###++++", "####...", "##.....", "......."], [120, 220, 240], [230, 255, 255]),
        }
    }
}

#[derive(Clone, Default)]
pub struct Mods {
    pub count: u32,
    pub bounces: u32,
    pub homing: bool,
    pub heavy: u32,
    pub speed: u32,
    pub trail: bool,
}

#[derive(Clone)]
pub struct Shot {
    pub card: Card,
    pub mods: Mods,
    pub payload: Option<Box<Shot>>,
}

struct Proj {
    card: Card,
    mods: Mods,
    payload: Option<Box<Shot>>,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    bounces: u32,
}

impl Proj {
    fn radius(&self) -> i32 {
        let base = match self.card {
            Card::Spark => 5,
            Card::Fireball => 8,
            Card::Bomb => 14,
            Card::AcidBall => 4,
            _ => 3,
        };
        (base as f32 * (1.0 + 0.5 * self.mods.heavy as f32)) as i32
    }
    fn damage(&self) -> f32 {
        let base = match self.card {
            Card::Spark => 9.0,
            Card::Fireball => 16.0,
            Card::Bomb => 30.0,
            Card::Frost => 8.0,
            Card::AcidBall => 14.0,
            _ => 4.0,
        };
        base * (1.0 + 0.6 * self.mods.heavy as f32)
    }
}

pub struct Spells {
    projs: Vec<Proj>,
    /// Where a teleport bolt landed, consumed by the game.
    pub teleport: Option<(f32, f32)>,
    beam: Option<(f32, f32, f32, f32)>,
    bolts: Vec<(Vec<(f32, f32)>, f32)>,
    /// Active black holes: (x, y, seconds left).
    pub holes: Vec<(f32, f32, f32)>,
    pub sounds: Vec<Sfx>,
}

impl Spells {
    pub fn new() -> Self {
        Self { projs: Vec::new(), teleport: None, beam: None, bolts: Vec::new(), holes: Vec::new(), sounds: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.projs.clear();
        self.holes.clear();
        self.beam = None;
        self.teleport = None;
    }

    /// Fire a shot from (x, y) towards angle `ang`.
    pub fn fire(&mut self, shot: &Shot, (x, y): (f32, f32), ang: f32, world: &mut World, fx: &mut Fx, mobs: &mut Mobs) -> u32 {
        let n = 1 + shot.mods.count;
        let mut gold = 0;
        for k in 0..n {
            let a = ang + (k as f32 - (n - 1) as f32 * 0.5) * 0.16;
            gold += self.fire_one(shot, (x, y), a, world, fx, mobs);
        }
        gold
    }

    fn fire_one(&mut self, shot: &Shot, (x, y): (f32, f32), ang: f32, world: &mut World, fx: &mut Fx, mobs: &mut Mobs) -> u32 {
        let (ux, uy) = (ang.cos(), ang.sin());
        let m = &shot.mods;
        let speed_mul = 1.6f32.powi(m.speed as i32) * 0.75f32.powi(m.heavy as i32);
        let (speed, life, lift, sfx) = match shot.card {
            Card::Spark => (240.0, 2.0, 0.0, Sfx::Whoosh),
            Card::Fireball => (170.0, 3.0, 20.0, Sfx::Fireball),
            Card::Bomb => (150.0, 1.8, 40.0, Sfx::Throw),
            Card::Frost => (210.0, 1.5, 0.0, Sfx::Whoosh),
            Card::Teleport => (320.0, 1.2, 0.0, Sfx::Zap),
            Card::AcidBall => (180.0, 2.5, 25.0, Sfx::Throw),
            Card::Seeds => {
                for k in 0..50 {
                    let (tx, ty) = (x + ux * k as f32, y + uy * k as f32);
                    if !world.mat(tx as i32, ty as i32).passable() {
                        break;
                    }
                    if let Some(i) = mobs.hit_test(tx, ty, 1.0) {
                        if mobs.tame(i, fx) {
                            self.sounds.push(Sfx::Chime);
                            return 0;
                        }
                        break;
                    }
                }
                for _ in 0..6 {
                    let a = ang + fx.rng.rangef(-0.35, 0.35);
                    let s = fx.rng.rangef(80.0, 140.0);
                    let sh = fx.rng.next_u32() as u8;
                    fx.debris(x, y, a.cos() * s, a.sin() * s - 20.0, M::Seed, sh);
                }
                self.sounds.push(Sfx::Throw);
                return 0;
            }
            Card::Torrent => {
                for _ in 0..3 {
                    let a = ang + fx.rng.rangef(-0.12, 0.12);
                    let s = fx.rng.rangef(130.0, 170.0) * speed_mul;
                    let sh = fx.rng.next_u32() as u8;
                    fx.debris(x, y, a.cos() * s, a.sin() * s, M::Water, sh);
                }
                self.sounds.push(Sfx::Splash);
                return 0;
            }
            Card::OilSpray => {
                for _ in 0..3 {
                    let a = ang + fx.rng.rangef(-0.14, 0.14);
                    let s = fx.rng.rangef(120.0, 160.0) * speed_mul;
                    let sh = fx.rng.next_u32() as u8;
                    fx.debris(x, y, a.cos() * s, a.sin() * s, M::Oil, sh);
                }
                self.sounds.push(Sfx::Splash);
                return 0;
            }
            Card::BlackHole => (90.0, 0.9, 0.0, Sfx::Zap),
            Card::DigBeam => {
                let (mut bx, mut by) = (x, y);
                let mut hit = false;
                for _ in 0..140 {
                    bx += ux * 0.5;
                    by += uy * 0.5;
                    let mat = world.mat(bx.floor() as i32, by.floor() as i32);
                    if (!mat.passable() && mat != M::Empty) || mobs.hit_test(bx, by, 0.0).is_some() {
                        hit = true;
                        break;
                    }
                }
                self.beam = Some((x, y, bx, by));
                if hit {
                    self.sounds.push(Sfx::Dig);
                    mobs.damage_area(bx, by, 3.0, 40.0 * (1.0 / 60.0) * 3.0, fx);
                    return fx.dissolve(world, bx, by, 2 + m.heavy as i32, 0.45);
                }
                return 0;
            }
            Card::Lightning => {
                let (mut bx, mut by) = (x, y);
                let mut pts = vec![(x, y)];
                for i in 0..200 {
                    bx += ux;
                    by += uy;
                    if i % 6 == 5 {
                        pts.push((bx + fx.rng.rangef(-2.0, 2.0), by + fx.rng.rangef(-2.0, 2.0)));
                    }
                    let mat = world.mat(bx.floor() as i32, by.floor() as i32);
                    if (!mat.passable() && mat != M::Empty) || mobs.hit_test(bx, by, 1.0).is_some() {
                        break;
                    }
                }
                pts.push((bx, by));
                mobs.damage_line(x, y, bx, by, 3.0, 26.0 * (1.0 + 0.6 * m.heavy as f32), fx);
                fx.explode(world, bx, by, 3 + m.heavy as i32 * 2, 0.6);
                fx.burst(bx, by, 25, 120.0, [255, 250, 200]);
                fx.flash = fx.flash.max(0.25);
                self.bolts.push((pts, 0.18));
                self.sounds.push(Sfx::Zap);
                if let Some(p) = &shot.payload {
                    let p = (**p).clone();
                    self.fire(&p, (bx - ux * 2.0, by - uy * 2.0), ang + std::f32::consts::PI, world, fx, mobs);
                }
                return 0;
            }
        };
        self.sounds.push(sfx);
        let speed = speed * speed_mul;
        self.projs.push(Proj {
            card: shot.card,
            mods: shot.mods.clone(),
            payload: shot.payload.clone(),
            x,
            y,
            vx: ux * speed,
            vy: uy * speed - lift,
            life,
            bounces: shot.mods.bounces + if shot.card == Card::Bomb { 99 } else { 0 },
        });
        0
    }

    pub fn step(&mut self, world: &mut World, fx: &mut Fx, mobs: &mut Mobs, dt: f32) {
        for b in self.bolts.iter_mut() {
            b.1 -= dt;
        }
        self.bolts.retain(|b| b.1 > 0.0);
        self.step_holes(world, fx, mobs, dt);
        let mut projs = std::mem::take(&mut self.projs);
        let mut spawned: Vec<(Shot, (f32, f32), f32)> = Vec::new();
        projs.retain_mut(|p| {
            let gravity = match p.card {
                Card::Spark => 30.0,
                Card::Fireball => 140.0,
                Card::Bomb => 260.0,
                Card::AcidBall => 200.0,
                Card::Frost => 40.0,
                _ => 0.0,
            } * (1.0 + p.mods.heavy as f32 * 0.5);
            p.vy += gravity * dt;
            p.life -= dt;
            if p.mods.homing {
                if let Some((tx, ty)) = mobs.nearest(p.x, p.y, 90.0) {
                    let sp = (p.vx * p.vx + p.vy * p.vy).sqrt();
                    let (dx, dy) = (tx - p.x, ty - p.y);
                    let d = (dx * dx + dy * dy).sqrt().max(0.1);
                    let k = (6.0 * dt).min(1.0);
                    p.vx += (dx / d * sp - p.vx) * k;
                    p.vy += (dy / d * sp - p.vy) * k;
                }
            }
            let dist = (p.vx * p.vx + p.vy * p.vy).sqrt() * dt;
            let n = (dist / 0.5).ceil().max(1.0) as i32;
            let (sx, sy) = (p.vx * dt / n as f32, p.vy * dt / n as f32);
            for _ in 0..n {
                let (nx, ny) = (p.x + sx, p.y + sy);
                let (cx, cy) = (nx.floor() as i32, ny.floor() as i32);
                if !world.in_window(cx, cy) {
                    return false;
                }
                let m = world.mat(cx, cy);
                let mob_hit = mobs.hit_test(nx, ny, 0.5);
                if let Some(i) = mob_hit {
                    mobs.hurt(i, p.damage(), (p.vx * 0.3, p.vy * 0.3 - 30.0), fx);
                    if p.card == Card::Frost {
                        mobs.freeze(i, 3.0);
                    }
                }
                let blocked = !m.passable() || mob_hit.is_some();
                if blocked {
                    if p.bounces > 0 && mob_hit.is_none() {
                        p.bounces -= 1;
                        if world.mat((p.x + sx).floor() as i32, p.y.floor() as i32).passable() {
                            p.vy = -p.vy * if p.card == Card::Bomb { 0.45 } else { 0.85 };
                            p.vx *= 0.8;
                        } else {
                            p.vx = -p.vx * if p.card == Card::Bomb { 0.45 } else { 0.85 };
                        }
                        self.sounds.push(Sfx::Tick);
                        break;
                    }
                    self.impact(p, world, fx, &mut spawned);
                    return false;
                }
                p.x = nx;
                p.y = ny;
                if p.mods.trail && fx.rng.chance(0.35) && m == M::Empty {
                    let f = Cell::fire(&mut world.rng);
                    world.set(cx, cy, f);
                }
                trail(p, cx, cy, m, world, fx);
            }
            if p.life <= 0.0 {
                self.impact(p, world, fx, &mut spawned);
                return false;
            }
            true
        });
        projs.append(&mut self.projs);
        self.projs = projs;
        for (shot, pos, ang) in spawned {
            self.fire(&shot, pos, ang, world, fx, mobs);
        }
    }

    /// Black holes rip loose material out of the world and suck it (and creatures) in.
    fn step_holes(&mut self, world: &mut World, fx: &mut Fx, mobs: &mut Mobs, dt: f32) {
        fx.attractors.clear();
        let mut done = Vec::new();
        for (k, h) in self.holes.iter_mut().enumerate() {
            h.2 -= dt;
            fx.attractors.push((h.0, h.1));
            mobs.pull(h.0, h.1, 45.0, dt, fx);
            for _ in 0..90 {
                let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                let r = fx.rng.f32().sqrt() * 24.0;
                let (cx, cy) = ((h.0 + a.cos() * r) as i32, (h.1 + a.sin() * r) as i32);
                let Some(c) = world.get(cx, cy) else { continue };
                let loose = match c.mat.kind() {
                    crate::materials::Kind::Empty => false,
                    crate::materials::Kind::Static => !matches!(c.mat, M::Chest | M::Altar) && fx.rng.chance(0.03),
                    _ => true,
                };
                if loose {
                    world.set(cx, cy, Cell::EMPTY);
                    if c.mat != M::Fire && c.mat.kind() != crate::materials::Kind::Gas {
                        let (dx, dy) = (h.0 - cx as f32, h.1 - cy as f32);
                        let d = (dx * dx + dy * dy).sqrt().max(1.0);
                        let tang = fx.rng.rangef(30.0, 70.0);
                        fx.debris(cx as f32, cy as f32, dx / d * 40.0 - dy / d * tang, dy / d * 40.0 + dx / d * tang, c.mat, c.shade);
                    }
                }
            }
            if fx.rng.chance(0.5) {
                let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                fx.spark(h.0 + a.cos() * 9.0, h.1 + a.sin() * 9.0, -a.sin() * 60.0, a.cos() * 60.0, 0.4, MAGIC);
            }
            if h.2 <= 0.0 {
                done.push(k);
            }
        }
        for k in done.into_iter().rev() {
            let (x, y, _) = self.holes.remove(k);
            fx.explode(world, x, y, 13, 0.4);
            fx.burst(x, y, 60, 180.0, MAGIC);
        }
    }

    fn impact(&mut self, p: &Proj, world: &mut World, fx: &mut Fx, spawned: &mut Vec<(Shot, (f32, f32), f32)>) {
        let r = p.radius();
        match p.card {
            Card::BlackHole => {
                self.holes.push((p.x, p.y, 2.8));
                self.sounds.push(Sfx::Thunder);
            }
            Card::Spark => fx.explode(world, p.x, p.y, r, 0.3),
            Card::Fireball => fx.explode(world, p.x, p.y, r, 1.0),
            Card::Bomb => fx.explode(world, p.x, p.y, r, 0.5),
            Card::Frost => {
                fx.freeze(world, p.x, p.y, 8 + p.mods.heavy as i32 * 3);
                self.sounds.push(Sfx::Freeze);
            }
            Card::Teleport => {
                fx.burst(p.x, p.y, 30, 80.0, MAGIC);
                self.teleport = Some((p.x - p.vx.signum() * 1.5, p.y - p.vy.signum() * 1.5));
                self.sounds.push(Sfx::Zap);
            }
            Card::AcidBall => {
                for _ in 0..(14 + r * 4) {
                    let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                    let s = fx.rng.rangef(30.0, 90.0);
                    fx.debris(p.x - p.vx.signum(), p.y - 1.0, a.cos() * s, a.sin() * s - 40.0, M::Acid, 128);
                }
                fx.explosions.push((p.x, p.y, 2));
                self.sounds.push(Sfx::Splash);
            }
            _ => {}
        }
        if let Some(payload) = &p.payload {
            // Release the carried spell, aimed back out of the surface it hit.
            let back = (-p.vy).atan2(-p.vx);
            let pos = (p.x - p.vx.signum() * 1.0, p.y - p.vy.signum() * 1.0);
            let mut ang = back;
            if payload.mods.count == 0 {
                ang += fx.rng.rangef(-0.5, 0.5);
            }
            spawned.push(((**payload).clone(), pos, ang));
        }
    }

    pub fn draw(&mut self, cv: &mut Canvas, time: f32) {
        for p in &self.projs {
            let (x, y) = (p.x as i32, p.y as i32);
            let (core, halo) = match p.card {
                Card::Spark => ([255, 250, 230], [255, 190, 90]),
                Card::Fireball => ([255, 230, 120], [255, 110, 20]),
                Card::Bomb => {
                    let blink = (p.life * if p.life < 0.6 { 16.0 } else { 6.0 }) as i32 % 2 == 0;
                    ([70, 66, 80], if blink { [255, 80, 50] } else { [40, 38, 46] })
                }
                Card::Frost => ([255, 255, 255], [150, 210, 255]),
                Card::Teleport => ([250, 230, 255], [170, 110, 255]),
                Card::BlackHole => ([10, 0, 20], [190, 110, 255]),
                Card::AcidBall => ([220, 255, 180], [110, 232, 64]),
                _ => ([255, 255, 255], [200, 200, 200]),
            };
            let big = p.mods.heavy > 0 || p.card == Card::Fireball;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                cv.put(x + dx, y + dy, halo);
                if big {
                    cv.put(x + dx * 2, y + dy * 2, halo);
                    cv.put(x + dx, y + dy.signum() - dx.signum(), halo);
                }
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
        for &(hx, hy, left) in &self.holes {
            let grow = (2.8 - left).min(0.5) * 2.0;
            let r = (4.0 * grow) as i32;
            for dy in -r - 3..=r + 3 {
                for dx in -r - 3..=r + 3 {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d <= r as f32 {
                        cv.put(hx as i32 + dx, hy as i32 + dy, [4, 0, 10]);
                    } else if d <= r as f32 + 2.5 {
                        let a = (dy as f32).atan2(dx as f32) + time * 6.0;
                        if (a * 3.0).sin() > -0.2 {
                            cv.put(hx as i32 + dx, hy as i32 + dy, if d < r as f32 + 1.2 { [255, 220, 255] } else { [170, 90, 255] });
                        }
                    }
                }
            }
        }
        for (pts, _) in &self.bolts {
            for w in pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) as i32).max(1);
                for k in 0..=n {
                    let t = k as f32 / n as f32;
                    cv.put((a.0 + (b.0 - a.0) * t) as i32, (a.1 + (b.1 - a.1) * t) as i32, [255, 255, 230]);
                }
            }
        }
    }

    pub fn end_frame(&mut self) {
        self.beam = None;
    }

    pub fn lights(&self, add: &mut impl FnMut(f32, f32, [f32; 3])) {
        for p in &self.projs {
            let c = match p.card {
                Card::Spark => [3.0, 2.2, 1.0],
                Card::Fireball => [5.0, 2.4, 0.6],
                Card::Bomb => [0.6, 0.2, 0.1],
                Card::Frost => [1.5, 2.5, 3.5],
                Card::Teleport => [2.5, 1.5, 4.0],
                Card::AcidBall => [0.8, 2.5, 0.5],
                _ => [1.0, 1.0, 1.0],
            };
            add(p.x, p.y, c);
        }
        if let Some((x0, y0, x1, y1)) = self.beam {
            for k in 0..8 {
                let t = k as f32 / 7.0;
                add(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, [0.6, 1.4, 1.8]);
            }
        }
        for &(hx, hy, _) in &self.holes {
            for k in 0..6 {
                let a = k as f32 * 1.047;
                add(hx + a.cos() * 6.0, hy + a.sin() * 6.0, [1.6, 0.6, 3.0]);
            }
        }
        for (pts, t) in &self.bolts {
            for p in pts {
                add(p.0, p.1, [4.0 * t * 5.0, 4.0 * t * 5.0, 3.0 * t * 5.0]);
            }
        }
    }
}

fn trail(p: &Proj, cx: i32, cy: i32, m: M, world: &mut World, fx: &mut Fx) {
    match p.card {
        Card::Fireball => {
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
        Card::Frost => {
            if m == M::Fire {
                world.set(cx, cy, Cell::EMPTY);
            }
            if fx.rng.chance(0.25) {
                let (vx, vy) = (fx.rng.rangef(-12.0, 12.0), fx.rng.rangef(-12.0, 12.0));
                fx.spark(p.x, p.y, vx, vy, 0.3, FROST);
            }
        }
        Card::Teleport => {
            if fx.rng.chance(0.4) {
                fx.spark(p.x, p.y, 0.0, 0.0, 0.35, MAGIC);
            }
        }
        Card::Spark => {
            if fx.rng.chance(0.3) {
                let (vx, vy) = (fx.rng.rangef(-15.0, 15.0), fx.rng.rangef(-15.0, 15.0));
                let life = fx.rng.rangef(0.1, 0.35);
                fx.spark(p.x, p.y, vx - p.vx * 0.05, vy - p.vy * 0.05, life, SPARK);
            }
        }
        _ => {}
    }
}
