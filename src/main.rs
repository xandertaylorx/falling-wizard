mod audio;
mod cell;
mod fx;
mod materials;
mod noise;
mod player;
mod world;
mod worldgen;

use audio::Audio;
use cell::Cell;
use fx::{Fx, Glow, GH, GW};
use macroquad::miniquad::{BlendFactor, BlendState, Equation, PipelineParams, ShaderSource};
use macroquad::prelude::*;
use materials::Material;
use noise::Rng;
use player::{Input, Player, MAX_MANA};
use std::time::Instant;
use world::{World, H, W};

const STEP: f32 = 1.0 / 60.0;
const SCREEN_W: f32 = 1280.0;
const SCREEN_H: f32 = 720.0;

const BRUSHES: [(KeyCode, Material); 8] = [
    (KeyCode::Key1, Material::Sand),
    (KeyCode::Key2, Material::Water),
    (KeyCode::Key3, Material::Oil),
    (KeyCode::Key4, Material::Fire),
    (KeyCode::Key5, Material::Acid),
    (KeyCode::Key6, Material::Stone),
    (KeyCode::Key7, Material::Smoke),
    (KeyCode::Key0, Material::Empty),
];

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

const ADDITIVE_FRAGMENT: &str = r#"#version 100
varying lowp vec2 uv;
uniform sampler2D Texture;
void main() {
    gl_FragColor = texture2D(Texture, uv);
}"#;

fn conf() -> Conf {
    Conf {
        window_title: "Falling Wizard".into(),
        window_width: SCREEN_W as i32,
        window_height: SCREEN_H as i32,
        window_resizable: false,
        high_dpi: false,
        ..Default::default()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--bench") {
        bench();
        return;
    }
    macroquad::Window::from_config(conf(), game(args));
}

/// Paint a disc of material. Fluids/powders are sprinkled into empty cells; stone and
/// the eraser overwrite; fire also ignites anything flammable it touches.
fn paint(world: &mut World, cx: i32, cy: i32, r: i32, m: Material) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let (x, y) = (cx + dx, cy + dy);
            let here = world.mat(x, y);
            match m {
                Material::Empty | Material::Stone => world.place(x, y, m),
                Material::Fire if here.flammable() => {
                    let f = Cell::fire(true, &mut world.rng);
                    world.set(x, y, f);
                }
                _ if here == Material::Empty && world.rng.chance(0.5) => world.place(x, y, m),
                _ => {}
            }
        }
    }
}

fn dig(world: &mut World, cx: i32, cy: i32, r: i32) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r && world.mat(cx + dx, cy + dy) != Material::Empty {
                world.set(cx + dx, cy + dy, Cell::EMPTY);
            }
        }
    }
}

/// Headless benchmark of simulation + CPU-side rendering on a generated world.
fn bench() {
    let mut world = World::new(1);
    worldgen::generate(&mut world, 1);
    let mut fx = Fx::new(1);
    let mut glow = Glow::new();
    let mut px = vec![0u8; W * H * 4];
    let mut gpx = vec![0u8; GW * GH * 4];
    let mut rng = Rng::new(3);
    let (mut sim, mut ren, mut worst) = (0.0f64, 0.0f64, 0.0f64);
    let steps = 1800;
    for i in 0..steps {
        if i % 20 == 0 {
            let (x, y) = (rng.range(10, W as i32 - 10) as f32, rng.range(30, H as i32 - 10) as f32);
            fx.explode(&mut world, x, y, 7);
        }
        if i % 90 == 0 {
            paint(&mut world, rng.range(20, 300), rng.range(20, 60), 8, Material::Water);
            paint(&mut world, rng.range(20, 300), rng.range(20, 60), 8, Material::Oil);
        }
        let t = Instant::now();
        world.step();
        fx.step(&mut world, STEP);
        fx.explosions.clear();
        let s = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        world.render(&mut px);
        fx.draw(&mut px);
        glow.compute(&world, &fx, &mut gpx);
        let r = t.elapsed().as_secs_f64() * 1000.0;
        sim += s;
        ren += r;
        worst = worst.max(s + r);
        if i % 300 == 0 {
            println!("step {i:4}: active chunks {:3}/{}  fire {:5}", world.active_chunks, world::CW * world::CH, world.fire_count);
        }
    }
    println!(
        "avg sim {:.3} ms, avg render {:.3} ms, worst frame {:.3} ms (budget 16.7 ms)",
        sim / steps as f64,
        ren / steps as f64,
        worst
    );
}

async fn game(args: Vec<String>) {
    // --autotest <secs> [shots-dir]: scripted spell-casting, prints FPS stats, saves screenshots.
    let autotest: Option<(f64, Option<String>)> = args
        .iter()
        .position(|a| a == "--autotest")
        .map(|i| (args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(10.0), args.get(i + 2).cloned()));

    let mut seed: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(1);
    if autotest.is_some() {
        seed = 1;
    }
    let mut world = World::new(seed);
    let (sx, sy) = worldgen::generate(&mut world, seed);
    let mut player = Player::new(sx, sy);
    let mut fx = Fx::new(seed);
    let mut glow = Glow::new();

    let mut img = Image::gen_image_color(W as u16, H as u16, BLACK);
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Nearest);
    let mut glow_img = Image::gen_image_color(GW as u16, GH as u16, BLACK);
    let glow_tex = Texture2D::from_image(&glow_img);
    glow_tex.set_filter(FilterMode::Linear);
    let additive = load_material(
        ShaderSource::Glsl { vertex: VERTEX, fragment: ADDITIVE_FRAGMENT },
        MaterialParams {
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One)),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .expect("additive material");

    let mut audio = Audio::new().await;
    if audio.is_none() {
        eprintln!("audio unavailable, running silent");
    } else if autotest.is_some() {
        println!("audio: synthesised sounds loaded");
    }
    let mut shake_rng = Rng::new(99);
    let mut brush = Material::Sand;
    let mut brush_r: i32 = 3;
    let mut acc = 0.0f32;
    let mut cast_cd = 0.0f32;
    let mut fps_avg = 60.0f32;
    let mut sim_ms = 0.0f32;
    let mut up_was_down = false;

    // Autotest bookkeeping.
    let t0 = get_time();
    let mut frame_times: Vec<f32> = Vec::new();
    let mut shots_taken = 0usize;
    let mut script_rng = Rng::new(5);
    let mut next_cast = 1.0f64;

    let mut cpu_times: Vec<f32> = Vec::new();
    loop {
        let frame_start = Instant::now();
        let dt = get_frame_time().min(0.05);
        let now = get_time() - t0;
        let scale = screen_width() / W as f32;
        let (mx, my) = mouse_position();
        let (mut cx, mut cy) = (mx / scale, my / scale);

        for (k, m) in BRUSHES {
            if is_key_pressed(k) {
                brush = m;
            }
        }
        let wheel = mouse_wheel().1;
        if wheel > 0.0 {
            brush_r = (brush_r + 1).min(20);
        } else if wheel < 0.0 {
            brush_r = (brush_r - 1).max(1);
        }
        if is_key_pressed(KeyCode::R) {
            seed = seed.wrapping_add(1);
            world = World::new(seed);
            let (sx, sy) = worldgen::generate(&mut world, seed);
            player = Player::new(sx, sy);
            fx.clear();
        }
        if is_key_pressed(KeyCode::Escape) {
            break;
        }

        let mut cast = is_mouse_button_down(MouseButton::Left);
        if let Some((secs, _)) = &autotest {
            // Scripted run: aim at random spots, fire, and drop some oil + fire at 2s.
            cx = script_rng.f32();
            cx = 40.0 + cx * 260.0;
            cy = 60.0 + (now as f32 * 37.0) % 110.0;
            cast = now >= next_cast;
            if cast {
                next_cast += 0.35;
            }
            if (2.0..2.5).contains(&now) {
                paint(&mut world, 160, 40, 10, Material::Oil);
            }
            if (3.0..3.02).contains(&now) {
                if let Some(i) = world.cells.iter().position(|c| c.mat == Material::Oil) {
                    let (x, y) = ((i % W) as f32, (i / W) as f32);
                    fx.explode(&mut world, x, y + 3.0, 7);
                }
            }
            if now > *secs {
                frame_times.sort_by(|a, b| b.partial_cmp(a).unwrap());
                let n = frame_times.len().max(1);
                let avg = frame_times.iter().sum::<f32>() / n as f32;
                let p99 = frame_times[(n / 100).min(n - 1)];
                cpu_times.sort_by(|a, b| b.partial_cmp(a).unwrap());
                println!("cpu per frame: avg {:.2} ms, worst {:.2} ms, 99th {:.2} ms", cpu_times.iter().sum::<f32>() / cpu_times.len() as f32, cpu_times[0], cpu_times[cpu_times.len() / 100]);
                println!(
                    "autotest: {n} frames, avg {:.1} fps, 1% low {:.1} fps, worst frame {:.1} ms, last sim {:.2} ms",
                    1.0 / avg,
                    1.0 / p99,
                    frame_times[0] * 1000.0,
                    sim_ms
                );
                std::process::exit(0);
            }
        }

        if is_mouse_button_down(MouseButton::Middle) || is_key_down(KeyCode::Space) {
            paint(&mut world, cx as i32, cy as i32, brush_r, brush);
        }
        if is_mouse_button_down(MouseButton::Right) {
            dig(&mut world, cx as i32, cy as i32, 3);
        }
        cast_cd -= dt;
        if cast && cast_cd <= 0.0 {
            let (tx, ty) = player.staff_tip(cx, cy);
            fx.cast(tx, ty, cx, cy);
            cast_cd = 0.2;
            if let Some(a) = &audio {
                a.whoosh();
            }
        }

        let up = is_key_down(KeyCode::W) || is_key_down(KeyCode::Up);
        let scripted = |a: f64, b: f64| autotest.is_some() && (a..b).contains(&now);
        let up = up || scripted(4.0, 4.8);
        let input = Input {
            left: is_key_down(KeyCode::A) || is_key_down(KeyCode::Left),
            right: is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) || scripted(1.2, 4.5),
            up,
            up_pressed: up && !up_was_down,
        };
        up_was_down = up;
        player.update(&world, &input, dt);

        // Fixed-rate simulation, independent of display refresh rate.
        acc += dt;
        let t_sim = Instant::now();
        let mut steps = 0;
        while acc >= STEP && steps < 3 {
            world.step();
            fx.step(&mut world, STEP);
            acc -= STEP;
            steps += 1;
        }
        if steps == 3 {
            acc = 0.0;
        }
        if steps > 0 {
            sim_ms = sim_ms * 0.9 + t_sim.elapsed().as_secs_f32() * 1000.0 / steps as f32 * 0.1;
        }
        for (ex, ey) in fx.explosions.drain(..) {
            player.knockback(ex, ey);
            if let Some(a) = &audio {
                a.thump();
            }
        }
        if let Some(a) = &mut audio {
            a.update(world.fire_count, dt);
        }

        // Build the frame on the CPU, upload as one texture.
        world.render(&mut img.bytes);
        fx.draw(&mut img.bytes);
        player.draw(&mut img.bytes, cx, cy, cast_cd <= 0.0);
        tex.update(&img);
        glow.compute(&world, &fx, &mut glow_img.bytes);
        glow_tex.update(&glow_img);

        let (ox, oy) = if fx.shake > 0.05 {
            (shake_rng.rangef(-1.0, 1.0) * fx.shake, shake_rng.rangef(-1.0, 1.0) * fx.shake)
        } else {
            (0.0, 0.0)
        };
        clear_background(BLACK);
        let sw = screen_width();
        let sh = screen_height();
        draw_texture_ex(&tex, ox, oy, WHITE, DrawTextureParams { dest_size: Some(vec2(sw, sh)), ..Default::default() });
        gl_use_material(&additive);
        draw_texture_ex(&glow_tex, ox, oy, WHITE, DrawTextureParams { dest_size: Some(vec2(sw, sh)), ..Default::default() });
        gl_use_default_material();

        // Brush outline.
        draw_circle_lines(mx, my, (brush_r as f32 + 0.5) * scale, 1.0, Color::new(1.0, 1.0, 1.0, 0.35));

        // HUD.
        fps_avg = fps_avg * 0.95 + (1.0 / get_frame_time().max(0.0001)) * 0.05;
        let fps_col = if fps_avg >= 58.0 { Color::new(0.6, 1.0, 0.6, 1.0) } else { Color::new(1.0, 0.5, 0.4, 1.0) };
        let hx = sw - 316.0;
        draw_rectangle(hx - 8.0, 6.0, 318.0, 74.0, Color::new(0.0, 0.0, 0.0, 0.45));
        draw_text(&format!("FPS {:.0}", fps_avg), hx, 26.0, 24.0, fps_col);
        draw_text(
            &format!("sim {:.2} ms   chunks {}/{}", sim_ms, world.active_chunks, world::CW * world::CH),
            hx + 86.0,
            26.0,
            18.0,
            LIGHTGRAY,
        );
        draw_text(&format!("Brush: {}  (size {})", brush.name(), brush_r), hx, 48.0, 20.0, WHITE);
        draw_rectangle(hx, 60.0, 140.0, 8.0, Color::new(0.15, 0.15, 0.3, 1.0));
        draw_rectangle(hx, 60.0, 140.0 * player.mana / MAX_MANA, 8.0, Color::new(0.45, 0.6, 1.0, 1.0));
        draw_text("mana", hx + 146.0, 68.0, 16.0, LIGHTGRAY);
        draw_text(
            "WASD/arrows move  W jump/levitate  LMB spark  RMB dig  MMB/Space paint  1-7,0 material  wheel size  R new world",
            10.0,
            sh - 10.0,
            16.0,
            Color::new(1.0, 1.0, 1.0, 0.5),
        );

        if let Some((secs, dir)) = &autotest {
            if now > 1.0 {
                frame_times.push(get_frame_time());
                cpu_times.push(frame_start.elapsed().as_secs_f32() * 1000.0);
            }
            if let Some(dir) = dir {
                let marks = [secs * 0.25, secs * 0.4, secs * 0.7];
                if shots_taken < marks.len() && now >= marks[shots_taken] {
                    get_screen_data().export_png(&format!("{dir}/shot_{shots_taken}.png"));
                    shots_taken += 1;
                }
            }
        }

        next_frame().await
    }
}
