//! Game state: the world, the wizard, spells and effects, plus the per-frame update.

use crate::audio::Sfx;
use crate::cell::Cell;
use crate::fx::Fx;
use crate::materials::Material as M;
use crate::player::{Input, Player};
use crate::spells::{Spell, Spells, SPELLS};
use crate::world::World;

pub const STEP: f32 = 1.0 / 60.0;

/// Materials available to the sandbox brush, in palette order.
pub const PALETTE: [M; 14] = [
    M::Sand,
    M::Water,
    M::Oil,
    M::Lava,
    M::Acid,
    M::Gunpowder,
    M::Seed,
    M::Fire,
    M::Snow,
    M::Dirt,
    M::Wood,
    M::Stone,
    M::Glass,
    M::Empty,
];

/// Per-frame player intent, built from the keyboard/mouse (or a script).
#[derive(Default)]
pub struct Controls {
    pub input: Input,
    /// Aim point in world cells.
    pub aim: (f32, f32),
    pub primary: bool,
    pub secondary: bool,
    pub paint: bool,
}

pub struct Game {
    pub world: World,
    pub player: Player,
    pub fx: Fx,
    pub spells: Spells,
    pub seed: u64,
    pub cam: (f32, f32),
    pub aim: (f32, f32),
    pub spell: usize,
    pub sandbox: bool,
    pub brush: usize,
    pub brush_r: i32,
    pub sounds: Vec<Sfx>,
    pub sim_ms: f32,
    spawn: (f32, f32),
    acc: f32,
    cooldown: f32,
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
            seed,
            cam,
            aim: cam,
            spell: 0,
            sandbox: false,
            brush: 0,
            brush_r: 4,
            sounds: Vec::new(),
            sim_ms: 0.0,
            spawn,
            acc: 0.0,
            cooldown: 0.0,
            time: 0.0,
        }
    }

    pub fn current_spell(&self) -> Spell {
        SPELLS[self.spell]
    }

    pub fn cast_ready(&self) -> bool {
        let s = self.current_spell();
        self.cooldown <= 0.0 && self.player.mana >= if s.continuous() { 1.0 } else { s.cost() }
    }

    pub fn respawn(&mut self) {
        let gold = self.player.gold;
        self.player = Player::new(self.spawn.0, self.spawn.1);
        self.player.gold = gold;
        self.spells.clear();
        self.world.recenter(self.spawn.0, self.spawn.1);
        self.cam = self.player.center();
    }

    /// Advance one frame. `idle` runs the world without the player acting (menus).
    pub fn update(&mut self, dt: f32, ctl: &Controls, idle: bool) {
        self.time += dt;
        self.aim = ctl.aim;
        let (ax, ay) = ctl.aim;
        let alive = !self.player.dead();

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
                let spell = self.current_spell();
                if ctl.primary && self.cooldown <= 0.0 {
                    let tip = self.player.staff_tip(ax, ay);
                    if spell.continuous() {
                        let cost = spell.cost() * dt;
                        if self.player.mana >= cost {
                            self.player.mana -= cost;
                            self.player.gold += self.spells.cast(spell, tip, (ax, ay), &mut self.world, &mut self.fx);
                        }
                    } else if self.player.mana >= spell.cost() {
                        self.player.mana -= spell.cost();
                        self.cooldown = spell.cooldown();
                        self.spells.cast(spell, tip, (ax, ay), &mut self.world, &mut self.fx);
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
        }

        let input = if idle || !alive { Input::default() } else { ctl.input };
        self.player.update(&self.world, &input, dt);
        if self.player.hurt_now > 0.5 {
            self.sounds.push(Sfx::Hurt);
        }
        if self.player.burning > 0.0 && self.fx.rng.chance(0.5) {
            let (cx, cy) = self.player.center();
            let (vx, vy) = (self.fx.rng.rangef(-10.0, 10.0), self.fx.rng.rangef(-40.0, -10.0));
            let x = cx + self.fx.rng.rangef(-1.5, 1.5);
            self.fx.spark(x, cy, vx, vy, 0.4, [255, 140, 40]);
        }

        // Fixed-rate simulation.
        let t0 = std::time::Instant::now();
        self.acc += dt;
        let mut steps = 0;
        while self.acc >= STEP && steps < 3 {
            self.world.step();
            self.spells.step(&mut self.world, &mut self.fx, STEP);
            self.fx.step(&mut self.world, STEP);
            // Chain reactions from gunpowder, a few per step to keep frames smooth.
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

        for (x, y, r) in self.fx.explosions.drain(..) {
            if !idle {
                self.player.blast(x, y, r);
            }
            self.sounds.push(if r <= 5 { Sfx::Pop } else { Sfx::Boom });
        }
        self.sounds.append(&mut self.spells.sounds);
        if let Some((tx, ty)) = self.spells.teleport.take() {
            self.player.teleport(tx, ty, &self.world);
        }

        // Camera follows the wizard (with a little look-ahead toward the cursor).
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

/// Paint a disc of material. Stone/eraser overwrite; others sprinkle into empty cells;
/// fire also ignites flammables.
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
                M::Stone | M::Dirt | M::Wood | M::Glass => world.place(x, y, m),
                M::Fire if here.flammable() => world.ignite_at(x, y),
                _ if here == M::Empty && world.rng.chance(0.5) => world.place(x, y, m),
                _ => {}
            }
        }
    }
}
