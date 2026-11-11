mod audio;
mod cell;
mod fx;
mod game;
mod materials;
mod mobs;
mod noise;
mod planets;
mod player;
mod render;
mod creatures;
mod rift;
mod saves;
mod settings;
mod spells;
mod ui;
mod updater;
mod weather;
mod world;
mod voxel;
mod play3d;
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
        icon: Some(macroquad::miniquad::conf::Icon {
            small: *include_bytes!("../assets/icon_16.rgba"),
            medium: *include_bytes!("../assets/icon_32.rgba"),
            big: *include_bytes!("../assets/icon_64.rgba"),
        }),
        ..Default::default()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version") {
        println!("falling-wizard {}", if env!("FW_COMMIT").is_empty() { "(dev build)" } else { env!("FW_COMMIT") });
        return;
    }
    if args.iter().any(|a| a == "--update") {
        updater::update_now();
        return;
    }
    if args.iter().any(|a| a == "--bench") {
        bench();
        return;
    }
    // Big draw buffers so whole voxel chunk meshes go out in one call.
    let cfg = macroquad::conf::Conf {
        miniquad_conf: conf(),
        update_on: None,
        default_filter_mode: FilterMode::Linear,
        draw_call_vertex_capacity: 70_000,
        draw_call_index_capacity: 110_000,
    };
    macroquad::Window::from_config(cfg, run(args));
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
    Planets,
    Warp,
    NewWorld,
    Worlds,
    /// Falling through a rift: the inside of a black hole.
    Rift,
    /// Playing a 3D voxel world.
    Play3D,
}

const RIFT_TRIP: f32 = 11.0;
/// Portal travel: zoom into the portal, hyperspace, dive into the planet.
const WARP_IN: f32 = 1.1;
const WARP_TRIP: f32 = 4.7;
/// Coming down into the new world after a warp.
const ARRIVE: f32 = 1.8;

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
    /// Hide the HUD (for --photos).
    clean: bool,
    updater: Option<updater::Updater>,
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
    /// Highlighted suggestion, and the text Tab is cycling completions for.
    sugg: usize,
    tab_base: Option<String>,
    warp_t: f32,
    warp_dest: Option<planets::Planet>,
    rift_t: f32,
    /// Where the portal you stepped into is, so the warp can zoom into it.
    warp_from: (f32, f32),
    /// Just arrived somewhere: seconds since, and the colour you're coming down through.
    arrive: Option<(f32, [u8; 3])>,
    rift_dest: Option<(planets::Planet, Option<f32>)>,
    nw: game::WorldOptions,
    nw_seed: String,
    nw_focus: Option<u8>,
    picker_list: Vec<planets::Planet>,
    /// The world list: saves on disk, selected row, delete awaiting confirmation, scroll.
    worlds: Vec<saves::SaveInfo>,
    world_sel: usize,
    world_del: bool,
    world_scroll: usize,
    /// Esc pressed this frame and not yet handled (so one press can't open and close a menu).
    esc: bool,
    /// The 3D world being played, if any.
    w3: Option<play3d::Play3D>,
}

async fn run(args: Vec<String>) {
    // --autotest <secs> [shots-dir]: scripted, muted playthrough with screenshots and FPS stats.
    let autotest: Option<(f64, Option<String>)> = args
        .iter()
        .position(|a| a == "--autotest")
        .map(|i| (args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(14.0), args.get(i + 2).cloned()));
    // --photos <dir>: a muted, HUD-free tour that saves screenshots for the website.
    let photos: Option<String> = args.iter().position(|a| a == "--photos").and_then(|i| args.get(i + 1).cloned());
    let autotest = if photos.is_some() { Some((29.5, None)) } else { autotest };
    let mute = autotest.is_some() || args.iter().any(|a| a == "--mute");

    let settings = Settings::load();
    if settings.fullscreen && autotest.is_none() {
        set_fullscreen(true);
    }
    let seed = if autotest.is_some() { 7 } else { random_seed() };
    let audio = if mute { None } else { Audio::new().await };
    let mut app = App {
        clean: false,
        updater: if autotest.is_some() { None } else { Some(updater::Updater::start()) },
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
        sugg: 0,
        tab_base: None,
        warp_t: 0.0,
        warp_dest: None,
        rift_t: 0.0,
        warp_from: (0.0, 0.0),
        arrive: None,
        rift_dest: None,
        nw: game::WorldOptions::new(1),
        nw_seed: String::new(),
        nw_focus: None,
        picker_list: Vec::new(),
        worlds: Vec::new(),
        world_sel: 0,
        world_del: false,
        world_scroll: 0,
        esc: false,
        w3: None,
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

        let script = if let Some(dir) = &photos {
            app.clean = true;
            Some(photo_tour(&mut app, now, dir))
        } else {
            autotest.as_ref().map(|(secs, _)| script_controls(&mut app, now, *secs))
        };
        app.frame(dt, script);
        if let Some(path) = PENDING_SHOT.with(|p| p.borrow_mut().take()) {
            get_screen_data().export_png(&path);
        }

        if let Some((secs, dir)) = &autotest {
            if now > 1.5 {
                frame_times.push(get_frame_time());
                cpu_times.push(frame_start.elapsed().as_secs_f32() * 1000.0);
            }
            let shots = [
                (1.0, "new_world"),
                (3.6, "surface_mobs"),
                (9.6, "portal"),
                (11.0, "picker"),
                (16.4, "planet"),
                (18.25, "shatter"),
                (23.6, "earth_return"),
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

thread_local! {
    static PENDING_SHOT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Ask for a screenshot once the current frame has been drawn.
fn app_shot(path: &str) {
    PENDING_SHOT.with(|p| *p.borrow_mut() = Some(path.to_string()));
}

/// A scripted photo tour across Earth for the website, saving clean screenshots.
fn photo_tour(app: &mut App, t: f64, dir: &str) -> Controls {
    let g = &mut app.game;
    let ctl = Controls { aim: (g.player.center().0 + 30.0, g.player.center().1 - 5.0), ..Default::default() };
    if app.screen == Screen::Menu {
        app.screen = Screen::Play;
    }
    g.player.god = true;
    g.player.infinite_flight = true;
    g.toasts.clear();
    g.chat.clear();
    let stops: [(f64, &[&str], &str); 10] = [
        (0.5, &["/tp lava", "/time set noon", "/weather clear"], "caves"),
        (2.8, &["/tp ocean", "/time set evening"], "ocean"),
        (5.1, &["/tp jungle", "/time set noon"], "jungle"),
        (7.4, &["/tp volcano", "/time set 0.7"], "volcano"),
        (9.7, &["/tp desert", "/time set noon", "/weather windy"], "desert"),
        (12.0, &["/tp tundra", "/weather storm", "/time set night"], "storm"),
        (14.3, &["/tp forest", "/weather clear", "/time set 0.27"], "forest"),
        (16.6, &["/tp island", "/time set noon"], "sky"),
        (18.9, &["/tp space"], "space"),
        (21.2, &["/tp plains", "/summon lion 2", "/summon tiger", "/summon slime 3", "/summon demoneye 2"], "creatures"),
    ];
    for (k, (at, cmds, name)) in stops.iter().enumerate() {
        if t >= *at && t < at + 0.02 {
            for c in cmds.iter() {
                g.command(c);
            }
        }
        if t >= at + 1.9 && t < at + 1.92 {
            app_shot(&format!("{dir}/{k}_{name}.png"));
        }
    }
    for (k, (at, idx, name)) in [(23.5, 1usize, "toxic"), (26.2, 4usize, "ice")].iter().enumerate() {
        if t >= *at && t < at + 0.02 {
            let p = planets::Planet::nearest(0, 0, 8)[*idx].clone();
            g.enter_planet(p);
            g.command("/time set noon");
        }
        if t >= at + 2.0 && t < at + 2.02 {
            app_shot(&format!("{dir}/{}_{name}.png", 10 + k));
        }
    }
    g.toasts.clear();
    g.player.vy = g.player.vy.min(0.0);
    ctl
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
    if at(0.3, 0.9) && app.screen == Screen::Menu {
        app.nw = game::WorldOptions::new(42);
        app.nw_seed = "42".into();
        app.nw_focus = Some(0);
        app.screen = Screen::NewWorld;
    }
    if at(1.1, 1.12) {
        app.screen = Screen::Menu;
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
        app.chat_buf = "/tp v".into();
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
    if at(8.0, 8.02) {
        g.command("/tp portal");
    }
    if at(10.0, 10.02) {
        app.picker_list = planets::Planet::nearest(0, 0, 31);
        app.screen = Screen::Planets;
    }
    if at(11.5, 11.52) && app.picker_list.len() > 2 {
        app.warp_dest = Some(app.picker_list[1].clone());
        app.warp_t = 0.0;
        app.screen = Screen::Warp;
    }
    if at(18.0, 18.02) {
        if let Some(((x0, _, x1, y1), _)) = g.portals.first().cloned() {
            let (cx, cy) = ((x0 + x1) as f32 * 0.5, y1 as f32 - 6.0);
            g.fx.explode(&mut g.world, cx + 6.0, cy, 6, 0.3);
        }
    }
    if at(19.5, 19.52) {
        g.command("/tp earth");
    }
    let _ = warp;

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
        if self.screen == Screen::Play3D {
            self.frame3d(dt);
            return;
        }
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
                if std::mem::take(&mut self.game.picker) {
                    self.picker_list = planets::Planet::nearest(self.game.planet.gx, self.game.planet.gy, 31);
                    self.screen = Screen::Planets;
                }
                if let Some(dest) = self.game.travel.take() {
                    let (px, py) = self.game.player.center();
                    self.warp_from = self
                        .game
                        .portals
                        .iter()
                        .map(|(r, _)| r)
                        .find(|r| px >= r.0 as f32 - 2.0 && px <= r.2 as f32 + 3.0 && py >= r.1 as f32 - 2.0 && py <= r.3 as f32 + 3.0)
                        .map(|r| ((r.0 + r.2) as f32 * 0.5 + 0.5, (r.1 + r.3) as f32 * 0.5 + 0.5))
                        .unwrap_or((px, py));
                    self.warp_dest = Some(dest);
                    self.warp_t = 0.0;
                    self.screen = Screen::Warp;
                }
                if let Some(dest) = self.game.rift_travel.take() {
                    self.rift_dest = Some(dest);
                    self.rift_t = 0.0;
                    self.screen = Screen::Rift;
                }
                let slash = is_key_pressed(KeyCode::Slash);
                if (is_key_pressed(KeyCode::T) || slash) && self.screen == Screen::Play {
                    while get_char_pressed().is_some() {}
                    self.chat_buf = if slash { "/".into() } else { String::new() };
                    self.screen = Screen::Chat;
                }
            }
            Screen::Planets => {
                let ctl = Controls { aim: self.game.aim, ..Default::default() };
                self.game.update(dt, &ctl, false);
                if self.take_esc() || is_mouse_button_pressed(MouseButton::Right) {
                    self.screen = Screen::Play;
                }
            }
            Screen::Rift => {
                let before = self.rift_t;
                self.rift_t += dt;
                let crossed = |at: f32| before < at && self.rift_t >= at;
                use audio::Sfx;
                for (at, sfx) in [(0.05, Sfx::Whoosh), (2.9, Sfx::Thunder), (5.5, Sfx::Whoosh), (8.3, Sfx::Boom), (8.45, Sfx::Thunder), (10.2, Sfx::Chime)] {
                    if crossed(at) {
                        self.game.sounds.push(sfx);
                    }
                }
                if self.rift_t >= RIFT_TRIP || (self.rift_t > 1.0 && self.take_esc()) {
                    if let Some((dest, x)) = self.rift_dest.take() {
                        self.game.arrive_through_rift(dest, x);
                        self.arrive = Some((0.0, [255, 246, 228]));
                    }
                    self.screen = Screen::Play;
                }
            }
            Screen::Warp => {
                self.warp_t += dt;
                if self.warp_t < WARP_IN {
                    // The world keeps going while the portal drinks you in.
                    let ctl = Controls { aim: self.game.aim, ..Default::default() };
                    self.game.update(dt, &ctl, true);
                }
                if self.warp_t >= WARP_TRIP {
                    if let Some(dest) = self.warp_dest.take() {
                        let sky = dest.sky_horizon;
                        self.game.enter_planet(dest);
                        self.arrive = Some((0.0, sky));
                    }
                    self.screen = Screen::Play;
                }
            }
            Screen::Chat => {
                let ctl = Controls { aim: self.game.aim, ..Default::default() };
                self.game.update(dt, &ctl, false);
                while let Some(c) = get_char_pressed() {
                    if !c.is_control() && self.chat_buf.len() < 120 {
                        self.chat_buf.push(c);
                        self.tab_base = None;
                        self.sugg = 0;
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    self.chat_buf.pop();
                    self.tab_base = None;
                    self.sugg = 0;
                }
                // Minecraft-style: arrows move the highlight, Tab inserts it (and keeps cycling).
                let base = self.tab_base.clone().unwrap_or_else(|| self.chat_buf.clone());
                let (start, opts) = game::suggest(&base);
                let apply = |sugg: usize, buf: &mut String| {
                    let pick = &opts[sugg % opts.len()];
                    let space = if start == 1 && game::usage(&format!("/{}", pick)).map_or(false, |u| u.contains('<') || u.contains('[')) { " " } else { "" };
                    *buf = format!("{}{}{}", &base[..start], pick, space);
                };
                let down = is_key_pressed(KeyCode::Down);
                let up = is_key_pressed(KeyCode::Up);
                if !opts.is_empty() && (down || up) {
                    let n = opts.len();
                    self.sugg = if down { (self.sugg + 1) % n } else { (self.sugg + n - 1) % n };
                    if self.tab_base.is_some() {
                        apply(self.sugg, &mut self.chat_buf);
                    }
                } else if up {
                    if let Some(h) = self.history.last() {
                        self.chat_buf = h.clone();
                        self.tab_base = None;
                        self.sugg = 0;
                    }
                } else if is_key_pressed(KeyCode::Tab) && !opts.is_empty() {
                    if self.tab_base.is_some() {
                        self.sugg = (self.sugg + 1) % opts.len();
                    }
                    apply(self.sugg, &mut self.chat_buf);
                    self.tab_base = if opts.len() > 1 { Some(base.clone()) } else { None };
                }
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                    let line = std::mem::take(&mut self.chat_buf);
                    self.tab_base = None;
                    self.sugg = 0;
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
        if let Some(a) = &mut self.arrive {
            a.0 += dt;
            if a.0 > ARRIVE {
                self.arrive = None;
            }
        }
        let descend = self.arrive.map(|(t, _)| (1.0 - t / ARRIVE).max(0.0)).unwrap_or(0.0);
        let view = View::new(self.game.cam.0, self.game.cam.1, sw, sh, self.game.zoom * (1.0 + descend.powi(3) * 4.0));
        let shake = if self.settings.shake && self.game.fx.shake > 0.05 && self.screen == Screen::Play {
            let s = self.game.fx.shake;
            (self.shake_rng.rangef(-s, s), self.shake_rng.rangef(-s, s))
        } else {
            (0.0, 0.0)
        };
        clear_background(BLACK);
        let time = self.game.time;
        if self.screen == Screen::Rift {
            self.draw_rift_trip();
        } else if self.screen == Screen::Warp && self.warp_t >= WARP_IN {
            self.draw_warp();
        } else if self.screen == Screen::Warp {
            self.draw_warp_in(sw, sh, time);
        } else {
            self.renderer.draw(&mut self.game, &view, shake, time);
            if let Some((_, col)) = self.arrive {
                self.draw_arrival(descend, col, sw, sh);
            }
        }

        // ----- sound -----
        let sounds: Vec<Sfx> = self.game.sounds.drain(..).collect();
        if let Some(a) = &mut self.audio {
            if matches!(self.screen, Screen::Play | Screen::Dead | Screen::Chat | Screen::Menu | Screen::Planets | Screen::Rift) {
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
                if !self.clean {
                    self.ui.hud(&mut self.game, &view, self.fps, self.settings.show_fps);
                    self.ui.chat(&self.game, None, self.game.time, 0);
                    self.crosshair(&view);
                }
                show_mouse(false);
            }
            Screen::Planets => {
                self.ui.hud(&mut self.game, &view, self.fps, false);
                if let Some(i) = self.ui.planet_picker(&self.game, &self.picker_list, self.game.time) {
                    let p = self.picker_list[i].clone();
                    self.game.toast(format!("PORTAL WAND TUNED TO {}", p.name), p.color());
                    self.game.dest = p;
                    self.screen = Screen::Play;
                }
            }
            Screen::Warp | Screen::Rift | Screen::Play3D => {}
            Screen::NewWorld => self.new_world_screen(),
            Screen::Worlds => self.worlds_screen(),
            Screen::Chat => {
                self.ui.hud(&mut self.game, &view, self.fps, self.settings.show_fps);
                let src = self.tab_base.clone().unwrap_or_else(|| self.chat_buf.clone());
                self.ui.chat(&self.game, Some((&self.chat_buf, &src)), self.game.time, self.sugg);
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
            KeyCode::P,
            KeyCode::V,
            KeyCode::C,
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
                broom: is_key_pressed(KeyCode::M),
                aim,
            },
            aim,
            primary: is_mouse_button_down(MouseButton::Left) && !on_hud,
            secondary: is_mouse_button_down(MouseButton::Right),
            paint: is_mouse_button_down(MouseButton::Middle) || (g.sandbox && is_key_down(KeyCode::Space)),
            interact: is_key_pressed(KeyCode::E),
            secondary_pressed: is_mouse_button_pressed(MouseButton::Right),
        }
    }

    /// Whether the cursor is over the hotbar strip at the bottom of the screen.
    fn ui_hover_hud(&self) -> bool {
        let (mx, my) = mouse_position();
        let (sw, sh) = (screen_width(), screen_height());
        my > sh - 70.0 && (mx - sw * 0.5).abs() < 430.0
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

    /// Through a rift: the event horizon looms with its glowing disk, you cross it into
    /// a twisting tunnel where the wizard stretches like spaghetti and time runs away,
    /// everything crushes into the singularity, and a white hole spits you out.
    fn draw_rift_trip(&mut self) {
        use std::f32::consts::TAU;
        let Some((dest, earth_x)) = self.rift_dest.clone() else { return };
        let t = self.rift_t;
        let (sw, sh) = (screen_width(), screen_height());
        let big = sw.max(sh);
        let px = 3.0;
        let dot = |x: f32, y: f32, c: Color| draw_rectangle((x / px).floor() * px, (y / px).floor() * px, px, px, c);
        let rgba = |c: [u8; 3], a: f32| Color::from_rgba(c[0], c[1], c[2], (a.clamp(0.0, 1.0) * 255.0) as u8);
        let unit = |k: i32, salt: u32| (noise::hash3(k, salt as i32, 0xB1AC) % 10000) as f32 / 10000.0;
        let shake = if t < 3.0 { (t / 3.0).powi(3) * 6.0 } else if t < 8.4 { 2.0 + ((t - 3.6) / 4.0).clamp(0.0, 1.0) * 6.0 } else { (1.0 - (t - 8.4)).max(0.0) * 10.0 };
        let (cx, cy) = (sw * 0.5 + (t * 53.0).sin() * shake, sh * 0.5 + (t * 41.0).cos() * shake);
        clear_background(Color::from_rgba(2, 1, 6, 255));
        let caption;

        if t < 3.6 {
            // ---- The event horizon looms ----
            let a_t = (t / 3.0).min(1.0);
            let rs = sh * 0.07 + a_t.powf(2.4) * big * 0.8;
            for k in 0..6 {
                draw_circle(cx, cy, rs * (1.25 + k as f32 * 0.22), Color::new(1.0, 0.5 + k as f32 * 0.04, 0.2, 0.05));
            }
            // Stars, bent around the hole (and a second, squashed image near the ring).
            for k in 0..700 {
                let a = unit(k, 1) * TAU + t * 0.04;
                let d0 = unit(k, 2) * big * 0.85 + 4.0;
                let d = d0 + rs * rs * 1.1 / d0;
                let c = if k % 5 == 0 { [170, 200, 255] } else if k % 7 == 0 { [255, 210, 170] } else { [235, 235, 255] };
                let tw = 0.5 + 0.5 * (t * 3.0 + k as f32).sin();
                dot(cx + a.cos() * d, cy + a.sin() * d, rgba(c, 0.35 + tw * 0.5));
                let d2 = rs * (1.06 + 0.25 * rs / (d0 + rs));
                dot(cx - a.cos() * d2, cy - a.sin() * d2, rgba(c, 0.3));
            }
            let disk = |k: i32, front: bool| -> Option<(f32, f32, Color)> {
                let rr = 1.25 + unit(k, 3).powi(2) * 1.9;
                let a = unit(k, 4) * TAU + t * 1.6 / rr;
                if front != (a.sin() > 0.0) {
                    return None;
                }
                let heat = 1.0 - (rr - 1.25) / 1.9;
                let beam = 0.45 + 0.55 * (1.0 - a.cos()) * 0.5 * 2.0;
                let c = [255, (120.0 + heat * 125.0) as u8, (40.0 + heat * 180.0) as u8];
                let (x, y) = (cx + a.cos() * rr * rs, cy + a.sin() * rr * rs * 0.14 - a.cos() * rs * 0.05);
                Some((x, y, rgba(c, (0.35 + beam * 0.65) * (0.5 + heat * 0.5))))
            };
            // The far side of the disk, lensed up over the top and under the bottom.
            for k in 0..2000 {
                let rr = 1.08 + unit(k, 5).powi(2) * 0.55;
                let a = unit(k, 6) * TAU + t * 0.9;
                let beam = 0.35 + 0.65 * (0.5 - 0.5 * a.cos());
                let heat = 1.0 - (rr - 1.08) / 0.55;
                let c = [255, (140.0 + heat * 110.0) as u8, (60.0 + heat * 170.0) as u8];
                dot(cx + a.cos() * rr * rs, cy + a.sin() * rr * rs * 0.92, rgba(c, (0.3 + beam * 0.7) * (0.35 + heat * 0.65)));
            }
            for k in 0..3200 {
                if let Some((x, y, c)) = disk(k, false) {
                    dot(x, y, c);
                }
            }
            draw_circle(cx, cy, rs, Color::from_rgba(0, 0, 0, 255));
            for k in 0..240 {
                let a = k as f32 / 240.0 * TAU;
                let flick = 0.6 + 0.4 * (a * 3.0 + t * 6.0).sin();
                dot(cx + a.cos() * rs * 1.02, cy + a.sin() * rs * 1.02, rgba([255, 236, 210], flick));
            }
            for k in 0..3200 {
                if let Some((x, y, c)) = disk(k, true) {
                    dot(x, y, c);
                }
            }
            if t > 2.9 {
                // Crossing over: black, a fading ring, and the signal breaking up.
                let c = ((t - 2.9) / 0.35).min(1.0);
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, c));
                for k in 0..14 {
                    let y = unit(k + (t * 30.0) as i32 * 31, 7) * sh;
                    let col = if k % 2 == 0 { [180, 120, 255] } else { [120, 220, 255] };
                    draw_rectangle(0.0, y, sw, 2.0 + unit(k, 8) * 6.0, rgba(col, 0.25 * c));
                }
                caption = ("EVENT HORIZON", Color::from_rgba(220, 200, 255, 255));
            } else {
                caption = ("FALLING TOWARDS THE EVENT HORIZON", Color::from_rgba(255, 210, 160, 255));
            }
        } else if t < 8.4 {
            // ---- Inside: a twisting tunnel, the sky crushed into one blue point ahead ----
            let p = ((t - 3.6) / 4.0).clamp(0.0, 1.0);
            let crush = ((t - 7.6) / 0.8).clamp(0.0, 1.0);
            let squeeze = (1.0 - crush).powi(2);
            for k in 0..6 {
                let r = (6.0 + p * 26.0) * (1.0 + k as f32 * 0.7) * (1.0 - crush * 0.6);
                draw_circle(cx, cy, r, Color::new(0.6, 0.75, 1.0, 0.09 + p * 0.05));
            }
            draw_circle(cx, cy, 3.0 + p * 5.0, Color::new(0.9, 0.95, 1.0, 1.0));
            let speed = 0.35 + p * 0.9;
            for i in 0..28 {
                let z = (i as f32 / 28.0 - (t - 3.6) * speed).rem_euclid(1.0).max(0.015);
                let rad = 22.0 / z * squeeze;
                if rad > big {
                    continue;
                }
                let twist = i as f32 * 0.45 + t * 1.3 + 0.6 / z;
                let near = 1.0 - z;
                for j in 0..44 {
                    let a = j as f32 / 44.0 * TAU + twist;
                    let wob = 1.0 + 0.14 * (3.0 * a + t * 2.0).sin();
                    let (x, y) = (cx + a.cos() * rad * wob, cy + a.sin() * rad * wob * 0.82);
                    let c = planets::hue_rgb(0.98 - near * 0.3, 0.75, 1.0);
                    dot(x + 3.0, y, rgba([255, 60, 90], near * 0.25));
                    dot(x - 3.0, y, rgba([60, 140, 255], near * 0.25));
                    dot(x, y, rgba(c, 0.25 + near * 0.75));
                }
            }
            // Bits of the world, stretched into long strands as they fall past.
            let mats: [[u8; 3]; 6] = [[226, 196, 110], [88, 160, 70], [130, 126, 136], [70, 130, 230], [255, 120, 40], [120, 84, 60]];
            for k in 0..150 {
                let a = unit(k, 9) * TAU + t * 0.2;
                let f = (unit(k, 10) + (t - 3.6) * (0.5 + p)).fract();
                let d = f * f * big * 0.75 * squeeze;
                let len = d * (0.15 + p * 0.5);
                let c = mats[k as usize % mats.len()];
                let (ux, uy) = (a.cos(), a.sin());
                draw_line(cx + ux * d, cy + uy * d, cx + ux * (d + len), cy + uy * (d + len), 2.0 + f * 3.0, rgba(c, 0.25 + f * 0.6));
            }
            // The wizard, spaghettified.
            if crush < 1.0 {
                const SPR: [&[u8; 5]; 10] = [b".h...", b".hh..", b".hsh.", b"hhhhh", b".ffe.", b".www.", b"rrwrr", b".rrr.", b".rrr.", b".b.b."];
                let base = (sh / 60.0).max(4.0);
                let sx = base * (1.0 - p * 0.8) * squeeze.max(0.05);
                let sy = base * (1.0 + p * p * 7.0) * squeeze.max(0.05);
                let (w0, h0) = (5.0 * sx, 10.0 * sy);
                for (row, line) in SPR.iter().enumerate() {
                    let bend = (row as f32 * 0.8 + t * 7.0).sin() * p * 14.0;
                    for (col, ch) in line.iter().enumerate() {
                        let c = match ch {
                            b'h' => [92, 70, 196],
                            b's' => [255, 220, 110],
                            b'f' => [236, 196, 160],
                            b'e' => [30, 20, 30],
                            b'w' => [230, 230, 236],
                            b'r' => [64, 50, 160],
                            b'b' => [60, 44, 40],
                            _ => continue,
                        };
                        let (x, y) = (cx - w0 * 0.5 + col as f32 * sx + bend, cy - h0 * 0.5 + row as f32 * sy);
                        let ghost = 2.0 + p * 10.0;
                        draw_rectangle(x - ghost, y, sx, sy, rgba([255, 40, 80], 0.3));
                        draw_rectangle(x + ghost, y, sx, sy, rgba([40, 160, 255], 0.3));
                        draw_rectangle(x, y, sx, sy, rgba(c, 1.0 - crush));
                    }
                }
            }
            // Instruments giving up.
            let dil = p * 34.0;
            let glitch = |s: &str, k: i32| if unit(k + (t * 12.0) as i32, 11) < p * 0.25 { s.chars().rev().collect::<String>() } else { s.to_string() };
            self.ui.text(&glitch(&format!("TIME DILATION  x {:.1}E{}", 1.0 + dil.fract() * 9.0, dil as i32), 1), 24.0, 24.0, 2.0, Color::from_rgba(150, 220, 255, 220));
            self.ui.text(&glitch(&format!("TO SINGULARITY  {:.0} KM", (1.0 - p).powi(3) * 41_000.0), 2), 24.0, 52.0, 2.0, Color::from_rgba(255, 170, 200, 220));
            self.ui.text(&glitch("OUTSIDE UNIVERSE  -  BLUESHIFTED TO A POINT", 3), 24.0, 80.0, 1.0, MUTED);
            caption = if crush > 0.0 { ("SINGULARITY", Color::from_rgba(255, 255, 255, 255)) } else { ("INSIDE THE BLACK HOLE", Color::from_rgba(190, 170, 255, 255)) };
            if crush > 0.0 {
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, crush * 0.85));
                let r = 2.0 + crush * crush * 10.0;
                for k in 0..5 {
                    draw_circle(cx, cy, r * (1.0 + k as f32 * 1.4), Color::new(1.0, 1.0, 1.0, 0.18 * crush));
                }
                draw_circle(cx, cy, r, WHITE);
            }
        } else {
            // ---- A white hole: a new sky blooms outwards and you're thrown out ----
            let e = ((t - 8.7) / 2.3).clamp(0.0, 1.0);
            let ease = 1.0 - (1.0 - e).powi(3);
            for k in 0..600 {
                let arm = (k % 3) as f32;
                let r = (k / 3) as f32 * 2.3 * ease;
                let a = arm * 2.094 + r * 0.011 - t * 0.3;
                let j = unit(k, 12) * 14.0 - 7.0;
                let c = planets::hue_rgb(0.62 + arm * 0.08 + r * 0.0004, 0.5, 0.9);
                dot(cx + a.cos() * (r + j), cy + a.sin() * (r + j) * 0.6, rgba(c, 0.35));
            }
            for k in 0..500 {
                let a = unit(k, 13) * TAU;
                let sp = 0.3 + unit(k, 14) * 0.9;
                let d = ease * big * 0.65 * sp;
                let c = planets::hue_rgb(unit(k, 15), 0.35, 1.0);
                let len = (1.0 - e) * 40.0 + 3.0;
                draw_line(cx + a.cos() * d, cy + a.sin() * d, cx + a.cos() * (d + len), cy + a.sin() * (d + len), 3.0, rgba(c, 1.0 - e * 0.6));
            }
            if e > 0.25 {
                let r = ((e - 0.25) / 0.75).powi(2) * sh * 0.3 + 4.0;
                self.ui.planet_sphere(&dest, cx, cy, r, 6.0, t);
            }
            let dc = dest.color();
            let (title, sub) = match earth_x {
                Some(x) => ("SOMEWHERE ON EARTH".to_string(), format!("{} BLOCKS FROM SPAWN", x.abs() as i64)),
                None => (dest.name.clone(), format!("{}   -   {:.0} LIGHT YEARS AWAY", dest.kind.name(), self.game.planet.light_years(&dest))),
            };
            if e > 0.2 {
                let a = ((e - 0.2) / 0.2).min(1.0);
                self.ui.text_center(&title, cx, sh - 96.0, 3.0, rgba(dc, a));
                self.ui.text_center(&sub, cx, sh - 60.0, 2.0, Color::new(0.75, 0.75, 0.82, a));
            }
            caption = ("WHITE HOLE", Color::from_rgba(255, 245, 220, 255));
            if t < 8.85 {
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(1.0, 1.0, 1.0, 1.0 - (t - 8.4) / 0.45 * 0.3));
            }
        }
        if !caption.0.is_empty() {
            self.ui.text_center(caption.0, sw * 0.5, 28.0, 2.0, caption.1);
        }
        if t < 0.4 {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.75, 0.6, 1.0, 1.0 - t / 0.4));
        }
        if t > RIFT_TRIP - 0.5 {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(1.0, 1.0, 1.0, ((t - (RIFT_TRIP - 0.5)) / 0.5).min(1.0)));
        }
        if t > 1.0 && t < RIFT_TRIP - 1.0 {
            self.ui.text_right("ESC TO SKIP", sw - 20.0, sh - 28.0, 1.0, Color::new(1.0, 1.0, 1.0, 0.3));
        }
    }

    /// Stepping into a portal: the view dives into the opening while the world spirals
    /// in after you, then the portal's colour takes over.
    fn draw_warp_in(&mut self, sw: f32, sh: f32, time: f32) {
        let p = (self.warp_t / WARP_IN).min(1.0);
        let ease = p * p * (3.0 - 2.0 * p);
        let (fx, fy) = self.warp_from;
        let (cx0, cy0) = self.game.cam;
        let cam = (cx0 + (fx - cx0) * ease, cy0 + (fy - cy0) * ease);
        let view = View::new(cam.0, cam.1, sw, sh, self.game.zoom * (1.0 + p.powf(2.2) * 7.0));
        self.renderer.draw(&mut self.game, &view, (0.0, 0.0), time);
        let hue = self.warp_dest.as_ref().map(|d| d.hue).unwrap_or(0.6);
        let (sx, sy) = view.to_screen(fx, fy);
        // Everything spirals into the portal.
        for k in 0..260 {
            let h = noise::hash3(k, 21, 9);
            let a0 = (h % 10000) as f32 / 10000.0 * std::f32::consts::TAU;
            let f = (((h >> 14) % 1000) as f32 / 1000.0 + self.warp_t * 0.9).fract();
            let d = (1.0 - f) * sw.max(sh) * 0.7;
            let a = a0 + f * 3.0;
            let c = planets::hue_rgb(hue + ((h >> 4) % 10) as f32 * 0.01, 0.6, 1.0);
            let len = 6.0 + f * 30.0 * p;
            let (ux, uy) = (a.cos(), a.sin());
            draw_line(sx + ux * d, sy + uy * d, sx + ux * (d + len), sy + uy * (d + len), 3.0, Color::from_rgba(c[0], c[1], c[2], (p * f * 220.0) as u8));
        }
        let c = planets::hue_rgb(hue, 0.7, 1.0);
        let a = ((p - 0.55) / 0.45).clamp(0.0, 1.0);
        draw_rectangle(0.0, 0.0, sw, sh, Color::from_rgba(c[0], c[1], c[2], (a * a * 255.0) as u8));
    }

    /// Coming down into a new world: zoomed in, out of the sky's colour, wind rushing up.
    fn draw_arrival(&self, descend: f32, col: [u8; 3], sw: f32, sh: f32) {
        if descend <= 0.0 {
            return;
        }
        let a = descend.powf(1.6);
        draw_rectangle(0.0, 0.0, sw, sh, Color::from_rgba(col[0], col[1], col[2], (a * 255.0) as u8));
        let t = 1.0 - descend;
        for k in 0..36 {
            let x = (noise::hash3(k, 8, 1) % 1000) as f32 / 1000.0 * sw;
            let y = ((noise::hash3(k, 9, 1) % 1000) as f32 / 1000.0 - t * 2.5).rem_euclid(1.0) * sh;
            let len = 30.0 + (noise::hash3(k, 10, 1) % 90) as f32;
            draw_rectangle((x / 3.0).floor() * 3.0, y, 3.0, len * descend, Color::new(1.0, 1.0, 1.0, 0.35 * descend));
        }
    }

    /// Lightspeed: a turning galaxy, stars stretching into streaks, then the planet rushing up.
    fn draw_warp(&mut self) {
        let Some(dest) = self.warp_dest.clone() else { return };
        let t = self.warp_t - WARP_IN;
        let (sw, sh) = (screen_width(), screen_height());
        let (cx, cy) = (sw * 0.5, sh * 0.5);
        clear_background(Color::from_rgba(3, 2, 10, 255));
        let pc = planets::hue_rgb(dest.hue, 0.7, 1.0);
        // A tunnel of portal-coloured rings, twisting as you go.
        for i in 0..18 {
            let z = (i as f32 / 18.0 - t * 0.6).rem_euclid(1.0).max(0.02);
            let rad = 30.0 / z;
            if rad > sw.max(sh) {
                continue;
            }
            let near = 1.0 - z;
            let fade = 1.0 - ((t - 2.2) / 0.8).clamp(0.0, 1.0);
            for j in 0..36 {
                let a = j as f32 / 36.0 * std::f32::consts::TAU + i as f32 * 0.5 + t * 0.9;
                draw_rectangle((cx + a.cos() * rad) / 4.0 * 4.0, (cy + a.sin() * rad * 0.8) / 4.0 * 4.0, 4.0, 4.0, Color::from_rgba(pc[0], pc[1], pc[2], (near * 120.0 * fade) as u8));
            }
        }
        let px = 4.0;
        let dot = |x: f32, y: f32, c: Color| draw_rectangle((x / px).floor() * px, (y / px).floor() * px, px, px, c);
        // Galaxy spiral.
        for k in 0..600 {
            let arm = (k % 3) as f32;
            let r = (k / 3) as f32 * 2.2;
            let a = arm * 2.094 + r * 0.011 + t * 0.35;
            let jitter = (noise::hash3(k, 1, 9) % 100) as f32 / 100.0 * 14.0 - 7.0;
            let c = planets::hue_rgb(0.62 + arm * 0.08 + r * 0.0004, 0.5, 0.9);
            dot(cx + a.cos() * (r + jitter), cy + a.sin() * (r + jitter) * 0.6, Color::from_rgba(c[0], c[1], c[2], 70));
        }
        // Lightspeed streaks.
        let speed = (t / 1.2).min(1.0).powi(2) * (1.0 - ((t - 2.6) / 0.8).clamp(0.0, 1.0) * 0.8);
        let dc = dest.color();
        for k in 0..320 {
            let h = noise::hash3(k, 7, 3);
            let ang = (h % 10000) as f32 / 10000.0 * std::f32::consts::TAU;
            let z = ((h >> 14) % 1000) as f32 / 1000.0;
            let z = (z + t * (0.25 + speed * 1.4)).fract();
            let d = z * z * z * sw.max(sh) * 0.75;
            let len = 4.0 + speed * z * 120.0;
            let col = if k % 4 == 0 { Color::from_rgba(dc[0], dc[1], dc[2], 230) } else { Color::from_rgba(220, 230, 255, (120.0 + z * 135.0) as u8) };
            let steps = (len / px) as i32 + 1;
            for s in 0..steps {
                let dd = d + s as f32 * px;
                dot(cx + ang.cos() * dd, cy + ang.sin() * dd, col);
            }
        }
        // The destination rushes up, fills the view, and you dive into its sky.
        if t > 1.7 {
            let k = ((t - 1.7) / 1.7).min(1.0);
            let r = k.powi(3) * sw.max(sh) * 1.3 + 6.0;
            // Atmosphere glow.
            let sky = dest.sky_horizon;
            for g in 0..4 {
                draw_circle(cx, cy, r * (1.05 + g as f32 * 0.06), Color::from_rgba(sky[0], sky[1], sky[2], (40.0 - g as f32 * 8.0) as u8));
            }
            self.ui.planet_sphere(&dest, cx, cy, r, (6.0 + k * 10.0).floor(), t);
        }
        if t > 3.1 {
            let sky = dest.sky_horizon;
            let a = ((t - 3.1) / 0.5).min(1.0);
            draw_rectangle(0.0, 0.0, sw, sh, Color::from_rgba(sky[0], sky[1], sky[2], (a * 255.0) as u8));
            // Clouds tearing past as you drop through the air.
            for k in 0..40 {
                let y = ((noise::hash3(k, 3, 5) % 1000) as f32 / 1000.0 - t * 1.6).rem_euclid(1.0) * sh;
                let x = (noise::hash3(k, 4, 5) % 1000) as f32 / 1000.0 * sw;
                let w = 40.0 + (noise::hash3(k, 6, 5) % 120) as f32;
                draw_rectangle((x / 4.0).floor() * 4.0, (y / 4.0).floor() * 4.0, w, 4.0 + (k % 3) as f32 * 4.0, Color::new(1.0, 1.0, 1.0, 0.5 * a));
            }
        }
        if t < 0.35 {
            draw_rectangle(0.0, 0.0, sw, sh, Color::from_rgba(pc[0], pc[1], pc[2], ((1.0 - t / 0.35) * 255.0) as u8));
        }
        let ly = self.game.planet.light_years(&dest);
        self.ui.text_center(&format!("WARPING TO {}", dest.name), cx, sh - 90.0, 3.0, Color::from_rgba(dc[0], dc[1], dc[2], 255));
        self.ui.text_center(&format!("{}   -   {:.1} LIGHT YEARS", dest.kind.name(), ly), cx, sh - 56.0, 2.0, MUTED);
    }

    fn menu(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.02, 0.01, 0.05, 0.2));
        let cx = sw * 0.5;
        let ty = sh * 0.2;
        let scale = (sw / 140.0).floor().clamp(4.0, 9.0);
        // Interactive background: sparkles follow the mouse, clicks cast spells into the world.
        let view = View::new(self.game.cam.0, self.game.cam.1, sw, sh, 1.0);
        let (mx, my) = mouse_position();
        let (wx, wy) = view.to_world(mx, my);
        let t = self.game.time;
        let hue = (t * 0.1).fract();
        let c = planets::hue_rgb(hue, 0.6, 1.0);
        let (vx, vy) = (self.shake_rng.rangef(-20.0, 20.0), self.shake_rng.rangef(-30.0, 5.0));
        self.game.fx.mote(wx, wy, vx, vy, 0.7, c, 1.2, 30.0, 0.0);
        let panel = (cx - 150.0, sh * 0.48 - 16.0, 300.0, 5.0 * 42.0 + 20.0);
        let over_panel = mx > panel.0 && mx < panel.0 + panel.2 && my > panel.1 && my < panel.1 + panel.3;
        if is_mouse_button_pressed(MouseButton::Left) && !over_panel && my > ty + scale * 9.0 {
            use spells::Card::*;
            let pool = [Fireball, Lightning, Bomb, Frost, Tornado, Seeds, AcidBall, Hurricane, BlackHole];
            let card = pool[self.shake_rng.range(0, pool.len() as i32) as usize];
            let from = (wx + self.shake_rng.rangef(-40.0, 40.0), wy - 60.0);
            let ang = (wy - from.1).atan2(wx - from.0);
            let shot = spells::Shot { card, mods: Default::default(), payload: None };
            let g = &mut self.game;
            g.spells.fire(&shot, from, ang, &mut g.world, &mut g.fx, &mut g.mobs);
        }
        // Waving title; letters hop when the mouse passes over them.
        let title = "FALLING WIZARD";
        let w = self.ui.font.width(title, scale);
        let mut x = cx - w * 0.5;
        for (i, ch) in title.chars().enumerate() {
            let s = ch.to_string();
            let lw = 6.0 * scale;
            let near = (mx - (x + lw * 0.5)).abs() < lw * 0.6 && (my - (ty + 3.5 * scale)).abs() < scale * 6.0;
            let bob = (t * 2.2 + i as f32 * 0.45).sin() * scale * 0.8 - if near { scale * 2.5 } else { 0.0 };
            let tint = planets::hue_rgb(t * 0.05 + i as f32 * 0.035, if near { 0.6 } else { 0.18 }, 1.0);
            let col = Color::from_rgba(tint[0], tint[1], tint[2], 255);
            self.ui.font.draw(&s, x + scale, ty + bob + scale, scale, Color::new(0.08, 0.04, 0.14, 0.8));
            self.ui.font.draw(&s, x, ty + bob, scale, col);
            x += lw;
        }
        self.ui.text_center("CLICK ANYWHERE TO CAST A SPELL", cx, ty + scale * 7.0 + 30.0, 1.0, MUTED);
        draw_rectangle(cx - 40.0, ty + scale * 7.0 + 18.0, 80.0, 2.0, ACCENT);
        let mut y = sh * 0.48;
        let _ = TEXT;
        let w = 260.0;
        self.ui.panel(cx - 150.0, y - 16.0, 300.0, 5.0 * 42.0 + 20.0);
        if self.ui.button("PLAY", cx, y, w) {
            self.open_worlds();
        }
        y += 42.0;
        if self.ui.button("NEW WORLD", cx, y, w) {
            self.worlds = saves::list();
            self.start_new_world();
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
            self.game.save();
            std::process::exit(0);
        }
        let mode = if self.game.options.creative { "CREATIVE" } else { "SURVIVAL" };
        self.ui.text(&format!("{}  -  {}  -  SEED {}", self.game.options.name.to_uppercase(), mode, self.game.seed), 20.0, sh - 28.0, 1.0, MUTED);
        self.update_badge(sw, sh);
    }

    /// Bottom-right of the main menu: version and one-click updates.
    fn update_badge(&mut self, sw: f32, sh: f32) {
        use updater::Status;
        let status = self.updater.as_ref().map(|u| u.status()).unwrap_or(Status::Unknown);
        let (label, color, clickable) = match status {
            Status::Checking => ("CHECKING FOR UPDATES", MUTED, false),
            Status::UpToDate => ("UP TO DATE  -  CHECK FOR UPDATES", MUTED, true),
            Status::Unknown => ("CHECK FOR UPDATES", MUTED, true),
            Status::Available => ("UPDATE AVAILABLE  -  CLICK TO INSTALL", ACCENT, true),
            Status::Updating => ("UPDATING IN THE BACKGROUND...", ACCENT, false),
            Status::Ready => ("UPDATE READY  -  CLICK TO RESTART", Color::from_rgba(140, 230, 150, 255), true),
            Status::Failed => ("UPDATE FAILED  -  CLICK TO RETRY", DANGER, true),
        };
        if status == Status::Updating {
            // Live progress: stage and a bar.
            let (stage, p) = self.updater.as_ref().map(|u| u.progress()).unwrap_or_default();
            let bw = 260.0;
            let (x, y) = (sw - 20.0 - bw, sh - 46.0);
            draw_rectangle(x - 10.0, y - 10.0, bw + 20.0, 44.0, Color::new(0.0, 0.0, 0.0, 0.55));
            self.ui.text(&format!("UPDATING  -  {}", stage), x, y, 1.0, ACCENT);
            self.ui.text_right(&format!("{}%", (p * 100.0).round() as i32), x + bw, y, 1.0, MUTED);
            draw_rectangle(x, y + 16.0, bw, 6.0, Color::new(1.0, 1.0, 1.0, 0.1));
            draw_rectangle(x, y + 16.0, bw * p.clamp(0.0, 1.0), 6.0, ACCENT);
            return;
        }
        let big = clickable && !matches!(status, Status::UpToDate | Status::Unknown);
        let w = self.ui.font.width(label, if big { 2.0 } else { 1.0 });
        let (x, y) = (sw - 20.0 - w, sh - 34.0);
        let (mx, my) = mouse_position();
        let hot = clickable && mx > x - 10.0 && mx < sw - 10.0 && my > y - 8.0 && my < y + 22.0;
        if clickable {
            draw_rectangle(x - 10.0, y - 8.0, w + 20.0, 30.0, Color::new(0.0, 0.0, 0.0, if hot { 0.7 } else { 0.45 }));
        }
        self.ui.text(label, x, y, if big { 2.0 } else { 1.0 }, color);
        if hot && is_mouse_button_pressed(MouseButton::Left) {
            if let Some(u) = &self.updater {
                match status {
                    Status::Ready => updater::Updater::relaunch(),
                    Status::UpToDate | Status::Unknown => u.check(),
                    _ => u.update(),
                }
            }
        }
    }

    /// Create a world: name, seed, game mode and options, with a preview of where you'll start.
    fn new_world_screen(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        let t = self.game.time;
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.02, 0.01, 0.05, 0.74));
        self.title("CREATE A WORLD", sh * 0.07, 5.0, TEXT);

        // Typing into the focused field.
        while let Some(c) = get_char_pressed() {
            if c.is_control() {
                continue;
            }
            match self.nw_focus {
                Some(0) if self.nw.name.len() < 24 => self.nw.name.push(c),
                Some(1) if self.nw_seed.len() < 18 => self.nw_seed.push(c),
                _ => {}
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            match self.nw_focus {
                Some(0) => {
                    self.nw.name.pop();
                }
                Some(1) => {
                    self.nw_seed.pop();
                }
                _ => {}
            }
        }
        if is_key_pressed(KeyCode::Tab) {
            self.nw_focus = Some(if self.nw_focus == Some(0) { 1 } else { 0 });
        }
        let seed: u64 = self.nw_seed.trim().parse().unwrap_or_else(|_| {
            self.nw_seed.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211)) % 1_000_000_000
        });

        // Left: the world you'll start on, spinning.
        let (lx, ly) = (sw * 0.27, sh * 0.47);
        let start = if self.nw.alien_start {
            let list = planets::Planet::nearest(0, 0, 12);
            list[1 + (seed as usize % (list.len() - 1))].clone()
        } else {
            planets::Planet::earth()
        };
        let r = (sh * 0.2).min(sw * 0.13);
        for k in 0..90 {
            let h = noise::hash3(k, 5, 2);
            let a = (h % 1000) as f32 / 1000.0 * std::f32::consts::TAU + t * 0.05;
            let d = r * 1.3 + ((h >> 10) % 100) as f32;
            draw_rectangle(lx + a.cos() * d, ly + a.sin() * d * 0.8, 2.0, 2.0, Color::new(0.9, 0.9, 1.0, 0.35));
        }
        self.ui.planet_sphere(&start, lx, ly, r, 4.0, t * 2.0);
        let c = start.color();
        self.ui.text_center(if self.nw.alien_start { "A STRANGE NEW WORLD" } else { "EARTH" }, lx, ly + r + 26.0, 3.0, Color::from_rgba(c[0], c[1], c[2], 255));
        self.ui.text_center(&format!("SEED {}", seed), lx, ly + r + 58.0, 1.0, MUTED);

        // Right: options.
        let (x, w) = (sw * 0.5, (sw * 0.42).min(560.0));
        let mut y = sh * 0.19;
        if self.ui.text_field("WORLD NAME", x, y, w, &self.nw.name.clone(), self.nw_focus == Some(0), t) {
            self.nw_focus = Some(0);
        }
        y += 36.0;
        if self.ui.text_field("SEED", x, y, w, &self.nw_seed.clone(), self.nw_focus == Some(1), t) {
            self.nw_focus = Some(1);
        }
        y += 46.0;
        let cw = w * 0.5 - 6.0;
        let wand = ["..+#+..", ".#...#.", "+..#..+", "#.#+#.#", "+..#..+", ".#...#.", "..+#+.."];
        let heart = [".##.##.", "#######", "#######", "#######", ".#####.", "..###..", "...#..."];
        if self.ui.mode_card(x, y, cw, 140.0, "CREATIVE", &["CAN'T BE HURT", "FLY FOREVER", "COMMANDS ON"], &wand, [255, 181, 74], self.nw.creative, t) && !self.nw.creative {
            self.nw.creative = true;
            self.nw.commands = true;
        }
        if self.ui.mode_card(x + cw + 12.0, y, cw, 140.0, "SURVIVAL", &["HEALTH AND FLIGHT", "REAL DANGER", "COMMANDS OFF"], &heart, [235, 90, 90], !self.nw.creative, t) && self.nw.creative {
            self.nw.creative = false;
            self.nw.commands = false;
        }
        y += 160.0;
        let mut v = self.nw.commands as usize;
        if self.ui.segmented("COMMANDS", x, y, w, &["OFF", "ON"], &mut v) {
            self.nw.commands = v == 1;
        }
        y += 34.0;
        self.ui.segmented("CREATURES", x, y, w, &["PEACEFUL", "NORMAL", "HORDES"], &mut self.nw.creatures);
        y += 34.0;
        let mut v = self.nw.weather as usize;
        if self.ui.segmented("WEATHER", x, y, w, &["ALWAYS CLEAR", "CHANGING"], &mut v) {
            self.nw.weather = v == 1;
        }
        y += 34.0;
        let mut v = self.nw.alien_start as usize;
        if self.ui.segmented("START ON", x, y, w, &["EARTH", "RANDOM PLANET"], &mut v) {
            self.nw.alien_start = v == 1;
        }
        y += 34.0;
        let mut v = self.nw.three_d as usize;
        if self.ui.segmented("WORLD", x, y, w, &["2D", "3D ISLAND"], &mut v) {
            self.nw.three_d = v == 1;
        }
        y += 56.0;
        draw_rectangle(x + w * 0.5 - 230.0, y, 220.0, 34.0, Color::new(1.0, 0.71, 0.29, 0.9));
        if self.ui.button("CREATE WORLD", x + w * 0.5 - 120.0, y, 220.0) || is_key_pressed(KeyCode::Enter) {
            let mut opts = self.nw.clone();
            opts.seed = seed;
            if opts.name.trim().is_empty() {
                opts.name = "New World".into();
            }
            let dir = saves::new_dir(&opts.name);
            if opts.three_d {
                let mut w = play3d::Play3D::new(opts.name.clone(), seed, opts.creative);
                w.dir = Some(dir);
                w.save();
                self.w3 = Some(w);
                self.screen = Screen::Play3D;
                return;
            }
            self.game = Game::with_options(opts);
            self.game.save_dir = Some(dir);
            self.game.save();
            self.game.cam = self.game.player.center();
            self.screen = Screen::Play;
        }
        if self.ui.button("BACK", x + w * 0.5 + 120.0, y, 160.0) || self.take_esc() {
            self.screen = if self.worlds.is_empty() { Screen::Menu } else { Screen::Worlds };
        }
    }

    fn open_worlds(&mut self) {
        self.game.save();
        self.worlds = saves::list();
        self.world_sel = 0;
        self.world_del = false;
        self.world_scroll = 0;
        self.screen = Screen::Worlds;
    }

    fn start_new_world(&mut self) {
        let seed = random_seed();
        self.nw = game::WorldOptions::new(seed);
        self.nw_seed = seed.to_string();
        self.nw_focus = Some(0);
        while get_char_pressed().is_some() {}
        self.screen = Screen::NewWorld;
    }

    /// One frame of a 3D world: its own update, drawing, pause menu and sound.
    fn frame3d(&mut self, dt: f32) {
        let Some(w) = &mut self.w3 else {
            self.screen = Screen::Menu;
            return;
        };
        w.update(dt, &mut self.ui);
        w.draw(&mut self.ui);
        let exit = w.pause_menu(&mut self.ui);
        if let Some(a) = &mut self.audio {
            a.master = self.settings.master;
            a.effects = self.settings.effects;
            a.ambience = self.settings.ambience;
            for s in w.sounds.drain(..) {
                a.play(s);
            }
            a.update(dt, w.vox.fires.len().min(400), 0);
        } else {
            w.sounds.clear();
        }
        if matches!(exit, play3d::Exit::Menu) {
            self.w3 = None;
            self.screen = Screen::Menu;
        }
    }

    fn play_world(&mut self, i: usize) {
        let Some(info) = self.worlds.get(i) else { return };
        if let Some(w) = play3d::Play3D::load(&info.dir) {
            self.w3 = Some(w);
            self.menu_x = 0.0;
            self.screen = Screen::Play3D;
            return;
        }
        match Game::load(&info.dir) {
            Some(g) => {
                self.game = g;
                self.game.save();
                self.menu_x = 0.0;
                self.screen = Screen::Play;
            }
            None => self.game.toast("COULDN'T OPEN THAT WORLD", [255, 120, 110]),
        }
    }

    /// Pick a saved world, Minecraft style.
    fn worlds_screen(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.02, 0.01, 0.05, 0.74));
        self.title("SELECT WORLD", sh * 0.08, 5.0, TEXT);
        let cx = sw * 0.5;
        let w = (sw * 0.6).clamp(360.0, 640.0);
        let x = cx - w * 0.5;
        let top = sh * 0.2;
        let row_h = 58.0;
        let rows = (((sh * 0.78 - 130.0) - top) / row_h).floor().max(2.0) as usize;
        let n = self.worlds.len();
        let (mx, my) = mouse_position();
        self.ui.panel(x - 12.0, top - 12.0, w + 24.0, rows as f32 * row_h + 16.0);
        if n == 0 {
            self.ui.text_center("NO WORLDS YET", cx, top + rows as f32 * row_h * 0.5 - 20.0, 3.0, TEXT);
            self.ui.text_center("CREATE ONE AND IT'S SAVED AUTOMATICALLY", cx, top + rows as f32 * row_h * 0.5 + 16.0, 1.0, MUTED);
        } else {
            let wheel = mouse_wheel().1;
            if wheel < 0.0 && self.world_scroll + rows < n {
                self.world_scroll += 1;
            } else if wheel > 0.0 && self.world_scroll > 0 {
                self.world_scroll -= 1;
            }
            if is_key_pressed(KeyCode::Down) && self.world_sel + 1 < n {
                self.world_sel += 1;
                self.world_del = false;
            }
            if is_key_pressed(KeyCode::Up) && self.world_sel > 0 {
                self.world_sel -= 1;
                self.world_del = false;
            }
            self.world_sel = self.world_sel.min(n - 1);
            if self.world_sel < self.world_scroll {
                self.world_scroll = self.world_sel;
            } else if self.world_sel >= self.world_scroll + rows {
                self.world_scroll = self.world_sel + 1 - rows;
            }
            let mut play = None;
            for (k, i) in (self.world_scroll..n.min(self.world_scroll + rows)).enumerate() {
                let ry = top + k as f32 * row_h;
                let hot = mx > x && mx < x + w && my > ry && my < ry + row_h - 6.0;
                let sel = i == self.world_sel;
                let bg = if sel { Color::new(1.0, 0.71, 0.29, 0.18) } else if hot { Color::new(1.0, 1.0, 1.0, 0.07) } else { Color::new(1.0, 1.0, 1.0, 0.03) };
                draw_rectangle(x, ry, w, row_h - 6.0, bg);
                if sel {
                    draw_rectangle(x, ry, 3.0, row_h - 6.0, ACCENT);
                }
                let info = &self.worlds[i];
                self.ui.text(&info.name.to_uppercase(), x + 16.0, ry + 9.0, 2.0, if sel { ACCENT } else { TEXT });
                let mode = if info.creative { "CREATIVE" } else { "SURVIVAL" };
                let planet = if info.planet.is_empty() { "EARTH".to_string() } else { info.planet.to_uppercase() };
                self.ui.text(&format!("{mode}  -  ON {planet}"), x + 16.0, ry + 32.0, 1.0, MUTED);
                self.ui.text_right(&saves::ago(info.played), x + w - 14.0, ry + 32.0, 1.0, MUTED);
                if hot && is_mouse_button_pressed(MouseButton::Left) {
                    if sel {
                        play = Some(i);
                    }
                    self.world_sel = i;
                    self.world_del = false;
                }
            }
            if n > rows {
                self.ui.text_center(&format!("{} WORLDS  -  SCROLL FOR MORE", n), cx, top + rows as f32 * row_h + 12.0, 1.0, MUTED);
            }
            if is_key_pressed(KeyCode::Enter) {
                play = Some(self.world_sel);
            }
            if let Some(i) = play {
                self.play_world(i);
                return;
            }
        }
        let mut y = top + rows as f32 * row_h + 34.0;
        let bw = (w - 12.0) * 0.5;
        if n > 0 {
            if self.ui.button("PLAY SELECTED WORLD", cx - bw * 0.5 - 6.0, y, bw) {
                self.play_world(self.world_sel);
                return;
            }
            let del = if self.world_del { "CLICK AGAIN TO DELETE" } else { "DELETE" };
            if self.ui.button(del, cx + bw * 0.5 + 6.0, y, bw) {
                if self.world_del {
                    let dir = self.worlds[self.world_sel].dir.clone();
                    if self.game.save_dir.as_ref() == Some(&dir) {
                        self.game.save_dir = None;
                    }
                    saves::delete(&dir);
                    self.worlds = saves::list();
                    self.world_del = false;
                } else {
                    self.world_del = true;
                }
            }
            y += 42.0;
        }
        if self.ui.button("CREATE NEW WORLD", cx - bw * 0.5 - 6.0, y, bw) {
            self.start_new_world();
        }
        if self.ui.button("OPEN WORLDS FOLDER", cx + bw * 0.5 + 6.0, y, bw) {
            saves::open_folder();
        }
        y += 42.0;
        if self.ui.button("BACK", cx, y, 200.0) || self.take_esc() {
            self.screen = Screen::Menu;
        }
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
            self.game.save();
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
            ("M", "BROOM: FLY FAST, SHIFT TO BOOST"),
            ("LEFT MOUSE", "CAST  (HOLD: BEAMS)"),
            ("RIGHT MOUSE", "DIG"),
            ("1-0 - = Q R G P V C / WHEEL", "CHOOSE SPELL"),
            ("TAB / MOUSE TO BOTTOM", "SHOW SPELL BAR, CLICK TO PICK"),
            ("E", "OPEN CHEST / USE ALTAR"),
            ("PORTAL WAND (P)", "RMB PICK PLANET, LMB OPEN PORTAL"),
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
