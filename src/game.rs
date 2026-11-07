//! Game state and the per-frame update: world, wizard, wands, creatures, weather.

use crate::audio::Sfx;
use crate::cell::Cell;
use crate::fx::{Fx, MAGIC};
use crate::materials::Material as M;
use crate::mobs::Mobs;
use crate::player::{Input, Player};
use crate::spells::{Card, Mods, Shot, Spells, SPELL_BAR};
use crate::weather::Weather;
use crate::planets::Planet;
use crate::world::World;
use crate::worldgen::{Generator, PORTAL_HOME};

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
    pub secondary_pressed: bool,
}

pub struct ChatLine {
    pub text: String,
    pub age: f32,
    pub color: [u8; 3],
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
    pub chat: Vec<ChatLine>,
    pub prompt: Option<String>,
    pub fizzle: f32,
    spawn: (f32, f32),
    acc: f32,
    cooldown: f32,
    hitstop: f32,
    booms: f32,
    in_space: bool,
    pub time: f32,
    pub planet: Planet,
    /// Where the Portal Wand is tuned to.
    pub dest: Planet,
    /// Open portals on this planet: interior rect and destination.
    pub portals: Vec<((i32, i32, i32, i32), Planet)>,
    /// Set when the wizard steps into a portal; the app plays the warp then calls `enter_planet`.
    pub travel: Option<Planet>,
    /// Planet picker requested (right click with the Portal Wand).
    pub picker: bool,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let gen = crate::worldgen::Generator::new(seed);
        let spawn = gen.spawn();
        let world = World::new(seed, spawn.0, spawn.1);
        let player = Player::new(spawn.0, spawn.1);
        let cam = player.center();
        let mut game = Self {
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
            chat: Vec::new(),
            prompt: None,
            fizzle: 0.0,
            spawn,
            acc: 0.0,
            cooldown: 0.0,
            hitstop: 0.0,
            booms: 0.0,
            in_space: false,
            time: 0.0,
            dest: Planet::nearest(0, 0, 2).into_iter().find(|p| !p.is_earth()).unwrap_or_else(Planet::earth),
            planet: Planet::earth(),
            portals: Vec::new(),
            travel: None,
            picker: false,
        };
        game.say("Press T to chat or / for commands. Try /help", [200, 200, 220]);
        game
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
                    self.spells.portal_hue = self.dest.hue;
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
                if self.current_spell() == crate::spells::Card::PortalWand {
                    if ctl.secondary_pressed {
                        self.picker = true;
                    }
                } else if ctl.secondary {
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

        self.hitstop = self.hitstop.max(std::mem::take(&mut self.fx.hitstop));
        self.booms += std::mem::take(&mut self.fx.kick);
        for (x, y, r, kind) in self.fx.explosions.drain(..).collect::<Vec<_>>() {
            if !idle {
                self.player.blast(x, y, r);
            }
            self.mobs.damage_area(x, y, r as f32 * 1.8, r as f32 * 3.0, kind, &mut self.fx);
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
        for (x, y) in std::mem::take(&mut self.spells.portal_hits) {
            let site = self.world.gen.portals_near(x as i32 - 30, x as i32 + 30).into_iter().find(|&(px, base, _)| (px as f32 - x).abs() < 16.0 && (base as f32 - 10.0 - y).abs() < 24.0);
            if let Some((px, base, style)) = site {
                let dest = self.dest.clone();
                self.activate_portal(px, base, style, dest.clone());
                self.toast(format!("PORTAL OPEN  -  {}", dest.name), dest.color());
                self.fx.flash = self.fx.flash.max(0.4);
                self.fx.shake = (self.fx.shake + 6.0).min(14.0);
                self.sounds.push(Sfx::Thunder);
            }
        }
        if self.travel.is_none() && !idle {
            let (cx, cy) = self.player.center();
            let (cx, cy) = (cx as i32, cy as i32);
            if self.world.mat(cx, cy) == M::Portal {
                if let Some((_, dest)) = self.portals.iter().find(|((x0, y0, x1, y1), _)| cx >= *x0 && cx <= *x1 && cy >= *y0 && cy <= *y1) {
                    self.travel = Some(dest.clone());
                    self.sounds.push(Sfx::Thunder);
                    self.sounds.push(Sfx::Zap);
                }
            }
        }
        if let Some((tx, ty)) = self.spells.teleport.take() {
            self.player.teleport(tx, ty, &self.world);
        }

        for h in &self.spells.holes {
            let (cx, cy) = self.player.center();
            let (dx, dy) = (h.x - cx, h.y - cy);
            let d = (dx * dx + dy * dy).sqrt().max(1.0);
            let reach = h.reach() * 2.2;
            if d < reach && !idle {
                let k = (1.0 - d / reach).powi(2) * 520.0 * dt;
                self.player.vx += dx / d * k;
                self.player.vy += dy / d * k;
                if d < h.core() + 2.0 {
                    self.player.hurt(60.0 * dt);
                }
            }
        }
        for t in &self.spells.tornadoes {
            let (cx, cy) = self.player.center();
            let (dx, up) = (t.x - cx, t.y - cy);
            if !idle && dx.abs() < 7.0 + up.max(0.0) * 0.25 && up > -4.0 && up < t.height() {
                self.player.vx += (dx * 6.0 + (self.time * 10.0).sin() * 200.0) * dt;
                self.player.vy = (self.player.vy - 380.0 * dt).max(-140.0);
            }
        }
        // Breaking the atmosphere: low gravity above it, fire on the way through.
        let (_, py) = self.player.center();
        let thin = ((-py - 420.0) / 100.0).clamp(0.0, 1.0);
        self.player.grav = self.planet.gravity * (1.0 - thin * 0.72);
        if !self.planet.weather && self.weather.kind != crate::weather::Sky::Clear {
            self.weather.set(crate::weather::Sky::Clear);
        }
        if !idle {
            let crossing = (-540.0..-430.0).contains(&py) && self.player.vy.abs() > 55.0;
            if crossing {
                let (cx, cy) = self.player.center();
                for _ in 0..5 {
                    let back = -self.player.vy.signum();
                    let (vx, vy) = (self.fx.rng.rangef(-30.0, 30.0), back * self.fx.rng.rangef(60.0, 140.0));
                    let col = if self.fx.rng.chance(0.5) { [255, 200, 120] } else { [255, 120, 50] };
                    let x = cx + self.fx.rng.rangef(-3.0, 3.0);
                    self.fx.spark(x, cy, vx, vy, 0.4, col);
                }
                self.fx.shake = self.fx.shake.max(1.5);
            }
            let up_high = if self.in_space { py < -470.0 } else { py < -490.0 };
            if up_high != self.in_space {
                self.in_space = up_high;
                self.fx.flash = self.fx.flash.max(0.4);
                self.fx.shake = (self.fx.shake + 6.0).min(14.0);
                self.sounds.push(Sfx::Thunder);
                self.sounds.push(Sfx::Whoosh);
                if up_high {
                    self.toast("LEAVING THE ATMOSPHERE", [170, 220, 255]);
                    self.toast("LOW GRAVITY", [200, 200, 220]);
                } else {
                    self.toast("RE-ENTERING THE ATMOSPHERE", [255, 190, 120]);
                }
            }
        }
        let wind = self.weather.wind_now() * (1.0 - thin);
        self.world.wind = wind;
        self.fx.wind = wind;
        let pc = self.player.center();
        self.weather.ambience(&self.world, &mut self.fx, pc);
        self.weather.update(dt, &mut self.world, &mut self.fx, pc, &mut self.sounds);
        for (x, y) in std::mem::take(&mut self.weather.strikes) {
            self.mobs.damage_area(x, y, 12.0, 45.0, crate::mobs::Dmg::Lightning, &mut self.fx);
            let (cx, cy) = self.player.center();
            if !idle && (cx - x).powi(2) + (cy - y).powi(2) < 10.0 * 10.0 {
                self.player.hurt(30.0);
                self.player.vy -= 90.0;
            }
        }

        // Zoom out a little during big chain reactions.
        self.booms *= (1.0 - 1.5 * dt).max(0.0);
        let target = if self.booms > 4.0 { 0.75 } else { 1.0 };
        self.zoom += (target - self.zoom) * (dt * 2.0).min(1.0);

        self.vents(dt);
        for t in self.toasts.iter_mut() {
            t.t += dt;
        }
        for c in self.chat.iter_mut() {
            c.age += dt;
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

impl Game {
    /// Volcanoes and geysers near the wizard erupt on their own schedules.
    fn vents(&mut self, dt: f32) {
        let (px, py) = self.player.center();
        for (volcano, x, top) in self.world.gen.features_near(px as i32 - 260, px as i32 + 260) {
            if (top as f32 - py).abs() > 220.0 {
                continue;
            }
            let phase = crate::noise::hash3(x, 3, 1) % 1000;
            let (period, dur) = if volcano { (45.0, 7.0) } else { (15.0, 3.0) };
            let t = (self.time + phase as f32 * 0.1) % period;
            if t >= dur {
                continue;
            }
            let (fx, fy) = (x as f32 + 0.5, top as f32 - 1.0);
            if !self.world.in_window(x, top) {
                continue;
            }
            if volcano {
                if t < dt {
                    self.sounds.push(Sfx::Thunder);
                    self.fx.shake = (self.fx.shake + 5.0).min(14.0);
                }
                for _ in 0..5 {
                    let (vx, vy) = (self.fx.rng.rangef(-70.0, 70.0), -self.fx.rng.rangef(160.0, 290.0));
                    let sh = self.fx.rng.next_u32() as u8;
                    self.fx.debris(fx, fy, vx, vy, M::Lava, sh);
                }
                let (vx, vy) = (self.fx.rng.rangef(-40.0, 40.0), -self.fx.rng.rangef(100.0, 220.0));
                let life = self.fx.rng.rangef(0.4, 1.0);
                self.fx.spark(fx, fy, vx, vy, life, [255, 160, 50]);
                if self.fx.rng.chance(0.6) {
                    let sm = Cell::gas(M::Smoke, &mut self.world.rng, 120, 220);
                    let sx = x + self.fx.rng.range(-2, 3);
                    if self.world.mat(sx, top - 3) == M::Empty {
                        self.world.set(sx, top - 3, sm);
                    }
                }
            } else {
                for _ in 0..6 {
                    let (vx, vy) = (self.fx.rng.rangef(-18.0, 18.0), -self.fx.rng.rangef(190.0, 300.0));
                    let sh = self.fx.rng.next_u32() as u8;
                    self.fx.debris(fx, fy, vx, vy, M::Water, sh);
                }
                if self.fx.rng.chance(0.5) && self.world.mat(x, top - 3) == M::Empty {
                    let st = Cell::gas(M::Steam, &mut self.world.rng, 40, 90);
                    self.world.set(x, top - 3, st);
                }
            }
        }
    }

    /// Fill a structure's archway with a shimmering portal to `dest`.
    pub fn activate_portal(&mut self, px: i32, base: i32, style: u32, dest: Planet) {
        let rect = Generator::portal_interior(px, base);
        let hue = (dest.hue * 255.0) as u8;
        for y in rect.1..=rect.3 {
            for x in rect.0..=rect.2 {
                if !Generator::portal_fill(x - px, y - base, style) {
                    continue;
                }
                let m = self.world.mat(x, y);
                if m == M::Empty || m == M::Portal || m.kind() == crate::materials::Kind::Gas || m.kind() == crate::materials::Kind::Liquid {
                    let mut c = Cell::new(M::Portal, &mut self.world.rng);
                    c.temp = hue;
                    self.world.set(x, y, c);
                }
            }
        }
        self.portals.retain(|(r, _)| *r != rect);
        self.portals.push((rect, dest));
        let (cx, cy) = (px as f32, base as f32 - 10.0);
        for k in 0..120 {
            let a = self.fx.rng.rangef(0.0, std::f32::consts::TAU);
            let sp = self.fx.rng.rangef(40.0, 200.0);
            let col = crate::planets::hue_rgb(self.spells.portal_hue + (k % 6) as f32 * 0.05, 0.7, 1.0);
            let life = self.fx.rng.rangef(0.5, 1.4);
            self.fx.mote(cx, cy, a.cos() * sp, a.sin() * sp, life, col, 1.5, 40.0, 0.0);
        }
    }

    /// Arrive on another planet, next to its portal, which opens back to where you came from.
    pub fn enter_planet(&mut self, dest: Planet) {
        let prev = std::mem::replace(&mut self.planet, dest.clone());
        let gen = Generator::with_planet(self.seed, dest.clone());
        let (px, base, style) = gen.portals_near(PORTAL_HOME - 1, PORTAL_HOME + 1)[0];
        let spawn = (px as f32 + 15.0, base as f32 - 9.0);
        self.world = World::with_gen(gen, self.seed, spawn.0, spawn.1);
        self.player.x = spawn.0;
        self.player.y = spawn.1;
        self.player.vx = 0.0;
        self.player.vy = 0.0;
        self.player.grapple = None;
        self.spawn = spawn;
        self.portals.clear();
        self.spells.clear();
        self.mobs.clear();
        self.fx.clear();
        self.spells.portal_hue = prev.hue;
        self.activate_portal(px, base, style, prev.clone());
        self.dest = prev;
        self.cam = self.player.center();
        self.in_space = false;
        self.travel = None;
        self.fx.flash = 1.0;
        if !dest.weather {
            self.weather.set(crate::weather::Sky::Clear);
        }
        self.toast(format!("WELCOME TO {}", dest.name), dest.color());
        self.toast(dest.kind.name().to_string(), [200, 200, 220]);
    }

    pub fn say(&mut self, text: impl Into<String>, color: [u8; 3]) {
        self.chat.push(ChatLine { text: text.into(), age: 0.0, color });
        if self.chat.len() > 50 {
            self.chat.remove(0);
        }
    }

    pub fn teleport_to(&mut self, x: f32, y: f32) {
        self.world.recenter(x, y);
        self.fx.dissolve(&mut self.world, x + 1.5, y + 4.0, 6, 1.0);
        self.player.teleport(x, y + 4.0, &self.world);
        self.player.vy = 0.0;
        self.cam = self.player.center();
        self.mobs.clear();
    }

    /// Chat line from the T / slash prompt.
    pub fn command(&mut self, line: &str) {
        const INFO: [u8; 3] = [200, 200, 220];
        const OK: [u8; 3] = [140, 230, 150];
        const ERR: [u8; 3] = [255, 120, 110];
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let Some(cmd) = line.strip_prefix('/') else {
            self.say(format!("<Wizard> {}", line), [235, 230, 220]);
            return;
        };
        self.say(format!("/{}", cmd), [150, 145, 160]);
        let args: Vec<String> = cmd.split_whitespace().map(|a| a.to_lowercase()).collect();
        let a = |i: usize| args.get(i).map(|s| s.as_str()).unwrap_or("");
        let (px, py) = self.player.center();
        match a(0) {
            "help" => {
                for l in [
                    "/tp <x> <y>  or  /tp spawn|volcano|geyser|ruin|island|space|caves|lava|ocean|beach|forest|plains|jungle|desert|tundra",
                    "/time set day|noon|night|midnight|<0-1>     /weather clear|cloudy|rain|storm|fog|windy",
                    "/summon <creature> [count]     /kill [pets]     /heal     /pos     /seed",
                    "/fly  (infinite flight)     /god  (can't be hurt)     /gold <n>     /clear",
                ] {
                    self.say(l, INFO);
                }
            }
            "tp" => {
                let target: Option<(f32, f32, String)> = match (a(1).parse::<f32>(), a(2).parse::<f32>()) {
                    (Ok(x), Ok(y)) => Some((x, y, format!("{} {}", x, y))),
                    _ => self.find_place(a(1), px, py),
                };
                match target {
                    Some((x, y, what)) => {
                        self.teleport_to(x, y);
                        self.say(format!("Teleported to {}", what), OK);
                    }
                    None => self.say("Try /tp volcano, /tp ruin, /tp space, /tp 0 -50 ...", ERR),
                }
            }
            "time" => {
                let t = match a(2) {
                    "day" | "morning" => Some(0.3),
                    "noon" => Some(0.5),
                    "evening" | "dusk" => Some(0.72),
                    "night" => Some(0.85),
                    "midnight" => Some(0.0),
                    v => v.parse::<f32>().ok().map(|v| v.rem_euclid(1.0)),
                };
                match (a(1), t) {
                    ("set", Some(t)) => {
                        self.weather.time = t;
                        self.say(format!("Time set to {:.2}", t), OK);
                    }
                    _ => {
                        let t = self.weather.time;
                        self.say(format!("Time is {:.2} (0 midnight, 0.5 noon). Use /time set night", t), INFO);
                    }
                }
            }
            "weather" => {
                use crate::weather::Sky;
                let kind = match a(1) {
                    "clear" | "sun" => Some(Sky::Clear),
                    "cloudy" | "clouds" => Some(Sky::Cloudy),
                    "rain" | "snow" => Some(Sky::Rain),
                    "storm" | "thunder" => Some(Sky::Storm),
                    "fog" => Some(Sky::Fog),
                    "windy" | "wind" | "sandstorm" => Some(Sky::Windy),
                    _ => None,
                };
                match kind {
                    Some(k) => {
                        self.weather.set(k);
                        self.say(format!("Weather set to {}", k.name().to_lowercase()), OK);
                    }
                    None => self.say("Use /weather clear|cloudy|rain|storm|fog|windy", ERR),
                }
            }
            "summon" | "spawn" => {
                let name: String = a(1).chars().filter(|c| c.is_alphanumeric()).collect();
                let found = crate::mobs::ALL_MOBS.iter().find(|k| k.name().to_lowercase().replace(' ', "") == name || format!("{:?}", k).to_lowercase() == name);
                match found {
                    Some(k) => {
                        let n = a(2).parse::<usize>().unwrap_or(1).clamp(1, 20);
                        for i in 0..n {
                            self.mobs.spawn(*k, px + 24.0 + i as f32 * 8.0, py - 20.0);
                        }
                        self.say(format!("Summoned {} {}", n, k.name()), OK);
                    }
                    None => {
                        let names: Vec<String> = crate::mobs::ALL_MOBS.iter().map(|k| k.name().to_lowercase().replace(' ', "")).collect();
                        self.say(format!("Creatures: {}", names.join(" ")), ERR);
                    }
                }
            }
            "kill" => {
                let pets = a(1) == "pets";
                let n = self.mobs.kill_all(pets);
                self.say(format!("Killed {} {}", n, if pets { "pets" } else { "creatures" }), OK);
            }
            "heal" => {
                self.player.hp = crate::player::MAX_HP;
                self.player.mana = crate::player::MAX_MANA;
                self.player.burning = 0.0;
                self.say("Healed", OK);
            }
            "fly" => {
                self.player.infinite_flight = !self.player.infinite_flight;
                let on = self.player.infinite_flight;
                self.say(format!("Infinite flight {}", if on { "on" } else { "off" }), OK);
            }
            "god" => {
                self.player.god = !self.player.god;
                let on = self.player.god;
                self.say(format!("God mode {}", if on { "on" } else { "off" }), OK);
            }
            "gold" | "give" => {
                let n = args.iter().find_map(|s| s.parse::<u32>().ok()).unwrap_or(100);
                self.player.gold += n;
                self.say(format!("+{} gold", n), OK);
            }
            "pos" => {
                let depth = py as i32 - self.world.surface_at(px as i32);
                self.say(format!("x {}  y {}  ({} below surface)", px as i32, py as i32, depth), INFO);
            }
            "seed" => {
                let s = self.seed;
                self.say(format!("World seed {}", s), INFO);
            }
            "clear" => self.chat.clear(),
            other => self.say(format!("Unknown command /{}. Type /help", other), ERR),
        }
    }

    /// Where `/tp <name>` goes.
    fn find_place(&self, what: &str, px: f32, py: f32) -> Option<(f32, f32, String)> {
        let gen = &self.world.gen;
        let (pxi, pyi) = (px as i32, py as i32);
        let surf = |x: i32| gen.surface(x) as f32;
        match what {
            "spawn" => {
                let (x, y) = gen.spawn();
                Some((x, y, "spawn".into()))
            }
            "volcano" | "geyser" => {
                let want = what == "volcano";
                let best = gen
                    .features_near(pxi - 20000, pxi + 20000)
                    .into_iter()
                    .filter(|f| f.0 == want)
                    .min_by_key(|f| (f.1 - pxi).abs())?;
                let x = best.1 as f32 + if want { 70.0 } else { 8.0 };
                Some((x, surf(x as i32) - 14.0, format!("the nearest {}", what)))
            }
            "ruin" | "chest" | "altar" => {
                let (gx0, gy0) = (pxi.div_euclid(130), pyi.div_euclid(130));
                let mut best: Option<(i32, i32, i32)> = None;
                for r in 0..12 {
                    for gy in gy0 - r..=gy0 + r {
                        for gx in gx0 - r..=gx0 + r {
                            if let Some((x, y, _, altar)) = gen.ruin_at(gx, gy) {
                                if what == "altar" && !altar {
                                    continue;
                                }
                                let d = (x - pxi).abs() + (y - pyi).abs();
                                if best.map_or(true, |b| d < b.2) {
                                    best = Some((x, y, d));
                                }
                            }
                        }
                    }
                    if best.is_some() {
                        break;
                    }
                }
                let (x, y, _) = best?;
                Some(((x - 4) as f32, (y - 7) as f32, format!("the nearest {}", what)))
            }
            "island" | "isles" | "sky" => {
                let gx0 = pxi.div_euclid(150);
                for r in 0..30 {
                    for gx in [gx0 - r, gx0 + r] {
                        for gy in -6..=-2 {
                            if let Some((x, y, _, _, false, _)) = gen.island_at(gx, gy) {
                                return Some((x as f32, (y - 12) as f32, "a sky island".into()));
                            }
                        }
                    }
                }
                None
            }
            "space" | "astral" | "moon" => {
                let gx0 = pxi.div_euclid(150);
                for r in 0..30 {
                    for gx in [gx0 - r, gx0 + r] {
                        for gy in -12..=-4 {
                            if let Some((x, y, _, _, true, _)) = gen.island_at(gx, gy) {
                                return Some((x as f32, (y - 12) as f32, "an astral island".into()));
                            }
                        }
                    }
                }
                Some((px, -700.0, "space".into()))
            }
            "portal" => {
                let best = gen.portals_near(pxi - 20000, pxi + 20000).into_iter().min_by_key(|p| (p.0 - pxi).abs())?;
                Some((best.0 as f32 + 15.0, best.1 as f32 - 9.0, "the nearest portal".into()))
            }
            "caves" | "cave" => Some((px, surf(pxi) + 110.0, "the caves".into())),
            "lava" | "deep" | "underworld" => Some((px, surf(pxi) + 300.0, "the lava caverns".into())),
            "beach" => {
                for k in 0..800 {
                    for x in [pxi + k * 25, pxi - k * 25] {
                        let o = gen.ocean(x);
                        if o > 0.05 && o < 0.3 && gen.surface(x) < crate::worldgen::SEA {
                            return Some((x as f32, surf(x) - 14.0, "the beach".into()));
                        }
                    }
                }
                None
            }
            "forest" | "desert" | "tundra" | "snow" | "ocean" | "plains" | "jungle" => {
                use crate::worldgen::Biome;
                let want = match what {
                    "forest" => Biome::Forest,
                    "desert" => Biome::Desert,
                    "ocean" => Biome::Ocean,
                    "plains" => Biome::Plains,
                    "jungle" => Biome::Jungle,
                    _ => Biome::Snow,
                };
                if want == Biome::Ocean {
                    for k in 0..800 {
                        for x in [pxi + k * 50, pxi - k * 50] {
                            if gen.ocean(x) > 0.9 {
                                return Some((x as f32, crate::worldgen::SEA as f32 - 16.0, "the ocean".into()));
                            }
                        }
                    }
                    return None;
                }
                for k in 0..400 {
                    for x in [pxi + k * 50, pxi - k * 50] {
                        if gen.biome(x) == want && gen.biome(x + 40) == want {
                            return Some((x as f32, surf(x) - 14.0, format!("the {}", what)));
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}

const COMMANDS: [&str; 13] = ["help", "tp", "time", "weather", "summon", "kill", "heal", "fly", "god", "gold", "pos", "seed", "clear"];
const PLACES: [&str; 18] = ["portal", "volcano", "geyser", "ruin", "altar", "island", "space", "caves", "lava", "ocean", "beach", "forest", "plains", "jungle", "desert", "tundra", "spawn", "~"];

/// Minecraft-style suggestions for a partly typed command: where the current word
/// starts in `buf`, and the options that match it.
pub fn suggest(buf: &str) -> (usize, Vec<String>) {
    let Some(body) = buf.strip_prefix('/') else { return (0, Vec::new()) };
    let parts: Vec<&str> = body.split(' ').collect();
    let idx = parts.len() - 1;
    let cur = parts[idx].to_lowercase();
    let start = buf.len() - parts[idx].len();
    let mobs: Vec<String> = crate::mobs::ALL_MOBS.iter().map(|k| k.name().to_lowercase().replace(' ', "")).collect();
    let opts: Vec<String> = match (idx, parts[0]) {
        (0, _) => COMMANDS.iter().map(|s| s.to_string()).collect(),
        (1, "tp") => PLACES.iter().filter(|p| **p != "~").map(|s| s.to_string()).collect(),
        (1, "time") => vec!["set".into()],
        (2, "time") if parts[1] == "set" => ["day", "noon", "evening", "night", "midnight"].iter().map(|s| s.to_string()).collect(),
        (1, "weather") => ["clear", "cloudy", "rain", "storm", "fog", "windy"].iter().map(|s| s.to_string()).collect(),
        (1, "summon") | (1, "spawn") => mobs,
        (2, "summon") | (2, "spawn") => ["1", "3", "5", "10"].iter().map(|s| s.to_string()).collect(),
        (1, "kill") => vec!["pets".into()],
        (1, "gold") | (1, "give") => ["100", "500", "1000"].iter().map(|s| s.to_string()).collect(),
        _ => Vec::new(),
    };
    (start, opts.into_iter().filter(|o| o.starts_with(&cur)).collect())
}

/// Usage hint for the command being typed.
pub fn usage(buf: &str) -> Option<&'static str> {
    let cmd = buf.strip_prefix('/')?.split(' ').next()?;
    Some(match cmd {
        "tp" => "/tp <place>  or  /tp <x> <y>",
        "time" => "/time set <day|noon|evening|night|midnight|0-1>",
        "weather" => "/weather <clear|cloudy|rain|storm|fog|windy>",
        "summon" | "spawn" => "/summon <creature> [count]",
        "kill" => "/kill [pets]",
        "gold" | "give" => "/gold <amount>",
        "fly" => "/fly   toggle infinite flight",
        "god" => "/god   toggle invulnerability",
        "heal" => "/heal",
        "pos" => "/pos   show your coordinates",
        "seed" => "/seed",
        "clear" => "/clear   clear the chat",
        "help" => "/help",
        _ => return None,
    })
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
