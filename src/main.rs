mod audio;
mod cell;
mod fx;
mod game;
mod materials;
mod mobs;
mod noise;
mod player;
mod render;
mod settings;
mod spells;
mod ui;
mod weather;
mod world;
mod worldgen;

use std::time::Instant;

use audio::{Audio, Sfx};
use game::{Controls, Game, PALETTE};
use macroquad::prelude::*;
use noise::Rng;
use player::Input;
use render::{Renderer, View};
use settings::Settings;
use ui::{Ui, ACCENT, DANGER, MUTED, TEXT};

fn conf() -> Conf {
    Conf {
        window_title: "Falling Wizard".into(),
        window_width: 1280,
        window_height: 720,
        window_resizable: true,
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
    macroquad::Window::from_config(conf(), run(args));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Menu,
    Play,
    Pause,
    Settings,
    Controls,
    Dead,
    Chat,
}

fn random_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) % 1_000_000
}

/// Headless benchmark: simulate while flying across the infinite world, casting spells.
fn bench() {
    let mut game = Game::new(1);
    let mut rng = Rng::new(3);
    let frames = 2400;
    let (mut total, mut worst) = (0.0f64, 0.0f64);
    for i in 0..frames {
        let (px, py) = game.player.center();
        if i % 200 == 199 {
            // Jump far away to force chunk streaming and generation.
            game.player.x += 300.0;
            game.player.y += if i % 400 == 199 { 150.0 } else { -100.0 };
        }
        let aim = (px + rng.rangef(-60.0, 60.0), py + rng.rangef(-20.0, 60.0));
        game.spell = (i / 120) % spells::SPELL_BAR.len();
        if i % 600 == 0 {
            game.mobs.spawn(mobs::MobKind::Dragon, px + 40.0, py - 30.0);
            game.mobs.spawn(mobs::MobKind::Zombie, px + 20.0, py - 5.0);
        }
        let ctl = Controls { input: Input { right: i % 300 < 150, ..Default::default() }, aim, primary: i % 7 == 0, ..Default::default() };
        let t = Instant::now();
        game.update(game::STEP, &ctl, false);
        if game.player.dead() {
            game.respawn();
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        total += ms;
        worst = worst.max(ms);
        if i % 400 == 0 {
            println!(
                "frame {i:5}: pos ({:7.0},{:5.0}) active sim chunks {:4}  fire {:5}  particles {:5}  mobs {:3}",
                px,
                py,
                game.world.active_chunks,
                game.world.fire_count,
                game.fx.count(),
                game.mobs.list.len()
            );
        }
    }
    println!("avg update {:.3} ms, worst {:.2} ms (budget 16.7 ms)", total / frames as f64, worst);
    material_checks();
}

/// Headless behaviour checks for growth, chain reactions and lava.
fn material_checks() {
    use cell::Cell;
    use materials::Material as M;
    let mut g = Game::new(5);
    let (ox, oy) = (g.world.ox + 200, g.world.oy + 200);
    let count = |g: &Game, m: M| (0..60).flat_map(|y| (0..60).map(move |x| (x, y))).filter(|(x, y)| g.world.mat(ox + x, oy + y) == m).count();
    let clear = |g: &mut Game| {
        for y in 0..60 {
            for x in 0..60 {
                g.world.set(ox + x, oy + y, Cell::EMPTY);
            }
        }
        for x in 0..60 {
            g.world.place(ox + x, oy + 50, M::Dirt);
        }
    };
    let run = |g: &mut Game, n: usize| {
        for _ in 0..n {
            g.world.step();
            g.fx.step(&mut g.world, game::STEP);
            let k = g.world.explosions.len().min(4);
            for (x, y, r) in g.world.explosions.drain(..k).collect::<Vec<_>>() {
                g.fx.explode(&mut g.world, x as f32, y as f32, r, 0.6);
            }
        }
    };

    clear(&mut g);
    for x in (5..55).step_by(6) {
        g.world.place(ox + x, oy + 49, M::Seed);
    }
    run(&mut g, 900);
    let backs = |g: &Game, m: M| (0..60).flat_map(|y| (0..60).map(move |x| (x, y))).filter(|(x, y)| g.world.back(ox + x, oy + y) == m).count();
    println!("seeds -> background plant cells {}, leaves {}", backs(&g, M::Plant), backs(&g, M::Leaves));

    clear(&mut g);
    for y in 30..50 {
        for x in 10..50 {
            g.world.place(ox + x, oy + y, M::Gunpowder);
        }
    }
    let before = count(&g, M::Gunpowder);
    g.world.ignite_at(ox + 10, oy + 49);
    run(&mut g, 600);
    println!("gunpowder {} -> {} after ignition ({} explosions)", before, count(&g, M::Gunpowder), g.fx.explosions.len());

    clear(&mut g);
    for y in 40..50 {
        for x in 10..30 {
            g.world.place(ox + x, oy + y, M::Lava);
        }
        for x in 30..50 {
            g.world.place(ox + x, oy + y, M::Water);
        }
    }
    run(&mut g, 300);
    let lava_line = format!("lava+water -> basalt {}, steam {}, lava left {}", count(&g, M::Basalt), count(&g, M::Steam), count(&g, M::Lava));
    clear(&mut g);
    for y in 30..50 {
        for x in 5..55 {
            g.world.place(ox + x, oy + y, M::Oil);
        }
    }
    g.world.ignite_at(ox + 30, oy + 30);
    let mut booms = 0;
    for _ in 0..240 {
        g.world.step();
        g.fx.step(&mut g.world, game::STEP);
        for (x, y, r) in g.world.explosions.drain(..).collect::<Vec<_>>() {
            booms += 1;
            g.fx.explode(&mut g.world, x as f32, y as f32, r, 0.6);
        }
    }
    println!("big oil pool lit -> {} explosions, oil left {}", booms, count(&g, M::Oil));
    if false {
    println!("lava+water -> basalt {}, steam {}, lava left {}", count(&g, M::Basalt), count(&g, M::Steam), count(&g, M::Lava));
    }
    println!("{lava_line}");
}

struct App {
    screen: Screen,
    back: Screen,
    game: Game,
    renderer: Renderer,
    ui: Ui,
    audio: Option<Audio>,
    settings: Settings,
    fps: f32,
    shake_rng: Rng,
    menu_x: f32,
    chat_buf: String,
    history: Vec<String>,
    /// Esc pressed this frame and not yet handled (so one press can't open and close a menu).
    esc: bool,
}

async fn run(args: Vec<String>) {
    // --autotest <secs> [shots-dir]: scripted, muted playthrough with screenshots and FPS stats.
    let autotest: Option<(f64, Option<String>)> = args
        .iter()
        .position(|a| a == "--autotest")
        .map(|i| (args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(14.0), args.get(i + 2).cloned()));
    let mute = autotest.is_some() || args.iter().any(|a| a == "--mute");

    let settings = Settings::load();
    if settings.fullscreen && autotest.is_none() {
        set_fullscreen(true);
    }
    let seed = if autotest.is_some() { 7 } else { random_seed() };
    let audio = if mute { None } else { Audio::new().await };
    let mut app = App {
        screen: Screen::Menu,
        back: Screen::Menu,
        game: Game::new(seed),
        renderer: Renderer::new(),
        ui: Ui::new(),
        audio,
        settings,
        fps: 60.0,
        shake_rng: Rng::new(99),
        menu_x: 0.0,
        chat_buf: String::new(),
        history: Vec::new(),
        esc: false,
    };

    let t0 = get_time();
    let mut frame_times: Vec<f32> = Vec::new();
    let mut cpu_times: Vec<f32> = Vec::new();
    let mut shot_k = 0usize;

    loop {
        let frame_start = Instant::now();
        let dt = get_frame_time().min(0.05);
        let now = get_time() - t0;
        app.fps = app.fps * 0.95 + (1.0 / get_frame_time().max(0.0001)) * 0.05;
        app.ui.begin();

        app.renderer.rays = if app.settings.hq_light { 28.0 } else { 12.0 };
        if is_key_pressed(KeyCode::F11) {
            app.settings.fullscreen = !app.settings.fullscreen;
            set_fullscreen(app.settings.fullscreen);
            app.settings.save();
        }

        let script = autotest.as_ref().map(|(secs, _)| script_controls(&mut app, now, *secs));
        app.frame(dt, script);

        if let Some((secs, dir)) = &autotest {
            if now > 1.5 {
                frame_times.push(get_frame_time());
                cpu_times.push(frame_start.elapsed().as_secs_f32() * 1000.0);
            }
            let shots = [
                (1.0, "menu"),
                (3.6, "surface_mobs"),
                (6.4, "spells"),
                (7.4, "chat"),
                (9.6, "volcano"),
                (11.6, "deep_lava"),
                (13.6, "sky_isles"),
                (15.6, "space"),
                (17.6, "night_storm"),
                (19.6, "far_biome"),
                (secs - 0.4, "settings"),
            ];
            if shot_k < shots.len() && now >= shots[shot_k].0 {
                if let Some(dir) = dir {
                    get_screen_data().export_png(&format!("{dir}/{}_{}.png", shot_k, shots[shot_k].1));
                }
                shot_k += 1;
            }
            if now > *secs {
                let mut ft = frame_times.clone();
                ft.sort_by(|a, b| b.partial_cmp(a).unwrap());
                cpu_times.sort_by(|a, b| b.partial_cmp(a).unwrap());
                let n = ft.len().max(1);
                let avg = ft.iter().sum::<f32>() / n as f32;
                println!(
                    "autotest: {n} frames, avg {:.1} fps, 1% low {:.1} fps | cpu/frame avg {:.2} ms, 99th {:.2} ms, worst {:.2} ms",
                    1.0 / avg,
                    1.0 / ft[(n / 100).min(n - 1)],
                    cpu_times.iter().sum::<f32>() / cpu_times.len().max(1) as f32,
                    cpu_times[(cpu_times.len() / 100).min(cpu_times.len().saturating_sub(1))],
                    cpu_times[0]
                );
                std::process::exit(0);
            }
        }
        next_frame().await;
    }
}

/// Scripted controls for --autotest. Drives screens too.
fn script_controls(app: &mut App, t: f64, secs: f64) -> Controls {
    let g = &mut app.game;
    let (px, py) = g.player.center();
    let mut ctl = Controls { aim: (px + 40.0, py), ..Default::default() };
    let at = |a: f64, b: f64| (a..b).contains(&t);
    if at(1.2, 1.3) && app.screen == Screen::Menu {
        g.cam = g.player.center();
        app.screen = Screen::Play;
    }
    if t < 1.2 {
        return ctl;
    }
    if at(1.3, 1.32) {
        use mobs::MobKind::*;
        for (k, kind) in [Zombie, Lion, Tiger, DemonEye, Slime, Skeleton].iter().enumerate() {
            g.mobs.spawn(*kind, px + 30.0 + k as f32 * 16.0, py - 30.0);
        }
    }
    ctl.input.right = at(1.4, 2.6);
    ctl.input.up = at(2.0, 2.5);
    ctl.input.up_pressed = at(2.0, 2.05);
    ctl.input.dash = at(2.7, 2.72);
    let fire = |spell: usize, aim: (f32, f32), g: &mut Game, ctl: &mut Controls| {
        g.spell = spell;
        ctl.aim = aim;
        ctl.primary = true;
    };
    if at(3.7, 4.4) {
        fire(0, (px + 60.0, py - 6.0), g, &mut ctl);
    } else if at(4.5, 4.6) {
        fire(13, (px + 40.0, py - 10.0), g, &mut ctl);
    } else if at(5.0, 5.8) {
        fire(9, (px + 10.0, py + 40.0), g, &mut ctl);
    }
    if at(6.2, 6.3) {
        fire(14, (px + 50.0, py), g, &mut ctl);
    }
    if at(5.9, 6.0) {
        fire(11, (px - 50.0, py - 25.0), g, &mut ctl);
        if !g.mobs.list.is_empty() {
            g.mobs.tame(0, &mut g.fx);
        }
    }
    if at(7.0, 7.02) {
        app.chat_buf = "/tp volcano".into();
        app.screen = Screen::Chat;
    }
    if at(7.6, 7.62) {
        g.command("/tp volcano");
        g.command("/time set noon");
        g.time = 0.0;
        app.screen = Screen::Play;
    }
    let warp = |dx: f32, y: f32, surface_rel: bool, g: &mut Game| {
        let x = g.player.x + dx;
        let y = if surface_rel { g.world.gen.surface(x as i32) as f32 + y } else { y };
        g.world.recenter(x, y);
        g.mobs.clear();
        g.fx.dissolve(&mut g.world, x + 1.5, y + 4.0, 7, 1.0);
        g.player.teleport(x, y + 6.0, &g.world);
        g.cam = g.player.center();
    };
    if at(10.0, 10.02) {
        warp(0.0, 300.0, true, g);
    }
    if at(12.0, 12.02) {
        warp(0.0, -330.0, false, g);
    }
    if at(14.0, 14.02) {
        warp(0.0, -640.0, false, g);
    }
    if at(16.0, 16.02) {
        warp(0.0, -20.0, true, g);
        g.weather.time = 0.02;
        g.weather.rain = 1.0;
        g.weather.storm = true;
    }
    if at(18.0, 18.02) {
        warp(2600.0, -20.0, true, g);
        g.weather.time = 0.45;
        g.weather.rain = 0.0;
        g.weather.storm = false;
    }
    if t > 12.0 && t < 16.0 {
        // Hover so the sky shots aren't of a falling wizard.
        g.player.vy = g.player.vy.min(0.0);
        g.player.flight = player::MAX_FLIGHT;
    }
    g.player.hp = g.player.hp.max(40.0);
    if t > secs - 1.0 && app.screen == Screen::Play {
        app.back = Screen::Pause;
        app.screen = Screen::Settings;
    }
    ctl
}

impl App {
    fn take_esc(&mut self) -> bool {
        std::mem::take(&mut self.esc)
    }

    fn frame(&mut self, dt: f32, script: Option<Controls>) {
        self.esc = is_key_pressed(KeyCode::Escape);
        let (sw, sh) = (screen_width(), screen_height());
        if let Some(a) = &mut self.audio {
            a.master = self.settings.master;
            a.effects = self.settings.effects;
            a.ambience = self.settings.ambience;
        }

        // ----- update -----
        match self.screen {
            Screen::Play => {
                let view = View::new(self.game.cam.0, self.game.cam.1, sw, sh, self.game.zoom);
                let ctl = match script {
                    Some(c) => c,
                    None => self.read_controls(&view),
                };
                self.game.update(dt, &ctl, false);
                if self.game.player.dead() {
                    self.screen = Screen::Dead;
                }
                if self.take_esc() {
                    self.screen = Screen::Pause;
                }
                let slash = is_key_pressed(KeyCode::Slash);
                if (is_key_pressed(KeyCode::T) || slash) && self.screen == Screen::Play {
                    while get_char_pressed().is_some() {}
                    self.chat_buf = if slash { "/".into() } else { String::new() };
                    self.screen = Screen::Chat;
                }
            }
            Screen::Chat => {
                let ctl = Controls { aim: self.game.aim, ..Default::default() };
                self.game.update(dt, &ctl, false);
                while let Some(c) = get_char_pressed() {
                    if !c.is_control() && self.chat_buf.len() < 120 {
                        self.chat_buf.push(c);
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    self.chat_buf.pop();
                }
                if is_key_pressed(KeyCode::Up) {
                    if let Some(h) = self.history.last() {
                        self.chat_buf = h.clone();
                    }
                }
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                    let line = std::mem::take(&mut self.chat_buf);
                    if !line.trim().is_empty() {
                        self.history.push(line.clone());
                    }
                    self.game.command(&line);
                    self.screen = Screen::Play;
                } else if self.take_esc() {
                    self.chat_buf.clear();
                    self.screen = Screen::Play;
                }
                if self.game.player.dead() {
                    self.screen = Screen::Dead;
                }
            }
            _ => {}
        }

        // ----- draw world -----
        let view = View::new(self.game.cam.0, self.game.cam.1, sw, sh, self.game.zoom);
        let shake = if self.settings.shake && self.game.fx.shake > 0.05 && self.screen == Screen::Play {
            let s = self.game.fx.shake;
            (self.shake_rng.rangef(-s, s), self.shake_rng.rangef(-s, s))
        } else {
            (0.0, 0.0)
        };
        clear_background(BLACK);
        let time = self.game.time;
        self.renderer.draw(&mut self.game, &view, shake, time);

        // ----- sound -----
        let sounds: Vec<Sfx> = self.game.sounds.drain(..).collect();
        if let Some(a) = &mut self.audio {
            if matches!(self.screen, Screen::Play | Screen::Dead | Screen::Chat) {
                for s in sounds {
                    a.play(s);
                }
            }
            let live = matches!(self.screen, Screen::Play | Screen::Dead | Screen::Menu | Screen::Chat);
            let (f, l) = if live { (self.renderer.fire_visible, self.renderer.lava_visible) } else { (0, 0) };
            a.update(dt, f, l);
        }

        // ----- UI -----
        match self.screen {
            Screen::Play => {
                self.ui.hud(&mut self.game, &view, self.fps, self.settings.show_fps);
                self.ui.chat(&self.game, None, self.game.time);
                self.crosshair(&view);
                show_mouse(false);
            }
            Screen::Chat => {
                self.ui.hud(&mut self.game, &view, self.fps, self.settings.show_fps);
                self.ui.chat(&self.game, Some(&self.chat_buf), self.game.time);
            }
            Screen::Menu => self.menu(),
            Screen::Pause => self.pause(),
            Screen::Settings => self.settings_screen(),
            Screen::Controls => self.controls_screen(),
            Screen::Dead => self.dead(),
        }
        if self.screen != Screen::Play {
            show_mouse(true);
        }
        if self.ui.clicked {
            if let Some(a) = &mut self.audio {
                a.play(Sfx::Click);
            }
        }
    }

    fn read_controls(&mut self, view: &View) -> Controls {
        let on_hud = self.ui_hover_hud();
        let g = &mut self.game;
        let (mx, my) = mouse_position();
        let aim = view.to_world(mx, my);
        let jump_key = !g.sandbox;
        let up = is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) || (jump_key && is_key_down(KeyCode::Space));
        let up_pressed = is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up) || (jump_key && is_key_pressed(KeyCode::Space));

        if is_key_pressed(KeyCode::B) {
            g.sandbox = !g.sandbox;
        }
        let keys = [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
            KeyCode::Key6,
            KeyCode::Key7,
            KeyCode::Key8,
            KeyCode::Key9,
            KeyCode::Key0,
            KeyCode::Minus,
            KeyCode::Equal,
            KeyCode::Q,
            KeyCode::R,
            KeyCode::G,
        ];
        for (k, key) in keys.iter().enumerate() {
            if is_key_pressed(*key) {
                if g.sandbox {
                    g.brush = k.min(PALETTE.len() - 1);
                } else if k < spells::SPELL_BAR.len() {
                    g.spell = k;
                }
            }
        }
        let wheel = mouse_wheel().1;
        let step = if wheel > 0.0 { -1 } else if wheel < 0.0 { 1 } else { 0 };
        let qe = is_key_pressed(KeyCode::X) as i32 - is_key_pressed(KeyCode::Z) as i32;
        if g.sandbox {
            g.brush_r = (g.brush_r - step).clamp(1, 20);
            if qe != 0 {
                g.brush = (g.brush as i32 + qe).rem_euclid(PALETTE.len() as i32) as usize;
            }
        } else if step + qe != 0 {
            g.spell = (g.spell as i32 + step + qe).rem_euclid(spells::SPELL_BAR.len() as i32) as usize;
        }

        Controls {
            input: Input {
                left: is_key_down(KeyCode::A) || is_key_down(KeyCode::Left),
                right: is_key_down(KeyCode::D) || is_key_down(KeyCode::Right),
                up,
                up_pressed,
                down: is_key_down(KeyCode::S) || is_key_down(KeyCode::Down),
                dash: is_key_pressed(KeyCode::LeftShift) || is_key_pressed(KeyCode::RightShift),
                grapple: is_key_pressed(KeyCode::F),
                aim,
            },
            aim,
            primary: is_mouse_button_down(MouseButton::Left) && !on_hud,
            secondary: is_mouse_button_down(MouseButton::Right),
            paint: is_mouse_button_down(MouseButton::Middle) || (g.sandbox && is_key_down(KeyCode::Space)),
            interact: is_key_pressed(KeyCode::E),
        }
    }

    /// Whether the cursor is over the hotbar strip at the bottom of the screen.
    fn ui_hover_hud(&self) -> bool {
        let (mx, my) = mouse_position();
        let (sw, sh) = (screen_width(), screen_height());
        my > sh - 80.0 && (mx - sw * 0.5).abs() < 420.0
    }

    fn crosshair(&self, view: &View) {
        let (mx, my) = mouse_position();
        if self.game.sandbox {
            draw_circle_lines(mx, my, (self.game.brush_r as f32 + 0.5) * view.scale, 1.0, Color::new(1.0, 1.0, 1.0, 0.5));
        }
        let c = if self.game.cast_ready() || self.game.sandbox { TEXT } else { MUTED };
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            draw_line(mx + dx * 4.0, my + dy * 4.0, mx + dx * 9.0, my + dy * 9.0, 2.0, c);
        }
    }

    fn dim(&self, a: f32) {
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.02, 0.01, 0.04, a));
    }

    fn title(&self, text: &str, y: f32, scale: f32, col: Color) {
        let cx = screen_width() * 0.5;
        let w = self.ui.font.width(text, scale);
        self.ui.font.draw(text, cx - w * 0.5 + scale, y + scale, scale, Color::new(0.08, 0.04, 0.14, 0.8));
        self.ui.font.draw(text, cx - w * 0.5, y, scale, col);
    }

    fn menu(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.02, 0.01, 0.05, 0.2));
        let cx = sw * 0.5;
        let ty = sh * 0.2;
        let scale = (sw / 140.0).floor().clamp(4.0, 9.0);
        self.title("FALLING WIZARD", ty, scale, Color::new(0.97, 0.93, 0.86, 1.0));
        draw_rectangle(cx - 40.0, ty + scale * 7.0 + 18.0, 80.0, 2.0, ACCENT);
        let mut y = sh * 0.48;
        let w = 260.0;
        self.ui.panel(cx - 150.0, y - 16.0, 300.0, 5.0 * 42.0 + 20.0);
        if self.ui.button("PLAY", cx, y, w) {
            self.game.cam = self.game.player.center();
            self.menu_x = 0.0;
            self.screen = Screen::Play;
        }
        y += 42.0;
        if self.ui.button("NEW WORLD", cx, y, w) {
            self.game = Game::new(random_seed());
            self.screen = Screen::Play;
        }
        y += 42.0;
        if self.ui.button("SETTINGS", cx, y, w) {
            self.back = Screen::Menu;
            self.screen = Screen::Settings;
        }
        y += 42.0;
        if self.ui.button("CONTROLS", cx, y, w) {
            self.back = Screen::Menu;
            self.screen = Screen::Controls;
        }
        y += 42.0;
        if self.ui.button("QUIT", cx, y, w) {
            std::process::exit(0);
        }
        self.ui.text(&format!("SEED {}", self.game.seed), 20.0, sh - 28.0, 1.0, MUTED);
        self.ui.text_right("V0.3", sw - 20.0, sh - 28.0, 1.0, MUTED);
    }

    fn pause(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        self.dim(0.55);
        let cx = sw * 0.5;
        self.title("PAUSED", sh * 0.24, 6.0, TEXT);
        let mut y = sh * 0.44;
        let w = 260.0;
        if self.ui.button("RESUME", cx, y, w) || self.take_esc() {
            self.screen = Screen::Play;
        }
        y += 42.0;
        if self.ui.button("SETTINGS", cx, y, w) {
            self.back = Screen::Pause;
            self.screen = Screen::Settings;
        }
        y += 42.0;
        if self.ui.button("CONTROLS", cx, y, w) {
            self.back = Screen::Pause;
            self.screen = Screen::Controls;
        }
        y += 42.0;
        if self.ui.button("MAIN MENU", cx, y, w) {
            self.menu_x = 0.0;
            self.screen = Screen::Menu;
        }
    }

    fn settings_screen(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        self.dim(0.6);
        let cx = sw * 0.5;
        self.title("SETTINGS", sh * 0.14, 5.0, TEXT);
        let (w, x) = (520.0, cx - 260.0);
        let mut y = sh * 0.3;
        self.ui.panel(x - 24.0, y - 20.0, w + 48.0, 8.0 * 40.0 + 40.0);
        self.ui.text("AUDIO", x, y - 6.0, 1.0, ACCENT);
        y += 12.0;
        let mut s = self.settings.clone();
        self.ui.slider("MASTER", x, y, w, &mut s.master);
        y += 40.0;
        self.ui.slider("EFFECTS", x, y, w, &mut s.effects);
        y += 40.0;
        self.ui.slider("AMBIENCE", x, y, w, &mut s.ambience);
        y += 52.0;
        self.ui.text("DISPLAY", x, y - 6.0, 1.0, ACCENT);
        y += 12.0;
        self.ui.toggle("SCREEN SHAKE", x, y, w, &mut s.shake);
        y += 40.0;
        self.ui.toggle("SHOW FPS", x, y, w, &mut s.show_fps);
        y += 40.0;
        if self.ui.toggle("FULLSCREEN  (F11)", x, y, w, &mut s.fullscreen) {
            set_fullscreen(s.fullscreen);
        }
        y += 40.0;
        self.ui.toggle("HIGH QUALITY LIGHTING", x, y, w, &mut s.hq_light);
        self.settings = s;
        y += 70.0;
        if self.ui.button("BACK", cx, y, 260.0) || self.take_esc() {
            self.settings.save();
            self.screen = self.back;
        }
    }

    fn controls_screen(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        self.dim(0.6);
        let cx = sw * 0.5;
        self.title("CONTROLS", sh * 0.1, 5.0, TEXT);
        let rows = [
            ("A D / ARROWS", "MOVE"),
            ("W / SPACE", "JUMP, WALL JUMP, HOLD TO FLY"),
            ("S", "GLIDE"),
            ("SHIFT", "DASH"),
            ("F", "GRAPPLING VINE"),
            ("LEFT MOUSE", "CAST SPELL"),
            ("RIGHT MOUSE", "DIG"),
            ("1-9 0 - = Q R G / WHEEL", "CHOOSE SPELL"),
            ("E", "OPEN CHEST / USE ALTAR"),
            ("T  /  /", "CHAT  /  COMMANDS  (/HELP)"),
            ("B", "SANDBOX BRUSH"),
            ("ESC  /  F11", "PAUSE  /  FULLSCREEN"),
        ];
        let (w, x) = (600.0, cx - 300.0);
        let mut y = sh * 0.24;
        self.ui.panel(x - 24.0, y - 18.0, w + 48.0, rows.len() as f32 * 28.0 + 24.0);
        for (k, a) in rows {
            self.ui.text(k, x, y, 2.0, ACCENT);
            self.ui.text_right(a, x + w, y, 2.0, TEXT);
            y += 28.0;
        }
        if self.ui.button("BACK", cx, y + 30.0, 260.0) || self.take_esc() {
            self.screen = self.back;
        }
    }

    fn dead(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        self.dim(0.5);
        let cx = sw * 0.5;
        self.title("YOU DIED", sh * 0.3, 7.0, DANGER);
        self.ui.text_center(&format!("GOLD COLLECTED  {}", self.game.player.gold), cx, sh * 0.3 + 80.0, 2.0, MUTED);
        if self.ui.button("RESPAWN", cx, sh * 0.55, 260.0) || is_key_pressed(KeyCode::Enter) {
            self.game.respawn();
            self.screen = Screen::Play;
        }
        if self.ui.button("MAIN MENU", cx, sh * 0.55 + 42.0, 260.0) {
            self.game.respawn();
            self.menu_x = 0.0;
            self.screen = Screen::Menu;
        }
    }
}
