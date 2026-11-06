//! Turns the world into pixels: one RGBA image for the cells, a low-res light map drawn
//! with multiply blending (darkness underground), and an additive glow for hot things.

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams, ShaderSource};
use macroquad::prelude::*;

use crate::cell::FIRE_FUEL;
use crate::game::Game;
use crate::materials::Material as M;
use crate::noise::{fbm, hash3, value_noise};
use crate::world::{WH, WW};

/// Target view height in cells; the pixel scale is chosen to fit it.
pub const VIEW_CELLS_H: f32 = 180.0;
/// World cells per light-map texel.
const LS: i32 = 4;

/// A pixel buffer addressed in world coordinates.
pub struct Canvas<'a> {
    pub px: &'a mut [u8],
    pub w: usize,
    pub h: usize,
    pub x0: i32,
    pub y0: i32,
}

impl Canvas<'_> {
    #[inline]
    pub fn put(&mut self, gx: i32, gy: i32, rgb: [u8; 3]) {
        let (x, y) = (gx - self.x0, gy - self.y0);
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let o = (y as usize * self.w + x as usize) * 4;
        self.px[o] = rgb[0];
        self.px[o + 1] = rgb[1];
        self.px[o + 2] = rgb[2];
    }
}

/// What part of the world is on screen.
#[derive(Clone, Copy)]
pub struct View {
    pub scale: f32,
    pub left: f32,
    pub top: f32,
    pub x0: i32,
    pub y0: i32,
    pub w: usize,
    pub h: usize,
}

impl View {
    pub fn new(cx: f32, cy: f32, sw: f32, sh: f32) -> Self {
        let scale = (sh / VIEW_CELLS_H).floor().max(1.0);
        let (vw, vh) = (sw / scale, sh / scale);
        let (left, top) = (cx - vw * 0.5, cy - vh * 0.5);
        Self {
            scale,
            left,
            top,
            x0: left.floor() as i32,
            y0: top.floor() as i32,
            w: vw.ceil() as usize + 1,
            h: vh.ceil() as usize + 1,
        }
    }

    pub fn to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        (self.left + sx / self.scale, self.top + sy / self.scale)
    }
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
varying lowp vec2 uv;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}"#;

const FRAGMENT: &str = r#"#version 100
varying lowp vec2 uv;
uniform sampler2D Texture;
void main() {
    gl_FragColor = texture2D(Texture, uv);
}"#;

fn blend_material(blend: BlendState) -> Material {
    load_material(
        ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT },
        MaterialParams { pipeline_params: PipelineParams { color_blend: Some(blend), ..Default::default() }, ..Default::default() },
    )
    .expect("blend material")
}

pub struct Renderer {
    img: Image,
    tex: Texture2D,
    dark: Image,
    dark_tex: Texture2D,
    glow: Image,
    glow_tex: Texture2D,
    multiply: Material,
    additive: Material,
    em: Vec<[f32; 3]>,
    tmp: Vec<[f32; 3]>,
    far: Vec<f32>,
    near: Vec<f32>,
    gw: usize,
    gh: usize,
    pub fire_visible: usize,
    pub lava_visible: usize,
}

impl Renderer {
    pub fn new() -> Self {
        let img = Image::gen_image_color(4, 4, BLACK);
        let tex = Texture2D::from_image(&img);
        Self {
            dark: img.clone(),
            dark_tex: Texture2D::from_image(&img),
            glow: img.clone(),
            glow_tex: Texture2D::from_image(&img),
            img,
            tex,
            multiply: blend_material(BlendState::new(Equation::Add, BlendFactor::Zero, BlendFactor::Value(BlendValue::SourceColor))),
            additive: blend_material(BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One)),
            em: Vec::new(),
            tmp: Vec::new(),
            far: Vec::new(),
            near: Vec::new(),
            gw: 0,
            gh: 0,
            fire_visible: 0,
            lava_visible: 0,
        }
    }

    fn ensure(&mut self, w: usize, h: usize, gw: usize, gh: usize) {
        if self.img.width as usize != w || self.img.height as usize != h {
            self.img = Image::gen_image_color(w as u16, h as u16, BLACK);
            self.tex = Texture2D::from_image(&self.img);
            self.tex.set_filter(FilterMode::Nearest);
            self.far = vec![0.0; w];
            self.near = vec![0.0; w];
        }
        if self.gw != gw || self.gh != gh {
            self.gw = gw;
            self.gh = gh;
            self.dark = Image::gen_image_color(gw as u16, gh as u16, WHITE);
            self.glow = Image::gen_image_color(gw as u16, gh as u16, BLACK);
            self.dark_tex = Texture2D::from_image(&self.dark);
            self.glow_tex = Texture2D::from_image(&self.glow);
            self.dark_tex.set_filter(FilterMode::Linear);
            self.glow_tex.set_filter(FilterMode::Linear);
            self.em = vec![[0.0; 3]; gw * gh];
            self.tmp = vec![[0.0; 3]; gw * gh];
        }
    }

    pub fn draw(&mut self, game: &mut Game, view: &View, shake: (f32, f32), time: f32) {
        let (w, h) = (view.w, view.h);
        let lgx0 = view.x0.div_euclid(LS) - 2;
        let lgy0 = view.y0.div_euclid(LS) - 2;
        let gw = w / LS as usize + 5;
        let gh = h / LS as usize + 5;
        self.ensure(w, h, gw, gh);
        self.em.fill([0.0; 3]);

        let world = &game.world;
        let frame = world.frame;

        // Emissive cells over the light map's (slightly larger) area.
        for ly in 0..gh as i32 * LS {
            let gy = lgy0 * LS + ly;
            let wy = gy - world.oy;
            if wy < 0 || wy as usize >= WH {
                continue;
            }
            for lx in 0..gw as i32 * LS {
                let gx = lgx0 * LS + lx;
                let wx = gx - world.ox;
                if wx < 0 || wx as usize >= WW {
                    continue;
                }
                let c = &world.cells[wy as usize * WW + wx as usize];
                let e = match c.mat {
                    M::Fire => {
                        let k = if c.temp >= FIRE_FUEL { 1.0 } else { 0.7 };
                        [k, 0.48 * k, 0.12 * k]
                    }
                    m => match m.emissive() {
                        Some(e) => e,
                        None => continue,
                    },
                };
                let v = &mut self.em[(ly / LS) as usize * gw + (lx / LS) as usize];
                v[0] += e[0];
                v[1] += e[1];
                v[2] += e[2];
            }
        }

        // Sky parallax silhouettes per column.
        for vx in 0..w {
            let fx = view.left * 0.45 + vx as f32;
            let nx = view.left * 0.7 + vx as f32;
            self.far[vx] = -30.0 - fbm(fx * 0.006, 1.0, 3, 31) * 90.0;
            self.near[vx] = 12.0 - fbm(nx * 0.011, 2.0, 3, 37) * 70.0;
        }

        let (mut fire, mut lava) = (0, 0);
        let px = &mut self.img.bytes;
        for vy in 0..h {
            let gy = view.y0 + vy as i32;
            let wy = gy - world.oy;
            for vx in 0..w {
                let gx = view.x0 + vx as i32;
                let wx = gx - world.ox;
                let o = (vy * w + vx) * 4;
                if wx < 0 || wy < 0 || wx as usize >= WW || wy as usize >= WH {
                    px[o..o + 4].copy_from_slice(&[0, 0, 0, 255]);
                    continue;
                }
                let i = wy as usize * WW + wx as usize;
                let c = world.cells[i];
                let bg = || {
                    let wall = world.wall[i];
                    if wall == 0 {
                        sky(view, vx, vy, &self.far, &self.near, time)
                    } else {
                        let depth = (gy - world.surf[wx as usize]) as f32;
                        let base = if depth < 30.0 {
                            lerp3([76, 56, 44], [56, 52, 64], depth / 30.0)
                        } else {
                            lerp3([56, 52, 64], [38, 32, 46], (depth - 220.0) / 80.0)
                        };
                        shade(base, wall, 0.55)
                    }
                };
                let rgb = match c.mat {
                    M::Empty => bg(),
                    M::Smoke => lerp3(bg(), M::Smoke.base_color(), (c.life as f32 / 90.0).min(1.0) * 0.85),
                    M::Steam => lerp3(bg(), M::Steam.base_color(), (c.life as f32 / 60.0).min(1.0) * 0.6),
                    M::Glass => lerp3(bg(), shade(M::Glass.base_color(), c.shade, 0.1), 0.45),
                    M::Water => {
                        let tw = (hash3(gx, gy, (frame / 8) as u32) & 15) as u8;
                        lerp3(bg(), shade(M::Water.base_color(), c.shade.saturating_add(tw), 0.14), 0.88)
                    }
                    M::Fire => {
                        fire += 1;
                        let hh = hash3(gx, gy, (frame / 2) as u32 ^ ((c.life as u32) << 8));
                        let heat = if c.temp >= FIRE_FUEL { 0.3 } else { 0.0 } + (c.life as f32 / 25.0).min(1.0) * 0.35;
                        let v = (hh & 255) as f32 / 255.0 * 0.6 + heat;
                        FIRE_PAL[((v * 5.0) as usize).min(4)]
                    }
                    M::Lava => {
                        lava += 1;
                        let hh = hash3(gx.div_euclid(2), gy.div_euclid(2), (frame / 9) as u32);
                        let v = (hh & 255) as f32 / 255.0 * 0.6 + c.shade as f32 / 255.0 * 0.4;
                        lerp3([196, 46, 8], [255, 176, 48], v)
                    }
                    M::Gold if hash3(gx, gy, (frame / 6) as u32) % 97 == 0 => [255, 250, 215],
                    M::Plant if c.shade > 248 => [232, 84, 96],
                    M::Plant if c.shade > 241 => [250, 220, 84],
                    M::Oil if c.temp > 20 => shade(lerp3(M::Oil.base_color(), [150, 60, 30], c.temp as f32 / 120.0), c.shade, 0.3),
                    m => shade(m.base_color(), c.shade, m.variation()),
                };
                px[o] = rgb[0];
                px[o + 1] = rgb[1];
                px[o + 2] = rgb[2];
                px[o + 3] = 255;
            }
        }
        self.fire_visible = fire;
        self.lava_visible = lava;

        // Moving things on top.
        let mut cv = Canvas { px: &mut self.img.bytes, w, h, x0: view.x0, y0: view.y0 };
        game.fx.draw(&mut cv);
        game.spells.draw(&mut cv, time);
        let ready = game.cast_ready();
        let (ax, ay) = game.aim;
        game.player.draw(&mut cv, ax, ay, ready);

        // Dynamic lights.
        {
            let (gw_i, gh_i) = (gw as i32, gh as i32);
            let em = &mut self.em;
            let mut add = |x: f32, y: f32, c: [f32; 3]| {
                let (lx, ly) = ((x.floor() as i32).div_euclid(LS) - lgx0, (y.floor() as i32).div_euclid(LS) - lgy0);
                if lx >= 0 && ly >= 0 && lx < gw_i && ly < gh_i {
                    let v = &mut em[(ly * gw_i + lx) as usize];
                    v[0] += c[0];
                    v[1] += c[1];
                    v[2] += c[2];
                }
            };
            game.fx.lights(&mut add);
            game.spells.lights(&mut add);
        }
        for _ in 0..3 {
            blur(&self.em, &mut self.tmp, gw, gh, 1, 0);
            blur(&self.tmp, &mut self.em, gw, gh, 0, 1);
        }

        // Compose the darkness and glow maps.
        let (pcx, pcy) = game.player.center();
        let alive = !game.player.dead();
        for j in 0..gh {
            for i in 0..gw {
                let wx = (lgx0 + i as i32) * LS + LS / 2;
                let wy = (lgy0 + j as i32) * LS + LS / 2;
                let depth = (wy - world.surface_at(wx)) as f32;
                let amb = if depth < 4.0 { 1.0 } else { (1.0 - (depth - 4.0) / 70.0).max(0.2) };
                let torch = if alive {
                    let d = ((wx as f32 - pcx).powi(2) + (wy as f32 - pcy).powi(2)).sqrt();
                    (1.0 - d / 48.0).max(0.0).powf(1.3) * 0.8
                } else {
                    0.0
                };
                let e = self.em[j * gw + i];
                let o = (j * gw + i) * 4;
                let l = [
                    amb + torch + e[0] * 0.35,
                    amb + torch * 0.85 + e[1] * 0.35,
                    amb * (0.92 + 0.08 * amb) + torch * 0.65 + e[2] * 0.35,
                ];
                for k in 0..3 {
                    self.dark.bytes[o + k] = (l[k].min(1.0) * 255.0) as u8;
                    self.glow.bytes[o + k] = ((1.0 - (-e[k] * 0.06).exp()) * 200.0) as u8;
                }
                self.dark.bytes[o + 3] = 255;
                self.glow.bytes[o + 3] = 255;
            }
        }

        self.tex.update(&self.img);
        self.dark_tex.update(&self.dark);
        self.glow_tex.update(&self.glow);

        let s = view.scale;
        let (ox, oy) = ((view.x0 as f32 - view.left) * s + shake.0, (view.y0 as f32 - view.top) * s + shake.1);
        draw_texture_ex(&self.tex, ox, oy, WHITE, DrawTextureParams { dest_size: Some(vec2(w as f32 * s, h as f32 * s)), ..Default::default() });
        let lx = ((lgx0 * LS) as f32 - view.left) * s + shake.0;
        let ly = ((lgy0 * LS) as f32 - view.top) * s + shake.1;
        let size = Some(vec2((gw as i32 * LS) as f32 * s, (gh as i32 * LS) as f32 * s));
        gl_use_material(&self.multiply);
        draw_texture_ex(&self.dark_tex, lx, ly, WHITE, DrawTextureParams { dest_size: size, ..Default::default() });
        gl_use_material(&self.additive);
        draw_texture_ex(&self.glow_tex, lx, ly, WHITE, DrawTextureParams { dest_size: size, ..Default::default() });
        gl_use_default_material();
        game.spells.end_frame();
    }
}

const FIRE_PAL: [[u8; 3]; 5] = [[150, 24, 8], [222, 58, 12], [255, 122, 20], [255, 188, 48], [255, 240, 150]];

fn sky(view: &View, vx: usize, vy: usize, far: &[f32], near: &[f32], time: f32) -> [u8; 3] {
    let wy = view.top + vy as f32;
    let t = ((wy + 200.0) / 240.0).clamp(0.0, 1.0);
    let c = lerp3([62, 106, 178], [182, 210, 230], t);
    if view.top * 0.7 + vy as f32 >= near[vx] {
        return lerp3(c, [84, 112, 150], 0.88);
    }
    if view.top * 0.45 + vy as f32 >= far[vx] {
        return lerp3(c, [126, 154, 192], 0.8);
    }
    let n = value_noise((view.left * 0.25 + vx as f32 + time * 1.5) * 0.03, (view.top * 0.25 + vy as f32) * 0.1, 55);
    if n > 0.6 {
        lerp3(c, [244, 246, 250], ((n - 0.6) * 3.0).min(0.85))
    } else {
        c
    }
}

fn blur(src: &[[f32; 3]], dst: &mut [[f32; 3]], w: usize, h: usize, sx: i32, sy: i32) {
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut acc = [0.0f32; 3];
            for k in -2..=2 {
                let (xx, yy) = (x + k * sx, y + k * sy);
                if xx < 0 || yy < 0 || xx >= w as i32 || yy >= h as i32 {
                    continue;
                }
                let s = src[yy as usize * w + xx as usize];
                acc[0] += s[0];
                acc[1] += s[1];
                acc[2] += s[2];
            }
            dst[y as usize * w + x as usize] = [acc[0] * 0.22, acc[1] * 0.22, acc[2] * 0.22];
        }
    }
}

#[inline]
pub fn shade(base: [u8; 3], s: u8, amt: f32) -> [u8; 3] {
    let f = 1.0 + (s as f32 / 255.0 - 0.5) * amt;
    [(base[0] as f32 * f).min(255.0) as u8, (base[1] as f32 * f).min(255.0) as u8, (base[2] as f32 * f).min(255.0) as u8]
}

#[inline]
pub fn lerp3(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}
