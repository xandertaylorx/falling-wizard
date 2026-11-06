//! The wizard: platformer physics against solid/powder cells, plus a pixel sprite.

use crate::fx::put;
use crate::materials::Kind;
use crate::world::{World, H, W};

/// Collision box in cells.
const PW: f32 = 3.0;
const PH: f32 = 8.0;

const GRAVITY: f32 = 260.0;
const WALK: f32 = 52.0;
const JUMP: f32 = 105.0;
pub const MAX_MANA: f32 = 100.0;

pub struct Input {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub up_pressed: bool,
}

pub struct Player {
    pub x: f32,
    pub y: f32,
    vx: f32,
    vy: f32,
    on_ground: bool,
    pub mana: f32,
    facing: f32,
    walk_t: f32,
}

// Sprite, drawn facing right. h = hat, f = face, e = eye, w = beard, r = robe, b = boots.
const SPRITE: [&[u8; 5]; 10] = [
    b".h...",
    b".hh..",
    b".hhh.",
    b"hhhhh",
    b".ffe.",
    b".www.",
    b"rrwrr",
    b".rrr.",
    b".rrr.",
    b".b.b.",
];

impl Player {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y, vx: 0.0, vy: 0.0, on_ground: false, mana: MAX_MANA, facing: 1.0, walk_t: 0.0 }
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

    fn in_liquid(&self, world: &World) -> bool {
        let (cx, cy) = ((self.x + PW * 0.5) as i32, (self.y + PH * 0.6) as i32);
        world.mat(cx, cy).kind() == Kind::Liquid
    }

    pub fn update(&mut self, world: &World, input: &Input, dt: f32) {
        // If sand buried us, climb out.
        if Self::overlaps(world, self.x, self.y) {
            for k in 1..=4 {
                if !Self::overlaps(world, self.x, self.y - k as f32) {
                    self.y -= k as f32;
                    break;
                }
            }
        }

        let dir = (input.right as i32 - input.left as i32) as f32;
        if dir != 0.0 {
            self.facing = dir;
        }
        let accel = if self.on_ground { 700.0 } else { 350.0 };
        let target = dir * WALK;
        self.vx += (target - self.vx).clamp(-accel * dt, accel * dt);

        let swimming = self.in_liquid(world);
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
        } else if input.up && !self.on_ground && !swimming && self.mana > 0.0 && self.vy > -70.0 {
            // Levitation: hold jump in the air, costs mana.
            self.vy -= 560.0 * dt;
            self.mana -= 40.0 * dt;
        }
        if self.on_ground {
            self.mana = (self.mana + 70.0 * dt).min(MAX_MANA);
        }
        self.vy = self.vy.min(170.0);

        // Horizontal move with 1-cell step-up so we can walk over bumps.
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

        self.x = self.x.clamp(0.0, W as f32 - PW);
        self.y = self.y.clamp(0.0, H as f32 - PH);
        self.on_ground = self.vy >= 0.0 && Self::overlaps(world, self.x, self.y + 1.0);
        if self.on_ground && dir != 0.0 {
            self.walk_t += dt * 10.0;
        }
    }

    /// Fling the wizard away from an explosion.
    pub fn knockback(&mut self, ex: f32, ey: f32) {
        let (cx, cy) = (self.x + PW * 0.5, self.y + PH * 0.5);
        let (dx, dy) = (cx - ex, cy - ey);
        let d = (dx * dx + dy * dy).sqrt();
        if d < 18.0 {
            let k = (1.0 - d / 18.0) * 160.0 / d.max(1.0);
            self.vx += dx * k;
            self.vy += dy * k - 40.0 * (1.0 - d / 18.0);
        }
    }

    fn hand(&self) -> (f32, f32) {
        (self.x + PW * 0.5 + self.facing * 1.5, self.y + 4.5)
    }

    /// Where spells leave the staff, given an aim point.
    pub fn staff_tip(&self, ax: f32, ay: f32) -> (f32, f32) {
        let (hx, hy) = self.hand();
        let (dx, dy) = (ax - hx, ay - hy);
        let l = (dx * dx + dy * dy).sqrt().max(0.001);
        (hx + dx / l * 5.0, hy + dy / l * 5.0)
    }

    pub fn draw(&mut self, px: &mut [u8], ax: f32, ay: f32, ready: bool) {
        if ax < self.x + PW * 0.5 - 1.0 {
            self.facing = -1.0;
        } else if ax > self.x + PW * 0.5 + 1.0 {
            self.facing = 1.0;
        }
        let bob = if (self.walk_t as i32) % 2 == 1 { 1 } else { 0 };
        let ox = self.x.round() as i32 - 1;
        let oy = self.y.round() as i32 - 2 + bob;
        for (row, line) in SPRITE.iter().enumerate() {
            for col in 0..5 {
                let ch = if self.facing > 0.0 { line[col] } else { line[4 - col] };
                let rgb = match ch {
                    b'h' => [92, 70, 196],
                    b'f' => [236, 196, 160],
                    b'e' => [30, 20, 30],
                    b'w' => [230, 230, 236],
                    b'r' => [64, 50, 160],
                    b'b' => [60, 44, 40],
                    _ => continue,
                };
                if row == 9 && bob == 1 {
                    continue;
                }
                put(px, ox + col as i32, oy + row as i32, rgb);
            }
        }
        // Staff pointing at the cursor.
        let (hx, hy) = self.hand();
        let (dx, dy) = (ax - hx, ay - hy);
        let l = (dx * dx + dy * dy).sqrt().max(0.001);
        for s in 0..5 {
            let t = s as f32;
            put(px, (hx + dx / l * t) as i32, (hy + dy / l * t) as i32, [130, 86, 44]);
        }
        let tip = if ready { [160, 230, 255] } else { [70, 110, 140] };
        put(px, (hx + dx / l * 5.0) as i32, (hy + dy / l * 5.0) as i32, tip);
    }
}
