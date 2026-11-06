//! Game state and the per-frame update: world, wizard, wands, creatures, weather.

use crate::audio::Sfx;
use crate::cell::Cell;
use crate::fx::{Fx, MAGIC};
use crate::materials::Material as M;
use crate::mobs::Mobs;
use crate::player::{Input, Player};
use crate::spells::{Card, Mods, Shot, Spells, SPELL_BAR};
use crate::weather::Weather;
use crate::world::World;

pub const STEP: f32 = 1.0 / 60.0;

/// Materials available to the sandbox brush, in palette order.
pub const PALETTE: [M; 18] = [
    M::Sand,
    M::Water,
    M::Oil,
    M::Lava,
    M::Acid,
    M::Gunpowder,
    M::Seed,
    M::Fire,
    M::Gas,
    M::Snow,
    M::Dirt,
    M::Wood,
    M::Stone,
    M::Brick,
    M::Glass,
    M::Crystal,
    M::Cloud,
    M::Empty,
];


#[derive(Default)]
pub struct Controls {
    pub input: Input,
    pub aim: (f32, f32),
    pub primary: bool,
    pub secondary: bool,
    pub paint: bool,
    pub interact: bool,
}

pub struct Toast {
    pub text: String,
    pub t: f32,
    pub color: [u8; 3],
}

pub struct Game {
    pub world: World,
    pub player: Player,
    pub fx: Fx,
    pub spells: Spells,
    pub mobs: Mobs,
    pub weather: Weather,
    pub seed: u64,
    pub cam: (f32, f32),
    pub aim: (f32, f32),
    pub zoom: f32,
    /// Selected hotbar slot in `SPELL_BAR`.
    pub spell: usize,
    pub sandbox: bool,
    pub brush: usize,
    pub brush_r: i32,
    pub sounds: Vec<Sfx>,
    pub sim_ms: f32,
    pub toasts: Vec<Toast>,
    pub prompt: Option<String>,
    pub fizzle: f32,
    spawn: (f32, f32),
    acc: f32,
    cooldown: f32,
    hitstop: f32,
    booms: f32,
    pub time: f32,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let gen = crate::worldgen::Generator::new(seed);
        let spawn = gen.spawn();
        let world = World::new(seed, spawn.0, spawn.1);
        let player = Player::new(spawn.0, spawn.1);
        let cam = player.center();
        Self {
            world,
            player,
            fx: Fx::new(seed),
            spells: Spells::new(),
            mobs: Mobs::new(seed),
            weather: Weather::new(seed),
            seed,
            cam,
            aim: cam,
            zoom: 1.0,
            spell: 0,
            sandbox: false,
            brush: 0,
            brush_r: 4,
            sounds: Vec::new(),
            sim_ms: 0.0,
            toasts: Vec::new(),
            prompt: None,
            fizzle: 0.0,
            spawn,
            acc: 0.0,
            cooldown: 0.0,
            hitstop: 0.0,
            booms: 0.0,
            time: 0.0,
        }
    }

    pub fn current_spell(&self) -> Card {
        SPELL_BAR[self.spell]
    }

    pub fn cast_ready(&self) -> bool {
        self.cooldown <= 0.0 && self.player.mana >= self.current_spell().mana()
    }

    pub fn toast(&mut self, text: impl Into<String>, color: [u8; 3]) {
        self.toasts.push(Toast { text: text.into(), t: 0.0, color });
    }

    pub fn respawn(&mut self) {
        let gold = self.player.gold;
        self.player = Player::new(self.spawn.0, self.spawn.1);
        self.player.gold = gold;
        self.spells.clear();
        self.mobs.clear();
        self.world.recenter(self.spawn.0, self.spawn.1);
        self.cam = self.player.center();
    }

    /// Find a chest or altar cell near the wizard.
    fn nearby(&self, m: M) -> Option<(i32, i32)> {
        let (px, py) = self.player.center();
        let (px, py) = (px as i32, py as i32);
        for r in 0..7 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if self.world.mat(px + dx, py + dy) == m {
                        return Some((px + dx, py + dy));
                    }
                }
            }
        }
        None
    }

    fn interact(&mut self, pressed: bool) {
        self.prompt = None;
        if let Some((cx, cy)) = self.nearby(M::Chest) {
            self.prompt = Some("E  OPEN CHEST".into());
            if pressed {
                for dy in -5..=5 {
                    for dx in -5..=5 {
                        if self.world.mat(cx + dx, cy + dy) == M::Chest {
                            self.world.set(cx + dx, cy + dy, Cell::EMPTY);
                        }
                    }
                }
                let depth = cy - self.world.surface_at(cx);
                let gold = 5 + (depth.unsigned_abs() / 15);
                self.player.gold += gold;
                self.toast(format!("+{} GOLD", gold), [255, 214, 120]);
                for _ in 0..(gold.min(40)) {
                    let (vx, vy) = (self.fx.rng.rangef(-60.0, 60.0), self.fx.rng.rangef(-120.0, -40.0));
                    let sh = self.fx.rng.next_u32() as u8;
                    self.fx.debris(cx as f32, cy as f32 - 2.0, vx, vy, M::Gold, sh);
                }
                self.fx.burst(cx as f32, cy as f32, 40, 90.0, [255, 210, 90]);
                self.sounds.push(Sfx::Chime);
            }
        } else if let Some((ax, ay)) = self.nearby(M::Altar) {
            self.prompt = Some("E  ALTAR: RESTORE AND SET CHECKPOINT".into());
            if pressed {
                self.spawn = (ax as f32 - 1.0, ay as f32 - 8.0);
                self.player.hp = crate::player::MAX_HP;
                self.player.mana = crate::player::MAX_MANA;
                self.player.flight = crate::player::MAX_FLIGHT;
                self.toast("RESTORED   CHECKPOINT SET", [255, 214, 120]);
                self.fx.burst(ax as f32, ay as f32, 50, 100.0, MAGIC);
                self.sounds.push(Sfx::Chime);
            }
        }
    }

    /// Advance one frame. `idle` runs the world without the player acting (menus).
    pub fn update(&mut self, dt: f32, ctl: &Controls, idle: bool) {
        self.time += dt;
        self.aim = ctl.aim;
        let (ax, ay) = ctl.aim;
        let alive = !self.player.dead();
        self.fizzle = (self.fizzle - dt).max(0.0);

        if !idle && alive {
            self.cooldown -= dt;
            if self.sandbox {
                if ctl.primary || ctl.paint {
                    paint(&mut self.world, ax as i32, ay as i32, self.brush_r, PALETTE[self.brush]);
                }
                if ctl.secondary {
                    paint(&mut self.world, ax as i32, ay as i32, self.brush_r, M::Empty);
                }
            } else {
                if ctl.primary && self.cooldown <= 0.0 {
                    let tip = self.player.staff_tip(ax, ay);
                    let ang = (ay - tip.1).atan2(ax - tip.0);
                    let card = self.current_spell();
                    if self.player.mana >= card.mana() {
                        self.player.mana -= card.mana();
                        self.cooldown = card.delay().max(0.03);
                        let shot = Shot { card, mods: Mods::default(), payload: None };
                        self.player.gold += self.spells.fire(&shot, tip, ang, &mut self.world, &mut self.fx, &mut self.mobs);
                    } else {
                        self.cooldown = 0.25;
                        self.fizzle = 0.3;
                        self.sounds.push(Sfx::Tick);
                    }
                }
                if ctl.secondary {
                    let (px, py) = self.player.center();
                    if (ax - px).powi(2) + (ay - py).powi(2) < 50.0 * 50.0 {
                        let g = self.fx.dissolve(&mut self.world, ax, ay, 3, 0.5);
                        self.player.gold += g;
                        self.sounds.push(Sfx::Dig);
                    }
                }
            }
            self.interact(ctl.interact);
        }

        let mut input = if idle || !alive { Input::default() } else { ctl.input };
        input.aim = ctl.aim;
        let was_dashing = self.player.dashing();
        self.player.update(&self.world, &input, dt);
        if self.player.dashing() {
            if !was_dashing {
                self.sounds.push(Sfx::Dash);
            }
            let (cx, cy) = self.player.center();
            let y = cy + self.fx.rng.rangef(-3.0, 3.0);
            self.fx.spark(cx, y, 0.0, 0.0, 0.25, MAGIC);
        }
        if self.player.hurt_now > 0.5 {
            self.sounds.push(Sfx::Hurt);
            let (cx, _) = self.player.center();
            let text = format!("-{}", self.player.hurt_now.round().max(1.0) as i32);
            self.mobs.popups.push(crate::mobs::Popup { x: cx, y: self.player.y - 3.0, text, color: [255, 110, 100], t: 0.0 });
        }
        if self.player.burning > 0.0 && self.fx.rng.chance(0.5) {
            let (cx, cy) = self.player.center();
            let (vx, vy) = (self.fx.rng.rangef(-10.0, 10.0), self.fx.rng.rangef(-40.0, -10.0));
            let x = cx + self.fx.rng.rangef(-1.5, 1.5);
            self.fx.spark(x, cy, vx, vy, 0.4, [255, 140, 40]);
        }

        // Fixed-rate simulation (paused briefly on big hits for weight).
        let t0 = std::time::Instant::now();
        if self.hitstop > 0.0 {
            self.hitstop -= dt;
        } else {
            self.acc += dt;
            let mut steps = 0;
            while self.acc >= STEP && steps < 3 {
                self.world.step();
                self.spells.step(&mut self.world, &mut self.fx, &mut self.mobs, STEP);
                self.fx.step(&mut self.world, STEP);
                let n = self.world.explosions.len().min(4);
                for (x, y, r) in self.world.explosions.drain(..n).collect::<Vec<_>>() {
                    self.fx.explode(&mut self.world, x as f32 + 0.5, y as f32 + 0.5, r, 0.6);
                }
                self.acc -= STEP;
                steps += 1;
            }
            if steps == 3 {
                self.acc = 0.0;
            }
            if steps > 0 {
                self.sim_ms = self.sim_ms * 0.9 + t0.elapsed().as_secs_f32() * 1000.0 / steps as f32 * 0.1;
            }
            let (pcx, pcy) = self.player.center();
            if !idle {
                self.mobs.spawn_tick(&self.world, pcx, pcy, self.weather.night(), dt);
            }
            self.mobs.update(&mut self.world, &mut self.player, &mut self.fx, dt);
        }

        for (x, y, r) in self.fx.explosions.drain(..).collect::<Vec<_>>() {
            if !idle {
                self.player.blast(x, y, r);
            }
            self.mobs.damage_area(x, y, r as f32 * 1.8, r as f32 * 3.0, &mut self.fx);
            self.sounds.push(if r <= 5 { Sfx::Pop } else { Sfx::Boom });
            self.booms += 1.0;
            if r >= 8 {
                self.hitstop = self.hitstop.max(0.05);
            }
        }
        if self.mobs.big_hits > 0 {
            self.hitstop = self.hitstop.max(0.08);
            self.mobs.big_hits = 0;
        }
        self.player.gold += std::mem::take(&mut self.mobs.gold);
        self.sounds.append(&mut self.mobs.sounds);
        self.sounds.append(&mut self.spells.sounds);
        if let Some((tx, ty)) = self.spells.teleport.take() {
            self.player.teleport(tx, ty, &self.world);
        }

        let pc = self.player.center();
        self.weather.update(dt, &mut self.world, &mut self.fx, pc, &mut self.sounds);

        // Zoom out a little during big chain reactions.
        self.booms *= (1.0 - 1.5 * dt).max(0.0);
        let target = if self.booms > 4.0 { 0.75 } else { 1.0 };
        self.zoom += (target - self.zoom) * (dt * 2.0).min(1.0);

        for t in self.toasts.iter_mut() {
            t.t += dt;
        }
        self.toasts.retain(|t| t.t < 3.0);

        if !idle {
            let (px, py) = self.player.center();
            let target = (px + (ax - px) * 0.15, py + (ay - py) * 0.15);
            let k = (dt * 8.0).min(1.0);
            self.cam.0 += (target.0 - self.cam.0) * k;
            self.cam.1 += (target.1 - self.cam.1) * k;
        }
        self.world.recenter(self.cam.0, self.cam.1);
        self.world.prefetch(1);
    }
}

/// Paint a disc of material. Solid building materials and the eraser overwrite;
/// others sprinkle into empty cells; fire also ignites flammables.
pub fn paint(world: &mut World, cx: i32, cy: i32, r: i32, m: M) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let (x, y) = (cx + dx, cy + dy);
            let here = world.mat(x, y);
            match m {
                M::Empty => {
                    if here != M::Empty {
                        world.set(x, y, Cell::EMPTY);
                    }
                }
                M::Stone | M::Dirt | M::Wood | M::Glass | M::Brick | M::Crystal | M::Cloud => world.place(x, y, m),
                M::Fire if here.flammable() => world.ignite_at(x, y),
                _ if here == M::Empty && world.rng.chance(0.5) => world.place(x, y, m),
                _ => {}
            }
        }
    }
}
