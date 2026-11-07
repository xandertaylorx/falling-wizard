//! Creatures: pixel sprites drawn in code, simple behaviours, biome/depth/time spawning.

use crate::audio::Sfx;
use crate::cell::Cell;
use crate::fx::Fx;
use crate::materials::{Kind as MK, Material as M};
use crate::noise::Rng;
use crate::player::{Player, PH, PW};
use crate::render::Canvas;
use crate::world::World;
use crate::worldgen::Biome;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    Zombie,
    Skeleton,
    Slime,
    DemonEye,
    Bat,
    FireImp,
    Lion,
    Tiger,
    Scorpion,
    SnowWolf,
    Yeti,
    Golem,
    Salamander,
    Worm,
    Dragon,
    Ghost,
    StarJelly,
    MoonHopper,
    VoidEye,
    Wisp,
    CloudSprite,
    Mushling,
}

#[derive(Clone, Copy, PartialEq)]
enum Ai {
    Walker,
    Pouncer,
    Hopper,
    Flyer,
    Ghost,
    Burrower,
    Dragon,
}

#[derive(Clone, Copy, PartialEq)]
enum Ranged {
    None,
    Bone,
    Rock,
    Fire,
    Snow,
    Void,
    Zap,
}

struct Spec {
    sprite: &'static [&'static str],
    pal: &'static [(u8, [u8; 3])],
    hp: f32,
    speed: f32,
    dmg: f32,
    ai: Ai,
    ranged: Ranged,
    gib: M,
    light: Option<[f32; 3]>,
    fire_immune: bool,
    water_hurts: bool,
    gold: u32,
}

const fn spec(kind: MobKind) -> Spec {
    use MobKind::*;
    match kind {
        Zombie => Spec {
            sprite: &[".ggg..", ".ggR..", ".ggg..", ".bbbgg", ".bbb..", ".bbb..", ".nnn..", ".n.n..", ".n.n.."],
            pal: &[(b'g', [104, 156, 92]), (b'R', [230, 40, 40]), (b'b', [70, 90, 140]), (b'n', [64, 54, 70])],
            hp: 40.0, speed: 16.0, dmg: 10.0, ai: Ai::Walker, ranged: Ranged::None, gib: M::Blood, light: None, fire_immune: false, water_hurts: false, gold: 2,
        },
        Skeleton => Spec {
            sprite: &[".www..", ".wkw..", ".www..", "..w...", ".wwww.", "..w.w.", "..w...", ".w.w..", ".w.w.."],
            pal: &[(b'w', [232, 228, 212]), (b'k', [30, 20, 30])],
            hp: 30.0, speed: 26.0, dmg: 8.0, ai: Ai::Walker, ranged: Ranged::Bone, gib: M::Bone, light: None, fire_immune: false, water_hurts: false, gold: 3,
        },
        Slime => Spec {
            sprite: &["..sss..", ".sssss.", "ssesesS", "sssssss", ".sssss."],
            pal: &[(b's', [80, 204, 112]), (b'e', [20, 50, 30]), (b'S', [200, 255, 214])],
            hp: 24.0, speed: 30.0, dmg: 6.0, ai: Ai::Hopper, ranged: Ranged::None, gib: M::Goo, light: None, fire_immune: false, water_hurts: false, gold: 1,
        },
        DemonEye => Spec {
            sprite: &["..wwww...", ".wwrwww..", "wwwwwwwt.", "wwwwwwwtt", "wwwwwwwt.", ".wwwrww..", "..wwww..."],
            pal: &[(b'w', [240, 236, 230]), (b'r', [200, 40, 40]), (b't', [170, 30, 30])],
            hp: 22.0, speed: 60.0, dmg: 9.0, ai: Ai::Flyer, ranged: Ranged::None, gib: M::Blood, light: None, fire_immune: false, water_hurts: false, gold: 2,
        },
        Bat => Spec {
            sprite: &["k.....k", "kk.b.kk", ".kbbbk.", "...b..."],
            pal: &[(b'k', [70, 46, 82]), (b'b', [104, 70, 104])],
            hp: 10.0, speed: 80.0, dmg: 5.0, ai: Ai::Flyer, ranged: Ranged::None, gib: M::Blood, light: None, fire_immune: false, water_hurts: false, gold: 1,
        },
        FireImp => Spec {
            sprite: &["r...r", ".rrr.", ".yry.", ".rrr.", "rrrrr", ".r.r.", "..f.."],
            pal: &[(b'r', [210, 64, 40]), (b'y', [255, 230, 90]), (b'f', [255, 170, 40])],
            hp: 26.0, speed: 50.0, dmg: 8.0, ai: Ai::Flyer, ranged: Ranged::Fire, gib: M::Fire, light: Some([1.2, 0.5, 0.1]), fire_immune: true, water_hurts: true, gold: 3,
        },
        Lion => Spec {
            sprite: &[".........mmm.", "........mmmmm", "tt.yyyyyymyem", ".t.yyyyyymmmm", "...yyyyyyymm.", "...yy.yy.yy..", "...yy.yy.yy.."],
            pal: &[(b'y', [214, 164, 84]), (b'm', [146, 84, 32]), (b'e', [40, 20, 10]), (b't', [184, 132, 62])],
            hp: 70.0, speed: 40.0, dmg: 16.0, ai: Ai::Pouncer, ranged: Ranged::None, gib: M::Blood, light: None, fire_immune: false, water_hurts: false, gold: 6,
        },
        Tiger => Spec {
            sprite: &[".........oo..", "........owoo.", "kk.okokokoooe", ".t.oooooooww.", "...okokokoo..", "...oo.oo.oo..", "...kk.kk.kk.."],
            pal: &[(b'o', [232, 130, 40]), (b'k', [30, 22, 20]), (b'w', [240, 236, 226]), (b'e', [90, 200, 60]), (b't', [232, 130, 40])],
            hp: 80.0, speed: 46.0, dmg: 18.0, ai: Ai::Pouncer, ranged: Ranged::None, gib: M::Blood, light: None, fire_immune: false, water_hurts: false, gold: 7,
        },
        Scorpion => Spec {
            sprite: &["...ttt....", "..t...t...", "......r...", "cssssssss.", ".s.s.s.s.."],
            pal: &[(b's', [70, 46, 32]), (b't', [92, 58, 34]), (b'c', [120, 70, 36]), (b'r', [220, 60, 40])],
            hp: 28.0, speed: 44.0, dmg: 10.0, ai: Ai::Walker, ranged: Ranged::None, gib: M::Blood, light: None, fire_immune: false, water_hurts: false, gold: 2,
        },
        SnowWolf => Spec {
            sprite: &[".........ww", "........wwe", "wwwwwwwwww.", ".wwwwwwww..", ".w.w...w.w.", ".w.w...w.w."],
            pal: &[(b'w', [216, 222, 232]), (b'e', [40, 70, 110])],
            hp: 44.0, speed: 52.0, dmg: 12.0, ai: Ai::Pouncer, ranged: Ranged::None, gib: M::Blood, light: None, fire_immune: false, water_hurts: false, gold: 4,
        },
        Yeti => Spec {
            sprite: &["..ffff..", ".fbebef.", ".fbbbbf.", "ffffffff", "ffffffff", "ffffffff", ".ffffff.", ".ffffff.", ".ff..ff.", ".ff..ff.", ".ff..ff."],
            pal: &[(b'f', [236, 240, 246]), (b'b', [120, 150, 192]), (b'e', [20, 20, 40])],
            hp: 120.0, speed: 22.0, dmg: 20.0, ai: Ai::Walker, ranged: Ranged::Snow, gib: M::Snow, light: None, fire_immune: false, water_hurts: false, gold: 10,
        },
        Golem => Spec {
            sprite: &["..rrrrrr..", ".rryrryrr.", ".rrrrrrrr.", "rr.rrrr.rr", "rr.rrrr.rr", "rr.rcrr.rr", "rr.rrrr.rr", "...rrrr...", "..rr..rr..", "..rr..rr..", ".rrr..rrr.", ".rrr..rrr."],
            pal: &[(b'r', [112, 102, 104]), (b'y', [255, 200, 80]), (b'c', [255, 140, 40])],
            hp: 200.0, speed: 14.0, dmg: 25.0, ai: Ai::Walker, ranged: Ranged::Rock, gib: M::Rubble, light: Some([0.8, 0.5, 0.1]), fire_immune: true, water_hurts: false, gold: 15,
        },
        Salamander => Spec {
            sprite: &[".........oo.", "tooooooooooe", "..oooooooo..", "..o.o..o.o.."],
            pal: &[(b'o', [232, 92, 32]), (b'e', [255, 240, 120]), (b't', [255, 160, 40])],
            hp: 40.0, speed: 34.0, dmg: 12.0, ai: Ai::Walker, ranged: Ranged::Fire, gib: M::Lava, light: Some([0.9, 0.35, 0.05]), fire_immune: true, water_hurts: true, gold: 5,
        },
        Worm => Spec {
            sprite: &["ww", "ww"],
            pal: &[(b'w', [150, 108, 140])],
            hp: 160.0, speed: 70.0, dmg: 18.0, ai: Ai::Burrower, ranged: Ranged::None, gib: M::Goo, light: None, fire_immune: false, water_hurts: false, gold: 14,
        },
        Dragon => Spec {
            sprite: &["dd", "dd"],
            pal: &[(b'd', [176, 44, 40])],
            hp: 400.0, speed: 95.0, dmg: 24.0, ai: Ai::Dragon, ranged: Ranged::Fire, gib: M::Blood, light: Some([0.6, 0.25, 0.05]), fire_immune: true, water_hurts: false, gold: 50,
        },
        Ghost => Spec {
            sprite: &["..ggg..", ".ggggg.", "gkgggkg", "ggggggg", "ggg.ggg", "ggggggg", "ggggggg", "g.ggg.g"],
            pal: &[(b'g', [216, 228, 255]), (b'k', [40, 40, 70])],
            hp: 30.0, speed: 30.0, dmg: 10.0, ai: Ai::Ghost, ranged: Ranged::None, gib: M::Smoke, light: Some([0.25, 0.3, 0.45]), fire_immune: true, water_hurts: false, gold: 4,
        },
        StarJelly => Spec {
            sprite: &["..jjjjj..", ".jjjjjjj.", "jjjsjjsjj", "jjjjjjjjj", ".j.j.j.j.", ".j.j.j.j.", "j..j.j..j", ".j..j..j."],
            pal: &[(b'j', [176, 146, 255]), (b's', [255, 255, 255])],
            hp: 34.0, speed: 22.0, dmg: 10.0, ai: Ai::Flyer, ranged: Ranged::None, gib: M::Crystal, light: Some([0.6, 0.45, 1.0]), fire_immune: false, water_hurts: false, gold: 6,
        },
        MoonHopper => Spec {
            sprite: &[".l...l.", ".l...l.", ".mmmmm.", "mmemmmm", "mmmmmmm", ".m...m."],
            pal: &[(b'm', [184, 184, 214]), (b'l', [224, 204, 244]), (b'e', [64, 40, 124])],
            hp: 30.0, speed: 40.0, dmg: 8.0, ai: Ai::Hopper, ranged: Ranged::None, gib: M::Goo, light: None, fire_immune: false, water_hurts: false, gold: 4,
        },
        VoidEye => Spec {
            sprite: &["..vvvvv..", ".vvvvvvv.", "vvvvvvvvv", "vvvvvvvvv", "vvvvvvvvv", "vvvvvvvvv", "vvvvvvvvv", ".vvvvvvv.", "..vvvvv.."],
            pal: &[(b'v', [44, 22, 64])],
            hp: 50.0, speed: 34.0, dmg: 12.0, ai: Ai::Flyer, ranged: Ranged::Void, gib: M::Gas, light: Some([0.5, 0.1, 0.8]), fire_immune: false, water_hurts: false, gold: 8,
        },
        Wisp => Spec {
            sprite: &[".w.", "wWw", ".w."],
            pal: &[(b'w', [150, 255, 230]), (b'W', [255, 255, 255])],
            hp: 8.0, speed: 90.0, dmg: 4.0, ai: Ai::Flyer, ranged: Ranged::None, gib: M::Empty, light: Some([0.4, 1.2, 1.0]), fire_immune: true, water_hurts: false, gold: 2,
        },
        CloudSprite => Spec {
            sprite: &["..ccccc..", ".ccccccc.", "cceccecc.", "ccccccccc", ".c.c.c.c."],
            pal: &[(b'c', [240, 244, 252]), (b'e', [60, 80, 124])],
            hp: 30.0, speed: 30.0, dmg: 8.0, ai: Ai::Flyer, ranged: Ranged::Zap, gib: M::Steam, light: None, fire_immune: false, water_hurts: false, gold: 4,
        },
        Mushling => Spec {
            sprite: &[".ccc.", "ccccc", ".fef.", ".fff.", ".f.f."],
            pal: &[(b'c', [90, 232, 212]), (b'f', [222, 212, 192]), (b'e', [30, 30, 30])],
            hp: 20.0, speed: 20.0, dmg: 6.0, ai: Ai::Walker, ranged: Ranged::None, gib: M::Mushroom, light: Some([0.15, 0.6, 0.55]), fire_immune: false, water_hurts: false, gold: 2,
        },
    }
}

pub const ALL_MOBS: [MobKind; 22] = {
    use MobKind::*;
    [
        Zombie, Skeleton, Slime, DemonEye, Bat, FireImp, Lion, Tiger, Scorpion, SnowWolf, Yeti, Golem, Salamander, Worm, Dragon, Ghost,
        StarJelly, MoonHopper, VoidEye, Wisp, CloudSprite, Mushling,
    ]
};

impl MobKind {
    pub fn name(self) -> &'static str {
        use MobKind::*;
        match self {
            Zombie => "Zombie",
            Skeleton => "Skeleton",
            Slime => "Slime",
            DemonEye => "Demon Eye",
            Bat => "Bat",
            FireImp => "Fire Imp",
            Lion => "Lion",
            Tiger => "Tiger",
            Scorpion => "Scorpion",
            SnowWolf => "Snow Wolf",
            Yeti => "Yeti",
            Golem => "Golem",
            Salamander => "Salamander",
            Worm => "Devourer",
            Dragon => "Dragon",
            Ghost => "Ghost",
            StarJelly => "Star Jelly",
            MoonHopper => "Moon Hopper",
            VoidEye => "Void Eye",
            Wisp => "Wisp",
            CloudSprite => "Cloud Sprite",
            Mushling => "Mushling",
        }
    }
}

pub struct Mob {
    pub kind: MobKind,
    pub x: f32,
    pub y: f32,
    vx: f32,
    vy: f32,
    pub hp: f32,
    max_hp: f32,
    w: f32,
    h: f32,
    t: f32,
    facing: f32,
    on_ground: bool,
    hurt: f32,
    frozen: f32,
    cd: f32,
    attack_cd: f32,
    wander: f32,
    breath: f32,
    segs: Vec<(f32, f32)>,
    pub pet: bool,
}

/// A body flung by its death; it tumbles and splatters where it lands.
struct Corpse {
    kind: MobKind,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    rot: f32,
    spin: f32,
    t: f32,
    facing: f32,
}

struct MobShot {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    kind: Ranged,
    life: f32,
}

pub struct Popup {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub color: [u8; 3],
    pub t: f32,
}

pub struct Mobs {
    pub list: Vec<Mob>,
    corpses: Vec<Corpse>,
    shots: Vec<MobShot>,
    timer: f32,
    rng: Rng,
    pub popups: Vec<Popup>,
    pub sounds: Vec<Sfx>,
    pub gold: u32,
    pub kills: u32,
    pub big_hits: u32,
}

const N_SEG_WORM: usize = 14;
const N_SEG_DRAGON: usize = 20;

impl Mobs {
    pub fn new(seed: u64) -> Self {
        Self { list: Vec::new(), corpses: Vec::new(), shots: Vec::new(), timer: 3.0, rng: Rng::new(seed ^ 0xBEA57), popups: Vec::new(), sounds: Vec::new(), gold: 0, kills: 0, big_hits: 0 }
    }

    pub fn clear(&mut self) {
        self.list.retain(|m| m.pet);
        self.corpses.clear();
        self.shots.clear();
    }

    pub fn spawn(&mut self, kind: MobKind, x: f32, y: f32) {
        let s = spec(kind);
        let (w, h) = match s.ai {
            Ai::Burrower | Ai::Dragon => (6.0, 6.0),
            _ => (s.sprite[0].len() as f32, s.sprite.len() as f32),
        };
        let segs = match s.ai {
            Ai::Burrower => vec![(x, y); N_SEG_WORM],
            Ai::Dragon => vec![(x, y); N_SEG_DRAGON],
            _ => Vec::new(),
        };
        self.list.push(Mob {
            kind,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            hp: s.hp,
            max_hp: s.hp,
            w,
            h,
            t: self.rng.f32() * 10.0,
            facing: 1.0,
            on_ground: false,
            hurt: 0.0,
            frozen: 0.0,
            cd: self.rng.rangef(1.0, 3.0),
            attack_cd: 0.0,
            wander: 0.0,
            breath: 0.0,
            segs,
            pet: false,
        });
    }

    fn center(m: &Mob) -> (f32, f32) {
        (m.x + m.w * 0.5, m.y + m.h * 0.5)
    }

    /// Index of the mob at (x, y), with `pad` cells of slack.
    pub fn hit_test(&self, x: f32, y: f32, pad: f32) -> Option<usize> {
        for (i, m) in self.list.iter().enumerate() {
            if m.pet {
                continue;
            }
            if x >= m.x - pad && x < m.x + m.w + pad && y >= m.y - pad && y < m.y + m.h + pad {
                return Some(i);
            }
            for s in &m.segs {
                if (x - s.0).abs() < 3.0 + pad && (y - s.1).abs() < 3.0 + pad {
                    return Some(i);
                }
            }
        }
        None
    }

    pub fn nearest(&self, x: f32, y: f32, range: f32) -> Option<(f32, f32)> {
        self.list
            .iter()
            .filter(|m| !m.pet)
            .map(Self::center)
            .map(|c| (c, (c.0 - x).powi(2) + (c.1 - y).powi(2)))
            .filter(|(_, d)| *d < range * range)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(c, _)| c)
    }

    pub fn hurt(&mut self, i: usize, dmg: f32, knock: (f32, f32), fx: &mut Fx) {
        let m = &mut self.list[i];
        if m.hp <= 0.0 {
            return;
        }
        m.hp -= dmg;
        m.hurt = 0.15;
        let heavy = matches!(spec(m.kind).ai, Ai::Dragon | Ai::Burrower) || m.kind == MobKind::Golem;
        if !heavy {
            m.vx += knock.0;
            m.vy += knock.1;
        }
        let (cx, cy) = Self::center(m);
        if dmg >= 1.0 {
            self.popups.push(Popup { x: cx, y: m.y - 2.0, text: format!("{}", dmg.round() as i32), color: [255, 236, 200], t: 0.0 });
        }
        fx.burst(cx, cy, 4, 40.0, [255, 240, 220]);
        self.sounds.push(Sfx::Hit);
    }

    pub fn damage_area(&mut self, x: f32, y: f32, r: f32, dmg: f32, fx: &mut Fx) {
        for i in 0..self.list.len() {
            if self.list[i].pet {
                continue;
            }
            let (cx, cy) = Self::center(&self.list[i]);
            let mut d = ((cx - x).powi(2) + (cy - y).powi(2)).sqrt();
            for s in &self.list[i].segs {
                d = d.min(((s.0 - x).powi(2) + (s.1 - y).powi(2)).sqrt());
            }
            if d < r {
                let k = 1.0 - d / r;
                let (dx, dy) = ((cx - x) / d.max(1.0), (cy - y) / d.max(1.0));
                self.hurt(i, dmg * (0.4 + 0.6 * k), (dx * 120.0 * k, dy * 120.0 * k - 60.0 * k), fx);
            }
        }
    }

    pub fn damage_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, dmg: f32, fx: &mut Fx) {
        let len = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt().max(1.0);
        let mut hit = vec![false; self.list.len()];
        let steps = (len / 2.0) as i32 + 1;
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            if let Some(i) = self.hit_test(x, y, r) {
                hit[i] = true;
            }
        }
        for (i, h) in hit.into_iter().enumerate() {
            if h {
                self.hurt(i, dmg, (0.0, -40.0), fx);
            }
        }
    }

    pub fn kill_all(&mut self, pets: bool) -> usize {
        let mut n = 0;
        for m in self.list.iter_mut().filter(|m| m.pet == pets) {
            m.hp = 0.0;
            n += 1;
        }
        n
    }

    pub fn pets(&self) -> usize {
        self.list.iter().filter(|m| m.pet).count()
    }

    /// Make a creature your pet (up to three; worms won't be tamed).
    pub fn tame(&mut self, i: usize, fx: &mut Fx) -> bool {
        if self.pets() >= 3 || self.list[i].kind == MobKind::Worm {
            return false;
        }
        let m = &mut self.list[i];
        m.pet = true;
        m.hp = m.max_hp;
        let (cx, _) = Self::center(m);
        self.popups.push(Popup { x: cx, y: m.y - 4.0, text: format!("{} TAMED!", m.kind.name()), color: [140, 255, 150], t: 0.0 });
        fx.burst(cx, m.y, 30, 70.0, [140, 255, 150]);
        true
    }

    /// Drag creatures towards a black hole; those at its heart are torn apart.
    pub fn pull(&mut self, x: f32, y: f32, r: f32, dt: f32, _fx: &mut Fx) {
        for m in self.list.iter_mut().filter(|m| !m.pet) {
            let (cx, cy) = Self::center(m);
            let (dx, dy) = (x - cx, y - cy);
            let d = (dx * dx + dy * dy).sqrt().max(0.5);
            if d < r {
                let k = (1.0 - d / r) * 420.0 * dt;
                m.vx += dx / d * k;
                m.vy += dy / d * k;
                m.x += dx / d * k * dt * 20.0;
                m.y += dy / d * k * dt * 20.0;
                if d < 5.0 {
                    m.hp -= 80.0 * dt;
                    m.hurt = 0.05;
                }
            }
        }
    }

    pub fn freeze(&mut self, i: usize, secs: f32) {
        self.list[i].frozen = secs;
    }

    /// Spawn creatures that fit where the player is: biome, depth, sky, time of day.
    pub fn spawn_tick(&mut self, world: &World, px: f32, py: f32, night: bool, dt: f32) {
        self.timer -= dt;
        if self.timer > 0.0 {
            return;
        }
        self.timer = self.rng.rangef(0.8, 2.2) * if night { 0.6 } else { 1.0 };
        let cap = if night { 16 } else { 11 };
        if self.list.len() - self.pets() >= cap {
            return;
        }
        let side = self.rng.sign() as f32;
        let x = px + side * self.rng.rangef(110.0, 180.0);
        let y = py + self.rng.rangef(-70.0, 70.0);
        let surf = world.gen.surface(x as i32);
        let depth = y as i32 - surf;
        let biome = world.gen.biome(x as i32);
        use MobKind::*;
        let table: &[(MobKind, f32)] = if y < -520.0 {
            &[(StarJelly, 3.0), (MoonHopper, 3.0), (VoidEye, 2.0), (Wisp, 2.0), (Dragon, 0.35)]
        } else if y < -230.0 {
            &[(CloudSprite, 3.0), (DemonEye, 1.0), (Wisp, 1.5), (Dragon, 0.4)]
        } else if depth < 10 {
            match (biome, night) {
                (Biome::Forest, false) => &[(Slime, 3.0), (Tiger, 1.0), (Bat, 0.3)],
                (Biome::Forest, true) => &[(Zombie, 4.0), (DemonEye, 3.0), (Slime, 1.0), (Ghost, 0.5)],
                (Biome::Desert, false) => &[(Scorpion, 3.0), (Lion, 2.0)],
                (Biome::Desert, true) => &[(Zombie, 2.0), (Ghost, 2.0), (Scorpion, 1.0), (DemonEye, 1.0)],
                (Biome::Snow, false) => &[(SnowWolf, 3.0), (Yeti, 1.0)],
                (Biome::Snow, true) => &[(Zombie, 2.0), (DemonEye, 2.0), (SnowWolf, 1.0), (Yeti, 0.6)],
            }
        } else if depth < 170 {
            &[(Skeleton, 3.0), (Bat, 3.0), (Slime, 1.5), (Mushling, 2.0), (Worm, 0.3), (Ghost, 0.4)]
        } else {
            &[(Golem, 1.5), (Salamander, 3.0), (FireImp, 3.0), (Worm, 0.8), (Ghost, 1.0), (Skeleton, 1.0)]
        };
        let total: f32 = table.iter().map(|t| t.1).sum();
        let mut r = self.rng.f32() * total;
        let mut kind = table[0].0;
        for (k, w) in table {
            if r < *w {
                kind = *k;
                break;
            }
            r -= w;
        }
        let s = spec(kind);
        let (w, h) = (s.sprite[0].len() as i32, s.sprite.len() as i32);
        let free = |x0: i32, y0: i32| (0..h).all(|dy| (0..w).all(|dx| world.mat(x0 + dx, y0 + dy).passable()) && world.in_window(x0, y0 + dy));
        let (xi, yi) = (x as i32, y as i32);
        match s.ai {
            Ai::Flyer | Ai::Ghost | Ai::Dragon => {
                if free(xi, yi) || s.ai == Ai::Ghost {
                    self.spawn(kind, x, y);
                }
            }
            Ai::Burrower => {
                if world.mat(xi, yi).solid() {
                    self.spawn(kind, x, y);
                }
            }
            _ => {
                for k in 0..60 {
                    let yy = yi - 30 + k;
                    if free(xi, yy) && world.mat(xi + w / 2, yy + h).solid() {
                        self.spawn(kind, x, yy as f32);
                        break;
                    }
                }
            }
        }
    }

    fn solid_at(world: &World, x: f32, y: f32, w: f32, h: f32) -> bool {
        let (x0, x1) = (x.floor() as i32, (x + w - 0.01).floor() as i32);
        let (y0, y1) = (y.floor() as i32, (y + h - 0.01).floor() as i32);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                if world.mat(cx, cy).solid() {
                    return true;
                }
            }
        }
        false
    }

    pub fn update(&mut self, world: &mut World, player: &mut Player, fx: &mut Fx, dt: f32) {
        let (px, py) = player.center();
        let alive = !player.dead();
        let mut new_shots = Vec::new();
        let hostiles: Vec<(usize, f32, f32)> = self.list.iter().enumerate().filter(|(_, m)| !m.pet).map(|(i, m)| (i, m.x + m.w * 0.5, m.y + m.h * 0.5)).collect();
        let mut pet_hits: Vec<(usize, f32, f32)> = Vec::new();
        for m in self.list.iter_mut() {
            let s = spec(m.kind);
            m.t += dt;
            m.hurt = (m.hurt - dt).max(0.0);
            m.frozen = (m.frozen - dt).max(0.0);
            m.cd -= dt;
            m.attack_cd -= dt;
            let (cx, cy) = Self::center(m);

            // Environment.
            let here = world.mat(cx as i32, cy as i32);
            let env = match here {
                M::Fire if !s.fire_immune => 14.0,
                M::Lava if !s.fire_immune => 50.0,
                M::Acid => 25.0,
                M::Water if s.water_hurts => 30.0,
                _ => 0.0,
            };
            if env > 0.0 {
                m.hp -= env * dt;
                m.hurt = m.hurt.max(0.05);
            }

            // Pets chase the nearest hostile creature, otherwise follow you.
            let mut target: Option<usize> = None;
            let (gx, gy) = if m.pet {
                let best = hostiles
                    .iter()
                    .map(|&(i, hx, hy)| (i, hx, hy, (hx - cx).powi(2) + (hy - cy).powi(2)))
                    .filter(|h| h.3 < 100.0 * 100.0)
                    .min_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
                match best {
                    Some((i, hx, hy, _)) => {
                        target = Some(i);
                        (hx, hy)
                    }
                    None => (px - m.facing * 10.0, py),
                }
            } else {
                (px, py)
            };
            let (dx, dy) = (gx - cx, gy - cy);
            let dist = (dx * dx + dy * dy).sqrt();
            let aggro = if m.pet { true } else { alive && dist < 170.0 };
            let dir = if m.pet && target.is_none() && dx.abs() < 10.0 {
                0.0
            } else if aggro {
                dx.signum()
            } else {
                m.wander -= dt;
                if m.wander <= 0.0 {
                    m.wander = 2.0 + (m.t * 13.7).sin().abs() * 3.0;
                    m.facing = -m.facing;
                }
                m.facing
            };
            if dir != 0.0 {
                m.facing = dir;
            }
            let slow = if m.frozen > 0.0 { 0.15 } else { 1.0 };

            match s.ai {
                Ai::Walker | Ai::Pouncer | Ai::Hopper => {
                    let ground = m.on_ground;
                    match s.ai {
                        Ai::Hopper => {
                            if ground {
                                m.vx *= 0.8;
                                if m.cd <= 0.0 && slow > 0.5 {
                                    m.vy = -(70.0 + (m.t * 7.3).sin().abs() * 40.0);
                                    m.vx = dir * s.speed * 1.5;
                                    m.cd = 0.8 + (m.t * 3.1).sin().abs() * 0.8;
                                }
                            }
                        }
                        _ => {
                            let target = dir * s.speed * if aggro { 1.0 } else { 0.4 } * slow;
                            let acc = if ground { 300.0 } else { 120.0 };
                            m.vx += (target - m.vx).clamp(-acc * dt, acc * dt);
                            if s.ai == Ai::Pouncer && ground && aggro && dist < 40.0 && m.cd <= 0.0 {
                                m.vy = -95.0;
                                m.vx = dir * 140.0;
                                m.cd = 2.0;
                            }
                        }
                    }
                    m.vy = (m.vy + 260.0 * dt).min(180.0);
                    // Horizontal with step-up and jump when blocked.
                    let nx = m.x + m.vx * dt;
                    if !Self::solid_at(world, nx, m.y, m.w, m.h) {
                        m.x = nx;
                    } else if !Self::solid_at(world, nx, m.y - 1.0, m.w, m.h) {
                        m.x = nx;
                        m.y -= 1.0;
                    } else {
                        if ground && s.ai != Ai::Hopper {
                            m.vy = -85.0;
                        }
                        m.vx = 0.0;
                    }
                    let ny = m.y + m.vy * dt;
                    if !Self::solid_at(world, m.x, ny, m.w, m.h) {
                        m.y = ny;
                        m.on_ground = false;
                    } else {
                        if m.vy > 0.0 {
                            m.on_ground = true;
                            if !Self::solid_at(world, m.x, ny.floor(), m.w, m.h) {
                                m.y = ny.floor();
                            }
                        }
                        m.vy = 0.0;
                    }
                    m.on_ground = Self::solid_at(world, m.x, m.y + 1.0, m.w, m.h);
                }
                Ai::Flyer | Ai::Ghost => {
                    let (tx, ty) = if aggro {
                        (px + (m.t * 1.3).sin() * 18.0, py - 10.0 + (m.t * 0.9).cos() * 12.0)
                    } else {
                        (cx + m.facing * 30.0, cy + (m.t * 0.7).sin() * 10.0)
                    };
                    let (ax, ay) = (tx - cx, ty - cy);
                    let l = (ax * ax + ay * ay).sqrt().max(1.0);
                    m.vx += ax / l * s.speed * 2.0 * dt * slow;
                    m.vy += ay / l * s.speed * 2.0 * dt * slow;
                    let sp = (m.vx * m.vx + m.vy * m.vy).sqrt();
                    if sp > s.speed {
                        m.vx *= s.speed / sp;
                        m.vy *= s.speed / sp;
                    }
                    if m.frozen > 0.0 {
                        m.vy += 100.0 * dt;
                    }
                    let (nx, ny) = (m.x + m.vx * dt, m.y + m.vy * dt);
                    if s.ai == Ai::Ghost || !Self::solid_at(world, nx, ny, m.w, m.h) {
                        m.x = nx;
                        m.y = ny;
                    } else {
                        m.vx = -m.vx * 0.6;
                        m.vy = -m.vy * 0.6;
                    }
                }
                Ai::Burrower => {
                    let inside = world.mat(cx as i32, cy as i32).solid();
                    if inside {
                        let (ax, ay) = if aggro { (dx, dy) } else { (m.facing, (m.t).sin()) };
                        let l = (ax * ax + ay * ay).sqrt().max(0.01);
                        let k = 2.5 * dt;
                        m.vx += (ax / l * s.speed - m.vx) * k;
                        m.vy += (ay / l * s.speed - m.vy) * k;
                        for (ddx, ddy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                            let (gx, gy) = (cx as i32 + ddx * 2, cy as i32 + ddy * 2);
                            if world.mat(gx, gy).kind() == MK::Static || world.mat(gx, gy).kind() == MK::Powder {
                                world.set(gx, gy, Cell::EMPTY);
                            }
                        }
                    } else {
                        m.vy += 150.0 * dt;
                    }
                    m.x += m.vx * dt * slow;
                    m.y += m.vy * dt * slow;
                }
                Ai::Dragon => {
                    let (tx, ty) = if aggro { (px - m.facing * 20.0 + (m.t * 0.8).sin() * 40.0, py - 35.0 + (m.t * 1.7).sin() * 20.0) } else { (cx + m.facing * 40.0, cy + (m.t).sin() * 8.0) };
                    let (ax, ay) = (tx - cx, ty - cy);
                    let l = (ax * ax + ay * ay).sqrt().max(1.0);
                    m.vx += ax / l * 160.0 * dt;
                    m.vy += ay / l * 160.0 * dt;
                    let sp = (m.vx * m.vx + m.vy * m.vy).sqrt();
                    if sp > s.speed {
                        m.vx *= s.speed / sp;
                        m.vy *= s.speed / sp;
                    }
                    m.x += m.vx * dt * slow;
                    m.y += m.vy * dt * slow;
                    if m.vx.abs() > 5.0 {
                        m.facing = m.vx.signum();
                    }
                    // Fire breath.
                    if aggro && !m.pet && dist < 80.0 && m.cd <= 0.0 {
                        m.breath = 1.0;
                        m.cd = 4.0;
                    }
                    if m.breath > 0.0 {
                        m.breath -= dt;
                        let (hx, hy) = (cx, cy);
                        let l = dist.max(1.0);
                        for _ in 0..2 {
                            let jit = (m.t * 50.0).sin() * 0.2;
                            new_shots.push(MobShot { x: hx, y: hy, vx: (dx / l + jit) * 130.0, vy: (dy / l - jit) * 130.0, kind: Ranged::Fire, life: 0.8 });
                        }
                    }
                }
            }
            // Segments follow the head.
            if !m.segs.is_empty() {
                let mut prev = Self::center(m);
                for sgm in m.segs.iter_mut() {
                    let (ddx, ddy) = (sgm.0 - prev.0, sgm.1 - prev.1);
                    let d = (ddx * ddx + ddy * ddy).sqrt();
                    if d > 3.0 {
                        sgm.0 = prev.0 + ddx / d * 3.0;
                        sgm.1 = prev.1 + ddy / d * 3.0;
                    }
                    prev = *sgm;
                }
            }

            // Ranged attacks.
            if !m.pet && aggro && s.ranged != Ranged::None && s.ai != Ai::Dragon && m.cd <= 0.0 && dist < 110.0 && m.frozen <= 0.0 {
                m.cd = 2.2 + (m.t * 5.1).sin().abs();
                let (cx, cy) = Self::center(m);
                let speed = match s.ranged {
                    Ranged::Void | Ranged::Zap => 110.0,
                    Ranged::Fire => 120.0,
                    _ => 150.0,
                };
                let lob = if matches!(s.ranged, Ranged::Bone | Ranged::Rock | Ranged::Snow) { -dist * 0.9 } else { 0.0 };
                let l = dist.max(1.0);
                new_shots.push(MobShot { x: cx, y: cy - 2.0, vx: dx / l * speed, vy: dy / l * speed + lob, kind: s.ranged, life: 3.0 });
            }

            // Pets bite their target.
            if m.pet {
                if let Some(ti) = target {
                    if dist < (m.w + m.h) * 0.4 + 4.0 && m.attack_cd <= 0.0 {
                        pet_hits.push((ti, s.dmg * 1.5, m.facing));
                        m.attack_cd = 0.6;
                    }
                }
                continue;
            }
            // Contact damage.
            if alive && m.attack_cd <= 0.0 && m.frozen <= 0.0 {
                let touching = |x: f32, y: f32, w: f32, h: f32| x < player.x + PW && x + w > player.x && y < player.y + PH && y + h > player.y;
                let hit = touching(m.x, m.y, m.w, m.h) || m.segs.iter().any(|sg| touching(sg.0 - 2.5, sg.1 - 2.5, 5.0, 5.0));
                if hit {
                    player.hurt(s.dmg);
                    player.vx += (player.x - m.x).signum() * 90.0;
                    player.vy -= 60.0;
                    m.attack_cd = 0.8;
                }
            }
        }
        self.shots.append(&mut new_shots);
        for (i, dmg, dir) in pet_hits {
            self.hurt(i, dmg, (dir * 60.0, -40.0), fx);
        }
        self.update_shots(world, player, fx, dt);
        self.update_corpses(world, fx, dt);

        // Deaths and despawns.
        let mut i = 0;
        while i < self.list.len() {
            let m = &self.list[i];
            let (cx, cy) = Self::center(m);
            let far = (cx - px).abs() > 280.0 || (cy - py).abs() > 220.0 || !world.in_window(cx as i32, cy as i32);
            if m.hp <= 0.0 {
                let m = self.list.swap_remove(i);
                self.gib(&m, fx);
                continue;
            }
            if far && m.pet {
                let m = &mut self.list[i];
                m.x = px - 6.0;
                m.y = py - 14.0;
                m.vx = 0.0;
                m.vy = 0.0;
                for sg in m.segs.iter_mut() {
                    *sg = (px, py - 14.0);
                }
            } else if far {
                self.list.swap_remove(i);
                continue;
            }
            i += 1;
        }
        for p in self.popups.iter_mut() {
            p.t += dt;
            p.y -= 12.0 * dt;
        }
        self.popups.retain(|p| p.t < 1.0);
    }

    fn gib(&mut self, m: &Mob, fx: &mut Fx) {
        let s = spec(m.kind);
        let (cx, cy) = Self::center(m);
        if m.segs.is_empty() && s.gib != M::Empty {
            // Ragdoll: fling the body; it splatters when it lands.
            let up = fx.rng.rangef(70.0, 140.0);
            self.corpses.push(Corpse {
                kind: m.kind,
                x: cx,
                y: cy,
                vx: m.vx * 1.4 + fx.rng.rangef(-40.0, 40.0),
                vy: m.vy.min(0.0) * 1.2 - up,
                rot: 0.0,
                spin: fx.rng.rangef(-14.0, 14.0),
                t: 0.0,
                facing: m.facing,
            });
        } else {
            self.splatter(m.kind, cx, cy, &m.segs, fx);
        }
        let big = s.hp >= 100.0;
        if big {
            self.big_hits += 1;
            fx.shake = (fx.shake + 6.0).min(14.0);
        }
        if !m.pet {
            self.gold += s.gold;
            self.kills += 1;
            self.popups.push(Popup { x: cx, y: cy - 6.0, text: format!("{}  +{} GOLD", m.kind.name(), s.gold), color: [255, 210, 90], t: 0.0 });
        }
        self.sounds.push(if big { Sfx::Boom } else { Sfx::Hit });
    }

    fn update_corpses(&mut self, world: &mut World, fx: &mut Fx, dt: f32) {
        let mut corpses = std::mem::take(&mut self.corpses);
        corpses.retain_mut(|c| {
            c.t += dt;
            c.vy += 260.0 * dt;
            c.rot += c.spin * dt;
            let n = ((c.vx.abs().max(c.vy.abs())) * dt).ceil().max(1.0) as i32;
            for _ in 0..n {
                let (nx, ny) = (c.x + c.vx * dt / n as f32, c.y + c.vy * dt / n as f32);
                let m = world.mat(nx as i32, (ny + 2.0) as i32);
                if m.solid() || m.kind() == MK::Liquid || !world.in_window(nx as i32, ny as i32) {
                    let speed = (c.vx * c.vx + c.vy * c.vy).sqrt();
                    if speed > 70.0 || c.t > 1.2 {
                        self.splatter(c.kind, c.x, c.y, &[], fx);
                        self.sounds.push(Sfx::Pop);
                        return false;
                    }
                    c.vy = -c.vy * 0.4;
                    c.vx *= 0.6;
                    c.spin *= 0.5;
                    break;
                }
                c.x = nx;
                c.y = ny;
            }
            if c.t > 3.0 {
                self.splatter(c.kind, c.x, c.y, &[], fx);
                return false;
            }
            true
        });
        self.corpses = corpses;
    }

    /// Burst a body into its gib material and sparks.
    fn splatter(&mut self, kind: MobKind, cx: f32, cy: f32, segs: &[(f32, f32)], fx: &mut Fx) {
        let s = spec(kind);
        let (w, h) = (s.sprite[0].len(), s.sprite.len());
        let mut parts: Vec<(f32, f32)> = vec![(cx, cy)];
        parts.extend(segs.iter().copied());
        for (x, y) in parts {
            let n = (w * h / 2).clamp(6, 40);
            for _ in 0..n {
                let a = fx.rng.rangef(0.0, std::f32::consts::TAU);
                let sp = fx.rng.rangef(30.0, 120.0);
                if s.gib != M::Empty {
                    let sh = fx.rng.next_u32() as u8;
                    fx.debris(x, y, a.cos() * sp, a.sin() * sp - 50.0, s.gib, sh);
                }
            }
            fx.burst(x, y, 12, 90.0, [255, 230, 200]);
        }
    }

    fn update_shots(&mut self, world: &mut World, player: &mut Player, fx: &mut Fx, dt: f32) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|s| {
            s.life -= dt;
            if matches!(s.kind, Ranged::Bone | Ranged::Rock | Ranged::Snow) {
                s.vy += 200.0 * dt;
            }
            let n = ((s.vx.abs().max(s.vy.abs())) * dt).ceil().max(1.0) as i32;
            for _ in 0..n {
                s.x += s.vx * dt / n as f32;
                s.y += s.vy * dt / n as f32;
                let (gx, gy) = (s.x.floor() as i32, s.y.floor() as i32);
                if !world.in_window(gx, gy) {
                    return false;
                }
                let hit_player = !player.dead() && s.x >= player.x && s.x < player.x + PW && s.y >= player.y && s.y < player.y + PH;
                let m = world.mat(gx, gy);
                if hit_player || !m.passable() {
                    if hit_player {
                        player.hurt(match s.kind {
                            Ranged::Rock => 14.0,
                            Ranged::Void => 12.0,
                            Ranged::Fire => 6.0,
                            _ => 8.0,
                        });
                        if s.kind == Ranged::Fire {
                            player.burning = player.burning.max(1.5);
                        }
                    }
                    match s.kind {
                        Ranged::Fire => {
                            world.ignite_at(gx, gy);
                            if world.mat(gx, gy - 1) == M::Empty {
                                let f = Cell::fire(&mut world.rng);
                                world.set(gx, gy - 1, f);
                            }
                        }
                        Ranged::Rock => fx.debris(s.x, s.y - 1.0, -s.vx * 0.2, -40.0, M::Rubble, 120),
                        Ranged::Snow => fx.debris(s.x, s.y - 1.0, 0.0, -20.0, M::Snow, 120),
                        Ranged::Bone => fx.debris(s.x, s.y - 1.0, 0.0, -20.0, M::Bone, 120),
                        _ => {}
                    }
                    fx.burst(s.x, s.y, 6, 50.0, shot_color(s.kind));
                    return false;
                }
            }
            s.life > 0.0
        });
        self.shots = shots;
    }

    pub fn draw(&self, cv: &mut Canvas, px: f32, py: f32) {
        for m in &self.list {
            let s = spec(m.kind);
            let tint = |c: [u8; 3]| -> [u8; 3] {
                if m.hurt > 0.0 {
                    [255, 255, 255]
                } else if m.frozen > 0.0 {
                    [(c[0] as u16 / 2 + 70) as u8, (c[1] as u16 / 2 + 100) as u8, (c[2] as u16 / 2 + 127) as u8]
                } else {
                    c
                }
            };
            match s.ai {
                Ai::Burrower => {
                    for (k, sg) in m.segs.iter().enumerate().rev() {
                        let r = if k < 2 { 3 } else { 2 };
                        let shade = if k % 2 == 0 { [150, 108, 140] } else { [124, 86, 118] };
                        disc(cv, sg.0, sg.1, r, tint(shade));
                    }
                    let (cx, cy) = Self::center(m);
                    disc(cv, cx, cy, 3, tint([110, 70, 104]));
                    cv.put(cx as i32 + (m.vx.signum() * 2.0) as i32, cy as i32, [255, 240, 200]);
                }
                Ai::Dragon => draw_dragon(cv, m, &tint),
                _ => {
                    let bob = if matches!(s.ai, Ai::Walker | Ai::Pouncer) && m.vx.abs() > 2.0 && ((m.t * 8.0) as i32) % 2 == 1 { 1 } else { 0 };
                    let flap = m.kind == MobKind::Bat && ((m.t * 10.0) as i32) % 2 == 1;
                    let h = s.sprite.len();
                    for (row, line) in s.sprite.iter().enumerate() {
                        let line = if flap { s.sprite[h - 1 - row].as_bytes() } else { line.as_bytes() };
                        let w = line.len();
                        for col in 0..w {
                            let ch = if m.facing >= 0.0 { line[col] } else { line[w - 1 - col] };
                            let Some(&(_, c)) = s.pal.iter().find(|p| p.0 == ch) else { continue };
                            let (gx, gy) = (m.x as i32 + col as i32, m.y as i32 + row as i32 + bob);
                            if m.kind == MobKind::Ghost {
                                cv.blend(gx, gy, tint(c), 0.55);
                            } else {
                                cv.put(gx, gy, tint(c));
                            }
                        }
                    }
                    // Eyes that watch you.
                    if matches!(m.kind, MobKind::DemonEye | MobKind::VoidEye) {
                        let (cx, cy) = Self::center(m);
                        let (dx, dy) = (px - cx, py - cy);
                        let l = (dx * dx + dy * dy).sqrt().max(1.0);
                        let (ix, iy) = ((cx + dx / l * 1.5) as i32, (cy + dy / l * 1.5) as i32);
                        let iris = if m.kind == MobKind::DemonEye { [60, 100, 210] } else { [210, 70, 255] };
                        for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0), (0, -1)] {
                            cv.put(ix + ox, iy + oy, tint(iris));
                        }
                        cv.put(ix, iy, [10, 10, 20]);
                    }
                    if m.pet {
                        let (hx, hy) = (m.x as i32 + m.w as i32 / 2 - 1, m.y as i32 - 4);
                        for (ox, oy) in [(0, 0), (2, 0), (0, 1), (1, 1), (2, 1), (1, 2)] {
                            cv.put(hx + ox, hy + oy, [255, 96, 130]);
                        }
                    }
                    // Health bar once damaged.
                    if m.hp < m.max_hp {
                        let w = m.w as i32;
                        let filled = ((m.hp / m.max_hp).max(0.0) * w as f32) as i32;
                        for k in 0..w {
                            let full = if m.pet { [90, 220, 110] } else { [230, 70, 70] };
                            cv.put(m.x as i32 + k, m.y as i32 - 2, if k < filled { full } else { [60, 20, 20] });
                        }
                    }
                }
            }
        }
        for c in &self.corpses {
            let s = spec(c.kind);
            let (w, h) = (s.sprite[0].len() as f32, s.sprite.len() as f32);
            let r = (w.max(h) * 0.75) as i32 + 1;
            let (sn, cs) = c.rot.sin_cos();
            for dy in -r..=r {
                for dx in -r..=r {
                    // Inverse-rotate each screen pixel back into the sprite.
                    let (fx_, fy_) = (dx as f32 * cs + dy as f32 * sn, -dx as f32 * sn + dy as f32 * cs);
                    let (sx, sy) = ((fx_ + w * 0.5).floor() as i32, (fy_ + h * 0.5).floor() as i32);
                    if sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 {
                        continue;
                    }
                    let line = s.sprite[sy as usize].as_bytes();
                    let col = if c.facing >= 0.0 { sx as usize } else { line.len() - 1 - sx as usize };
                    if let Some(&(_, rgb)) = s.pal.iter().find(|p| p.0 == line[col]) {
                        cv.put(c.x as i32 + dx, c.y as i32 + dy, [(rgb[0] as f32 * 0.8) as u8, (rgb[1] as f32 * 0.8) as u8, (rgb[2] as f32 * 0.8) as u8]);
                    }
                }
            }
        }
        for s in &self.shots {
            cv.put(s.x as i32, s.y as i32, shot_color(s.kind));
            cv.put(s.x as i32 + 1, s.y as i32, shot_color(s.kind));
        }
    }

    pub fn lights(&self, add: &mut impl FnMut(f32, f32, [f32; 3])) {
        for m in &self.list {
            if let Some(l) = spec(m.kind).light {
                let (cx, cy) = Self::center(m);
                add(cx, cy, [l[0] * 3.0, l[1] * 3.0, l[2] * 3.0]);
                if m.breath > 0.0 {
                    add(cx, cy, [6.0, 2.5, 0.5]);
                }
            }
        }
        for s in &self.shots {
            if matches!(s.kind, Ranged::Fire | Ranged::Void | Ranged::Zap) {
                let c = shot_color(s.kind);
                add(s.x, s.y, [c[0] as f32 / 100.0, c[1] as f32 / 100.0, c[2] as f32 / 100.0]);
            }
        }
    }
}

fn shot_color(k: Ranged) -> [u8; 3] {
    match k {
        Ranged::Bone => [230, 226, 210],
        Ranged::Rock => [120, 110, 108],
        Ranged::Fire => [255, 150, 40],
        Ranged::Snow => [240, 246, 255],
        Ranged::Void => [200, 80, 255],
        Ranged::Zap => [255, 255, 160],
        Ranged::None => [255, 255, 255],
    }
}

fn disc(cv: &mut Canvas, x: f32, y: f32, r: i32, c: [u8; 3]) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r + 1 {
                cv.put(x as i32 + dx, y as i32 + dy, c);
            }
        }
    }
}

fn draw_dragon(cv: &mut Canvas, m: &Mob, tint: &dyn Fn([u8; 3]) -> [u8; 3]) {
    let n = m.segs.len();
    for (k, sg) in m.segs.iter().enumerate().rev() {
        let t = k as f32 / n as f32;
        let r = (3.5 * (1.0 - t * 0.75)).round() as i32;
        disc(cv, sg.0, sg.1, r, tint([176, 44, 40]));
        if k % 2 == 0 {
            cv.put(sg.0 as i32, sg.1 as i32 - r - 1, tint([240, 200, 90]));
        }
        cv.put(sg.0 as i32, sg.1 as i32 + r - 1, tint([230, 170, 80]));
        // Wings on the third segment.
        if k == 3 {
            let flap = (m.t * 6.0).sin();
            for i in 0..10 {
                let span = 10 - i;
                let (wx, wy) = (sg.0 as i32 - (m.facing * i as f32 * 0.6) as i32, sg.1 as i32 - (span as f32 * flap) as i32 - 2);
                for j in 0..(span / 2).max(1) {
                    cv.put(wx, wy + j, tint([140, 30, 36]));
                }
            }
        }
    }
    let (cx, cy) = (m.x + m.w * 0.5, m.y + m.h * 0.5);
    disc(cv, cx, cy, 4, tint([190, 52, 44]));
    let f = m.facing as i32;
    for k in 0..4 {
        cv.put(cx as i32 + f * (4 + k), cy as i32 + 1, tint([190, 52, 44]));
        cv.put(cx as i32 + f * (4 + k), cy as i32 + 2, tint([230, 170, 80]));
    }
    cv.put(cx as i32 + f * 2, cy as i32 - 1, [255, 230, 60]);
    cv.put(cx as i32 - f, cy as i32 - 5, [240, 230, 200]);
    cv.put(cx as i32 - f * 2, cy as i32 - 6, [240, 230, 200]);
}
