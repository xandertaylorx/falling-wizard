//! The wizard: platformer physics against solid cells, health, mana, flight, and a sprite.

use crate::materials::{Kind, Material as M};
use crate::render::Canvas;
use crate::world::World;

/// Collision box in cells.
pub const PW: f32 = 3.0;
pub const PH: f32 = 8.0;

const GRAVITY: f32 = 260.0;
const WALK: f32 = 54.0;
const JUMP: f32 = 108.0;
pub const MAX_HP: f32 = 100.0;
pub const MAX_MANA: f32 = 100.0;
pub const MAX_FLIGHT: f32 = 100.0;

#[derive(Default, Clone, Copy)]
pub struct Input {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub up_pressed: bool,
}

pub struct Player {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    on_ground: bool,
    pub hp: f32,
    pub mana: f32,
    pub flight: f32,
    pub burning: f32,
    pub gold: u32,
    pub hurt_flash: f32,
    /// Damage taken this frame (for sounds).
    pub hurt_now: f32,
    facing: f32,
    walk_t: f32,
}

// Drawn facing right. h hat, s star, f face, e eye, w beard, r robe, b boots.
const SPRITE: [&[u8; 5]; 10] = [b".h...", b".hh..", b".hsh.", b"hhhhh", b".ffe.", b".www.", b"rrwrr", b".rrr.", b".rrr.", b".b.b."];

impl Player {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            on_ground: false,
            hp: MAX_HP,
            mana: MAX_MANA,
            flight: MAX_FLIGHT,
            burning: 0.0,
            gold: 0,
            hurt_flash: 0.0,
            hurt_now: 0.0,
            facing: 1.0,
            walk_t: 0.0,
        }
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + PW * 0.5, self.y + PH * 0.5)
    }

    pub fn dead(&self) -> bool {
        self.hp <= 0.0
    }

    fn overlaps(world: &World, x: f32, y: f32) -> bool {
        let x0 = x.floor() as i32;
        let x1 = (x + PW - 0.001).floor() as i32;
        let y0 = y.floor() as i32;
        let y1 = (y + PH - 0.001).floor() as i32;
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                if world.mat(cx, cy).solid() {
                    return true;
                }
            }
        }
        false
    }

    pub fn hurt(&mut self, amount: f32) {
        if amount <= 0.0 || self.dead() {
            return;
        }
        self.hp -= amount;
        self.hurt_now += amount;
        self.hurt_flash = 0.15;
    }

    pub fn teleport(&mut self, tx: f32, ty: f32, world: &World) {
        self.x = tx - PW * 0.5;
        self.y = ty - PH + 1.0;
        self.vx = 0.0;
        self.vy = 0.0;
        self.unstick(world);
    }

    fn unstick(&mut self, world: &World) {
        if !Self::overlaps(world, self.x, self.y) {
            return;
        }
        for k in 1..=12 {
            for (dx, dy) in [(0.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (0.0, 1.0)] {
                let (nx, ny) = (self.x + dx * k as f32, self.y + dy * k as f32);
                if !Self::overlaps(world, nx, ny) {
                    self.x = nx;
                    self.y = ny;
                    return;
                }
            }
        }
    }

    pub fn update(&mut self, world: &World, input: &Input, dt: f32) {
        self.hurt_now = 0.0;
        self.hurt_flash = (self.hurt_flash - dt).max(0.0);
        self.unstick(world);

        // What are we touching?
        let (mut fire, mut lava, mut acid, mut water) = (false, false, false, false);
        let (x0, y0) = (self.x.floor() as i32, self.y.floor() as i32);
        for cy in y0..y0 + PH as i32 {
            for cx in x0..x0 + PW as i32 {
                match world.mat(cx, cy) {
                    M::Fire => fire = true,
                    M::Lava => lava = true,
                    M::Acid => acid = true,
                    M::Water => water = true,
                    _ => {}
                }
            }
        }
        if fire {
            self.hurt(18.0 * dt);
            self.burning = self.burning.max(2.0);
        }
        if lava {
            self.hurt(70.0 * dt);
            self.burning = 3.0;
        }
        if acid {
            self.hurt(30.0 * dt);
        }
        if water {
            self.burning = 0.0;
        }
        if self.burning > 0.0 {
            self.burning -= dt;
            self.hurt(7.0 * dt);
        }

        let dir = (input.right as i32 - input.left as i32) as f32;
        if dir != 0.0 {
            self.facing = dir;
        }
        let accel = if self.on_ground { 700.0 } else { 380.0 };
        self.vx += (dir * WALK - self.vx).clamp(-accel * dt, accel * dt);

        let (cx, cy) = ((self.x + PW * 0.5) as i32, (self.y + PH * 0.6) as i32);
        let swimming = world.mat(cx, cy).kind() == Kind::Liquid;
        self.vy += GRAVITY * dt * if swimming { 0.35 } else { 1.0 };
        if swimming {
            self.vx *= 1.0 - 3.0 * dt;
            self.vy *= 1.0 - 3.0 * dt;
            if input.up {
                self.vy -= 420.0 * dt;
            }
        }
        if input.up_pressed && self.on_ground {
            self.vy = -JUMP;
        } else if input.up && !self.on_ground && !swimming && self.flight > 0.0 && self.vy > -75.0 {
            self.vy -= 580.0 * dt;
            self.flight -= 38.0 * dt;
        }
        if self.on_ground {
            self.flight = (self.flight + 80.0 * dt).min(MAX_FLIGHT);
        }
        self.mana = (self.mana + 22.0 * dt).min(MAX_MANA);
        if !self.dead() {
            self.hp = (self.hp + 1.5 * dt).min(MAX_HP);
        }
        self.vy = self.vy.min(190.0);

        let total = self.vx * dt;
        let n = total.abs().ceil().max(1.0) as i32;
        let step = total / n as f32;
        for _ in 0..n {
            let nx = self.x + step;
            if !Self::overlaps(world, nx, self.y) {
                self.x = nx;
            } else if self.on_ground && !Self::overlaps(world, nx, self.y - 1.0) {
                self.x = nx;
                self.y -= 1.0;
            } else {
                self.vx = 0.0;
                break;
            }
        }

        let impact = self.vy;
        let total = self.vy * dt;
        let n = total.abs().ceil().max(1.0) as i32;
        let step = total / n as f32;
        for _ in 0..n {
            let ny = self.y + step;
            if !Self::overlaps(world, self.x, ny) {
                self.y = ny;
            } else {
                if step > 0.0 {
                    if !Self::overlaps(world, self.x, ny.floor()) {
                        self.y = ny.floor();
                    }
                    if impact > 175.0 && !swimming {
                        self.hurt((impact - 175.0) * 0.9);
                    }
                }
                self.vy = 0.0;
                break;
            }
        }

        self.on_ground = self.vy >= 0.0 && Self::overlaps(world, self.x, self.y + 1.0);
        if self.on_ground && dir != 0.0 {
            self.walk_t += dt * 10.0;
        }
    }

    /// Fling the wizard away from an explosion and hurt them a bit (your spells can hurt you).
    pub fn blast(&mut self, ex: f32, ey: f32, r: i32) {
        let (cx, cy) = self.center();
        let (dx, dy) = (cx - ex, cy - ey);
        let d = (dx * dx + dy * dy).sqrt();
        let reach = r as f32 * 2.4 + 6.0;
        if d < reach {
            let k = 1.0 - d / reach;
            let push = k * (60.0 + r as f32 * 10.0) / d.max(1.0);
            self.vx += dx * push;
            self.vy += dy * push - 40.0 * k;
            self.hurt(k * r as f32 * 2.2);
        }
    }

    fn hand(&self) -> (f32, f32) {
        (self.x + PW * 0.5 + self.facing * 1.5, self.y + 4.5)
    }

    pub fn staff_tip(&self, ax: f32, ay: f32) -> (f32, f32) {
        let (hx, hy) = self.hand();
        let (dx, dy) = (ax - hx, ay - hy);
        let l = (dx * dx + dy * dy).sqrt().max(0.001);
        (hx + dx / l * 5.0, hy + dy / l * 5.0)
    }

    pub fn draw(&mut self, cv: &mut Canvas, ax: f32, ay: f32, ready: bool) {
        if self.dead() {
            return;
        }
        let (cx, _) = self.center();
        if ax < cx - 1.0 {
            self.facing = -1.0;
        } else if ax > cx + 1.0 {
            self.facing = 1.0;
        }
        let bob = if (self.walk_t as i32) % 2 == 1 { 1 } else { 0 };
        let ox = self.x.round() as i32 - 1;
        let oy = self.y.round() as i32 - 2 + bob;
        let hurt = self.hurt_flash > 0.0;
        for (row, line) in SPRITE.iter().enumerate() {
            for col in 0..5 {
                let ch = if self.facing > 0.0 { line[col] } else { line[4 - col] };
                if row == 9 && bob == 1 {
                    continue;
                }
                let rgb = match ch {
                    b'h' => [92, 70, 196],
                    b's' => [255, 220, 110],
                    b'f' => [236, 196, 160],
                    b'e' => [30, 20, 30],
                    b'w' => [230, 230, 236],
                    b'r' => [64, 50, 160],
                    b'b' => [60, 44, 40],
                    _ => continue,
                };
                let rgb = if hurt { [255, 90, 80] } else { rgb };
                cv.put(ox + col as i32, oy + row as i32, rgb);
            }
        }
        let (hx, hy) = self.hand();
        let (dx, dy) = (ax - hx, ay - hy);
        let l = (dx * dx + dy * dy).sqrt().max(0.001);
        for s in 0..5 {
            let t = s as f32;
            cv.put((hx + dx / l * t) as i32, (hy + dy / l * t) as i32, [130, 86, 44]);
        }
        let tip = if ready { [170, 235, 255] } else { [70, 110, 140] };
        cv.put((hx + dx / l * 5.0) as i32, (hy + dy / l * 5.0) as i32, tip);
    }
}
