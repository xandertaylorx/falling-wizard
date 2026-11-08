//! Rendering. The CPU writes two images for the visible area (plus a margin):
//! colour (albedo) and light (emission + opacity). On the GPU, a ray-marching pass
//! casts rays from every cell through the light image, so sky light pours into cave
//! mouths, lava lights up caverns, and terrain casts real shadows. A blur pass smooths
//! the rays, and a composite pass combines everything with tone mapping, bloom and
//! a vignette.

use macroquad::miniquad::{PipelineParams, ShaderSource, UniformType};
use macroquad::prelude::*;

use crate::cell::FIRE_FUEL;
use crate::game::Game;
use crate::materials::{Kind, Material as M};
use crate::noise::{fbm, hash3, value_noise};
use crate::world::{WH, WW};

pub const VIEW_CELLS_H: f32 = 180.0;
/// Extra cells rendered around the view so off-screen lights still shine in.
const MARGIN: i32 = 28;
/// Emission is stored divided by this so bright lights fit in 8 bits.
const E: f32 = 4.0;

pub struct Canvas<'a> {
    pub px: &'a mut [u8],
    pub w: usize,
    pub h: usize,
    pub x0: i32,
    pub y0: i32,
}

impl Canvas<'_> {
    #[inline]
    pub fn blend(&mut self, gx: i32, gy: i32, rgb: [u8; 3], a: f32) {
        let (x, y) = (gx - self.x0, gy - self.y0);
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let o = (y as usize * self.w + x as usize) * 4;
        for k in 0..3 {
            self.px[o + k] = (self.px[o + k] as f32 * (1.0 - a) + rgb[k] as f32 * a) as u8;
        }
    }

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
    pub fn new(cx: f32, cy: f32, sw: f32, sh: f32, zoom: f32) -> Self {
        let scale = (sh / VIEW_CELLS_H).floor().max(1.0) * zoom;
        let (vw, vh) = (sw / scale, sh / scale);
        let (left, top) = (cx - vw * 0.5, cy - vh * 0.5);
        Self { scale, left, top, x0: left.floor() as i32, y0: top.floor() as i32, w: vw.ceil() as usize + 1, h: vh.ceil() as usize + 1 }
    }

    pub fn to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        (self.left + sx / self.scale, self.top + sy / self.scale)
    }

    pub fn to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        ((wx - self.left) * self.scale, (wy - self.top) * self.scale)
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

const GI_FRAG: &str = r#"#version 100
precision highp float;
varying lowp vec2 uv;
uniform sampler2D Texture;
uniform vec2 u_texel;
uniform float u_rays;
uniform float u_seed;
float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233)) + u_seed) * 43758.5453); }
void main() {
    vec2 p0 = uv;
    vec3 acc = vec3(0.0);
    float jitter = hash(floor(uv / u_texel));
    for (int r = 0; r < 40; r++) {
        if (float(r) >= u_rays) break;
        float a = (float(r) + jitter) * 6.2831853 / u_rays;
        vec2 dir = vec2(cos(a), sin(a)) * u_texel;
        vec2 p = p0;
        float trans = 1.0;
        float stp = 1.0;
        for (int s = 0; s < 40; s++) {
            p += dir * stp;
            if (p.x < 0.0 || p.y < 0.0 || p.x > 1.0 || p.y > 1.0) break;
            vec4 c = texture2D(Texture, p);
            acc += c.rgb * trans * min(stp, 2.0);
            trans *= pow(1.0 - c.a, stp);
            if (trans < 0.02) break;
            stp *= 1.08;
        }
    }
    gl_FragColor = vec4(acc / u_rays * 0.5, 1.0);
}"#;

const BLUR_FRAG: &str = r#"#version 100
precision mediump float;
varying lowp vec2 uv;
uniform sampler2D Texture;
uniform vec2 u_texel;
void main() {
    vec2 o = u_texel * 1.5;
    vec3 c = texture2D(Texture, uv).rgb * 0.25;
    c += (texture2D(Texture, uv + vec2(o.x, 0.0)).rgb + texture2D(Texture, uv - vec2(o.x, 0.0)).rgb
        + texture2D(Texture, uv + vec2(0.0, o.y)).rgb + texture2D(Texture, uv - vec2(0.0, o.y)).rgb) * 0.125;
    c += (texture2D(Texture, uv + o).rgb + texture2D(Texture, uv - o).rgb
        + texture2D(Texture, uv + vec2(o.x, -o.y)).rgb + texture2D(Texture, uv + vec2(-o.x, o.y)).rgb) * 0.0625;
    gl_FragColor = vec4(c, 1.0);
}"#;

const COMP_FRAG: &str = r#"#version 100
precision mediump float;
varying lowp vec2 uv;
uniform sampler2D Texture;
uniform sampler2D Light;
uniform sampler2D Emit;
uniform vec2 u_size;
uniform vec2 u_player;
uniform float u_flash;
uniform float u_ambient;
uniform float u_flip;
void main() {
    vec4 alb = texture2D(Texture, uv);
    vec4 em = texture2D(Emit, uv);
    vec2 luv = vec2(uv.x, mix(uv.y, 1.0 - uv.y, u_flip));
    vec3 L = texture2D(Light, luv).rgb * 8.0;
    float d = length((uv - u_player) * u_size);
    float torch = max(0.0, 1.0 - d / 64.0);
    L += vec3(1.0, 0.86, 0.66) * torch * torch * 0.95;
    L = max(L, vec3(alb.a * 0.97, alb.a, alb.a * 1.03));
    L += vec3(u_flash);
    L = max(L, vec3(u_ambient));
    vec3 col;
    if (abs(em.a - 0.5) < 0.03) {
        col = alb.rgb + max(L - 1.0, 0.0) * 0.15;
    } else {
        col = alb.rgb * L * 1.5;
        col += em.rgb * 3.2 * (0.6 + 0.4 * alb.rgb);
        col += max(L - 1.1, 0.0) * 0.18;
        col = 1.0 - exp(-col * 1.35);
        col = mix(vec3(dot(col, vec3(0.299, 0.587, 0.114))), col, 1.12);
    }
    vec2 q = uv * 2.0 - 1.0;
    float vig = 1.0 - 0.18 * pow(clamp(length(q * vec2(0.85, 1.0)), 0.0, 1.5), 2.6);
    gl_FragColor = vec4(col * vig, 1.0);
}"#;

fn material(frag: &str, uniforms: &[(&str, UniformType)], textures: &[&str]) -> Material {
    load_material(
        ShaderSource::Glsl { vertex: VERTEX, fragment: frag },
        MaterialParams {
            pipeline_params: PipelineParams::default(),
            uniforms: uniforms.iter().map(|(n, t)| UniformDesc::new(n, *t)).collect(),
            textures: textures.iter().map(|s| s.to_string()).collect(),
        },
    )
    .unwrap_or_else(|e| panic!("shader failed to compile: {e:?}"))
}

pub struct Renderer {
    albedo: Image,
    emit: Image,
    alb_tex: Texture2D,
    emit_tex: Texture2D,
    light_rt: RenderTarget,
    blur_rt: RenderTarget,
    gi: Material,
    blur: Material,
    comp: Material,
    far: Vec<f32>,
    near: Vec<f32>,
    w: usize,
    h: usize,
    pub fire_visible: usize,
    pub lava_visible: usize,
    pub rays: f32,
    pub flip: f32,
    lens_tmp: Vec<u8>,
}

impl Renderer {
    pub fn new() -> Self {
        let img = Image::gen_image_color(4, 4, BLACK);
        Self {
            alb_tex: Texture2D::from_image(&img),
            emit_tex: Texture2D::from_image(&img),
            albedo: img.clone(),
            emit: img,
            light_rt: render_target(4, 4),
            blur_rt: render_target(4, 4),
            gi: material(GI_FRAG, &[("u_texel", UniformType::Float2), ("u_rays", UniformType::Float1), ("u_seed", UniformType::Float1)], &[]),
            blur: material(BLUR_FRAG, &[("u_texel", UniformType::Float2)], &[]),
            comp: material(
                COMP_FRAG,
                &[
                    ("u_size", UniformType::Float2),
                    ("u_player", UniformType::Float2),
                    ("u_flash", UniformType::Float1),
                    ("u_ambient", UniformType::Float1),
                    ("u_flip", UniformType::Float1),
                ],
                &["Light", "Emit"],
            ),
            far: Vec::new(),
            near: Vec::new(),
            w: 0,
            h: 0,
            fire_visible: 0,
            lava_visible: 0,
            rays: 24.0,
            flip: 0.0,
            lens_tmp: Vec::new(),
        }
    }

    fn ensure(&mut self, w: usize, h: usize) {
        if self.w == w && self.h == h {
            return;
        }
        self.w = w;
        self.h = h;
        self.albedo = Image::gen_image_color(w as u16, h as u16, BLACK);
        self.emit = Image::gen_image_color(w as u16, h as u16, BLACK);
        self.alb_tex = Texture2D::from_image(&self.albedo);
        self.alb_tex.set_filter(FilterMode::Nearest);
        self.emit_tex = Texture2D::from_image(&self.emit);
        self.emit_tex.set_filter(FilterMode::Nearest);
        self.light_rt = render_target(w as u32, h as u32);
        self.light_rt.texture.set_filter(FilterMode::Linear);
        self.blur_rt = render_target(w as u32, h as u32);
        self.blur_rt.texture.set_filter(FilterMode::Linear);
        self.far = vec![0.0; w];
        self.near = vec![0.0; w];
    }

    pub fn draw(&mut self, game: &mut Game, view: &View, shake: (f32, f32), time: f32) {
        let (x0, y0) = (view.x0 - MARGIN, view.y0 - MARGIN);
        let (w, h) = (view.w + 2 * MARGIN as usize, view.h + 2 * MARGIN as usize);
        self.ensure(w, h);

        let world = &game.world;
        let frame = world.frame;
        let clouds = game.weather.clouds;
        let day = game.weather.daylight() * (1.0 - clouds * 0.35);
        let tod = game.weather.time;
        let rain = game.weather.rain;

        for vx in 0..w {
            let fx = view.left * 0.45 + (vx as i32 - MARGIN) as f32;
            let nx = view.left * 0.7 + (vx as i32 - MARGIN) as f32;
            self.far[vx] = -30.0 - fbm(fx * 0.006, 1.0, 3, 31) * 90.0;
            self.near[vx] = 12.0 - fbm(nx * 0.011, 2.0, 3, 37) * 70.0;
        }
        let wind = game.weather.wind_now();
        let sway_amp = 0.55 + (wind.abs() / 45.0).min(1.2);
        let planet = &game.world.gen.planet;
        let tint = planet.tint;
        let tinted = !planet.is_earth();
        let scene = planet.sky_scene();
        let atmo = world.climate.atmo as f32;
        let sky_ctx = SkyCtx { view: *view, day, tod, rain, clouds, top: planet.sky_top, horizon: planet.sky_horizon, time, far: &self.far, near: &self.near, atmo, scene: &scene };
        let sky_light = [0.92 * day + 0.04, 0.95 * day + 0.05, 1.0 * day + 0.09];
        // Void wastelands on screen: their sky turns to open space.
        let here = (game.planet.gx, game.planet.gy);
        let (vl, vt, vr, vb) = (x0 as f32, y0 as f32, (x0 + w as i32) as f32, (y0 + h as i32) as f32);
        let blights: Vec<&crate::rift::Blight> = game
            .blights
            .iter()
            .filter(|b| b.planet == here && b.x + b.r * 1.2 > vl && b.x - b.r * 1.2 < vr && b.y + b.r * 1.2 > vt && b.y - b.r * 1.2 < vb)
            .collect();
        let void_at = |gx: i32, gy: i32| -> f32 { blights.iter().map(|b| b.cover(gx as f32, gy as f32)).fold(0.0f32, f32::max) };

        let (mut fire, mut lava) = (0, 0);
        let alb = &mut self.albedo.bytes;
        let emit = &mut self.emit.bytes;
        for vy in 0..h {
            let gy = y0 + vy as i32;
            let wy = gy - world.oy;
            for vx in 0..w {
                let gx = x0 + vx as i32;
                let wx = gx - world.ox;
                let o = (vy * w + vx) * 4;
                if wx < 0 || wy < 0 || wx as usize >= WW || wy as usize >= WH {
                    alb[o..o + 4].copy_from_slice(&[0, 0, 0, 255]);
                    emit[o..o + 4].copy_from_slice(&[0, 0, 0, 255]);
                    continue;
                }
                let i = wy as usize * WW + wx as usize;
                let c = world.cells[i];
                let wall = world.wall[i];
                // Background foliage sways in the wind.
                let mut back = c.back;
                if matches!(back, M::Leaves | M::Plant) || (back == M::Empty && wall == 0) {
                    let sw = ((time * 1.7 + gx as f32 * 0.05 + gy as f32 * 0.31).sin() * sway_amp).round() as i32;
                    let sx = wx + sw;
                    if sw != 0 && sx >= 0 && (sx as usize) < WW {
                        let b = world.cells[wy as usize * WW + sx as usize].back;
                        if matches!(b, M::Leaves | M::Plant | M::Empty) && matches!(back, M::Leaves | M::Plant | M::Empty) {
                            back = b;
                        }
                    }
                }
                let tree = if back != M::Empty {
                    let hsh = if back == M::Wood { hash3(gx, gy.div_euclid(3), 7) } else { hash3(gx, gy, 5) };
                    let base = match back {
                        M::Plant if hsh % 41 == 0 => [232, 84, 96],
                        M::Plant if hsh % 37 == 0 => [250, 220, 84],
                        M::Leaves if gy < -1000 => [60, 60, 60],
                        m => m.base_color(),
                    };
                    let col = shade(base, hsh as u8, back.variation());
                    Some([(col[0] as f32 * 0.84) as u8, (col[1] as f32 * 0.84) as u8, (col[2] as f32 * 0.84) as u8])
                } else {
                    None
                };
                let is_sky = c.mat == M::Empty && wall == 0 && tree.is_none();
                let bg = || {
                    if let Some(t) = tree {
                        t
                    } else if wall == 0 {
                        let base = sky(&sky_ctx, vx, vy, gy);
                        let v = if blights.is_empty() { 0.0 } else { void_at(gx, gy) };
                        if v > 0.0 {
                            let sp = crate::rift::void_sky(gx, gy, time);
                            lerp3(base, [sp[0].min(255.0) as u8, sp[1].min(255.0) as u8, sp[2].min(255.0) as u8], v)
                        } else {
                            base
                        }
                    } else {
                        let depth = (gy - world.surf[wx as usize]) as f32;
                        let base = if depth < 30.0 {
                            lerp3([86, 64, 50], [64, 60, 72], depth / 30.0)
                        } else {
                            lerp3([64, 60, 72], [44, 38, 54], (depth - 220.0) / 80.0)
                        };
                        let mut col = shade(base, wall, 0.5);
                        let solid_n = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dy)| world.mat(gx + dx, gy + dy).solid()).count();
                        if solid_n > 0 {
                            let k = 1.0 - solid_n as f32 * 0.11;
                            col = [(col[0] as f32 * k) as u8, (col[1] as f32 * k) as u8, (col[2] as f32 * k) as u8];
                        }
                        col
                    }
                };
                // Colour, emitted light (0..E) and opacity.
                let (mut rgb, mut e, a): ([u8; 3], [f32; 3], f32) = match c.mat {
                    M::Empty => {
                        let (e, a) = match back {
                            M::Leaves => ([0.0; 3], 0.3),
                            M::Wood => ([0.0; 3], 0.45),
                            M::Plant => ([0.0; 3], 0.12),
                            M::Mushroom => ([0.2, 0.8, 0.75], 0.15),
                            M::Rune => ([0.9, 0.6, 2.0], 0.3),
                            M::Crystal => ([0.35, 0.8, 1.2], 0.3),
                            M::PortalStone | M::Basalt | M::Brick | M::Gold => ([0.0; 3], 0.45),
                            _ if is_sky => (sky_light, 0.5),
                            _ => ([0.0; 3], 0.0),
                        };
                        (bg(), e, a)
                    }
                    M::Smoke => (lerp3(bg(), M::Smoke.base_color(), (c.life as f32 / 90.0).min(1.0) * 0.85), [0.0; 3], 0.12),
                    M::Steam => (lerp3(bg(), M::Steam.base_color(), (c.life as f32 / 60.0).min(1.0) * 0.6), [0.0; 3], 0.05),
                    M::Gas => (lerp3(bg(), M::Gas.base_color(), 0.22), [0.0; 3], 0.02),
                    M::Glass => (lerp3(bg(), shade(M::Glass.base_color(), c.shade, 0.1), 0.45), [0.0; 3], 0.05),
                    M::Water => {
                        let tw = (hash3(gx, gy, (frame / 8) as u32) & 15) as u8;
                        let mut col = lerp3(bg(), shade(M::Water.base_color(), c.shade.saturating_add(tw), 0.14), 0.82);
                        if world.mat(gx, gy - 1) == M::Empty {
                            col = lerp3(col, [200, 230, 255], 0.35);
                        }
                        (col, [0.0; 3], 0.1)
                    }
                    M::Fire => {
                        fire += 1;
                        let hh = hash3(gx, gy, (frame / 2) as u32 ^ ((c.life as u32) << 8));
                        let heat = if c.temp >= FIRE_FUEL { 0.3 } else { 0.0 } + (c.life as f32 / 25.0).min(1.0) * 0.35;
                        let v = (hh & 255) as f32 / 255.0 * 0.6 + heat;
                        let col = FIRE_PAL[((v * 5.0) as usize).min(4)];
                        (col, [col[0] as f32 / 255.0 * 1.6, col[1] as f32 / 255.0 * 1.2, col[2] as f32 / 255.0 * 0.6], 0.15)
                    }
                    M::Lava => {
                        lava += 1;
                        let hh = hash3(gx.div_euclid(2), gy.div_euclid(2), (frame / 9) as u32);
                        let v = (hh & 255) as f32 / 255.0 * 0.6 + c.shade as f32 / 255.0 * 0.4;
                        (lerp3([196, 46, 8], [255, 176, 48], v), [2.0, 0.8, 0.15], 1.0)
                    }
                    M::Gold if hash3(gx, gy, (frame / 6) as u32) % 97 == 0 => ([255, 250, 215], [0.6, 0.55, 0.3], 1.0),
                    M::Portal => {
                        let hue = c.temp as f32 / 255.0;
                        let (dxp, dyp) = (gx as f32, gy as f32);
                        let swirl = ((dxp * 0.45 + dyp * 0.3 + time * 5.0).sin() + (dyp * 0.5 - time * 3.0 + dxp * 0.2).sin()) * 0.25 + 0.5;
                        let spark = hash3(gx, gy, (frame / 3) as u32) % 23 == 0;
                        let col = if spark { [255, 255, 255] } else { crate::planets::hue_rgb(hue + (swirl - 0.5) * 0.12, 0.45 + swirl * 0.4, 0.65 + swirl * 0.35) };
                        (col, [col[0] as f32 / 255.0 * 2.2, col[1] as f32 / 255.0 * 2.2, col[2] as f32 / 255.0 * 2.2], 0.15)
                    }
                    M::Plant if c.shade > 248 => ([232, 84, 96], [0.0; 3], 0.3),
                    M::Plant if c.shade > 241 => ([250, 220, 84], [0.0; 3], 0.3),
                    M::Oil if c.temp > 20 => (shade(lerp3(M::Oil.base_color(), [150, 60, 30], c.temp as f32 / 120.0), c.shade, 0.3), [0.0; 3], 0.6),
                    m => {
                        let col = shade(m.base_color(), c.shade, m.variation());
                        let em = m.emissive().map(|e| [e[0] * 1.5, e[1] * 1.5, e[2] * 1.5]).unwrap_or([0.0; 3]);
                        let a = match m {
                            M::Leaves => 0.45,
                            M::Plant | M::Mushroom | M::Sprout => 0.3,
                            M::Cloud => 0.15,
                            M::Acid | M::Blood | M::Goo | M::Oil => 0.35,
                            _ => 1.0,
                        };
                        (col, em, a)
                    }
                };
                // Bevel solid edges: lit tops, shaded undersides.
                if a >= 1.0 && matches!(c.mat.kind(), Kind::Static | Kind::Powder) {
                    let up = world.mat(gx, gy - 1);
                    let dn = world.mat(gx, gy + 1);
                    let k = if !up.solid() && up.kind() != Kind::Liquid {
                        1.22
                    } else if !dn.solid() && dn.kind() != Kind::Liquid {
                        0.78
                    } else if !world.mat(gx - 1, gy).solid() || !world.mat(gx + 1, gy).solid() {
                        1.06
                    } else {
                        1.0
                    };
                    rgb = [(rgb[0] as f32 * k).min(255.0) as u8, (rgb[1] as f32 * k).min(255.0) as u8, (rgb[2] as f32 * k).min(255.0) as u8];
                }
                if is_sky {
                    e = sky_light;
                }
                let terrain = (matches!(c.mat.kind(), Kind::Static | Kind::Powder) && c.mat.emissive().is_none() && !c.mat.indestructible())
                    || (c.mat == M::Empty && wall > 0 && tree.is_none());
                if tinted && terrain {
                    rgb = [(rgb[0] as f32 * tint[0]).min(255.0) as u8, (rgb[1] as f32 * tint[1]).min(255.0) as u8, (rgb[2] as f32 * tint[2]).min(255.0) as u8];
                }
                // Ambient sky light: full outdoors, fading with depth below the surface.
                let depth = (gy - world.surf[wx as usize]) as f32;
                let open = if gy < -260 { 0.9 } else { day * 0.85 + 0.15 };
                let amb = if depth <= 3.0 { 1.0 } else { (1.0 - (depth - 3.0) / 70.0).max(0.0) } * open;
                alb[o] = rgb[0];
                alb[o + 1] = rgb[1];
                alb[o + 2] = rgb[2];
                alb[o + 3] = (amb * 255.0) as u8;
                emit[o] = ((e[0] / E).min(1.0) * 255.0) as u8;
                emit[o + 1] = ((e[1] / E).min(1.0) * 255.0) as u8;
                emit[o + 2] = ((e[2] / E).min(1.0) * 255.0) as u8;
                emit[o + 3] = (a * 255.0) as u8;
            }
        }
        self.fire_visible = fire;
        self.lava_visible = lava;

        // Moving things.
        {
            let mut cv = Canvas { px: &mut self.albedo.bytes, w, h, x0, y0 };
            game.fx.draw(&mut cv);
            game.weather.draw(&mut cv);
            let (ppx, ppy) = game.player.center();
            game.mobs.draw(&mut cv, ppx, ppy);
            game.spells.draw(&mut cv, time);
            let ready = game.cast_ready();
            let (ax, ay) = game.aim;
            game.player.draw(&mut cv, ax, ay, ready);
        }
        // Black holes bend the light around them.
        for hole in &game.spells.holes {
            lens(&mut self.albedo.bytes, &mut self.lens_tmp, w, h, hole.x - x0 as f32, hole.y - y0 as f32, hole.reach() * 1.3, hole.core(), hole.age);
        }
        // Rifts: windows onto another galaxy.
        let (ppx, ppy) = game.player.center();
        for r in game.rifts.iter().filter(|r| r.planet == (game.planet.gx, game.planet.gy)) {
            draw_rift(&mut self.albedo.bytes, &mut self.emit.bytes, &mut self.lens_tmp, w, h, (x0, y0), r, time, (ppx, ppy));
        }
        {
            let emit = &mut self.emit.bytes;
            let mut add = |x: f32, y: f32, c: [f32; 3]| {
                let (lx, ly) = (x.floor() as i32 - x0, y.floor() as i32 - y0);
                if lx < 0 || ly < 0 || lx as usize >= w || ly as usize >= h {
                    return;
                }
                let o = (ly as usize * w + lx as usize) * 4;
                for k in 0..3 {
                    let v = emit[o + k] as f32 + (c[k] * 0.35 / E).min(1.0) * 255.0;
                    emit[o + k] = v.min(255.0) as u8;
                }
            };
            game.fx.lights(&mut add);
            game.spells.lights(&mut add);
            game.mobs.lights(&mut add);
            game.weather.lights(&mut add);
        }

        self.alb_tex.update(&self.albedo);
        self.emit_tex.update(&self.emit);

        // GPU: rays -> blur -> composite.
        let (wf, hf) = (w as f32, h as f32);
        let texel = vec2(1.0 / wf, 1.0 / hf);
        let cam = |rt: &RenderTarget| {
            let mut c = Camera2D::from_display_rect(Rect::new(0.0, 0.0, wf, hf));
            c.render_target = Some(rt.clone());
            c
        };
        set_camera(&cam(&self.light_rt));
        clear_background(BLACK);
        gl_use_material(&self.gi);
        self.gi.set_uniform("u_texel", texel);
        self.gi.set_uniform("u_rays", self.rays);
        self.gi.set_uniform("u_seed", (frame % 64) as f32 * 0.37);
        draw_texture_ex(&self.emit_tex, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(wf, hf)), ..Default::default() });
        set_camera(&cam(&self.blur_rt));
        clear_background(BLACK);
        gl_use_material(&self.blur);
        self.blur.set_uniform("u_texel", texel);
        draw_texture_ex(&self.light_rt.texture, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(wf, hf)), ..Default::default() });
        gl_use_default_material();
        set_default_camera();

        let s = view.scale;
        let ox = (x0 as f32 - view.left) * s + shake.0;
        let oy = (y0 as f32 - view.top) * s + shake.1;
        let (ppx, ppy) = game.player.center();
        let pu = if game.player.dead() { vec2(-10.0, -10.0) } else { vec2((ppx - x0 as f32) / wf, (ppy - y0 as f32) / hf) };
        gl_use_material(&self.comp);
        self.comp.set_texture("Light", self.blur_rt.texture.clone());
        self.comp.set_texture("Emit", self.emit_tex.clone());
        self.comp.set_uniform("u_size", vec2(wf, hf));
        self.comp.set_uniform("u_player", pu);
        self.comp.set_uniform("u_flash", game.fx.flash);
        self.comp.set_uniform("u_ambient", 0.15f32);
        self.comp.set_uniform("u_flip", self.flip);
        draw_texture_ex(&self.alb_tex, ox, oy, WHITE, DrawTextureParams { dest_size: Some(vec2(wf * s, hf * s)), ..Default::default() });
        gl_use_default_material();
        game.fx.draw_fine(view.left - shake.0 / s, view.top - shake.1 / s, s);
        // Fog and sandstorm haze near the surface.
        let (_, ppy) = game.player.center();
        let near_surface = 1.0 - ((ppy - game.world.surface_at(ppx as i32) as f32) / 60.0).clamp(0.0, 1.0);
        let fog = game.weather.fog * near_surface;
        if fog > 0.02 {
            let (sw, sh) = (screen_width(), screen_height());
            let n = 64;
            for k in 0..n {
                let a = fog * 0.32 * (0.55 + k as f32 / n as f32 * 0.48);
                let (y0, y1) = ((sh * k as f32 / n as f32).floor(), (sh * (k + 1) as f32 / n as f32).floor());
                draw_rectangle(0.0, y0, sw, y1 - y0, Color::new(0.78, 0.8, 0.84, a));
            }
        }
        if game.weather.sand > 0.02 {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.86, 0.7, 0.45, game.weather.sand * 0.28));
        }
        game.spells.end_frame();
    }
}

/// Swirl and magnify the pixels around a black hole, blacken its core and draw a
/// bright photon ring, like light bending around it.
#[allow(clippy::too_many_arguments)]
fn lens(px: &mut [u8], tmp: &mut Vec<u8>, w: usize, h: usize, cx: f32, cy: f32, r: f32, core: f32, age: f32) {
    tmp.clear();
    tmp.extend_from_slice(px);
    let ri = r.ceil() as i32;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (x, y) = (cx as i32 + dx, cy as i32 + dy);
            if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                continue;
            }
            let d = ((dx * dx + dy * dy) as f32).sqrt();
            if d > r {
                continue;
            }
            let o = (y as usize * w + x as usize) * 4;
            if d < core {
                px[o..o + 3].copy_from_slice(&[0, 0, 0]);
                continue;
            }
            let ring = (d - core - 1.2).abs();
            if ring < 1.0 && core > 0.5 {
                let a = (dy as f32).atan2(dx as f32);
                let hot = 0.6 + 0.4 * (a * 2.0 + age * 5.0).sin();
                px[o..o + 3].copy_from_slice(&[(255.0 * hot) as u8, (190.0 * hot) as u8, 255]);
                continue;
            }
            // Sample from further out and rotated: magnification plus swirl.
            let t = 1.0 - d / r;
            let src_d = d + t * t * (r - d) * 0.6 + core;
            let ang = (dy as f32).atan2(dx as f32) + t * t * 2.4 + age * 0.4 * t;
            let (sx, sy) = ((cx + ang.cos() * src_d) as i32, (cy + ang.sin() * src_d) as i32);
            if sx < 0 || sy < 0 || sx as usize >= w || sy as usize >= h {
                continue;
            }
            let so = (sy as usize * w + sx as usize) * 4;
            let dim = 1.0 - t * t * 0.55;
            let tint = t * t * 0.35;
            px[o] = (tmp[so] as f32 * dim + 120.0 * tint) as u8;
            px[o + 1] = (tmp[so + 1] as f32 * dim + 40.0 * tint) as u8;
            px[o + 2] = (tmp[so + 2] as f32 * dim + 200.0 * tint) as u8;
        }
    }
}

/// A rift: the galaxy inside (shifting with parallax as you move, like a window),
/// the surrounding world mirrored into its rim, a shimmering edge, and light bent
/// around the outside with a hint of prism.
#[allow(clippy::too_many_arguments)]
fn draw_rift(alb: &mut [u8], emit: &mut [u8], tmp: &mut Vec<u8>, w: usize, h: usize, o: (i32, i32), r: &crate::rift::Rift, t: f32, eye: (f32, f32)) {
    let (hw, hh) = r.size();
    if hw < 0.3 {
        return;
    }
    tmp.clear();
    tmp.extend_from_slice(alb);
    let (cx, cy) = (r.x - o.0 as f32, r.y - o.1 as f32);
    let (par_x, par_y) = ((eye.0 - r.x) * 0.3, (eye.1 - r.y) * 0.3);
    let ex = (hw * 2.6) as i32 + 4;
    let ey = (hh * 1.5) as i32 + 4;
    let sample = |x: f32, y: f32| -> Option<usize> {
        let (sx, sy) = (x as i32, y as i32);
        if sx < 0 || sy < 0 || sx as usize >= w || sy as usize >= h {
            None
        } else {
            Some((sy as usize * w + sx as usize) * 4)
        }
    };
    // Shattering: jagged glowing cracks race outward through the world.
    if r.shatter >= 0.0 {
        let k = (r.shatter / crate::rift::SHATTER_TIME).min(1.0);
        for c in 0..14 {
            let mut a = (crate::noise::hash3(c, 1, r.seed) % 1000) as f32 / 1000.0 * std::f32::consts::TAU;
            let len = k.powf(1.4) * (hh * 1.8 + (crate::noise::hash3(c, 2, r.seed) % 20) as f32);
            let (mut x, mut y) = (cx, cy);
            let mut s = 0.0;
            while s < len {
                a += ((crate::noise::hash3(c, s as i32, r.seed) % 100) as f32 / 100.0 - 0.5) * 0.7;
                x += a.cos();
                y += a.sin();
                s += 1.0;
                let (ix, iy) = (x as i32, y as i32);
                if ix < 0 || iy < 0 || ix as usize >= w || iy as usize >= h {
                    break;
                }
                let i = (iy as usize * w + ix as usize) * 4;
                let hot = 1.0 - s / len.max(1.0) * 0.5;
                alb[i..i + 3].copy_from_slice(&[(255.0 * hot) as u8, (190.0 * hot) as u8, 255]);
                alb[i + 3] = 255;
                emit[i] = emit[i].max((200.0 * hot) as u8);
                emit[i + 1] = emit[i + 1].max((90.0 * hot) as u8);
                emit[i + 2] = emit[i + 2].max(255);
            }
        }
    }
    let white_out = if r.shatter >= 0.0 { (r.shatter / crate::rift::SHATTER_TIME).powi(2) } else { 0.0 };
    for dy in -ey..=ey {
        for dx in -ex..=ex {
            let (x, y) = (cx as i32 + dx, cy as i32 + dy);
            if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                continue;
            }
            let (wx, wy) = (x as f32 + o.0 as f32 + 0.5, y as f32 + o.1 as f32 + 0.5);
            let d = r.dist(wx, wy, t);
            let i = (y as usize * w + x as usize) * 4;
            if d < 1.0 {
                let (u, v) = (wx - r.x, wy - r.y);
                let mut col = if r.white {
                    let k = 1.0 - d * 0.5;
                    [255.0 * k + 40.0, 245.0 * k + 30.0, 220.0 * k + 20.0]
                } else {
                    crate::rift::galaxy(u * 1.4 - par_x, v * 1.4 - par_y, t, r.seed)
                };
                // Mirror: the world just outside the edge, reflected and swirled inward.
                let fres = d.powi(3) * if r.white { 0.0 } else { 0.45 };
                if fres > 0.01 {
                    let mx = cx + (dx as f32).signum() * (hw * 2.0 - (dx as f32).abs()) + (t * 0.8 + v * 0.2).sin() * 1.5;
                    if let Some(si) = sample(mx, y as f32) {
                        for k in 0..3 {
                            col[k] = col[k] * (1.0 - fres) + tmp[si + k] as f32 * fres * 1.2;
                        }
                    }
                }
                // Shimmering rim.
                let rim = ((d - 0.78) / 0.22).clamp(0.0, 1.0).powi(2);
                let flick = 0.75 + 0.25 * (t * 7.0 + v * 0.7 + r.seed as f32).sin();
                let rc = if r.white { [255.0, 255.0, 255.0] } else { [225.0, 200.0, 255.0] };
                for k in 0..3 {
                    col[k] = col[k] * (1.0 - white_out) + 255.0 * white_out;
                }
                for k in 0..3 {
                    col[k] = col[k] * (1.0 - rim * 0.85) + rc[k] * rim * 0.85 * flick;
                    alb[i + k] = col[k].min(255.0) as u8;
                }
                alb[i + 3] = 255;
                let glow = if r.white { 1.0 } else { 0.35 + rim * 0.6 };
                for k in 0..3 {
                    emit[i + k] = emit[i + k].max((col[k] * glow).min(255.0) as u8);
                }
            } else if d < 2.4 && !r.white {
                // Outside: light bends around the tear, splitting slightly by colour.
                let f = 1.0 - (d - 1.0) / 1.4;
                let pull = f * f * 0.45;
                let (rx, ry) = (dx as f32, dy as f32);
                let swirl = f * f * 0.5 * (t * 0.3).sin();
                let (sn, cs) = swirl.sin_cos();
                for (k, spread) in [(0usize, 1.06f32), (1, 1.0), (2, 0.94)] {
                    let s = 1.0 + pull * spread;
                    let (qx, qy) = (rx * cs - ry * sn, rx * sn + ry * cs);
                    if let Some(si) = sample(cx + qx * s, cy + qy * s) {
                        alb[i + k] = tmp[si + k];
                    }
                }
                let halo = f.powi(3) * 0.5;
                alb[i] = (alb[i] as f32 * (1.0 - halo) + 160.0 * halo) as u8;
                alb[i + 1] = (alb[i + 1] as f32 * (1.0 - halo) + 110.0 * halo) as u8;
                alb[i + 2] = (alb[i + 2] as f32 * (1.0 - halo) + 255.0 * halo) as u8;
                let e = (f.powi(4) * 150.0) as u8;
                emit[i] = emit[i].max(e / 2);
                emit[i + 1] = emit[i + 1].max(e / 3);
                emit[i + 2] = emit[i + 2].max(e);
            }
        }
    }
}

const FIRE_PAL: [[u8; 3]; 5] = [[150, 24, 8], [222, 58, 12], [255, 122, 20], [255, 188, 48], [255, 240, 150]];

struct SkyCtx<'a> {
    view: View,
    day: f32,
    tod: f32,
    rain: f32,
    clouds: f32,
    top: [u8; 3],
    horizon: [u8; 3],
    time: f32,
    far: &'a [f32],
    near: &'a [f32],
    /// Height of the atmosphere's edge (0 = airless).
    atmo: f32,
    scene: &'a crate::planets::SkyScene,
}

/// Sky: day/night gradient, sunset glow, sun and moon, clouds, parallax mountains,
/// and a fade to starry space with a ringed planet as you fly higher.
fn sky(c: &SkyCtx, vx: usize, vy: usize, gy: i32) -> [u8; 3] {
    let v = &c.view;
    let (sx, sy) = (vx as f32 - MARGIN as f32, vy as f32 - MARGIN as f32);
    let t = ((gy as f32 + 200.0) / 240.0).clamp(0.0, 1.0);
    let day = lerp3(c.top, c.horizon, t);
    let dim = |k: [u8; 3], f: f32| [(k[0] as f32 * f) as u8, (k[1] as f32 * f) as u8, (k[2] as f32 * f) as u8];
    let night = lerp3(lerp3([6, 10, 28], dim(c.top, 0.12), 0.5), lerp3([26, 34, 62], dim(c.horizon, 0.2), 0.5), t);
    let mut col = lerp3(night, day, c.day);
    let dusk = (1.0 - (c.day - 0.35).abs() * 4.0).clamp(0.0, 1.0);
    col = lerp3(col, [236, 140, 96], dusk * t * 0.55);
    col = lerp3(col, lerp3([60, 64, 76], [150, 156, 166], c.day), c.clouds * 0.45 + c.rain * 0.3);
    // Airless worlds are always under open space; elsewhere it fades in as you climb.
    let space = if c.atmo <= 0.0 { 1.0 } else { ((-(gy as f32) - (c.atmo - 220.0)) / 340.0).clamp(0.0, 1.0) };
    col = lerp3(col, [3, 3, 10], space);
    // Faint nebula behind the stars, its colour set by where in the galaxy you are.
    if space > 0.0 && c.scene.nebula_amt > 0.0 {
        let n = value_noise((v.left * 0.02 + sx) * 0.012, (v.top * 0.02 + sy) * 0.02, 91);
        let k = ((n - 0.45) * 2.2).clamp(0.0, 1.0) * c.scene.nebula_amt * space;
        col = lerp3(col, c.scene.nebula, k);
    }
    // The edge of the atmosphere: a thin glowing band, with aurora curtains at night.
    let band = (gy as f32 + c.atmo).abs();
    if c.atmo > 0.0 && band < 14.0 {
        let wave = (v.left * 0.6 + sx) * 0.045 + c.time * 0.7;
        let k = (1.0 - band / 14.0).powi(2) * (0.45 + 0.25 * wave.sin());
        col = lerp3(col, lerp3(c.scene.rim[0], c.scene.rim[1], (wave * 0.5).sin() * 0.5 + 0.5), k);
    }
    if c.atmo > 0.0 && c.day < 0.3 && (-(gy as f32)) > c.atmo - 100.0 && (-(gy as f32)) < c.atmo + 40.0 {
        let curtain = ((v.left * 0.3 + sx) * 0.09 + c.time * 0.4).sin() * ((v.left * 0.3 + sx) * 0.023).sin();
        if curtain > 0.35 {
            let k = (curtain - 0.35) * 0.9 * (1.0 - c.day / 0.3);
            col = lerp3(col, c.scene.rim[0], k * 0.5);
        }
    }

    // Stars, and this planet's neighbours hanging in space.
    let stars = (1.0 - c.day * 1.4).max(space).max(0.0) * (1.0 - c.rain);
    if stars > 0.0 {
        let (px, py) = ((v.left * 0.05 + sx) as i32, (v.top * 0.05 + sy) as i32);
        let hh = hash3(px, py, 77);
        if hh % 211 == 0 {
            let tw = 0.6 + 0.4 * ((c.time * 3.0 + (hh % 13) as f32).sin());
            col = lerp3(col, [255, 255, 240], stars * tw);
        }
    }
    if space > 0.2 {
        for b in &c.scene.bodies {
            let (pcx, pcy) = (v.view_w() * b.fx - v.left * b.depth, b.y - 30.0 - v.top * b.depth);
            let (dx, dy) = (sx - pcx, sy - pcy);
            let d = (dx * dx + dy * dy).sqrt();
            if d > b.r * 2.0 + 2.0 {
                continue;
            }
            let ring = b.ring.is_some() && ((dx * 0.9 + dy * 2.6).abs() < b.r * 0.12 + 0.8) && d > b.r * 0.9 && d < b.r * 1.9;
            if ring && dy < 0.0 && d < b.r {
                // The far side of the ring hides behind the planet.
            } else if ring {
                return lerp3(col, b.ring.unwrap_or([200, 200, 200]), space * 0.8);
            }
            if d < b.r {
                let lit = (1.0 - (dx + dy * 0.6) / (b.r * 1.7)).clamp(0.15, 1.0);
                let base = match b.style {
                    0 => {
                        if ((dy / b.r * 7.0 + value_noise(dx * 0.1, dy * 0.6, 4) * 3.0) as i32) % 2 == 0 {
                            b.a
                        } else {
                            b.b
                        }
                    }
                    1 => {
                        let cr = value_noise(dx / b.r * 3.0 + b.r, dy / b.r * 3.0, 8);
                        if cr > 0.62 {
                            b.b
                        } else {
                            b.a
                        }
                    }
                    2 => {
                        let cl = value_noise(dx / b.r * 2.4 + c.time * 0.05, dy / b.r * 4.0, 12);
                        if cl > 0.6 {
                            b.b
                        } else if value_noise(dx / b.r * 1.8, dy / b.r * 1.8, 13) > 0.55 {
                            [70, 140, 70]
                        } else {
                            b.a
                        }
                    }
                    4 => {
                        if value_noise(dx / b.r * 2.6 + b.r, dy / b.r * 2.6, 15) > 0.52 {
                            b.a
                        } else {
                            b.b
                        }
                    }
                    _ => {
                        let hot = value_noise(dx * 0.3, dy * 0.3 + c.time * 0.1, 14);
                        if hot > 0.58 {
                            b.b
                        } else {
                            b.a
                        }
                    }
                };
                return lerp3(col, shade(base, (lit * 255.0) as u8, 0.9), space);
            } else if d < b.r + 1.5 && b.style == 2 {
                col = lerp3(col, [140, 190, 255], space * 0.5);
            }
        }
    }

    // Sun by day, moon by night.
    let phase = c.tod;
    let (bx, by, r, bc) = if c.day > 0.05 {
        let p = ((phase - 0.22) / 0.56).clamp(0.0, 1.0);
        let (bx, by) = (p * v.view_w(), v.view_h() * (0.6 - (p * std::f32::consts::PI).sin() * 0.45));
        // A binary system: a smaller companion sun trails the first.
        if let Some(tc) = c.scene.twin {
            let d2 = ((sx - bx + 26.0).powi(2) + (sy - by - 9.0).powi(2)).sqrt();
            if d2 < c.scene.sun_r * 0.55 {
                return tc;
            } else if d2 < c.scene.sun_r * 1.8 {
                col = lerp3(col, tc, (1.0 - d2 / (c.scene.sun_r * 1.8)).powi(2) * 0.5);
            }
        }
        (bx, by, c.scene.sun_r, c.scene.sun)
    } else {
        let p = ((phase + 0.28).fract() / 0.56).clamp(0.0, 1.0);
        (p * v.view_w(), v.view_h() * (0.6 - (p * std::f32::consts::PI).sin() * 0.45), 5.0, [226, 230, 240])
    };
    let d = ((sx - bx).powi(2) + (sy - by).powi(2)).sqrt();
    if d < r {
        return bc;
    } else if d < r * 3.0 && c.day > 0.05 {
        col = lerp3(col, [255, 236, 200], (1.0 - d / (r * 3.0)).powi(2) * 0.5);
    }

    if space < 0.6 {
        let m = 1.0 - space / 0.6;
        let hz = c.horizon;
        let sc = |k: f32, b: [u8; 3]| [(hz[0] as f32 * k * 0.6 + b[0] as f32 * 0.4) as u8, (hz[1] as f32 * k * 0.6 + b[1] as f32 * 0.4) as u8, (hz[2] as f32 * k * 0.6 + b[2] as f32 * 0.4) as u8];
        if v.top * 0.7 + sy >= c.near[vx] {
            return lerp3(col, lerp3([20, 24, 40], sc(0.5, [84, 112, 150]), c.day), 0.88 * m);
        }
        if v.top * 0.45 + sy >= c.far[vx] {
            return lerp3(col, lerp3([30, 36, 58], sc(0.72, [126, 154, 192]), c.day), 0.8 * m);
        }
    }
    let n = value_noise((v.left * 0.25 + sx + c.time * 1.5) * 0.03, (v.top * 0.25 + sy) * 0.1, 55);
    let cloudy = 0.66 - c.clouds * 0.3;
    if n > cloudy && space < 0.8 {
        let cc = lerp3(lerp3([60, 64, 84], [244, 246, 250], c.day.max(0.15)), [92, 96, 108], c.rain * 0.7);
        col = lerp3(col, cc, ((n - cloudy) * 3.0).min(0.85) * (1.0 - space));
    }
    col
}

impl View {
    fn view_w(&self) -> f32 {
        self.w as f32
    }
    fn view_h(&self) -> f32 {
        self.h as f32
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
