//! Creature art: each creature is drawn from body parts every frame (legs that walk,
//! tails that sway, wings that flap, eyes that look at you), then outlined so it reads
//! clearly against any background. Drawn facing right; the caller mirrors it.

use crate::mobs::MobKind;
use std::f32::consts::PI;

/// What a creature is doing, for its pose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pose {
    Normal,
    /// Low to the ground, creeping up on prey.
    Crouch,
    Sleep,
    /// Mid-pounce, legs stretched out.
    Leap,
    /// Bats hanging from the ceiling.
    Roost,
    /// Buried in sand with just the tail showing.
    Hidden,
    Howl,
    Dead,
}

pub struct Anim {
    pub t: f32,
    /// Walk cycle phase (advances with distance travelled).
    pub walk: f32,
    /// 0 standing .. 1 full speed.
    pub speed: f32,
    pub vy: f32,
    pub ground: bool,
    pub pose: Pose,
    /// 0 when not attacking, else 0..1 through the attack.
    pub attack: f32,
    /// Direction to look, already in facing-right space.
    pub look: (f32, f32),
    pub seed: u32,
    pub w: f32,
    pub h: f32,
}

/// A small RGBA buffer around the creature's hitbox, with a margin for tails and wings.
pub struct Pix {
    pub w: i32,
    pub h: i32,
    pub m: i32,
    pub px: Vec<[u8; 4]>,
}

pub type Rgb = [u8; 3];

pub fn mul(c: Rgb, k: f32) -> Rgb {
    [(c[0] as f32 * k).min(255.0) as u8, (c[1] as f32 * k).min(255.0) as u8, (c[2] as f32 * k).min(255.0) as u8]
}

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [(a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8, (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8, (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8]
}

fn hash(a: u32, b: u32) -> u32 {
    crate::noise::hash3(a as i32, b as i32, 0xC2EA)
}

impl Pix {
    pub fn new(w: f32, h: f32, m: i32) -> Pix {
        let (w, h) = (w.ceil() as i32 + m * 2, h.ceil() as i32 + m * 2);
        Pix { w, h, m, px: vec![[0; 4]; (w * h) as usize] }
    }

    fn idx(&self, x: f32, y: f32) -> Option<usize> {
        let (x, y) = (x.floor() as i32 + self.m, y.floor() as i32 + self.m);
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    pub fn put(&mut self, x: f32, y: f32, c: Rgb) {
        if let Some(i) = self.idx(x, y) {
            self.px[i] = [c[0], c[1], c[2], 255];
        }
    }

    pub fn put_a(&mut self, x: f32, y: f32, c: Rgb, a: f32) {
        if let Some(i) = self.idx(x, y) {
            self.px[i] = [c[0], c[1], c[2], (a.clamp(0.0, 1.0) * 255.0) as u8];
        }
    }

    /// Paint only over pixels already drawn (stripes, spots).
    pub fn over(&mut self, x: f32, y: f32, c: Rgb) {
        if let Some(i) = self.idx(x, y) {
            if self.px[i][3] > 0 {
                self.px[i] = [c[0], c[1], c[2], self.px[i][3]];
            }
        }
    }

    /// Filled ellipse, lit from above.
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgb) {
        self.ellipse_a(cx, cy, rx, ry, c, 1.0);
    }

    pub fn ellipse_a(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgb, a: f32) {
        let (rx, ry) = (rx.max(0.5), ry.max(0.5));
        for y in (cy - ry - 1.0).floor() as i32..=(cy + ry + 1.0).ceil() as i32 {
            for x in (cx - rx - 1.0).floor() as i32..=(cx + rx + 1.0).ceil() as i32 {
                let (u, v) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if u * u + v * v <= 1.0 {
                    let k = 1.14 - (v + 1.0) * 0.16;
                    self.put_a(x as f32, y as f32, mul(c, k), a);
                }
            }
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb) {
        let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 1.5).ceil().max(1.0) as i32;
        for k in 0..=n {
            let t = k as f32 / n as f32;
            self.put(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, c);
        }
    }

    /// A limb: a line `thick` cells wide.
    pub fn limb(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, c: Rgb) {
        let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0).ceil().max(1.0) as i32;
        let r = thick * 0.5;
        for k in 0..=n {
            let t = k as f32 / n as f32;
            let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            if r <= 0.5 {
                self.put(x, y, c);
            } else {
                for dy in [-r * 0.5, r * 0.5] {
                    for dx in [-r * 0.5, r * 0.5] {
                        self.put(x + dx, y + dy, c);
                    }
                }
            }
        }
    }

    pub fn tri(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: Rgb) {
        self.tri_a(a, b, c, col, 1.0);
    }

    pub fn tri_a(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: Rgb, al: f32) {
        let (x0, x1) = (a.0.min(b.0).min(c.0).floor() as i32, a.0.max(b.0).max(c.0).ceil() as i32);
        let (y0, y1) = (a.1.min(b.1).min(c.1).floor() as i32, a.1.max(b.1).max(c.1).ceil() as i32);
        let edge = |p: (f32, f32), q: (f32, f32), r: (f32, f32)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let pt = (x as f32 + 0.5, y as f32 + 0.5);
                let (e0, e1, e2) = (edge(a, b, pt), edge(b, c, pt), edge(c, a, pt));
                if (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0) {
                    self.put_a(x as f32, y as f32, col, al);
                }
            }
        }
    }

    /// Eye: a coloured pixel with a glint, looking towards `look`.
    pub fn eye(&mut self, x: f32, y: f32, iris: Rgb, look: (f32, f32)) {
        self.put(x, y, iris);
        let (lx, ly) = (if look.0 > 0.3 { 1.0 } else if look.0 < -0.3 { -1.0 } else { 0.0 }, if look.1 > 0.5 { 1.0 } else { 0.0 });
        if lx == 0.0 && ly == 0.0 {
            return;
        }
        self.put(x + lx, y + ly, mul(iris, 0.5));
    }

    /// Dark outline around everything solid, so creatures stand out from the world.
    pub fn outline(&mut self) {
        let src = self.px.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let i = (y * self.w + x) as usize;
                if src[i][3] > 0 {
                    continue;
                }
                let mut best: Option<[u8; 4]> = None;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= self.w || ny >= self.h {
                        continue;
                    }
                    let n = src[(ny * self.w + nx) as usize];
                    if n[3] > 200 {
                        best = Some(n);
                        break;
                    }
                }
                if let Some(n) = best {
                    self.px[i] = [n[0] / 5 + 6, n[1] / 5 + 4, n[2] / 5 + 10, 230];
                }
            }
        }
    }
}

struct Quad {
    len: f32,
    body: f32,
    leg: f32,
    head: f32,
    fur: Rgb,
    belly: Rgb,
    eye: Rgb,
    pointed: bool,
    snout: f32,
    /// 0 thin with a tuft, 1 bushy, 2 long and striped.
    tail: u8,
    mane: Option<Rgb>,
    stripes: Option<Rgb>,
    back: Option<Rgb>,
}

/// Four-legged animals: lions, tigers, wolves.
fn quad(p: &mut Pix, a: &Anim, q: &Quad) {
    let (w, h) = (a.w, a.h);
    let moving = a.speed.min(1.0);
    let sleep = a.pose == Pose::Sleep || a.pose == Pose::Dead;
    let crouch = if a.pose == Pose::Crouch { 0.45 } else { 0.0 };
    let leap = a.pose == Pose::Leap || (!a.ground && a.vy.abs() > 30.0);
    let leg = if sleep { 0.8 } else { q.leg * (1.0 - crouch) };
    let bob = (a.walk * 2.0).sin().abs() * 0.5 * moving;
    let breathe = (a.t * if sleep { 1.3 } else { 2.4 }).sin() * 0.3;
    let hip = h - leg - 0.6 - bob;
    let (bx, rx, ry) = (w * 0.45, q.len * 0.5, q.body * 0.5 + breathe * 0.25);
    let by = hip - ry * 0.55;
    let far = mul(q.fur, 0.7);
    let legs = [(bx - rx * 0.62, 0.0, false), (bx + rx * 0.58, PI * 0.5, false), (bx - rx * 0.62, PI, true), (bx + rx * 0.58, PI * 1.5, true)];
    let draw_leg = |p: &mut Pix, (lx, off, near): (f32, f32, bool)| {
        let col = if near { q.fur } else { far };
        let front = lx > bx;
        if sleep {
            let dir = if front { 1.0 } else { -0.4 };
            p.limb(lx, hip, lx + dir * q.leg * 0.7, h - 0.6, 1.4, col);
            return;
        }
        let ph = a.walk + off;
        let ang = if leap { if front { 1.0 } else { -1.1 } } else { ph.sin() * 0.6 * moving };
        let lift = (-(ph.cos())).max(0.0) * 1.3 * moving * (!leap as i32 as f32);
        let knee = (lx + ang.sin() * leg * 0.5 - if front { 0.0 } else { 0.4 }, hip + leg * 0.5 - lift * 0.4);
        let foot = (lx + ang.sin() * leg, hip + ang.cos().abs() * leg - lift);
        p.limb(lx, hip - 0.5, knee.0, knee.1, 1.8, col);
        p.limb(knee.0, knee.1, foot.0, foot.1, 1.4, col);
        p.put(foot.0 + 0.5, foot.1, mul(col, 0.55));
    };
    for l in legs.iter().filter(|l| !l.2) {
        draw_leg(p, *l);
    }
    // Tail.
    let (mut tx, mut ty) = (bx - rx * 0.92, by - ry * 0.35);
    let n = if q.tail == 2 { 8 } else { 6 };
    let mut ang = PI + if sleep { -0.2 } else { 0.55 + moving * 0.3 };
    for i in 0..n {
        ang += (a.t * 3.0 + i as f32 * 0.7).sin() * 0.16 - if q.tail == 0 { 0.12 } else { 0.05 };
        let (nx, ny) = (tx + ang.cos() * 1.3, ty + ang.sin() * 1.3 + if q.tail == 1 { 0.35 } else { 0.0 });
        let thick = if q.tail == 1 { 2.6 - i as f32 * 0.2 } else { 1.2 };
        let c = if q.tail == 2 && i % 2 == 1 { q.stripes.unwrap_or(q.fur) } else { q.fur };
        p.limb(tx, ty, nx, ny, thick, c);
        tx = nx;
        ty = ny;
    }
    if q.tail == 0 {
        p.ellipse(tx, ty, 1.2, 1.2, q.mane.unwrap_or(mul(q.fur, 0.6)));
    }
    // Body, back and belly.
    p.ellipse(bx, by, rx, ry, q.fur);
    if let Some(bk) = q.back {
        p.ellipse(bx - rx * 0.1, by - ry * 0.45, rx * 0.8, ry * 0.45, bk);
    }
    p.ellipse(bx + rx * 0.05, by + ry * 0.45, rx * 0.75, ry * 0.42, q.belly);
    if let Some(st) = q.stripes {
        let mut x = bx - rx + 1.0;
        while x < bx + rx - 1.0 {
            for k in 0..(q.body as i32 + 1) {
                let y = by - ry + k as f32;
                p.over(x + k as f32 * 0.25, y, st);
            }
            x += 2.4;
        }
    }
    for l in legs.iter().filter(|l| l.2) {
        draw_leg(p, *l);
    }
    // Head.
    let lunge = a.attack * 2.2;
    let howl = a.pose == Pose::Howl;
    let hx = bx + rx * 0.95 + lunge;
    let hy = by - ry * 0.8 + (a.walk * 2.0).sin() * 0.3 * moving + if sleep { ry * 1.0 } else { 0.0 } + if a.pose == Pose::Crouch { ry * 0.4 } else { 0.0 } - if howl { 1.2 } else { 0.0 };
    let r = q.head;
    if let Some(mane) = q.mane {
        for k in 0..18 {
            let ang = k as f32 / 18.0 * PI * 2.0;
            let rr = r * 1.65 + ((hash(a.seed, k) % 10) as f32 * 0.08) + (a.t * 2.0 + k as f32).sin() * 0.15;
            p.limb(hx - 0.8, hy + 0.3, hx - 0.8 + ang.cos() * rr, hy + 0.3 + ang.sin() * rr, 1.4, if k % 3 == 0 { mul(mane, 0.8) } else { mane });
        }
        p.ellipse(hx - 0.8, hy + 0.3, r * 1.45, r * 1.45, mane);
    }
    // Ears.
    if q.pointed {
        p.tri((hx - r * 0.7, hy - r * 0.4), (hx - r * 0.1, hy - r * 0.6), (hx - r * 0.55, hy - r * 1.9), q.fur);
    } else {
        p.ellipse(hx - r * 0.5, hy - r * 0.8, 0.9, 0.9, q.fur);
    }
    p.ellipse(hx, hy, r, r * 0.92, q.fur);
    let (sx, sy) = (hx + r * 0.85, hy + r * 0.3 - if howl { 0.8 } else { 0.0 });
    p.ellipse(sx, sy, q.snout, q.snout * 0.65, q.belly);
    p.put(sx + q.snout - 0.5, sy - q.snout * 0.3, [30, 20, 20]);
    if a.attack > 0.0 || howl {
        p.line(sx - q.snout * 0.5, sy + q.snout * 0.6, sx + q.snout, sy + q.snout * 0.8, [160, 30, 40]);
        p.put(sx + q.snout * 0.6, sy + q.snout * 0.3, [250, 250, 240]);
    }
    if let Some(st) = q.stripes {
        p.over(hx - r * 0.2, hy - r * 0.6, st);
        p.over(hx - r * 0.5, hy, st);
    }
    let (ex, ey) = (hx + r * 0.3, hy - r * 0.3);
    if sleep || howl {
        p.put(ex, ey, mul(q.fur, 0.45));
        p.put(ex - 1.0, ey, mul(q.fur, 0.45));
    } else {
        p.eye(ex, ey, q.eye, a.look);
    }
}

/// Two legs walking: returns nothing, draws from the hip down.
fn legs2(p: &mut Pix, a: &Anim, hx: f32, hy: f32, len: f32, thick: f32, c: Rgb, stride: f32) {
    let moving = a.speed.min(1.0);
    for (k, off) in [(0, PI), (1, 0.0)] {
        let ph = a.walk + off;
        let col = if k == 0 { mul(c, 0.72) } else { c };
        let ang = if a.ground { ph.sin() * stride * moving } else { 0.35 * if k == 0 { -1.0 } else { 1.0 } };
        let lift = (-ph.cos()).max(0.0) * 1.0 * moving;
        let fx = hx + ang.sin() * len;
        let fy = hy + ang.cos() * len - lift;
        p.limb(hx + k as f32 * 0.6 - 0.3, hy, fx, fy, thick, col);
        p.put(fx + 0.6, fy - 0.2, mul(col, 0.5));
    }
}

/// Paint one creature. `w`/`h` in `a` is its hitbox; the result has a margin around it.
pub fn paint(kind: MobKind, a: &Anim) -> Pix {
    use MobKind::*;
    let mut p = Pix::new(a.w, a.h, 7);
    let (w, h) = (a.w, a.h);
    let t = a.t;
    let moving = a.speed.min(1.0);
    match kind {
        Lion => {
            let male = a.seed % 4 != 0;
            quad(&mut p, a, &Quad { len: 12.0, body: 5.0, leg: 3.8, head: 2.4, fur: [214, 166, 88], belly: [238, 206, 146], eye: [80, 50, 20], pointed: false, snout: 1.5, tail: 0, mane: male.then_some([150, 82, 34]), stripes: None, back: Some([200, 150, 76]) });
        }
        Tiger => {
            quad(&mut p, a, &Quad { len: 13.0, body: 4.6, leg: 3.4, head: 2.3, fur: [236, 132, 42], belly: [244, 238, 226], eye: [120, 210, 70], pointed: false, snout: 1.4, tail: 2, mane: None, stripes: Some([36, 24, 22]), back: None });
        }
        SnowWolf => {
            quad(&mut p, a, &Quad { len: 10.0, body: 4.0, leg: 4.0, head: 2.0, fur: [222, 228, 238], belly: [244, 246, 252], eye: [90, 170, 230], pointed: true, snout: 1.7, tail: 1, mane: None, stripes: None, back: Some([150, 160, 178]) });
        }
        MoonHopper => {
            // A long-eared moon rabbit: big folded hind legs, squash and stretch when hopping.
            let air = !a.ground;
            let stretch = if air { 1.18 } else { 1.0 + (t * 2.0).sin() * 0.03 };
            let (bx, by) = (w * 0.45, h - 3.2 * stretch);
            let fur = [196, 196, 226];
            let dark = [150, 146, 190];
            p.ellipse(bx - 2.0, h - 1.6, 2.4, 1.4, dark);
            p.ellipse(bx, by, 3.0 / stretch, 2.6 * stretch, fur);
            p.ellipse(bx - 2.9, by - 0.4, 1.0, 1.0, [244, 240, 255]);
            let (hx, hy) = (bx + 2.2, by - 2.4 * stretch);
            let ear = if air { -0.5 } else { (t * 1.7).sin() * 0.15 };
            for (k, c) in [(0.0, mul(fur, 0.8)), (0.9, fur)] {
                p.limb(hx - 0.6 + k * 0.5, hy - 1.0, hx - 1.6 + k + ear * 3.0, hy - 5.0, 1.3, c);
                p.put(hx - 1.2 + k + ear * 2.0, hy - 3.5, [240, 190, 220]);
            }
            p.ellipse(hx, hy, 1.9, 1.7, fur);
            p.eye(hx + 0.7, hy - 0.4, [80, 50, 150], a.look);
            p.put(hx + 1.8, hy + 0.3, [230, 150, 190]);
            p.limb(bx + 1.6, by + 1.6, bx + 2.4 + if air { 1.0 } else { 0.0 }, h - 0.6, 1.0, fur);
        }
        Zombie => {
            // Shambling, arms out, one glowing eye, torn clothes.
            let lurch = (a.walk * 0.5).sin() * 0.6 * moving;
            let skin = mix([112, 160, 96], [140, 150, 90], (a.seed % 5) as f32 / 5.0);
            let shirt = [[70, 90, 140], [120, 60, 60], [90, 110, 70]][(a.seed % 3) as usize];
            let hipy = h - 4.4;
            legs2(&mut p, a, w * 0.45, hipy, 4.0, 1.5, [62, 54, 72], 0.4);
            let (sx, sy) = (w * 0.45 + lurch * 0.5 + 0.6, hipy - 4.2);
            p.limb(w * 0.45, hipy, sx, sy, 3.2, shirt);
            for k in 0..3 {
                p.over(w * 0.45 - 1.0 + k as f32 * 1.2, hipy - (k % 2) as f32, mul(shirt, 0.6));
            }
            let arm = (t * 2.0).sin() * 0.4 + a.attack * 1.5;
            p.limb(sx, sy + 0.6, sx + 4.4, sy + 0.4 + arm * 0.3, 1.1, mul(skin, 0.75));
            p.limb(sx + 0.3, sy + 1.0, sx + 4.6, sy + 1.4 - arm * 0.3, 1.2, skin);
            let (hx, hy) = (sx + 0.8 + lurch, sy - 2.0);
            p.ellipse(hx, hy, 1.9, 2.0, skin);
            p.put(hx + 0.8, hy - 0.4, [255, 60, 50]);
            p.put(hx - 0.3, hy - 0.4, [30, 40, 30]);
            p.line(hx + 0.2, hy + 1.0, hx + 1.4, hy + 1.2, [70, 40, 40]);
            p.put(hx - 0.8, hy - 1.6, [60, 80, 50]);
        }
        Skeleton => {
            let bone = [234, 230, 214];
            let shade = [190, 186, 170];
            let hipy = h - 4.4;
            legs2(&mut p, a, w * 0.42, hipy, 4.2, 1.0, bone, 0.45);
            let (sx, sy) = (w * 0.42, hipy - 4.0);
            p.line(w * 0.42, hipy, sx, sy, shade);
            p.line(sx - 1.0, hipy, sx + 1.0, hipy, bone);
            for k in 0..3 {
                let y = sy + 0.8 + k as f32 * 1.0;
                p.line(sx - 1.4, y, sx + 1.4, y, if k % 2 == 0 { bone } else { shade });
            }
            // Bow arm raised when throwing.
            let raise = if a.attack > 0.0 { -1.6 } else { (t * 1.5).sin() * 0.3 };
            p.line(sx, sy + 0.5, sx + 2.6, sy + 1.6 + raise, bone);
            p.line(sx + 2.6, sy + 1.6 + raise, sx + 3.6, sy + 0.2 + raise, bone);
            p.line(sx, sy + 0.5, sx - 1.6, sy + 3.0, shade);
            let (hx, hy) = (sx + 0.4, sy - 2.0);
            p.ellipse(hx, hy, 1.9, 1.8, bone);
            p.put(hx + 0.7, hy - 0.2, [20, 14, 20]);
            p.put(hx - 0.5, hy - 0.2, [20, 14, 20]);
            if a.attack > 0.0 {
                p.put(hx + 0.7, hy - 0.2, [255, 80, 60]);
            }
            p.line(hx - 0.6, hy + 1.3, hx + 1.2, hy + 1.3, shade);
        }
        Yeti => {
            // Hulking, hunched, knuckle-walking, with a roar.
            let fur = [236, 240, 248];
            let face = [120, 150, 196];
            let hipy = h - 4.0;
            legs2(&mut p, a, w * 0.42, hipy, 3.6, 2.4, fur, 0.35);
            let (bx, by) = (w * 0.45, hipy - 4.4);
            p.ellipse(bx, by, 4.4, 5.0, fur);
            for k in 0..10 {
                let hsh = hash(a.seed, k);
                p.over(bx - 3.5 + (hsh % 7) as f32, by - 4.0 + ((hsh >> 4) % 8) as f32, [212, 218, 232]);
            }
            let swing = (a.walk).sin() * 0.8 * moving;
            p.limb(bx + 2.4, by - 1.6, bx + 4.4 + swing, h - 1.0 - a.attack * 3.0, 2.4, mul(fur, 0.82));
            let (hx, hy) = (bx + 2.6, by - 4.6);
            p.ellipse(hx, hy, 2.4, 2.2, fur);
            p.ellipse(hx + 0.8, hy + 0.3, 1.6, 1.4, face);
            p.put(hx + 0.4, hy - 0.2, [20, 20, 40]);
            p.put(hx + 1.6, hy - 0.2, [20, 20, 40]);
            if a.attack > 0.0 {
                p.ellipse(hx + 1.0, hy + 1.2, 0.9, 0.8, [120, 30, 50]);
            }
            p.limb(bx - 2.4, by - 1.0, bx - 3.4 - swing, h - 1.2, 2.2, mul(fur, 0.7));
        }
        Golem => {
            // Stone blocks, cracks, a molten core and glowing eyes; heavy stomp.
            let stone = [116, 106, 108];
            let dark = [80, 72, 78];
            let core = [255, 150, 40];
            let stomp = (a.walk).sin().abs() * 0.6 * moving;
            let hipy = h - 4.4;
            for (k, off) in [(0, PI), (1, 0.0)] {
                let lift = (-(a.walk + off).cos()).max(0.0) * 1.2 * moving;
                let x = w * 0.5 + if k == 0 { -2.2 } else { 1.4 };
                let c = if k == 0 { dark } else { stone };
                p.ellipse(x, hipy + 2.0 - lift, 1.7, 2.4, c);
            }
            let (bx, by) = (w * 0.5, hipy - 4.4 + stomp);
            p.ellipse(bx, by, 4.6, 4.6, stone);
            p.ellipse(bx + 0.6, by - 0.4, 1.5, 1.5, core);
            p.put(bx + 0.6, by - 0.4, [255, 240, 150]);
            for (x0, y0, x1, y1) in [(-3.0, -2.0, -1.0, 1.0), (2.0, 1.0, 3.5, 3.0), (-2.0, 2.0, 0.0, 3.5)] {
                p.line(bx + x0, by + y0, bx + x1, by + y1, [60, 52, 58]);
            }
            p.over(bx - 3.0, by - 3.5, [90, 140, 70]);
            p.over(bx - 2.0, by - 4.0, [90, 140, 70]);
            let swing = (a.walk).sin() * 0.6 * moving - a.attack * 2.0;
            p.ellipse(bx + 4.4, by + 1.6 + swing, 1.7, 2.6, stone);
            p.ellipse(bx - 4.4, by + 1.6 - swing, 1.6, 2.4, dark);
            let (hx, hy) = (bx + 0.8, by - 5.4 + stomp * 0.5);
            p.ellipse(hx, hy, 2.4, 1.9, stone);
            p.line(hx - 0.2, hy - 0.2, hx + 1.8, hy - 0.2, [255, 200, 80]);
        }
        Mushling => {
            // A little walking mushroom: wobbly cap with spots, stubby feet.
            let cap = [90, 232, 212];
            let stem = [226, 216, 196];
            let wob = (t * 6.0).sin() * 0.3 * moving + (t * 1.5).sin() * 0.15;
            let hipy = h - 1.6;
            legs2(&mut p, a, w * 0.5, hipy, 1.5, 1.2, mul(stem, 0.85), 0.7);
            p.ellipse(w * 0.5, hipy - 1.8, 1.8, 2.0, stem);
            p.put(w * 0.5 + 0.8, hipy - 2.2, [30, 30, 30]);
            p.put(w * 0.5 - 0.2, hipy - 2.2, [30, 30, 30]);
            let (cx, cy) = (w * 0.5 + wob, hipy - 4.2);
            p.ellipse(cx, cy, 3.4, 2.0, cap);
            for k in 0..4 {
                let hsh = hash(a.seed, k);
                p.over(cx - 2.0 + (hsh % 5) as f32, cy - 1.2 + ((hsh >> 3) % 2) as f32, [236, 255, 250]);
            }
        }
        Scorpion => {
            // Low armoured body, skittering legs, snapping pincers, tail curled over the back.
            let shell = [112, 70, 40];
            let dark = [74, 46, 30];
            if a.pose == Pose::Hidden {
                for k in 0..4 {
                    p.put(w * 0.4 - k as f32 * 0.8, h - 1.0 - k as f32 * 0.9, shell);
                }
                p.put(w * 0.4 - 3.4, h - 4.0, [230, 60, 40]);
                p.put(w * 0.6, h - 0.6, [20, 10, 10]);
                p.put(w * 0.6 + 1.0, h - 0.6, [20, 10, 10]);
            } else {
                let by = h - 2.4;
                for k in 0..4 {
                    let x = w * 0.3 + k as f32 * 1.7;
                    let ph = a.walk * 1.6 + k as f32 * 1.3;
                    p.line(x, by, x - 0.8 + ph.sin() * 0.8 * moving, h - 0.5, dark);
                }
                p.ellipse(w * 0.45, by, 3.8, 1.5, shell);
                for k in 0..3 {
                    p.over(w * 0.45 - 2.0 + k as f32 * 1.8, by - 1.0, mul(shell, 1.25));
                }
                // Tail segments arcing over.
                let strike = a.attack;
                let (mut tx, mut ty) = (w * 0.45 - 3.4, by - 0.5);
                for k in 0..6 {
                    let ang = -PI * 0.5 - 0.9 + k as f32 * (0.5 + strike * 0.12) + (t * 2.0).sin() * 0.05;
                    let (nx, ny) = (tx + ang.cos() * 1.4 - 0.2, ty + ang.sin() * 1.4);
                    p.limb(tx, ty, nx, ny, 1.6 - k as f32 * 0.15, if k % 2 == 0 { shell } else { dark });
                    tx = nx;
                    ty = ny;
                }
                p.put(tx + 0.6, ty + 0.6, [235, 60, 40]);
                // Pincers.
                let snap = ((t * 6.0).sin() * 0.5 + 0.5) * if a.attack > 0.0 { 1.0 } else { 0.3 };
                let (cx, cy) = (w * 0.45 + 3.8, by - 0.2);
                p.limb(cx - 0.6, cy, cx + 1.4, cy - 0.6, 1.2, shell);
                p.ellipse(cx + 2.0, cy - 0.8, 1.1, 0.9, shell);
                p.put(cx + 2.9, cy - 1.4 - snap, dark);
                p.put(cx + 2.9, cy - 0.2 + snap, dark);
                p.put(w * 0.45 + 3.0, by - 0.9, [20, 10, 10]);
            }
        }
        Salamander => {
            // A long glowing lizard whose body snakes as it runs.
            let skin = [236, 96, 34];
            let spot = [255, 222, 90];
            let n = 9;
            let mut pts = Vec::new();
            for k in 0..n {
                let x = w - 2.0 - k as f32 * 1.5;
                let y = h - 2.2 + (a.walk * 1.2 - k as f32 * 0.7).sin() * 0.6 * moving;
                pts.push((x, y));
            }
            for (k, &(x, y)) in pts.iter().enumerate() {
                if k == 2 || k == 5 {
                    let ph = a.walk * 1.4 + k as f32;
                    p.limb(x, y, x + 1.2 + ph.sin() * moving, h - 0.4, 1.0, mul(skin, 0.75));
                    p.limb(x, y, x - 1.0 - ph.sin() * moving, h - 0.4, 1.0, mul(skin, 0.6));
                }
            }
            for (k, w2) in pts.windows(2).enumerate() {
                let th = if k < 6 { 2.4 } else { 1.4 };
                p.limb(w2[0].0, w2[0].1, w2[1].0, w2[1].1, th, skin);
            }
            for (k, &(x, y)) in pts.iter().enumerate().skip(1).step_by(2) {
                let _ = k;
                p.over(x, y - 0.6, spot);
            }
            let (hx, hy) = (pts[0].0 + 0.6, pts[0].1 - 0.3);
            p.ellipse(hx, hy, 1.7, 1.2, skin);
            p.eye(hx + 0.4, hy - 0.5, [255, 250, 160], a.look);
            if a.attack > 0.0 {
                p.put(hx + 1.6, hy + 0.4, [255, 200, 60]);
            }
        }
        Slime => {
            // Wobbly translucent blob: squashes before a hop, stretches in the air.
            let hue = [[80, 210, 112], [80, 150, 240], [240, 110, 190], [240, 200, 70]][(a.seed % 4) as usize];
            let (sq, st) = if !a.ground { (0.85, 1.2) } else { (1.0 + (t * 4.0).sin().abs() * 0.12, 1.0 - (t * 4.0).sin().abs() * 0.1) };
            let (rx, ry) = (w * 0.5 * sq, h * 0.55 * st);
            let (cx, cy) = (w * 0.5, h - ry);
            p.ellipse_a(cx, cy, rx, ry, hue, 0.85);
            p.ellipse_a(cx - rx * 0.2, cy + ry * 0.2, rx * 0.4, ry * 0.35, mul(hue, 0.6), 0.9);
            p.put(cx - rx * 0.5, cy - ry * 0.5, [240, 255, 245]);
            p.put(cx - rx * 0.5 + 1.0, cy - ry * 0.5, [200, 240, 220]);
            p.put(cx + rx * 0.25, cy - ry * 0.1, [20, 40, 30]);
            p.put(cx + rx * 0.6, cy - ry * 0.1, [20, 40, 30]);
        }
        Bat => {
            let body = [76, 52, 90];
            let wing = [104, 74, 118];
            let (cx, cy) = (w * 0.5, h * 0.5);
            if a.pose == Pose::Roost {
                // Hanging upside down, wrapped in its wings.
                p.ellipse(cx, cy + 0.5, 1.6, 2.4, wing);
                p.put(cx - 0.5, cy - 2.2, body);
                p.put(cx + 0.5, cy - 2.2, body);
                p.put(cx - 0.5, cy + 2.4, [255, 220, 120]);
                p.put(cx + 0.6, cy + 2.4, [255, 220, 120]);
            } else {
                let flap = (t * 16.0).sin();
                for side in [-1.0f32, 1.0] {
                    let tip = (cx + side * 4.6, cy - flap * 3.0);
                    let mid = (cx + side * 2.8, cy - flap * 1.6 + 1.5);
                    p.tri((cx, cy - 0.5), tip, mid, if side > 0.0 { wing } else { mul(wing, 0.75) });
                    p.tri((cx, cy + 1.0), mid, (cx + side * 4.2, cy - flap * 1.5 + 1.0), mul(wing, 0.85));
                    p.line(cx, cy - 0.5, tip.0, tip.1, body);
                }
                p.ellipse(cx, cy + 0.4, 1.3, 1.7, body);
                p.put(cx - 0.8, cy - 1.5, body);
                p.put(cx + 0.8, cy - 1.5, body);
                p.put(cx + 0.5, cy - 0.2, [255, 220, 120]);
                p.put(cx - 0.4, cy - 0.2, [255, 220, 120]);
            }
        }
        DemonEye | VoidEye => {
            let void = kind == VoidEye;
            let white = if void { [52, 26, 74] } else { [244, 238, 232] };
            let iris = if void { [210, 80, 255] } else { [60, 110, 220] };
            let r = h * 0.42;
            let (cx, cy) = (w * 0.55, h * 0.5);
            // Trailing nerve or tentacles.
            let n = if void { 4 } else { 1 };
            for k in 0..n {
                let (mut x, mut y) = (cx - r * 0.8, cy + (k as f32 - (n - 1) as f32 * 0.5) * 1.6);
                for s in 0..6 {
                    let ang = PI + (t * 6.0 + s as f32 * 0.8 + k as f32).sin() * 0.45;
                    let (nx, ny) = (x + ang.cos() * 1.3, y + ang.sin() * 1.3);
                    p.limb(x, y, nx, ny, if void { 1.0 } else { 1.6 - s as f32 * 0.2 }, if void { [120, 50, 170] } else { [190, 40, 40] });
                    x = nx;
                    y = ny;
                }
            }
            p.ellipse(cx, cy, r, r, white);
            if !void {
                for k in 0..4 {
                    let ang = k as f32 * 1.6 + 0.4;
                    p.line(cx + ang.cos() * r * 0.4, cy + ang.sin() * r * 0.4, cx + ang.cos() * r * 0.95, cy + ang.sin() * r * 0.95, [210, 70, 70]);
                }
            }
            let (ix, iy) = (cx + a.look.0 * r * 0.4, cy + a.look.1 * r * 0.4);
            p.ellipse(ix, iy, r * 0.5, r * 0.5, iris);
            p.ellipse(ix, iy, r * 0.22, r * 0.25, [10, 8, 16]);
            p.put(ix - r * 0.25, iy - r * 0.3, [255, 255, 255]);
        }
        FireImp => {
            let skin = [214, 60, 40];
            let wing = [150, 40, 40];
            let flap = (t * 14.0).sin();
            let (cx, cy) = (w * 0.5, h * 0.45);
            p.tri((cx - 0.5, cy), (cx - 4.0, cy - 2.0 - flap * 2.0), (cx - 2.6, cy + 1.6), wing);
            // Flame tail.
            for k in 0..4 {
                let f = ((t * 20.0 + k as f32 * 2.0).sin() * 0.5 + 0.5) * 0.8;
                let c = mix([255, 220, 80], [255, 90, 20], k as f32 / 3.0);
                p.put(cx - 0.5 + f, h - 1.0 + k as f32 * 0.9 - 1.0, c);
            }
            p.ellipse(cx, cy + 1.2, 1.8, 2.2, skin);
            p.limb(cx + 1.0, cy + 1.0, cx + 2.8, cy + 2.0 - a.attack * 2.0, 1.0, skin);
            let (hx, hy) = (cx + 0.3, cy - 1.8);
            p.ellipse(hx, hy, 1.7, 1.6, skin);
            p.line(hx - 1.0, hy - 1.2, hx - 1.6, hy - 2.8, [60, 30, 30]);
            p.line(hx + 1.0, hy - 1.2, hx + 1.8, hy - 2.6, [60, 30, 30]);
            p.put(hx + 0.8, hy - 0.2, [255, 240, 100]);
            p.put(hx - 0.2, hy - 0.2, [255, 240, 100]);
            p.tri((cx + 0.5, cy), (cx + 3.6, cy - 2.2 - flap * 2.0), (cx + 2.4, cy + 1.6), mul(wing, 1.2));
        }
        Ghost => {
            // A drifting sheet with a rippling hem, arms reaching when it hunts.
            let c = [222, 232, 255];
            let (cx, top) = (w * 0.5, 1.0);
            p.ellipse_a(cx, top + 3.0, w * 0.45, 3.2, c, 0.7);
            for y in 0..((h - 3.0) as i32) {
                let yy = top + 3.0 + y as f32;
                let half = w * 0.45 + y as f32 * 0.08;
                for x in 0..(half * 2.0) as i32 {
                    let xx = cx - half + x as f32;
                    let hem = h - 1.0 - ((xx * 1.4 + t * 6.0).sin() * 1.0 + 0.8);
                    if yy < hem {
                        p.put_a(xx, yy, mul(c, 1.0 - y as f32 * 0.02), 0.62);
                    }
                }
            }
            let reach = a.attack.max(if a.speed > 0.6 { 0.5 } else { 0.0 });
            p.limb(cx + w * 0.4, top + 5.0, cx + w * 0.4 + 2.0 * reach + 0.6, top + 5.5, 1.0, c);
            p.ellipse(cx + 0.9, top + 3.0, 0.8, 1.1, [30, 30, 60]);
            p.ellipse(cx - 1.5, top + 3.0, 0.8, 1.1, [30, 30, 60]);
            p.ellipse(cx - 0.3, top + 5.4, 0.7, 0.9 + a.attack, [30, 30, 60]);
        }
        StarJelly => {
            // A floating jellyfish: the bell pulses and tentacles trail behind.
            let pulse = (t * 3.0).sin();
            let bell = [180, 150, 255];
            let (cx, cy) = (w * 0.5, 3.0);
            for k in 0..5 {
                let x0 = cx - 2.8 + k as f32 * 1.4;
                let mut x = x0;
                for s in 0..6 {
                    let y = cy + 2.0 + s as f32 * 1.1;
                    let nx = x0 + (t * 2.5 + s as f32 * 0.7 + k as f32).sin() * 0.8;
                    p.put_a(nx, y, mix(bell, [255, 255, 255], s as f32 / 8.0), 0.8 - s as f32 * 0.1);
                    x = nx;
                }
                let _ = x;
            }
            p.ellipse_a(cx, cy, 3.8 + pulse * 0.5, 2.6 - pulse * 0.3, bell, 0.85);
            p.ellipse_a(cx, cy - 0.4, 2.0, 1.2, [230, 210, 255], 0.9);
            for k in 0..3 {
                p.put(cx - 2.0 + k as f32 * 2.0, cy + 0.6, [255, 255, 255]);
            }
        }
        Wisp => {
            let (cx, cy) = (w * 0.5, h * 0.5);
            p.ellipse_a(cx, cy, 2.0, 2.0, [150, 255, 230], 0.5);
            p.ellipse(cx, cy, 1.1, 1.1, [230, 255, 250]);
            for k in 0..3 {
                let ang = t * 5.0 + k as f32 * 2.09;
                p.put(cx + ang.cos() * 2.6, cy + ang.sin() * 2.6, [180, 255, 240]);
            }
        }
        CloudSprite => {
            // A grumpy little storm cloud with a face; crackles when it attacks.
            let c = if a.attack > 0.0 { [190, 196, 214] } else { [244, 246, 252] };
            let bob = (t * 2.0).sin() * 0.4;
            for (x, y, r) in [(0.3, 0.6, 2.4), (0.55, 0.45, 2.8), (0.78, 0.6, 2.2), (0.45, 0.7, 2.2), (0.68, 0.72, 2.0)] {
                p.ellipse(w * x, h * y + bob, r, r * 0.85, c);
            }
            let (fx, fy) = (w * 0.6, h * 0.55 + bob);
            p.put(fx - 1.0, fy, [50, 70, 110]);
            p.put(fx + 1.0, fy, [50, 70, 110]);
            p.line(fx - 1.2, fy - 1.0, fx - 0.2, fy - 0.6, [50, 70, 110]);
            p.line(fx + 1.2, fy - 1.0, fx + 0.2, fy - 0.6, [50, 70, 110]);
            if a.attack > 0.0 {
                p.line(w * 0.5, h - 0.5, w * 0.45, h + 2.0, [255, 255, 160]);
                p.line(w * 0.45, h + 2.0, w * 0.55, h + 3.5, [255, 255, 160]);
            }
            for k in 0..3 {
                let y = h - 0.2 + ((t * 8.0 + k as f32 * 2.0) % 3.0);
                p.put_a(w * 0.35 + k as f32 * 2.0, y, [140, 170, 230], 0.6);
            }
        }
        Fish => {
            // Body with a wagging tail fin; colour varies fish to fish.
            let cols = [[240, 160, 60], [90, 170, 240], [240, 220, 90], [230, 90, 120], [120, 220, 170]];
            let c = cols[(a.seed % cols.len() as u32) as usize];
            let wag = (t * 10.0 * (0.5 + moving)).sin();
            let (cx, cy) = (w * 0.55, h * 0.5);
            p.tri((cx - 2.6, cy), (cx - 4.6, cy - 1.8 + wag * 0.8), (cx - 4.6, cy + 1.8 + wag * 0.8), mul(c, 0.8));
            p.ellipse(cx, cy, 2.9, 1.7, c);
            p.ellipse(cx + 0.3, cy + 0.6, 2.0, 0.8, mix(c, [255, 255, 255], 0.4));
            p.tri((cx - 0.8, cy - 1.4), (cx + 0.8, cy - 1.4), (cx - 1.2, cy - 2.8), mul(c, 0.75));
            p.put(cx + 1.6, cy - 0.4, [20, 20, 30]);
        }
        Shark => {
            let top = [104, 120, 140];
            let belly = [226, 230, 236];
            let wag = (t * 5.0 * (0.4 + moving)).sin();
            let (cx, cy) = (w * 0.5, h * 0.55);
            p.tri((cx - 8.0, cy), (cx - 11.0, cy - 3.6 + wag), (cx - 10.5, cy + 2.6 + wag), mul(top, 0.85));
            p.ellipse(cx, cy, 9.0, 2.8, top);
            p.ellipse(cx + 1.0, cy + 1.2, 7.4, 1.4, belly);
            p.tri((cx - 1.5, cy - 2.4), (cx + 1.8, cy - 2.4), (cx - 1.5, cy - 6.0), top);
            p.tri((cx + 1.0, cy + 1.8), (cx + 3.0, cy + 1.8), (cx - 0.5, cy + 4.0), mul(top, 0.8));
            for k in 0..3 {
                p.over(cx + 4.0 - k as f32 * 0.8, cy - 0.5, mul(top, 0.7));
            }
            p.put(cx + 6.2, cy - 0.8, [20, 20, 30]);
            let jaw = a.attack * 1.6;
            if jaw > 0.1 {
                p.tri((cx + 5.0, cy + 0.8), (cx + 9.0, cy + 0.6), (cx + 8.0, cy + 0.8 + jaw), [140, 30, 40]);
                p.put(cx + 7.5, cy + 0.9, [250, 250, 250]);
                p.put(cx + 6.5, cy + 0.9, [250, 250, 250]);
            }
        }
        Worm | Dragon => {}
    }
    if kind != Ghost {
        p.outline();
    }
    p
}
