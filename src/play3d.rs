//! Playing a 3D world: the wizard in third person on a voxel island, a mouse-look
//! camera, spells that blast, burn, freeze, dig and build, and a broom to fly on.

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Comparison, Equation, PipelineParams, UniformType};
use macroquad::prelude::*;

use crate::audio::Sfx;
use crate::materials::{Kind, Material as M};
use crate::noise::Rng;
use crate::ui::{Ui, ACCENT, MUTED, TEXT};
use crate::voxel::{opaque, Vox, SEA, SUN, SX, SY, SZ};

const HALF_W: f32 = 0.3;
const HEIGHT: f32 = 1.7;
const GRAVITY: f32 = 26.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Spell {
    Fireball,
    Lightning,
    Frost,
    Bomb,
    Dig,
    Build,
}

const SPELLS: [Spell; 6] = [Spell::Fireball, Spell::Lightning, Spell::Frost, Spell::Bomb, Spell::Dig, Spell::Build];
const BUILD: [M; 9] = [M::Sand, M::Water, M::Lava, M::Stone, M::Wood, M::Oil, M::Glass, M::Gunpowder, M::Blossom];

impl Spell {
    fn name(self) -> &'static str {
        match self {
            Spell::Fireball => "FIREBALL",
            Spell::Lightning => "LIGHTNING",
            Spell::Frost => "FROST",
            Spell::Bomb => "BOMB",
            Spell::Dig => "DIG",
            Spell::Build => "BUILD",
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Spell::Fireball => "TAP: FIREBALL   HOLD: FIRE BEAM THAT MELTS ROCK",
            Spell::Lightning => "HOLD FOR A CRACKLING STREAM",
            Spell::Frost => "HOLD: FREEZE WATER, LAVA TO OBSIDIAN",
            Spell::Bomb => "THROWN, THEN A HUGE BLAST",
            Spell::Dig => "HOLD TO DIG",
            Spell::Build => "HOLD TO PLACE   Q / E: MATERIAL",
        }
    }
}

struct Particle {
    pos: Vec3,
    vel: Vec3,
    life: f32,
    col: [u8; 3],
    size: f32,
    grav: f32,
    /// Becomes a voxel where it lands.
    mat: Option<M>,
}

struct Shot {
    pos: Vec3,
    vel: Vec3,
    bomb: bool,
    life: f32,
}

pub enum Exit {
    Stay,
    Menu,
}

pub struct Play3D {
    pub vox: Vox,
    pub name: String,
    pub seed: u64,
    pub creative: bool,
    pub dir: Option<std::path::PathBuf>,
    pos: Vec3,
    vel: Vec3,
    yaw: f32,
    pitch: f32,
    on_ground: bool,
    broom: bool,
    spell: usize,
    build: usize,
    hold: f32,
    cool: f32,
    shots: Vec<Shot>,
    parts: Vec<Particle>,
    bolts: Vec<(Vec<Vec3>, f32)>,
    beam: Option<(Vec3, Vec3, u8)>,
    rng: Rng,
    acc: f32,
    pub time: f32,
    shake: f32,
    paused: bool,
    last_mouse: (f32, f32),
    autosave: f32,
    pub sounds: Vec<Sfx>,
    mat_solid: Material,
    mat_water: Material,
    clouds: Vec<(Vec3, Vec3)>,
}

const VOX_VERT: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec4 color;
varying highp vec3 wpos;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    color = color0 / 255.0;
    wpos = position;
}"#;

const VOX_FRAG: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying highp vec3 wpos;
uniform vec3 u_cam;
uniform vec3 u_fog;
uniform vec3 u_tint;
uniform float u_density;
void main() {
    float d = length(wpos - u_cam);
    float f = 1.0 - exp(-pow(d * u_density, 1.6));
    vec3 c = color.rgb * u_tint;
    float lum = dot(c, vec3(0.299, 0.587, 0.114));
    c *= mix(vec3(0.86, 0.93, 1.1), vec3(1.07, 1.0, 0.88), smoothstep(0.1, 0.7, lum));
    c = mix(c, c * c * (3.0 - 2.0 * c), 0.35);
    c = mix(c, u_fog, clamp(f, 0.0, 1.0));
    gl_FragColor = vec4(c, color.a);
}"#;

fn vox_material(depth_write: bool) -> Material {
    load_material(
        ShaderSource::Glsl { vertex: VOX_VERT, fragment: VOX_FRAG },
        MaterialParams {
            pipeline_params: PipelineParams {
                depth_test: Comparison::LessOrEqual,
                depth_write,
                color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha))),
                ..Default::default()
            },
            uniforms: vec![
                UniformDesc::new("u_cam", UniformType::Float3),
                UniformDesc::new("u_fog", UniformType::Float3),
                UniformDesc::new("u_tint", UniformType::Float3),
                UniformDesc::new("u_density", UniformType::Float1),
            ],
            textures: vec![],
        },
    )
    .unwrap_or_else(|e| panic!("voxel shader failed: {e:?}"))
}

fn col(c: [u8; 3], a: u8) -> Color {
    Color::from_rgba(c[0], c[1], c[2], a)
}

impl Play3D {
    pub fn new(name: String, seed: u64, creative: bool) -> Self {
        let vox = Vox::new(seed);
        let mut rng = Rng::new(seed ^ 0xC10D);
        let clouds = (0..14)
            .map(|_| {
                let p = vec3(rng.rangef(-40.0, SX as f32 + 40.0), SY as f32 + rng.rangef(6.0, 22.0), rng.rangef(-40.0, SZ as f32 + 40.0));
                let s = vec3(rng.rangef(10.0, 26.0), rng.rangef(2.0, 4.0), rng.rangef(8.0, 18.0));
                (p, s)
            })
            .collect();
        let mut g = Play3D {
            vox,
            name,
            seed,
            creative,
            dir: None,
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            yaw: 0.6,
            pitch: -0.25,
            on_ground: false,
            broom: false,
            spell: 0,
            build: 0,
            hold: 0.0,
            cool: 0.0,
            shots: Vec::new(),
            parts: Vec::new(),
            bolts: Vec::new(),
            beam: None,
            rng,
            acc: 0.0,
            time: 0.0,
            shake: 0.0,
            paused: false,
            last_mouse: mouse_position(),
            autosave: 0.0,
            sounds: Vec::new(),
            mat_solid: vox_material(true),
            // Depth writes stay on: with them off this backend drops the depth test too.
            mat_water: vox_material(true),
            clouds,
        };
        g.pos = g.spawn();
        g
    }

    /// A grassy spot near the middle of the island.
    fn spawn(&self) -> Vec3 {
        let (cx, cz) = (SX / 2, SZ / 2);
        for r in 0..80 {
            for k in 0..16 {
                let a = k as f32 / 16.0 * std::f32::consts::TAU;
                let (x, z) = (cx + (a.cos() * r as f32) as i32, cz + (a.sin() * r as f32) as i32);
                let t = self.vox.top(x, z);
                if t > SEA && t < SEA + 20 && self.vox.get(x, t, z) == M::Grass {
                    return vec3(x as f32 + 0.5, t as f32 + 1.0, z as f32 + 0.5);
                }
            }
        }
        vec3(cx as f32, SY as f32 - 4.0, cz as f32)
    }

    // ---------- saving ----------

    pub fn save(&mut self) {
        let Some(dir) = &self.dir else { return };
        let _ = std::fs::create_dir_all(dir);
        let meta = format!(
            "name={}\nseed={}\ncreative={}\ncommands=true\ndim=3d\nplanet_name=3D ISLAND\nplayed={}\npos3={},{},{}\n",
            self.name,
            self.seed,
            self.creative,
            crate::saves::now(),
            self.pos.x,
            self.pos.y,
            self.pos.z
        );
        let _ = std::fs::write(dir.join("world.txt"), meta);
        let _ = std::fs::write(dir.join("voxels.bin"), self.vox.encode());
        self.vox.edited = false;
        self.autosave = 0.0;
    }

    pub fn load(dir: &std::path::Path) -> Option<Play3D> {
        let meta = crate::saves::read_meta(dir)?;
        let get = |k: &str| meta.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone()).unwrap_or_default();
        if get("dim") != "3d" {
            return None;
        }
        let mut g = Play3D::new(get("name"), get("seed").parse().ok()?, get("creative") == "true");
        if let Ok(data) = std::fs::read(dir.join("voxels.bin")) {
            g.vox.decode(&data);
        }
        let p: Vec<f32> = get("pos3").split(',').filter_map(|v| v.parse().ok()).collect();
        if p.len() == 3 {
            g.pos = vec3(p[0], p[1], p[2]);
        }
        g.dir = Some(dir.to_path_buf());
        Some(g)
    }

    // ---------- camera ----------

    fn forward(&self) -> Vec3 {
        vec3(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), self.yaw.cos() * self.pitch.cos())
    }

    fn camera(&self) -> (Vec3, Vec3) {
        let target = self.pos + vec3(0.0, 1.45, 0.0);
        let f = self.forward();
        let mut dist = 6.5;
        let mut t = 0.3;
        while t < dist {
            let p = target - f * t;
            if opaque(self.vox.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)) {
                dist = (t - 0.4).max(0.6);
                break;
            }
            t += 0.2;
        }
        let side = vec3(-self.yaw.cos(), 0.0, self.yaw.sin()) * 0.7;
        let mut eye = target - f * dist + side * (dist / 6.5);
        if self.shake > 0.0 {
            eye += vec3(self.rng_peek(1), self.rng_peek(2), self.rng_peek(3)) * self.shake * 0.04;
        }
        (eye, eye + f)
    }

    fn rng_peek(&self, k: u32) -> f32 {
        (crate::noise::hash3((self.time * 60.0) as i32, k as i32, 9) % 1000) as f32 / 500.0 - 1.0
    }

    /// Where the crosshair points: the first thing hit from the camera.
    fn aim(&self) -> (Vec3, Option<((i32, i32, i32), (i32, i32, i32))>) {
        let (eye, at) = self.camera();
        let dir = (at - eye).normalize();
        match self.vox.raycast(eye, dir, 140.0, self.spell_hits_liquid()) {
            Some(hit) => (vec3(hit.0 .0 as f32 + 0.5, hit.0 .1 as f32 + 0.5, hit.0 .2 as f32 + 0.5), Some(hit)),
            None => (eye + dir * 120.0, None),
        }
    }

    fn spell_hits_liquid(&self) -> bool {
        matches!(SPELLS[self.spell], Spell::Frost | Spell::Build | Spell::Lightning)
    }

    fn hand(&self) -> Vec3 {
        let right = vec3(-self.yaw.cos(), 0.0, self.yaw.sin());
        self.pos + vec3(0.0, 1.05, 0.0) + right * 0.45
    }

    // ---------- physics ----------

    fn blocked(&self, p: Vec3) -> bool {
        let (x0, x1) = ((p.x - HALF_W).floor() as i32, (p.x + HALF_W - 0.001).floor() as i32);
        let (y0, y1) = (p.y.floor() as i32, (p.y + HEIGHT - 0.001).floor() as i32);
        let (z0, z1) = ((p.z - HALF_W).floor() as i32, (p.z + HALF_W - 0.001).floor() as i32);
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if self.vox.solid(x, y, z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn move_body(&mut self, dt: f32) {
        let d = self.vel * dt;
        let n = (d.length() / 0.2).ceil().max(1.0) as i32;
        let s = d / n as f32;
        self.on_ground = false;
        for _ in 0..n {
            for axis in 0..3 {
                let mut p = self.pos;
                p[axis] += s[axis];
                if !self.blocked(p) {
                    self.pos = p;
                    continue;
                }
                if axis != 1 {
                    // Step up one block when walking into a ledge.
                    let up = p + vec3(0.0, 1.05, 0.0);
                    if (self.on_ground || self.vel.y <= 0.0) && !self.broom && !self.blocked(up) && self.blocked(self.pos - vec3(0.0, 0.05, 0.0)) {
                        self.pos = up;
                        continue;
                    }
                    self.vel[axis] = 0.0;
                } else {
                    if s.y < 0.0 {
                        self.on_ground = true;
                        self.pos.y = self.pos.y.floor().max(p.y.ceil() - 0.0);
                    }
                    self.vel.y = 0.0;
                }
            }
        }
        if self.blocked(self.pos - vec3(0.0, 0.05, 0.0)) && self.vel.y <= 0.0 {
            self.on_ground = true;
        }
    }

    // ---------- update ----------

    pub fn update(&mut self, dt: f32, ui: &mut Ui) -> Exit {
        if is_key_pressed(KeyCode::Escape) {
            self.paused = !self.paused;
        }
        if self.paused {
            set_cursor_grab(false);
            show_mouse(true);
            return Exit::Stay;
        }
        set_cursor_grab(true);
        show_mouse(false);
        let _ = ui;
        self.time += dt;
        self.cool -= dt;
        self.shake = (self.shake - dt * 18.0).max(0.0);
        self.autosave += dt;
        if self.autosave > 60.0 {
            self.save();
        }

        // Mouse look.
        let m = mouse_position();
        let (dx, dy) = (m.0 - self.last_mouse.0, m.1 - self.last_mouse.1);
        self.last_mouse = m;
        if dx.abs() < 400.0 && dy.abs() < 400.0 {
            self.yaw -= dx * 0.0042;
            self.pitch = (self.pitch - dy * 0.0042).clamp(-1.35, 1.2);
        }

        // Choosing spells.
        for (k, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5, KeyCode::Key6].iter().enumerate() {
            if is_key_pressed(*key) {
                self.spell = k;
            }
        }
        let wheel = mouse_wheel().1;
        if wheel != 0.0 {
            self.spell = (self.spell as i32 + if wheel > 0.0 { -1 } else { 1 }).rem_euclid(SPELLS.len() as i32) as usize;
        }
        if is_key_pressed(KeyCode::E) {
            self.build = (self.build + 1) % BUILD.len();
        }
        if is_key_pressed(KeyCode::Q) {
            self.build = (self.build + BUILD.len() - 1) % BUILD.len();
        }
        if is_key_pressed(KeyCode::M) {
            self.broom = !self.broom;
            self.vel.y = self.vel.y.max(4.0);
            self.sounds.push(Sfx::Whoosh);
        }

        // Movement relative to where the camera faces.
        let fwd = vec3(self.yaw.sin(), 0.0, self.yaw.cos());
        let right = vec3(-self.yaw.cos(), 0.0, self.yaw.sin());
        let mut wish = Vec3::ZERO;
        if is_key_down(KeyCode::W) {
            wish += fwd;
        }
        if is_key_down(KeyCode::S) {
            wish -= fwd;
        }
        if is_key_down(KeyCode::D) {
            wish += right;
        }
        if is_key_down(KeyCode::A) {
            wish -= right;
        }
        let wish = wish.normalize_or_zero();
        let sprint = is_key_down(KeyCode::LeftShift);
        let feet = (self.pos.x.floor() as i32, (self.pos.y + 0.5).floor() as i32, self.pos.z.floor() as i32);
        let swimming = matches!(self.vox.get(feet.0, feet.1, feet.2).kind(), Kind::Liquid);
        if self.broom {
            // Broom: fly wherever you look, fast.
            let look = self.forward();
            let mut v = Vec3::ZERO;
            if is_key_down(KeyCode::W) {
                v += look;
            }
            if is_key_down(KeyCode::S) {
                v -= look;
            }
            if is_key_down(KeyCode::D) {
                v += right;
            }
            if is_key_down(KeyCode::A) {
                v -= right;
            }
            if is_key_down(KeyCode::Space) {
                v.y += 1.0;
            }
            if is_key_down(KeyCode::LeftControl) {
                v.y -= 1.0;
            }
            let speed = if sprint { 36.0 } else { 20.0 };
            let target = v.normalize_or_zero() * speed + vec3(0.0, (self.time * 3.0).sin() * 0.4, 0.0);
            self.vel += (target - self.vel) * (dt * 3.5).min(1.0);
            if self.vel.length() > 12.0 && self.rng.chance(0.6) {
                let back = self.pos + vec3(0.0, 0.6, 0.0) - self.vel.normalize() * 1.2;
                self.parts.push(Particle { pos: back, vel: Vec3::ZERO, life: 0.5, col: [255, 236, 160], size: 0.1, grav: -1.0, mat: None });
            }
        } else {
            let speed = if sprint { 10.0 } else { 6.0 };
            let accel = if self.on_ground { 14.0 } else { 5.0 };
            let target = wish * speed;
            self.vel.x += (target.x - self.vel.x) * (dt * accel).min(1.0);
            self.vel.z += (target.z - self.vel.z) * (dt * accel).min(1.0);
            if swimming {
                self.vel.y = (self.vel.y - GRAVITY * 0.25 * dt) * (1.0 - 2.0 * dt);
                if is_key_down(KeyCode::Space) {
                    self.vel.y += 22.0 * dt;
                }
            } else {
                self.vel.y -= GRAVITY * dt;
                if is_key_pressed(KeyCode::Space) && self.on_ground {
                    self.vel.y = 9.0;
                } else if is_key_down(KeyCode::Space) && !self.on_ground && self.creative && self.vel.y < 7.0 {
                    // Creative: hold space to fly up.
                    self.vel.y += 48.0 * dt;
                }
            }
            self.vel.y = self.vel.y.max(-40.0);
        }
        self.move_body(dt);
        if self.pos.y < -8.0 {
            self.pos = self.spawn();
            self.vel = Vec3::ZERO;
        }
        self.pos.y = self.pos.y.min(SY as f32 + 40.0);

        // Casting.
        let primary = is_mouse_button_down(MouseButton::Left);
        self.hold = if primary { self.hold + dt } else { 0.0 };
        self.beam = None;
        if is_mouse_button_down(MouseButton::Right) && self.cool <= 0.0 {
            self.dig(0.9);
            self.cool = 0.06;
        }
        if primary {
            self.cast(dt);
        }

        // Simulation at a fixed rate.
        self.acc += dt;
        let mut steps = 0;
        while self.acc >= 1.0 / 30.0 && steps < 2 {
            self.vox.step();
            self.acc -= 1.0 / 30.0;
            steps += 1;
        }
        if steps == 2 {
            self.acc = 0.0;
        }
        for (p, kind) in std::mem::take(&mut self.vox.puffs) {
            let n = if kind == 0 { 4 } else { 1 };
            for _ in 0..n {
                let v = vec3(self.rng.rangef(-0.6, 0.6), self.rng.rangef(1.0, 2.5), self.rng.rangef(-0.6, 0.6));
                let c = if kind == 0 { [230, 236, 244] } else { [90, 86, 92] };
                self.parts.push(Particle { pos: p, vel: v, life: self.rng.rangef(0.8, 1.6), col: c, size: 0.35, grav: -0.5, mat: None });
            }
        }
        self.step_shots(dt);
        self.step_parts(dt);
        for b in self.bolts.iter_mut() {
            b.1 -= dt;
        }
        self.bolts.retain(|b| b.1 > 0.0);
        let budget = if self.vox.dirty_left() > 100 { 24 } else { 6 };
        self.vox.remesh(self.pos, budget);
        Exit::Stay
    }

    fn cast(&mut self, dt: f32) {
        let spell = SPELLS[self.spell];
        let (target, hit) = self.aim();
        let hand = self.hand();
        let dir = (target - hand).normalize_or_zero();
        match spell {
            Spell::Fireball if self.hold > 0.25 => {
                // Fire beam: sets everything along it alight and melts what it hits.
                self.beam = Some((hand, target, 0));
                for _ in 0..4 {
                    let t = self.rng.f32();
                    let p = hand + (target - hand) * t;
                    let v = vec3(self.rng.rangef(-0.5, 0.5), self.rng.rangef(0.5, 2.0), self.rng.rangef(-0.5, 0.5));
                    let c = [[255, 240, 150], [255, 188, 48], [255, 122, 20]][self.rng.range(0, 3) as usize];
                    self.parts.push(Particle { pos: p, vel: v, life: 0.4, col: c, size: 0.18, grav: -2.0, mat: None });
                }
                if self.cool <= 0.0 {
                    self.cool = 0.05;
                    if let Some((c, prev)) = hit {
                        self.vox.ignite(prev.0, prev.1, prev.2);
                        for _ in 0..6 {
                            let (x, y, z) = (c.0 + self.rng.range(-1, 2), c.1 + self.rng.range(-1, 2), c.2 + self.rng.range(-1, 2));
                            let m = self.vox.get(x, y, z);
                            if m.flammable() {
                                self.vox.ignite(x, y, z);
                            } else if matches!(m, M::Stone | M::Dirt | M::Sand | M::Gravel | M::Basalt | M::Snow | M::Ice | M::Obsidian | M::Glass) && self.rng.chance(0.25) {
                                let to = match m {
                                    M::Sand => M::Glass,
                                    M::Snow | M::Ice => M::Water,
                                    _ => M::Lava,
                                };
                                self.vox.set(x, y, z, to);
                            } else if m == M::Water && self.rng.chance(0.4) {
                                self.vox.set(x, y, z, M::Empty);
                                self.vox.puffs.push((vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5), 0));
                            }
                        }
                    }
                    self.sounds.push(Sfx::Fireball);
                    self.shake = self.shake.max(1.0);
                }
            }
            Spell::Fireball if self.cool <= 0.0 && self.hold <= dt * 1.5 => {
                self.shots.push(Shot { pos: hand, vel: dir * 42.0, bomb: false, life: 3.0 });
                self.cool = 0.3;
                self.sounds.push(Sfx::Fireball);
            }
            Spell::Bomb if self.cool <= 0.0 => {
                self.shots.push(Shot { pos: hand, vel: dir * 26.0 + vec3(0.0, 6.0, 0.0), bomb: true, life: 2.2 });
                self.cool = 0.7;
                self.sounds.push(Sfx::Throw);
            }
            Spell::Lightning if self.cool <= 0.0 => {
                self.cool = if self.hold > 0.2 { 0.07 } else { 0.4 };
                let mut pts = vec![hand];
                let len = (target - hand).length();
                let n = ((len / 1.5) as usize).clamp(3, 60);
                for k in 1..n {
                    let t = k as f32 / n as f32;
                    let j = (1.0 - (t - 0.5).abs() * 2.0) * 0.9;
                    pts.push(hand + (target - hand) * t + vec3(self.rng.rangef(-j, j), self.rng.rangef(-j, j), self.rng.rangef(-j, j)));
                }
                pts.push(target);
                self.bolts.push((pts, if self.hold > 0.2 { 0.08 } else { 0.25 }));
                if let Some((c, prev)) = hit {
                    for _ in 0..8 {
                        let (x, y, z) = (c.0 + self.rng.range(-2, 3), c.1 + self.rng.range(-2, 3), c.2 + self.rng.range(-2, 3));
                        match self.vox.get(x, y, z) {
                            M::Sand => self.vox.set(x, y, z, M::Glass),
                            M::Water if self.rng.chance(0.2) => {
                                self.vox.set(x, y, z, M::Empty);
                                self.vox.puffs.push((vec3(x as f32, y as f32, z as f32), 0));
                            }
                            m if m.flammable() => self.vox.ignite(x, y, z),
                            _ => {}
                        }
                    }
                    if self.rng.chance(0.4) && !self.vox.get(c.0, c.1, c.2).indestructible() && self.vox.get(c.0, c.1, c.2) != M::Water {
                        self.vox.set(c.0, c.1, c.2, M::Empty);
                    }
                    let _ = prev;
                }
                self.burst(target, 10, [210, 235, 255], 6.0);
                self.shake = self.shake.max(if self.hold > 0.2 { 1.5 } else { 5.0 });
                self.sounds.push(Sfx::Zap);
                if self.hold <= 0.2 {
                    self.sounds.push(Sfx::Thunder);
                }
            }
            Spell::Frost => {
                self.beam = Some((hand, target, 1));
                if self.cool <= 0.0 {
                    self.cool = 0.05;
                    if let Some((c, _)) = hit {
                        let r = 1 + (self.hold * 1.5).min(3.0) as i32;
                        for _ in 0..(10 + r * 6) {
                            let (x, y, z) = (c.0 + self.rng.range(-r, r + 1), c.1 + self.rng.range(-r, r + 1), c.2 + self.rng.range(-r, r + 1));
                            match self.vox.get(x, y, z) {
                                M::Water => self.vox.set(x, y, z, M::Ice),
                                M::Lava => self.vox.set(x, y, z, M::Obsidian),
                                M::Fire => self.vox.set(x, y, z, M::Empty),
                                M::Empty if opaque(self.vox.get(x, y - 1, z)) && self.rng.chance(0.15) => self.vox.set(x, y, z, M::Snow),
                                _ => {}
                            }
                        }
                    }
                    self.burst(target, 4, [210, 240, 255], 3.0);
                    self.sounds.push(Sfx::Freeze);
                }
            }
            Spell::Dig if self.cool <= 0.0 => {
                self.dig(1.6);
                self.cool = 0.06;
            }
            Spell::Build if self.cool <= 0.0 => {
                self.cool = 0.05;
                if let Some((_, prev)) = hit {
                    let m = BUILD[self.build];
                    for _ in 0..3 {
                        let (x, y, z) = (prev.0 + self.rng.range(-1, 2), prev.1 + self.rng.range(-1, 2), prev.2 + self.rng.range(-1, 2));
                        if self.vox.get(x, y, z) == M::Empty && !self.overlaps_player(x, y, z) {
                            self.vox.set(x, y, z, m);
                        }
                    }
                    self.sounds.push(Sfx::Tick);
                }
            }
            _ => {}
        }
    }

    fn overlaps_player(&self, x: i32, y: i32, z: i32) -> bool {
        let p = self.pos;
        (x as f32 + 1.0) > p.x - HALF_W && (x as f32) < p.x + HALF_W && (z as f32 + 1.0) > p.z - HALF_W && (z as f32) < p.z + HALF_W && (y as f32 + 1.0) > p.y && (y as f32) < p.y + HEIGHT
    }

    fn dig(&mut self, r: f32) {
        let (_, hit) = self.aim();
        let Some((c, _)) = hit else { return };
        let ri = r.ceil() as i32;
        for y in -ri..=ri {
            for z in -ri..=ri {
                for x in -ri..=ri {
                    if ((x * x + y * y + z * z) as f32) > r * r {
                        continue;
                    }
                    let (px, py, pz) = (c.0 + x, c.1 + y, c.2 + z);
                    let m = self.vox.get(px, py, pz);
                    if m == M::Empty || m.indestructible() || py < 1 {
                        continue;
                    }
                    self.vox.set(px, py, pz, M::Empty);
                    if self.rng.chance(0.3) {
                        let v = vec3(self.rng.rangef(-2.0, 2.0), self.rng.rangef(1.0, 4.0), self.rng.rangef(-2.0, 2.0));
                        self.parts.push(Particle { pos: vec3(px as f32 + 0.5, py as f32 + 0.5, pz as f32 + 0.5), vel: v, life: 0.6, col: m.base_color(), size: 0.18, grav: 1.0, mat: None });
                    }
                }
            }
        }
        self.sounds.push(Sfx::Dig);
    }

    /// Blow a sphere out of the island, flinging debris that lands as loose matter.
    fn explode(&mut self, c: Vec3, r: f32, fire: bool) {
        let ri = r.ceil() as i32;
        let (cx, cy, cz) = (c.x.floor() as i32, c.y.floor() as i32, c.z.floor() as i32);
        let mut chain = Vec::new();
        for y in -ri..=ri {
            for z in -ri..=ri {
                for x in -ri..=ri {
                    let d = ((x * x + y * y + z * z) as f32).sqrt();
                    if d > r + self.rng.rangef(-0.6, 0.4) {
                        continue;
                    }
                    let (px, py, pz) = (cx + x, cy + y, cz + z);
                    let m = self.vox.get(px, py, pz);
                    if m == M::Empty || m.indestructible() || py < 1 {
                        continue;
                    }
                    if m == M::Gunpowder {
                        chain.push(vec3(px as f32, py as f32, pz as f32));
                    }
                    self.vox.set(px, py, pz, M::Empty);
                    if self.rng.chance(0.18) && self.parts.len() < 2500 {
                        let out = vec3(x as f32, y as f32 + 1.5, z as f32).normalize_or_zero() * self.rng.rangef(6.0, 16.0);
                        let loose = match m {
                            M::Stone | M::Basalt | M::Obsidian | M::Brick => Some(M::Gravel),
                            M::Dirt | M::Grass => Some(M::Dirt),
                            M::Sand | M::Gravel | M::Snow | M::Ash => Some(m),
                            M::Water | M::Lava | M::Oil => Some(m),
                            _ => None,
                        };
                        self.parts.push(Particle { pos: vec3(px as f32 + 0.5, py as f32 + 0.5, pz as f32 + 0.5), vel: out, life: 3.0, col: m.base_color(), size: 0.3, grav: 1.0, mat: loose });
                    }
                }
            }
        }
        if fire {
            for _ in 0..(r * r * 3.0) as usize {
                let p = c + vec3(self.rng.rangef(-r, r), self.rng.rangef(-r, r), self.rng.rangef(-r, r)) * 1.1;
                let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                self.vox.ignite(x, y, z);
            }
        }
        self.burst(c, (20.0 + r * 10.0) as usize, [255, 190, 90], r * 3.0);
        self.burst(c, (10.0 + r * 4.0) as usize, [255, 250, 220], r * 2.0);
        let near = 1.0 - ((c - self.pos).length() / 60.0).min(1.0);
        self.shake = self.shake.max(r * 3.0 * near);
        self.sounds.push(if r > 4.0 { Sfx::Boom } else { Sfx::Pop });
        for p in chain.into_iter().take(6) {
            self.shots.push(Shot { pos: p, vel: Vec3::ZERO, bomb: true, life: 0.05 + self.rng.rangef(0.0, 0.2) });
        }
    }

    fn burst(&mut self, c: Vec3, n: usize, colr: [u8; 3], speed: f32) {
        for _ in 0..n {
            if self.parts.len() > 3000 {
                break;
            }
            let v = vec3(self.rng.rangef(-1.0, 1.0), self.rng.rangef(-0.6, 1.4), self.rng.rangef(-1.0, 1.0)).normalize_or_zero() * self.rng.rangef(speed * 0.3, speed);
            self.parts.push(Particle { pos: c, vel: v, life: self.rng.rangef(0.3, 0.8), col: colr, size: 0.14, grav: 0.6, mat: None });
        }
    }

    fn step_shots(&mut self, dt: f32) {
        let mut boom = Vec::new();
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|s| {
            s.life -= dt;
            s.vel.y -= if s.bomb { 18.0 } else { 3.0 } * dt;
            let steps = ((s.vel.length() * dt) / 0.25).ceil().max(1.0) as i32;
            for _ in 0..steps {
                let n = s.pos + s.vel * dt / steps as f32;
                let m = self.vox.get(n.x.floor() as i32, n.y.floor() as i32, n.z.floor() as i32);
                if opaque(m) || matches!(m.kind(), Kind::Liquid) {
                    if s.bomb && s.life > 0.3 && opaque(m) {
                        s.vel *= -0.35;
                        break;
                    }
                    boom.push((s.pos, s.bomb));
                    return false;
                }
                s.pos = n;
            }
            if !s.bomb && self.rng.chance(0.8) {
                self.parts.push(Particle { pos: s.pos, vel: vec3(0.0, 0.5, 0.0), life: 0.35, col: [255, 150, 40], size: 0.22, grav: -1.0, mat: None });
            }
            if s.life <= 0.0 {
                boom.push((s.pos, s.bomb));
                return false;
            }
            true
        });
        shots.append(&mut self.shots);
        self.shots = shots;
        for (p, bomb) in boom {
            let fire = !bomb || self.rng.chance(0.5);
            self.explode(p, if bomb { 6.5 } else { 3.2 }, fire);
        }
    }

    fn step_parts(&mut self, dt: f32) {
        let mut parts = std::mem::take(&mut self.parts);
        parts.retain_mut(|p| {
            p.life -= dt;
            p.vel.y -= GRAVITY * p.grav * dt;
            let n = p.pos + p.vel * dt;
            let (x, y, z) = (n.x.floor() as i32, n.y.floor() as i32, n.z.floor() as i32);
            if opaque(self.vox.get(x, y, z)) {
                if let Some(m) = p.mat {
                    let (px, py, pz) = (p.pos.x.floor() as i32, p.pos.y.floor() as i32, p.pos.z.floor() as i32);
                    if self.vox.get(px, py, pz) == M::Empty {
                        self.vox.set(px, py, pz, m);
                    }
                    return false;
                }
                p.vel *= -0.2;
                return p.life > 0.0;
            }
            p.pos = n;
            p.life > 0.0
        });
        parts.append(&mut self.parts);
        self.parts = parts;
    }

    // ---------- drawing ----------

    pub fn draw(&mut self, ui: &mut Ui) {
        // Sky and light over a slow day.
        let day = ((self.time / 300.0 * std::f32::consts::TAU).cos() * 0.5 + 0.5) * 0.75 + 0.25;
        let sky_top = vec3(0.30, 0.52, 0.86) * day + vec3(0.03, 0.03, 0.08);
        let horizon = vec3(0.86, 0.78, 0.66) * day + vec3(0.06, 0.05, 0.1);
        let (eye, at) = self.camera();
        clear_background(Color::new(sky_top.x, sky_top.y, sky_top.z, 1.0));
        // A gradient towards the horizon, drawn as screen-space bands before the 3D pass.
        let (sw, sh) = (screen_width(), screen_height());
        let look_up = self.pitch.clamp(-1.2, 1.2);
        let hz = sh * (0.5 + look_up * 0.45);
        for k in 0..40 {
            let t = k as f32 / 40.0;
            let y0 = hz - sh * 0.6 * (1.0 - t);
            let c = sky_top.lerp(horizon, t * t);
            draw_rectangle(0.0, y0, sw, sh * 0.6 / 40.0 + 1.0, Color::new(c.x, c.y, c.z, 1.0));
        }
        draw_rectangle(0.0, hz, sw, sh, Color::new(horizon.x, horizon.y, horizon.z, 1.0));

        set_camera(&Camera3D { position: eye, target: at, up: vec3(0.0, 1.0, 0.0), fovy: 1.05, ..Default::default() });

        // The sun, far off in the direction its shadows were baked from.
        let sun = eye + vec3(SUN[0], SUN[1], SUN[2]).normalize() * 400.0;
        draw_sphere(sun, 22.0, None, Color::new(1.0, 0.95, 0.8, day));
        draw_sphere(sun, 34.0, None, Color::new(1.0, 0.85, 0.6, 0.18 * day));
        for (p, s) in &self.clouds {
            let drift = vec3((self.time * 0.6) % (SX as f32 + 80.0), 0.0, 0.0);
            let mut q = *p + drift;
            if q.x > SX as f32 + 40.0 {
                q.x -= SX as f32 + 80.0;
            }
            draw_cube(q, *s, None, Color::new(0.97 * day + 0.03, 0.97 * day + 0.03, day, 0.85));
        }

        let fog = horizon;
        for m in [&self.mat_solid, &self.mat_water] {
            m.set_uniform("u_cam", eye);
            m.set_uniform("u_fog", fog);
            m.set_uniform("u_tint", vec3(1.0, 0.98, 0.95) * (0.35 + 0.65 * day));
            m.set_uniform("u_density", 0.006f32);
        }
        self.vox.draw(eye, &self.mat_solid, &self.mat_water);

        // Flames.
        for (k, &i) in self.vox.fires.iter().enumerate().take(2500) {
            let i = i as i32;
            let (x, z, y) = (i % SX, (i / SX) % SZ, i / (SX * SZ));
            let f = ((self.time * 14.0 + k as f32 * 1.7).sin() * 0.5 + 0.5) * 0.25;
            let c = vec3(x as f32 + 0.5, y as f32 + 0.35 + f, z as f32 + 0.5);
            draw_cube(c, vec3(0.7 - f, 0.7 + f, 0.7 - f), None, Color::new(1.0, 0.45 + f, 0.1, 0.9));
            draw_cube(c + vec3(0.0, 0.25, 0.0), vec3(0.35, 0.45, 0.35), None, Color::new(1.0, 0.9, 0.5, 0.95));
        }
        for p in &self.parts {
            let a = (p.life * 3.0).min(1.0);
            draw_cube(p.pos, vec3(p.size, p.size, p.size), None, col(p.col, (a * 255.0) as u8));
        }
        for s in &self.shots {
            let c = if s.bomb { Color::new(0.3, 0.28, 0.34, 1.0) } else { Color::new(1.0, 0.7, 0.2, 1.0) };
            draw_sphere(s.pos, if s.bomb { 0.35 } else { 0.3 }, None, c);
        }
        for (pts, _) in &self.bolts {
            for w in pts.windows(2) {
                draw_line_3d(w[0], w[1], WHITE);
                draw_line_3d(w[0] + vec3(0.05, 0.0, 0.0), w[1] + vec3(0.05, 0.0, 0.0), Color::new(0.6, 0.8, 1.0, 0.8));
            }
        }
        if let Some((a, b, kind)) = self.beam {
            let n = ((b - a).length() * 2.0) as i32;
            for k in 0..n {
                let t = k as f32 / n.max(1) as f32;
                let wob = (k as f32 * 0.8 - self.time * 30.0).sin() * 0.08;
                let p = a + (b - a) * t + vec3(0.0, wob, 0.0);
                let s = 0.12 + t * 0.25;
                let c = if kind == 1 { Color::new(0.8, 0.95, 1.0, 0.7) } else { Color::new(1.0, 0.55 + 0.4 * (1.0 - t), 0.15, 0.85) };
                draw_cube(p, vec3(s, s, s), None, c);
            }
        }
        self.draw_wizard();
        set_default_camera();
        self.hud(ui);
    }

    fn draw_wizard(&self) {
        let p = self.pos;
        let face = vec3(self.yaw.sin(), 0.0, self.yaw.cos());
        let robe = Color::from_rgba(64, 50, 160, 255);
        let hat = Color::from_rgba(92, 70, 196, 255);
        let bob = if self.on_ground && self.vel.length() > 1.0 { (self.time * 12.0).sin().abs() * 0.06 } else { 0.0 };
        let o = p + vec3(0.0, bob, 0.0);
        if self.broom {
            let back = -vec3(self.vel.x, 0.0, self.vel.z).normalize_or(face);
            draw_line_3d(o + vec3(0.0, 0.45, 0.0) - back * 1.0, o + vec3(0.0, 0.45, 0.0) + back * 1.0, Color::from_rgba(150, 100, 52, 255));
            draw_cube(o + vec3(0.0, 0.45, 0.0) + back * 1.25, vec3(0.45, 0.4, 0.45), None, Color::from_rgba(214, 176, 92, 255));
        }
        draw_cube(o + vec3(0.0, 0.55, 0.0), vec3(0.62, 1.1, 0.62), None, robe);
        draw_cube(o + vec3(0.0, 1.28, 0.0), vec3(0.44, 0.4, 0.44), None, Color::from_rgba(236, 196, 160, 255));
        draw_cube(o + vec3(0.0, 1.12, 0.0) + face * 0.2, vec3(0.34, 0.3, 0.1), None, Color::from_rgba(230, 230, 236, 255));
        draw_cube(o + vec3(0.0, 1.5, 0.0), vec3(0.78, 0.08, 0.78), None, hat);
        for (k, s) in [0.5f32, 0.36, 0.22, 0.1].iter().enumerate() {
            draw_cube(o + vec3(0.0, 1.6 + k as f32 * 0.16, 0.0) - face * (k as f32 * 0.04), vec3(*s, 0.17, *s), None, hat);
        }
        draw_cube(o + vec3(0.0, 1.62, 0.0) + face * 0.26, vec3(0.1, 0.1, 0.02), None, Color::from_rgba(255, 220, 110, 255));
        // The staff, pointing where you aim.
        let (target, _) = self.aim();
        let h = self.hand();
        let d = (target - h).normalize_or_zero();
        draw_line_3d(h - d * 0.3, h + d * 1.0, Color::from_rgba(130, 86, 44, 255));
        draw_sphere(h + d * 1.05, 0.09, None, Color::from_rgba(170, 235, 255, 255));
    }

    fn hud(&self, ui: &mut Ui) {
        let (sw, sh) = (screen_width(), screen_height());
        let (cx, cy) = (sw * 0.5, sh * 0.5);
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            draw_line(cx + dx * 4.0, cy + dy * 4.0, cx + dx * 10.0, cy + dy * 10.0, 2.0, TEXT);
        }
        // Spells along the bottom, quietly.
        let n = SPELLS.len();
        let w = 118.0;
        let x0 = cx - (n as f32 * w) * 0.5;
        for (k, s) in SPELLS.iter().enumerate() {
            let sel = k == self.spell;
            let x = x0 + k as f32 * w;
            draw_rectangle(x + 2.0, sh - 40.0, w - 4.0, 26.0, Color::new(0.05, 0.045, 0.08, if sel { 0.85 } else { 0.5 }));
            if sel {
                draw_rectangle_lines(x + 2.0, sh - 40.0, w - 4.0, 26.0, 2.0, ACCENT);
            }
            ui.text_center(&format!("{} {}", k + 1, s.name()), x + w * 0.5, sh - 32.0, 1.0, if sel { ACCENT } else { MUTED });
        }
        let s = SPELLS[self.spell];
        let mut hint = s.hint().to_string();
        if s == Spell::Build {
            hint = format!("{}   -   {}", hint, BUILD[self.build].name().to_uppercase());
        }
        ui.text_center(&hint, cx, sh - 62.0, 1.0, TEXT);
        ui.text(&format!("{}  -  3D ISLAND", self.name.to_uppercase()), 18.0, 18.0, 1.0, TEXT);
        ui.text("WASD MOVE   SPACE JUMP/FLY   SHIFT SPRINT   M BROOM   RMB DIG   ESC PAUSE", 18.0, 34.0, 1.0, MUTED);
        if self.time < 6.0 && self.vox.dirty_left() > 40 {
            ui.text_center("BUILDING THE ISLAND...", cx, sh * 0.3, 2.0, TEXT);
        }
    }

    /// The pause overlay. Returns Menu when the player chose to leave.
    pub fn pause_menu(&mut self, ui: &mut Ui) -> Exit {
        if !self.paused {
            return Exit::Stay;
        }
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.02, 0.01, 0.05, 0.6));
        ui.text_center("PAUSED", sw * 0.5, sh * 0.3, 4.0, TEXT);
        if ui.button("RESUME", sw * 0.5, sh * 0.45, 260.0) {
            self.paused = false;
            self.last_mouse = mouse_position();
        }
        if ui.button("SAVE AND QUIT TO MENU", sw * 0.5, sh * 0.45 + 48.0, 260.0) {
            self.save();
            self.paused = false;
            set_cursor_grab(false);
            show_mouse(true);
            return Exit::Menu;
        }
        Exit::Stay
    }
}
