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
    pub down: bool,
    pub dash: bool,
    pub grapple: bool,
    /// Hop on or off the broom.
    pub broom: bool,
    /// Aim point, for the grappling vine.
    pub aim: (f32, f32),
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
    dash_t: f32,
    dash_cd: f32,
    pub grapple: Option<(f32, f32)>,
    pub god: bool,
    /// Gravity multiplier (low in space).
    pub grav: f32,
    pub infinite_flight: bool,
    /// Wall the wizard is sliding on (-1 left, 1 right, 0 none).
    wall: f32,
    /// Riding the broom: fast free flight in any direction.
    pub broom: bool,
    /// Broom tilt, following speed, for drawing.
    tilt: f32,
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
            dash_t: 0.0,
            dash_cd: 0.0,
            grapple: None,
            god: false,
            grav: 1.0,
            infinite_flight: false,
            wall: 0.0,
            broom: false,
            tilt: 0.0,
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
        if amount <= 0.0 || self.dead() || self.dash_t > 0.0 || self.god {
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
        let (mut fire, mut lava, mut acid, mut water, mut sink) = (false, false, false, false, false);
        let (x0, y0) = (self.x.floor() as i32, self.y.floor() as i32);
        for cy in y0..y0 + PH as i32 {
            for cx in x0..x0 + PW as i32 {
                match world.mat(cx, cy) {
                    M::Fire => fire = true,
                    M::Lava => lava = true,
                    M::Acid => acid = true,
                    M::Water => water = true,
                    M::Quicksand => sink = true,
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
        self.dash_t = (self.dash_t - dt).max(0.0);
        self.dash_cd = (self.dash_cd - dt).max(0.0);
        if input.dash && self.dash_cd <= 0.0 {
            let d = if dir != 0.0 { dir } else { self.facing };
            self.vx = d * if self.broom { 340.0 } else { 230.0 };
            if !self.broom {
                self.vy = self.vy.min(0.0) * 0.2;
            }
            self.dash_t = 0.16;
            self.dash_cd = 0.55;
            self.grapple = None;
        }
        if input.grapple {
            self.grapple = if self.grapple.is_some() { None } else { self.cast_vine(world, input.aim) };
        }
        if input.broom {
            self.broom = !self.broom;
            self.grapple = None;
            if self.broom {
                self.vy = self.vy.min(-40.0);
            }
        }
        if self.broom {
            self.ride(input, dir, dt);
        } else {
        let accel = if self.on_ground { 700.0 } else { 380.0 };
        if self.dash_t <= 0.0 {
            self.vx += (dir * WALK - self.vx).clamp(-accel * dt, accel * dt);
        }

        let (cx, cy) = ((self.x + PW * 0.5) as i32, (self.y + PH * 0.6) as i32);
        let swimming = world.mat(cx, cy).kind() == Kind::Liquid;
        if self.dash_t <= 0.0 {
            self.vy += GRAVITY * self.grav * dt * if swimming { 0.35 } else { 1.0 };
        }
        if let Some((ax, ay)) = self.grapple {
            let (cx, cy) = self.center();
            let (gx, gy) = (ax - cx, ay - cy);
            let d = (gx * gx + gy * gy).sqrt();
            if d < 4.0 || input.up_pressed {
                self.grapple = None;
                if input.up_pressed {
                    self.vy = -JUMP * 0.8;
                }
            } else {
                self.vx += gx / d * 700.0 * dt;
                self.vy += gy / d * 700.0 * dt - GRAVITY * dt * 0.8;
                self.vx *= 1.0 - 2.0 * dt;
                self.vy *= 1.0 - 2.0 * dt;
            }
        }
        if sink {
            self.vx *= 1.0 - 8.0 * dt;
            self.vy = self.vy.min(8.0);
        }
        // Wall slide and wall jump.
        self.wall = 0.0;
        if !self.on_ground && dir != 0.0 && Self::overlaps(world, self.x + dir * 0.6, self.y) {
            self.wall = dir;
            self.vy = self.vy.min(28.0);
        }
        // Glide: hold down to drift gently.
        if input.down && !self.on_ground && self.vy > 30.0 {
            self.vy = 30.0;
        }
        if swimming {
            self.vx *= 1.0 - 3.0 * dt;
            self.vy *= 1.0 - 3.0 * dt;
            if input.up {
                self.vy -= 420.0 * dt;
            }
        }
        if input.up_pressed && (self.on_ground || sink) {
            self.vy = if sink { -JUMP * 0.5 } else { -JUMP };
        } else if input.up_pressed && self.wall != 0.0 {
            self.vy = -JUMP * 0.95;
            self.vx = -self.wall * 95.0;
            self.flight = (self.flight + 10.0).min(MAX_FLIGHT);
        } else if input.up && !self.on_ground && !swimming && self.flight > 0.0 && self.vy > -75.0 {
            self.vy -= 580.0 * dt;
            self.flight -= 38.0 * dt;
        }
        if self.on_ground {
            self.flight = (self.flight + 80.0 * dt).min(MAX_FLIGHT);
        } else if !input.up {
            // Recharges slowly while gliding or falling, so you can keep climbing.
            self.flight = (self.flight + 22.0 * dt).min(MAX_FLIGHT);
        }
        }
        self.mana = (self.mana + 22.0 * dt).min(MAX_MANA);
        if !self.dead() {
            self.hp = (self.hp + 1.5 * dt).min(MAX_HP);
        }
        if self.infinite_flight {
            self.flight = MAX_FLIGHT;
        }
        if !self.broom {
            self.vy = self.vy.min(190.0 * self.grav.max(0.4));
        }
        if self.dash_t <= 0.0 && self.grapple.is_none() && !self.broom {
            self.vx = self.vx.clamp(-120.0, 120.0);
        }

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

        let total = self.vy * dt;
        let n = total.abs().ceil().max(1.0) as i32;
        let step = total / n as f32;
        for _ in 0..n {
            let ny = self.y + step;
            if !Self::overlaps(world, self.x, ny) {
                self.y = ny;
            } else {
                if step > 0.0 && !Self::overlaps(world, self.x, ny.floor()) {
                    self.y = ny.floor();
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

    /// Broom flight: steer in any direction, no gravity to fight, a gentle hover when
    /// you let go. It's three times faster than walking; SHIFT gives a burst.
    fn ride(&mut self, input: &Input, dir: f32, dt: f32) {
        const SPEED: f32 = 175.0;
        let vert = (input.down as i32 - input.up as i32) as f32;
        let hover = if vert == 0.0 { (self.walk_t * 3.0).sin() * 5.0 } else { 0.0 };
        let (tx, ty) = (dir * SPEED, vert * SPEED * 0.8 + hover);
        let k = (dt * if dir != 0.0 || vert != 0.0 { 3.5 } else { 2.0 }).min(1.0);
        if self.dash_t <= 0.0 {
            self.vx += (tx - self.vx) * k;
            self.vy += (ty - self.vy) * k;
        }
        self.tilt += ((self.vx / SPEED) * 0.5 - self.tilt) * (dt * 6.0).min(1.0);
        self.walk_t += dt;
        self.flight = MAX_FLIGHT;
        self.wall = 0.0;
    }

    fn cast_vine(&self, world: &World, aim: (f32, f32)) -> Option<(f32, f32)> {
        let (hx, hy) = self.hand();
        let (dx, dy) = (aim.0 - hx, aim.1 - hy);
        let l = (dx * dx + dy * dy).sqrt().max(0.001);
        for k in 0..160 {
            let t = k as f32 * 0.5;
            let (x, y) = (hx + dx / l * t, hy + dy / l * t);
            if world.mat(x.floor() as i32, y.floor() as i32).solid() {
                return Some((x, y));
            }
        }
        None
    }

    pub fn dashing(&self) -> bool {
        self.dash_t > 0.0
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

    /// The broom: a wooden handle under the wizard, a straw bristle tail behind, and a
    /// shimmer of sparks streaming off it at speed.
    fn draw_broom(&self, cv: &mut Canvas) {
        let back = -self.facing;
        let (cx, y) = (self.x + PW * 0.5, self.y + PH - 1.0);
        for k in -5..=6 {
            let t = k as f32;
            let px = cx - back * t;
            let py = y + t * self.tilt * back * 0.25;
            cv.put(px.round() as i32, py.round() as i32, if k % 3 == 0 { [120, 78, 40] } else { [150, 100, 52] });
        }
        for k in 0..5 {
            let px = cx + back * (6.0 + k as f32);
            let py = y - self.tilt * back * 1.5;
            let spread = (k as f32 * 0.5).ceil() as i32;
            for s in -spread..=spread {
                let col = if (k + s) % 2 == 0 { [214, 176, 92] } else { [186, 146, 70] };
                cv.put(px.round() as i32, py.round() as i32 + s, col);
            }
        }
        if self.vx.abs() > 60.0 {
            let n = (self.vx.abs() / 40.0) as i32;
            for k in 0..n {
                let px = cx + back * (11.0 + k as f32 * 2.0);
                let wob = ((self.walk_t * 30.0 + k as f32 * 1.7).sin() * 1.2).round() as i32;
                cv.blend(px as i32, y as i32 + wob, [255, 236, 160], 0.6 - k as f32 * 0.08);
            }
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
        let bob = if self.broom { 0 } else if (self.walk_t as i32) % 2 == 1 { 1 } else { 0 };
        if self.broom {
            self.draw_broom(cv);
        }
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
        if let Some((gx, gy)) = self.grapple {
            let n = ((gx - hx).abs().max((gy - hy).abs()) as i32).max(1);
            for k in 0..=n {
                let t = k as f32 / n as f32;
                let sway = ((k as f32) * 0.8).sin() * 0.5;
                cv.put((hx + (gx - hx) * t + sway) as i32, (hy + (gy - hy) * t) as i32, if k % 3 == 0 { [90, 170, 60] } else { [60, 130, 50] });
            }
        }
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
