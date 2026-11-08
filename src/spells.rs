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
    Wind,
    Hurricane,
    Tornado,
    PortalWand,
}

pub const SPELL_BAR: [Card; 16] = [
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
    Card::Wind,
    Card::Hurricane,
    Card::Tornado,
    Card::PortalWand,
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
            Card::Wind => "Wind",
            Card::Hurricane => "Hurricane",
            Card::Tornado => "Tornado",
            Card::PortalWand => "Portal Wand",
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
            Card::Wind => "Hold to blow a gale",
            Card::Hurricane => "Huge blast that flings everything",
            Card::Tornado => "A twister that rips up the land",
            Card::PortalWand => "Right click: pick a planet. Shoot a portal structure",
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
            Card::Wind => 0.15,
            Card::Hurricane => 40.0,
            Card::Tornado => 30.0,
            Card::PortalWand => 5.0,
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
            Card::Wind => 0.02,
            Card::Hurricane => 1.0,
            Card::Tornado => 0.8,
            Card::PortalWand => 0.5,
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
            Card::Wind => (["#####..", "......#", "####..#", "....##.", "######.", "......#", "..####."], [200, 228, 240], [255, 255, 255]),
            Card::Hurricane => (["..###..", ".#...#.", "#..+..#", "#.+.#.#", "#..#..#", ".#...#.", "..##..."], [170, 210, 255], [255, 255, 255]),
            Card::Tornado => (["#######", ".#####.", "..###..", "..##...", "...##..", "...#...", "..+...."], [176, 176, 188], [230, 230, 240]),
            Card::PortalWand => (["..+#+..", ".#...#.", "+..#..+", "#.#+#.#", "+..#..+", ".#...#.", "..+#+.."], [120, 230, 255], [255, 120, 220]),
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

/// A black hole: it grows, swallows the world around it, then collapses.
pub struct Hole {
    pub x: f32,
    pub y: f32,
    /// Seconds alive.
    pub age: f32,
    eaten: Vec<(M, u8)>,
}

pub const HOLE_LIFE: f32 = 4.5;
pub const TWISTER_LIFE: f32 = 7.0;

/// A tornado travelling along the ground.
pub struct Twister {
    pub x: f32,
    pub y: f32,
    dir: f32,
    pub age: f32,
}

impl Twister {
    pub fn height(&self) -> f32 {
        let grow = (self.age / 0.6).min(1.0);
        let fade = ((TWISTER_LIFE - self.age) / 0.8).clamp(0.0, 1.0);
        52.0 * grow * fade
    }
}

impl Hole {
    /// Reach of its pull, growing as it feeds.
    pub fn reach(&self) -> f32 {
        14.0 + (self.age / HOLE_LIFE).min(1.0) * 34.0
    }
    /// Radius of the black core.
    pub fn core(&self) -> f32 {
        if self.age > HOLE_LIFE - 0.25 {
            ((HOLE_LIFE - self.age) / 0.25).max(0.0) * 5.0
        } else {
            (self.age * 8.0).min(2.0 + self.age * 0.8)
        }
    }
}

pub struct Spells {
    projs: Vec<Proj>,
    /// Where a teleport bolt landed, consumed by the game.
    pub teleport: Option<(f32, f32)>,
    beam: Option<(f32, f32, f32, f32)>,
    bolts: Vec<(Vec<(f32, f32)>, f32)>,
    pub holes: Vec<Hole>,
    /// Where two black holes merged this frame; the game tears a rift there.
    pub rifts_born: Vec<(f32, f32)>,
    pub tornadoes: Vec<Twister>,
    /// Hue of the planet the Portal Wand is tuned to.
    pub portal_hue: f32,
    /// Where portal bolts landed this step, for the game to check against structures.
    pub portal_hits: Vec<(f32, f32)>,
    pub sounds: Vec<Sfx>,
}

impl Spells {
    pub fn new() -> Self {
        Self { projs: Vec::new(), teleport: None, beam: None, bolts: Vec::new(), holes: Vec::new(), rifts_born: Vec::new(), tornadoes: Vec::new(), portal_hue: 0.58, portal_hits: Vec::new(), sounds: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.projs.clear();
        self.holes.clear();
        self.tornadoes.clear();
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
            Card::PortalWand => (260.0, 1.4, 0.0, Sfx::Zap),
            Card::Wind => {
                self.gust((x, y), (ux, uy), false, world, fx, mobs);
                return 0;
            }
            Card::Hurricane => {
                self.gust((x, y), (ux, uy), true, world, fx, mobs);
                self.sounds.push(Sfx::Thunder);
                self.sounds.push(Sfx::Whoosh);
                fx.shake = (fx.shake + 9.0).min(14.0);
                return 0;
            }
            Card::Tornado => {
                if self.tornadoes.len() >= 3 {
                    self.tornadoes.remove(0);
                }
                let dir = if ux.abs() < 0.05 { 1.0 } else { ux.signum() };
                self.tornadoes.push(Twister { x: x + dir * 6.0, y, dir, age: 0.0 });
                self.sounds.push(Sfx::Whoosh);
                return 0;
            }
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
                    mobs.damage_area(bx, by, 3.0, 40.0 * (1.0 / 60.0) * 3.0, crate::mobs::Dmg::Physical, fx);
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
                // Forks branching off the main bolt.
                for _ in 0..4 {
                    let k = fx.rng.range(1, pts.len().max(2) as i32) as usize;
                    let (mut fxp, mut fyp) = pts[k.min(pts.len() - 1)];
                    let mut fork = vec![(fxp, fyp)];
                    let a = ang + fx.rng.rangef(-1.1, 1.1);
                    for _ in 0..fx.rng.range(3, 7) {
                        fxp += a.cos() * 5.0 + fx.rng.rangef(-2.5, 2.5);
                        fyp += a.sin() * 5.0 + fx.rng.rangef(-2.5, 2.5);
                        fork.push((fxp, fyp));
                    }
                    self.bolts.push((fork, 0.28));
                }
                // Chain to nearby creatures.
                for (i, mx, my) in mobs.near_list(bx, by, 50.0, 3) {
                    let mut arc = vec![(bx, by)];
                    for s in 1..6 {
                        let t = s as f32 / 6.0;
                        arc.push((bx + (mx - bx) * t + fx.rng.rangef(-3.0, 3.0), by + (my - by) * t + fx.rng.rangef(-3.0, 3.0)));
                    }
                    arc.push((mx, my));
                    self.bolts.push((arc, 0.3));
                    mobs.hurt_by(i, 18.0, (0.0, -60.0), crate::mobs::Dmg::Lightning, fx);
                    fx.burst(mx, my, 14, 80.0, [200, 230, 255]);
                }
                fx.explode(world, bx, by, 3 + m.heavy as i32 * 2, 0.6);
                world.scorch(bx as i32, by as i32, 6, 0.8);
                fx.burst(bx, by, 50, 160.0, [210, 235, 255]);
                fx.burst(bx, by, 20, 90.0, [255, 250, 200]);
                fx.flash = fx.flash.max(0.6);
                fx.shake = (fx.shake + 8.0).min(14.0);
                fx.hitstop = fx.hitstop.max(0.07);
                fx.kick += 3.0;
                self.bolts.push((pts, 0.35));
                self.sounds.push(Sfx::Zap);
                self.sounds.push(Sfx::Thunder);
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
        self.step_twisters(world, fx, mobs, dt);
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
                let mob_hit = if p.card == Card::PortalWand { None } else { mobs.hit_test(nx, ny, 0.5) };
                if let Some(i) = mob_hit {
                    use crate::mobs::Dmg;
                    let kind = match p.card {
                        Card::Fireball => Dmg::Fire,
                        Card::Frost => Dmg::Frost,
                        Card::AcidBall => Dmg::Acid,
                        Card::Bomb => Dmg::Blast,
                        _ => Dmg::Physical,
                    };
                    mobs.hurt_by(i, p.damage(), (p.vx * 0.3, p.vy * 0.3 - 30.0), kind, fx);
                    if p.card == Card::Frost {
                        mobs.freeze(i, 3.0);
                    }
                }
                let frame = p.card == Card::PortalWand && {
                    let b = world.back(cx, cy);
                    (b != M::Empty && !b.foliage()) || m == M::Portal
                };
                let blocked = !m.passable() || mob_hit.is_some() || frame;
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

    /// Two black holes caught in each other's pull orbit faster and faster, then merge
    /// in a flash that rips a lasting hole in the world.
    fn inspiral(&mut self, world: &mut World, fx: &mut Fx, dt: f32) {
        let n = self.holes.len();
        let mut merged = None;
        'pairs: for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (&self.holes[i], &self.holes[j]);
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let d = (dx * dx + dy * dy).sqrt().max(0.01);
                if d > (a.reach() + b.reach()) * 0.6 {
                    continue;
                }
                let (mx, my) = ((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                let half = d * 0.5;
                let new_half = (half - dt * (5.0 + 60.0 / (half + 2.0))).max(0.0);
                let ang = dy.atan2(dx) + dt * (1.5 + 70.0 / (half + 3.0));
                let (c, s) = (ang.cos(), ang.sin());
                self.holes[i].x = mx - c * new_half;
                self.holes[i].y = my - s * new_half;
                self.holes[j].x = mx + c * new_half;
                self.holes[j].y = my + s * new_half;
                for k in [i, j] {
                    self.holes[k].age = self.holes[k].age.min(HOLE_LIFE - 0.6);
                }
                // The closer they get, the harder the ground shakes and the brighter the swirl.
                let close = 1.0 - (new_half / 30.0).min(1.0);
                fx.shake = fx.shake.max(2.0 + close * 9.0);
                for _ in 0..(4.0 + close * 16.0) as usize {
                    let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                    let r = new_half + fx.rng.rangef(2.0, 10.0);
                    let sp = 90.0 + close * 160.0;
                    let col = if fx.rng.chance(0.5) { [255, 230, 190] } else { [170, 140, 255] };
                    fx.spark(mx + a.cos() * r, my + a.sin() * r, -a.sin() * sp, a.cos() * sp, 0.5, col);
                }
                if new_half < 1.5 {
                    merged = Some((i, j, mx, my));
                    break 'pairs;
                }
            }
        }
        let Some((i, j, mx, my)) = merged else { return };
        self.holes.remove(j);
        self.holes.remove(i);
        // Merger: a white flash, a ring of gravitational waves, and a tear left behind.
        fx.flash = 1.0;
        fx.shake = 18.0;
        fx.hitstop = fx.hitstop.max(0.12);
        fx.explode(world, mx, my, 14, 0.0);
        for k in 0..220 {
            let a = k as f32 / 220.0 * std::f32::consts::TAU;
            let sp = if k % 2 == 0 { 340.0 } else { 220.0 };
            let col = if k % 3 == 0 { [255, 255, 255] } else { [190, 150, 255] };
            fx.spark(mx + a.cos() * 3.0, my + a.sin() * 3.0, a.cos() * sp, a.sin() * sp, 0.7, col);
        }
        self.sounds.push(Sfx::Thunder);
        self.sounds.push(Sfx::Boom);
        self.rifts_born.push((mx, my));
    }

    /// Black holes tear everything loose (rock included), spiral it inwards and eat it.
    fn step_holes(&mut self, world: &mut World, fx: &mut Fx, mobs: &mut Mobs, dt: f32) {
        fx.attractors.clear();
        self.inspiral(world, fx, dt);
        let mut done = Vec::new();
        for (k, h) in self.holes.iter_mut().enumerate() {
            h.age += dt;
            let reach = h.reach();
            fx.attractors.push((h.x, h.y));
            mobs.pull(h.x, h.y, reach * 1.6, dt, fx);
            for _ in 0..260 {
                let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                let d = fx.rng.f32().sqrt() * reach;
                let (cx, cy) = ((h.x + a.cos() * d) as i32, (h.y + a.sin() * d) as i32);
                let Some(c) = world.get(cx, cy) else { continue };
                if c.back.foliage() && fx.rng.chance(0.5) {
                    world.set_back(cx, cy, M::Empty);
                    let (dx, dy) = (h.x - cx as f32, h.y - cy as f32);
                    let dl = (dx * dx + dy * dy).sqrt().max(1.0);
                    let col = crate::render::shade(c.back.base_color(), fx.rng.next_u32() as u8, 0.4);
                    fx.mote(cx as f32, cy as f32, dx / dl * 30.0 - dy / dl * 60.0, dy / dl * 30.0 + dx / dl * 60.0, 1.5, col, 0.0, 0.0, 0.0);
                }
                if c.mat == M::Empty {
                    continue;
                }
                // Closer cells are ripped free more easily; nothing is immune.
                let grip = 1.0 - d / reach;
                if c.mat.indestructible() {
                    continue;
                }
                let p = match c.mat.kind() {
                    crate::materials::Kind::Static => 0.05 + grip * grip * 0.55,
                    _ => 0.35 + grip * 0.65,
                };
                if !fx.rng.chance(p) {
                    continue;
                }
                world.set(cx, cy, Cell::EMPTY);
                if matches!(c.mat.kind(), crate::materials::Kind::Gas) {
                    continue;
                }
                let (dx, dy) = (h.x - cx as f32, h.y - cy as f32);
                let dl = (dx * dx + dy * dy).sqrt().max(1.0);
                let swirl = fx.rng.rangef(40.0, 90.0);
                let mat = if c.mat == M::Fire { M::Empty } else { c.mat };
                if mat == M::Empty {
                    fx.spark(cx as f32, cy as f32, dx / dl * 30.0 - dy / dl * swirl, dy / dl * 30.0 + dx / dl * swirl, 0.8, [255, 170, 80]);
                } else {
                    fx.debris(cx as f32, cy as f32, dx / dl * 25.0 - dy / dl * swirl, dy / dl * 25.0 + dx / dl * swirl, mat, c.shade);
                    if h.eaten.len() < 600 {
                        h.eaten.push((mat, c.shade));
                    }
                }
            }
            // Accretion disk: hot sparks spiralling in.
            for _ in 0..6 {
                let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                let r = fx.rng.rangef(6.0, reach * 0.8);
                let sp = 140.0 / (r * 0.15 + 1.0);
                let hot = fx.rng.f32();
                let col = if hot < 0.4 { [255, 210, 140] } else if hot < 0.75 { [220, 120, 255] } else { [120, 170, 255] };
                fx.spark(h.x + a.cos() * r, h.y + a.sin() * r * 0.55, -a.sin() * sp, a.cos() * sp * 0.55, 0.6, col);
            }
            if h.age >= HOLE_LIFE {
                done.push(k);
            }
        }
        for k in done.into_iter().rev() {
            let h = self.holes.remove(k);
            fx.explode(world, h.x, h.y, 16, 0.5);
            fx.burst(h.x, h.y, 120, 260.0, MAGIC);
            fx.flash = fx.flash.max(0.5);
            // Spit out a share of everything it swallowed.
            for (i, (m, shade)) in h.eaten.iter().enumerate() {
                if i % 2 == 1 {
                    continue;
                }
                let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                let sp = fx.rng.rangef(80.0, 260.0);
                fx.debris(h.x, h.y, a.cos() * sp, a.sin() * sp - 40.0, *m, *shade);
            }
            self.sounds.push(Sfx::Boom);
            self.sounds.push(Sfx::Thunder);
        }
    }

    /// Wind in a cone. A hurricane rips up the ground and flings everything hard.
    fn gust(&mut self, (x, y): (f32, f32), (ux, uy): (f32, f32), strong: bool, world: &mut World, fx: &mut Fx, mobs: &mut Mobs) {
        use crate::materials::Kind as K;
        let (len, half, samples) = if strong { (130.0, 0.55, 1600) } else { (70.0, 0.3, 120) };
        for _ in 0..samples {
            let along = fx.rng.f32().sqrt() * len;
            let side = fx.rng.rangef(-1.0, 1.0) * along * half;
            let (px, py) = (x + ux * along - uy * side, y + uy * along + ux * side);
            let (cx, cy) = (px.floor() as i32, py.floor() as i32);
            let Some(c) = world.get(cx, cy) else { continue };
            let near = 1.0 - along / len;
            let speed = if strong { fx.rng.rangef(220.0, 380.0) * (0.5 + near) } else { fx.rng.rangef(80.0, 140.0) };
            let (vx, vy) = (ux * speed + fx.rng.rangef(-20.0, 20.0), uy * speed - fx.rng.rangef(10.0, 50.0));
            if c.back.foliage() && (if strong { fx.rng.chance(0.6 * near + 0.2) } else { c.back == M::Leaves && fx.rng.chance(0.03) }) {
                world.set_back(cx, cy, M::Empty);
                let col = crate::render::shade(c.back.base_color(), fx.rng.next_u32() as u8, 0.4);
                let life = fx.rng.rangef(1.0, 2.5);
                fx.mote(px, py, vx * 0.6, vy * 0.6, life, col, 0.0, 60.0, 40.0);
            }
            match c.mat.kind() {
                K::Powder | K::Liquid => {
                    if strong || fx.rng.chance(0.25) {
                        world.set(cx, cy, Cell::EMPTY);
                        fx.debris(px, py, vx, vy, c.mat, c.shade);
                    }
                }
                K::Gas | K::Fire => {
                    let (tx, ty) = ((px + ux * 3.0) as i32, (py + uy * 3.0) as i32);
                    if world.mat(tx, ty) == M::Empty {
                        world.set(tx, ty, c);
                        world.set(cx, cy, Cell::EMPTY);
                    }
                }
                K::Static if strong && !matches!(c.mat, M::Chest | M::Altar) && !c.mat.indestructible() => {
                    let hardness = match c.mat {
                        M::Basalt => 0.03,
                        M::Stone | M::Brick | M::MoonRock => 0.12,
                        _ => 0.4,
                    };
                    if fx.rng.chance(hardness * near * near) {
                        world.set(cx, cy, Cell::EMPTY);
                        let m = match c.mat {
                            M::Stone | M::Basalt | M::Brick | M::Glass | M::MoonRock => M::Rubble,
                            M::Grass => M::Dirt,
                            M::Ice => M::Snow,
                            M::Wood | M::Leaves | M::Plant => M::Empty,
                            other => other,
                        };
                        if m != M::Empty {
                            fx.debris(px, py, vx, vy, m, c.shade);
                        }
                    }
                }
                _ => {}
            }
        }
        // Visible gusts.
        let streaks = if strong { 220 } else { 5 };
        for _ in 0..streaks {
            let along = fx.rng.f32() * len * if strong { 0.5 } else { 0.3 };
            let side = fx.rng.rangef(-1.0, 1.0) * (along * half + 2.0);
            let sp = fx.rng.rangef(220.0, 360.0);
            let a = fx.rng.rangef(0.35, 0.7);
            let life = fx.rng.rangef(0.15, 0.4) * if strong { 1.6 } else { 1.0 };
            fx.mote(x + ux * along - uy * side, y + uy * along + ux * side, ux * sp, uy * sp, life, [(225.0 * a + 30.0) as u8, (235.0 * a + 20.0) as u8, (245.0 * a + 10.0) as u8], 0.0, 0.0, 0.0);
        }
        fx.push_cone(x, y, ux, uy, len, half, if strong { 280.0 } else { 16.0 });
        mobs.push_cone(x, y, (ux, uy), len, half, if strong { 420.0 } else { 26.0 }, if strong { 35.0 } else { 0.0 }, fx);
    }

    fn step_twisters(&mut self, world: &mut World, fx: &mut Fx, mobs: &mut Mobs, dt: f32) {
        use crate::materials::Kind as K;
        fx.tornadoes.clear();
        for t in self.tornadoes.iter_mut() {
            t.age += dt;
            t.x += t.dir * 36.0 * dt;
            // Hug the ground: find the surface under the funnel.
            let gx = t.x as i32;
            let mut gy = t.y as i32 - 24;
            while gy < t.y as i32 + 40 && !world.mat(gx, gy + 1).solid() {
                gy += 1;
            }
            if (gy as f32 - t.y).abs() < 30.0 {
                t.y += (gy as f32 - t.y) * (dt * 6.0).min(1.0);
            }
            if (1..14).all(|k| world.mat(gx + t.dir as i32 * 4, t.y as i32 - k).solid()) {
                t.dir = -t.dir;
            }
            let h = t.height();
            fx.tornadoes.push((t.x, t.y, h));
            mobs.twister(t.x, t.y, h, dt, fx);
            for _ in 0..70 {
                let up = fx.rng.f32() * fx.rng.f32() * h;
                let w = 4.0 + up * 0.25;
                let (px, py) = (t.x + fx.rng.rangef(-w, w), t.y + 3.0 - up);
                let (cx, cy) = (px as i32, py as i32);
                let Some(c) = world.get(cx, cy) else { continue };
                if c.back.foliage() && fx.rng.chance(0.35) {
                    world.set_back(cx, cy, M::Empty);
                    let col = crate::render::shade(c.back.base_color(), fx.rng.next_u32() as u8, 0.4);
                    let vx = fx.rng.rangef(-40.0, 40.0);
                    fx.mote(px, py, vx, -60.0, 2.5, col, 0.0, 60.0, 30.0);
                }
                let loose = match c.mat.kind() {
                    K::Powder | K::Liquid => true,
                    K::Static => matches!(c.mat, M::Grass | M::Dirt | M::Snow | M::Leaves | M::Plant | M::Ice) && fx.rng.chance(0.12),
                    _ => false,
                };
                if loose {
                    world.set(cx, cy, Cell::EMPTY);
                    let m = match c.mat {
                        M::Grass => M::Dirt,
                        M::Ice => M::Snow,
                        M::Leaves | M::Plant => M::Empty,
                        other => other,
                    };
                    if m != M::Empty {
                        let (vx, vy) = (fx.rng.rangef(-60.0, 60.0), -fx.rng.rangef(60.0, 140.0));
                        fx.debris(px, py, vx, vy, m, c.shade);
                    }
                }
            }
            for _ in 0..3 {
                let up = fx.rng.f32() * h;
                let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                let r = 2.0 + up * 0.24;
                let g = fx.rng.rangef(150.0, 210.0) as u8;
                fx.mote(t.x + a.cos() * r, t.y - up, -a.sin() * 80.0, -40.0, 0.5, [g, g, (g as u16 + 8).min(255) as u8], 0.0, 0.0, 0.0);
            }
        }
        self.tornadoes.retain(|t| t.age < TWISTER_LIFE);
    }

    fn impact(&mut self, p: &Proj, world: &mut World, fx: &mut Fx, spawned: &mut Vec<(Shot, (f32, f32), f32)>) {
        let r = p.radius();
        match p.card {
            Card::PortalWand => {
                for k in 0..90 {
                    let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                    let sp = fx.rng.rangef(30.0, 170.0);
                    let col = crate::planets::hue_rgb(self.portal_hue + (k % 5) as f32 * 0.06 - 0.12, 0.75, 1.0);
                    let life = fx.rng.rangef(0.4, 1.1);
                    fx.mote(p.x, p.y, a.cos() * sp, a.sin() * sp, life, col, 1.2, 60.0, 0.0);
                }
                self.portal_hits.push((p.x - p.vx.signum(), p.y - p.vy.signum()));
                self.sounds.push(Sfx::Chime);
            }
            Card::BlackHole => {
                self.holes.push(Hole { x: p.x, y: p.y, age: 0.0, eaten: Vec::new() });
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
                fx.explosions.push((p.x, p.y, 2, crate::mobs::Dmg::Acid));
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
                Card::PortalWand => ([255, 255, 255], crate::planets::hue_rgb(self.portal_hue + (p.life * 6.0).sin() * 0.08, 0.8, 1.0)),
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
        for t in &self.tornadoes {
            let h = t.height() as i32;
            for k in 0..h {
                let r = 2.0 + k as f32 * 0.24;
                for j in 0..4 {
                    let ang = t.age * 14.0 - k as f32 * 0.32 + j as f32 * 1.571;
                    let front = ang.sin() > 0.0;
                    let x = t.x + ang.cos() * r;
                    let c = if front { [214, 214, 222] } else { [150, 150, 160] };
                    cv.blend(x as i32, t.y as i32 - k, c, if front { 0.55 } else { 0.3 });
                }
            }
        }
        for (pts, life) in &self.bolts {
            if *life < 0.25 && ((life * 40.0) as i32) % 2 == 1 {
                continue; // afterglow strobe
            }
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
        for h in &self.holes {
            let r = h.core() + 3.0;
            for k in 0..10 {
                let a = k as f32 * 0.628 + h.age * 3.0;
                add(h.x + a.cos() * r * 1.4, h.y + a.sin() * r * 0.8, [2.6, 1.2, 3.6]);
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
