//! Minimal immediate-mode UI with a built-in 5x7 pixel font, plus the in-game HUD.

use macroquad::prelude::*;

use crate::game::{Game, PALETTE};
use crate::player::{MAX_FLIGHT, MAX_HP, MAX_MANA};
use crate::render::{shade, View};
use crate::spells::SPELL_BAR;

pub const TEXT: Color = Color::new(0.92, 0.89, 0.84, 1.0);
pub const MUTED: Color = Color::new(0.6, 0.57, 0.55, 1.0);
pub const ACCENT: Color = Color::new(1.0, 0.71, 0.29, 1.0);
pub const PANEL: Color = Color::new(0.05, 0.045, 0.08, 0.82);
pub const LINE: Color = Color::new(1.0, 1.0, 1.0, 0.1);
pub const DANGER: Color = Color::new(0.92, 0.36, 0.32, 1.0);

const GLYPHS: &[(char, [&str; 7])] = &[
    ('A', [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('B', ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."]),
    ('C', [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."]),
    ('D', ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."]),
    ('E', ["#####", "#....", "#....", "####.", "#....", "#....", "#####"]),
    ('F', ["#####", "#....", "#....", "####.", "#....", "#....", "#...."]),
    ('G', [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".####"]),
    ('H', ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('I', [".###.", "..#..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('J', ["..###", "...#.", "...#.", "...#.", "#..#.", "#..#.", ".##.."]),
    ('K', ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"]),
    ('L', ["#....", "#....", "#....", "#....", "#....", "#....", "#####"]),
    ('M', ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"]),
    ('N', ["#...#", "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#"]),
    ('O', [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('P', ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('Q', [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"]),
    ('S', [".####", "#....", "#....", ".###.", "....#", "....#", "####."]),
    ('T', ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('U', ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "#.#.#", ".#.#."]),
    ('X', ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"]),
    ('Y', ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."]),
    ('Z', ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
    ('0', [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('2', [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"]),
    ('3', ["####.", "....#", "....#", ".###.", "....#", "....#", "####."]),
    ('4', ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."]),
    ('.', [".....", ".....", ".....", ".....", ".....", ".....", "..#.."]),
    (',', [".....", ".....", ".....", ".....", ".....", "..#..", ".#..."]),
    (':', [".....", "..#..", ".....", ".....", ".....", "..#..", "....."]),
    ('!', ["..#..", "..#..", "..#..", "..#..", "..#..", ".....", "..#.."]),
    ('?', [".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#.."]),
    ('-', [".....", ".....", ".....", ".###.", ".....", ".....", "....."]),
    ('+', [".....", "..#..", "..#..", "#####", "..#..", "..#..", "....."]),
    ('/', ["....#", "...#.", "...#.", "..#..", ".#...", ".#...", "#...."]),
    ('(', ["...#.", "..#..", ".#...", ".#...", ".#...", "..#..", "...#."]),
    (')', [".#...", "..#..", "...#.", "...#.", "...#.", "..#..", ".#..."]),
    ('%', ["##..#", "##..#", "...#.", "..#..", ".#...", "#..##", "#..##"]),
    ('\'', ["..#..", "..#..", ".....", ".....", ".....", ".....", "....."]),
    ('<', ["...#.", "..#..", ".#...", "#....", ".#...", "..#..", "...#."]),
    ('>', [".#...", "..#..", "...#.", "....#", "...#.", "..#..", ".#..."]),
    ('=', [".....", ".....", "#####", ".....", "#####", ".....", "....."]),
    ('_', [".....", ".....", ".....", ".....", ".....", ".....", "#####"]),
    ('[', [".###.", ".#...", ".#...", ".#...", ".#...", ".#...", ".###."]),
    (']', [".###.", "...#.", "...#.", "...#.", "...#.", "...#.", ".###."]),
    ('#', [".#.#.", ".#.#.", "#####", ".#.#.", "#####", ".#.#.", ".#.#."]),
    ('*', [".....", "#.#.#", ".###.", "#####", ".###.", "#.#.#", "....."]),
    ('&', [".##..", "#..#.", "#.#..", ".#...", "#.#.#", "#..#.", ".##.#"]),
    ('|', ["..#..", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('"', [".#.#.", ".#.#.", ".....", ".....", ".....", ".....", "....."]),
];

pub struct Font {
    tex: Texture2D,
    index: [u8; 128],
}

impl Font {
    pub fn new() -> Self {
        let n = GLYPHS.len();
        let mut img = Image::gen_image_color((n * 6) as u16, 8, Color::new(0.0, 0.0, 0.0, 0.0));
        let mut index = [255u8; 128];
        for (gi, (ch, rows)) in GLYPHS.iter().enumerate() {
            index[*ch as usize] = gi as u8;
            for (y, row) in rows.iter().enumerate() {
                for (x, b) in row.bytes().enumerate() {
                    if b == b'#' {
                        img.set_pixel((gi * 6 + x) as u32, y as u32, WHITE);
                    }
                }
            }
        }
        let tex = Texture2D::from_image(&img);
        tex.set_filter(FilterMode::Nearest);
        Self { tex, index }
    }

    pub fn width(&self, text: &str, s: f32) -> f32 {
        (text.chars().count() as f32 * 6.0 - 1.0).max(0.0) * s
    }

    pub fn draw(&self, text: &str, x: f32, y: f32, s: f32, color: Color) {
        let (x, y) = (x.round(), y.round());
        for (k, ch) in text.chars().enumerate() {
            let c = ch.to_ascii_uppercase() as usize;
            if c >= 128 || self.index[c] == 255 {
                continue;
            }
            let gi = self.index[c] as f32;
            draw_texture_ex(
                &self.tex,
                x + k as f32 * 6.0 * s,
                y,
                color,
                DrawTextureParams { source: Some(Rect::new(gi * 6.0, 0.0, 5.0, 7.0)), dest_size: Some(vec2(5.0 * s, 7.0 * s)), ..Default::default() },
            );
        }
    }

    pub fn draw_shadowed(&self, text: &str, x: f32, y: f32, s: f32, color: Color) {
        self.draw(text, x + s.max(1.0), y + s.max(1.0), s, Color::new(0.0, 0.0, 0.0, 0.55 * color.a));
        self.draw(text, x, y, s, color);
    }
}

pub struct Ui {
    pub font: Font,
    mouse: Vec2,
    pressed: bool,
    down: bool,
    next_id: u32,
    dragging: Option<u32>,
    /// A widget was clicked this frame (for the click sound).
    pub clicked: bool,
    /// The mouse is over a HUD element (so clicks shouldn't reach the world).
    pub hovering: bool,
    carousel: f32,
    carousel_target: f32,
}

impl Ui {
    pub fn new() -> Self {
        Self { font: Font::new(), mouse: Vec2::ZERO, pressed: false, down: false, next_id: 0, dragging: None, clicked: false, hovering: false, carousel: 0.0, carousel_target: 0.0 }
    }

    pub fn begin(&mut self) {
        let (mx, my) = mouse_position();
        self.mouse = vec2(mx, my);
        self.pressed = is_mouse_button_pressed(MouseButton::Left);
        self.down = is_mouse_button_down(MouseButton::Left);
        if !self.down {
            self.dragging = None;
        }
        self.next_id = 0;
        self.clicked = false;
        self.hovering = false;
    }

    fn over(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse.x >= x && self.mouse.x < x + w && self.mouse.y >= y && self.mouse.y < y + h
    }

    pub fn text(&self, s: &str, x: f32, y: f32, scale: f32, col: Color) {
        self.font.draw_shadowed(s, x, y, scale, col);
    }

    pub fn text_center(&self, s: &str, cx: f32, y: f32, scale: f32, col: Color) {
        self.font.draw_shadowed(s, cx - self.font.width(s, scale) * 0.5, y, scale, col);
    }

    pub fn text_right(&self, s: &str, rx: f32, y: f32, scale: f32, col: Color) {
        self.font.draw_shadowed(s, rx - self.font.width(s, scale), y, scale, col);
    }

    pub fn panel(&self, x: f32, y: f32, w: f32, h: f32) {
        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.0, LINE);
    }

    /// A centred text button. Returns true when clicked.
    pub fn button(&mut self, label: &str, cx: f32, y: f32, w: f32) -> bool {
        let h = 34.0;
        let x = cx - w * 0.5;
        let hot = self.over(x, y, w, h);
        if hot {
            draw_rectangle(x, y, w, h, Color::new(1.0, 1.0, 1.0, 0.05));
            draw_rectangle(x, y, 2.0, h, ACCENT);
        }
        self.text_center(label, cx, y + 10.0, 2.0, if hot { ACCENT } else { TEXT });
        let hit = hot && self.pressed;
        self.clicked |= hit;
        hit
    }

    /// Labelled horizontal slider in [0, 1].
    pub fn slider(&mut self, label: &str, x: f32, y: f32, w: f32, value: &mut f32) {
        let id = self.next_id;
        self.next_id += 1;
        let (tx, tw) = (x + w * 0.45, w * 0.42);
        let hot = self.over(x, y, w, 30.0);
        self.text(label, x, y + 10.0, 2.0, if hot { TEXT } else { MUTED });
        if hot && self.pressed {
            self.dragging = Some(id);
            self.clicked = true;
        }
        if self.dragging == Some(id) {
            *value = ((self.mouse.x - tx) / tw).clamp(0.0, 1.0);
        }
        if hot {
            if is_key_pressed(KeyCode::Left) {
                *value = (*value - 0.05).max(0.0);
            }
            if is_key_pressed(KeyCode::Right) {
                *value = (*value + 0.05).min(1.0);
            }
        }
        let cy = y + 16.0;
        draw_rectangle(tx, cy - 1.0, tw, 2.0, LINE);
        draw_rectangle(tx, cy - 1.0, tw * *value, 2.0, ACCENT);
        draw_rectangle(tx + tw * *value - 3.0, cy - 7.0, 6.0, 14.0, if hot || self.dragging == Some(id) { ACCENT } else { TEXT });
        self.text_right(&format!("{}%", (*value * 100.0).round() as i32), x + w, y + 10.0, 2.0, MUTED);
    }

    /// Labelled on/off toggle. Returns true when changed.
    pub fn toggle(&mut self, label: &str, x: f32, y: f32, w: f32, value: &mut bool) -> bool {
        let hot = self.over(x, y, w, 30.0);
        self.text(label, x, y + 10.0, 2.0, if hot { TEXT } else { MUTED });
        let (bx, by) = (x + w - 44.0, y + 8.0);
        draw_rectangle(bx, by, 44.0, 16.0, if *value { Color::new(1.0, 0.71, 0.29, 0.25) } else { LINE });
        draw_rectangle(if *value { bx + 26.0 } else { bx + 2.0 }, by + 2.0, 16.0, 12.0, if *value { ACCENT } else { MUTED });
        if hot && self.pressed {
            *value = !*value;
            self.clicked = true;
            return true;
        }
        false
    }

    /// A row of options, one selected. Returns true when changed.
    pub fn segmented(&mut self, label: &str, x: f32, y: f32, w: f32, options: &[&str], value: &mut usize) -> bool {
        self.text(label, x, y + 9.0, 1.0, MUTED);
        let (bx, bw) = (x + w * 0.34, w * 0.66);
        let cw = bw / options.len() as f32;
        let mut changed = false;
        for (k, o) in options.iter().enumerate() {
            let cx = bx + k as f32 * cw;
            let sel = *value == k;
            let hot = self.over(cx, y, cw - 4.0, 26.0);
            draw_rectangle(cx, y, cw - 4.0, 26.0, if sel { Color::new(1.0, 0.71, 0.29, 0.22) } else if hot { Color::new(1.0, 1.0, 1.0, 0.07) } else { Color::new(1.0, 1.0, 1.0, 0.03) });
            draw_rectangle_lines(cx, y, cw - 4.0, 26.0, 1.0, if sel { ACCENT } else { LINE });
            self.text_center(o, cx + (cw - 4.0) * 0.5, y + 9.0, 1.0, if sel { ACCENT } else { TEXT });
            if hot && self.pressed && !sel {
                *value = k;
                changed = true;
                self.clicked = true;
            }
        }
        changed
    }

    /// A one-line text box. Returns true when clicked (to take focus).
    pub fn text_field(&mut self, label: &str, x: f32, y: f32, w: f32, value: &str, focused: bool, time: f32) -> bool {
        self.text(label, x, y + 9.0, 1.0, MUTED);
        let (bx, bw) = (x + w * 0.34, w * 0.66 - 4.0);
        let hot = self.over(bx, y, bw, 26.0);
        draw_rectangle(bx, y, bw, 26.0, Color::new(0.0, 0.0, 0.0, 0.45));
        draw_rectangle_lines(bx, y, bw, 26.0, 1.0, if focused { ACCENT } else if hot { Color::new(1.0, 1.0, 1.0, 0.3) } else { LINE });
        let caret = if focused && (time * 2.5) as i32 % 2 == 0 { "_" } else { "" };
        self.font.draw(&format!("{}{}", value, caret), bx + 8.0, y + 6.0, 2.0, TEXT);
        let hit = hot && self.pressed;
        self.clicked |= hit;
        hit
    }

    /// A big selectable card with a pixel icon, title and blurb. Returns true when clicked.
    #[allow(clippy::too_many_arguments)]
    pub fn mode_card(&mut self, x: f32, y: f32, w: f32, h: f32, title: &str, blurb: &[&str], icon: &[&str; 7], col: [u8; 3], selected: bool, time: f32) -> bool {
        let hot = self.over(x, y, w, h);
        let lift = if selected { -4.0 + (time * 3.0).sin() * 1.5 } else if hot { -2.0 } else { 0.0 };
        let y = y + lift;
        draw_rectangle(x, y, w, h, if selected { Color::new(0.1, 0.08, 0.14, 0.95) } else { PANEL });
        draw_rectangle_lines(x, y, w, h, if selected { 2.0 } else { 1.0 }, if selected { Color::from_rgba(col[0], col[1], col[2], 255) } else if hot { Color::new(1.0, 1.0, 1.0, 0.3) } else { LINE });
        draw_rectangle(x, y, w, 3.0, Color::from_rgba(col[0], col[1], col[2], if selected { 255 } else { 90 }));
        let accent = [255, 255, 255];
        self.icon(icon, col, accent, x + w * 0.5 - 21.0, y + 18.0, 6.0, if selected { 1.0 } else { 0.6 });
        self.text_center(title, x + w * 0.5, y + 72.0, 2.0, if selected { Color::from_rgba(col[0], col[1], col[2], 255) } else { TEXT });
        for (k, line) in blurb.iter().enumerate() {
            self.text_center(line, x + w * 0.5, y + 98.0 + k as f32 * 14.0, 1.0, MUTED);
        }
        let hit = hot && self.pressed;
        self.clicked |= hit;
        hit
    }

    /// Draws a 7x7 icon at the given pixel scale.
    pub fn icon(&self, rows: &[&str; 7], main: [u8; 3], accent: [u8; 3], x: f32, y: f32, s: f32, alpha: f32) {
        for (j, row) in rows.iter().enumerate() {
            for (i, b) in row.bytes().enumerate() {
                let c = match b {
                    b'#' => main,
                    b'+' => accent,
                    _ => continue,
                };
                draw_rectangle(x + i as f32 * s, y + j as f32 * s, s, s, Color::from_rgba(c[0], c[1], c[2], (alpha * 255.0) as u8));
            }
        }
    }

    fn hot(&mut self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let hot = self.over(x, y, w, h);
        if hot {
            self.hovering = true;
        }
        hot
    }

    pub fn hud(&mut self, game: &mut Game, view: &View, fps: f32, show_fps: bool) {
        let (sw, sh) = (screen_width(), screen_height());
        let p = &game.player;

        // Vitals, top-left.
        // Creative shows only mana; survival adds health and flight.
        let mana_col = if game.fizzle > 0.0 { Color::new(1.0, 1.0, 1.0, 1.0) } else { Color::new(0.38, 0.56, 1.0, 1.0) };
        let mut bars = vec![];
        if !game.options.creative {
            bars.push((p.hp / MAX_HP, Color::new(0.88, 0.3, 0.3, 1.0)));
        }
        bars.push((p.mana / MAX_MANA, mana_col));
        if !game.options.creative {
            bars.push((p.flight / MAX_FLIGHT, Color::new(0.86, 0.86, 0.78, 1.0)));
        }
        for (k, (v, col)) in bars.iter().enumerate() {
            let y = 20.0 + k as f32 * 12.0;
            draw_rectangle(20.0, y, 150.0, 5.0, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_rectangle(20.0, y, 150.0 * v.clamp(0.0, 1.0), 5.0, *col);
        }
        let gy = 24.0 + bars.len() as f32 * 12.0;
        draw_rectangle(20.0, gy, 8.0, 8.0, Color::from_rgba(236, 190, 64, 255));
        draw_rectangle(22.0, gy + 2.0, 2.0, 2.0, Color::from_rgba(255, 248, 210, 255));
        self.text(&format!("{}", p.gold), 34.0, gy + 1.0, 1.0, TEXT);

        // Place, time and weather, top-right.
        let (px, py) = p.center();
        let surf = game.world.surface_at(px as i32);
        let depth = ((py - surf as f32) / 4.0).round() as i32;
        let place = if !game.planet.is_earth() && py > -240.0 && depth <= 1 {
            format!("{}  -  {}", game.planet.name, game.planet.kind.name())
        } else if py < -520.0 {
            "ASTRAL EXPANSE".to_string()
        } else if py < -240.0 {
            "SKY ISLES".to_string()
        } else if depth > 1 {
            format!("DEPTH {}M", depth)
        } else if depth < -12 {
            format!("{}  ALT {}M", game.world.gen.biome(px as i32).name().to_uppercase(), -depth)
        } else {
            game.world.gen.biome(px as i32).name().to_uppercase()
        };
        self.text_right(&place, sw - 20.0, 20.0, 2.0, TEXT);
        self.text_right(&format!("{}  -  {}", game.weather.clock_text(), game.weather.label()), sw - 20.0, 40.0, 1.0, MUTED);
        if show_fps {
            self.text_right(&format!("{} FPS  SIM {:.1}MS", fps.round() as i32, game.sim_ms), sw - 20.0, 54.0, 1.0, MUTED);
        }

        // Floating damage numbers and gold.
        for pop in &game.mobs.popups {
            let (x, y) = view.to_screen(pop.x, pop.y);
            let a = (1.0 - pop.t).clamp(0.0, 1.0);
            let c = Color::from_rgba(pop.color[0], pop.color[1], pop.color[2], (a * 255.0) as u8);
            self.text_center(&pop.text, x, y, 1.0, c);
        }

        // Toasts and the interaction prompt.
        for (k, t) in game.toasts.iter().enumerate() {
            let a = (1.0 - (t.t - 2.2).max(0.0) / 0.8).clamp(0.0, 1.0);
            let c = Color::from_rgba(t.color[0], t.color[1], t.color[2], (a * 255.0) as u8);
            self.text_center(&t.text, sw * 0.5, sh * 0.2 + k as f32 * 22.0, 2.0, c);
        }
        if let Some(pr) = &game.prompt {
            self.text_center(pr, sw * 0.5, sh * 0.62, 2.0, TEXT);
        }

        if game.sandbox {
            let n = PALETTE.len();
            let size = 32.0;
            let gap = 4.0;
            let total = n as f32 * size + (n - 1) as f32 * gap;
            let x0 = (sw - total) * 0.5;
            let y = sh - size - 20.0;
            for (k, m) in PALETTE.iter().enumerate() {
                let x = x0 + k as f32 * (size + gap);
                let hot = self.hot(x, y, size, size);
                if hot && self.pressed {
                    game.brush = k;
                    self.clicked = true;
                }
                draw_rectangle(x, y, size, size, PANEL);
                let c = if *m == crate::materials::Material::Empty { [40, 36, 46] } else { shade(m.base_color(), 140, 0.2) };
                draw_rectangle(x + 7.0, y + 7.0, size - 14.0, size - 14.0, Color::from_rgba(c[0], c[1], c[2], 255));
                if *m == crate::materials::Material::Empty {
                    draw_line(x + 9.0, y + 9.0, x + size - 9.0, y + size - 9.0, 2.0, DANGER);
                }
                let sel = k == game.brush;
                draw_rectangle_lines(x, y, size, size, if sel { 2.0 } else { 1.0 }, if sel { ACCENT } else { LINE });
            }
            let label = format!("{}   SIZE {}", PALETTE[game.brush].name(), game.brush_r);
            self.text_center(&label.to_uppercase(), sw * 0.5, y - 22.0, 2.0, TEXT);
            self.text(&"SANDBOX  -  B TO RETURN".to_string(), x0, y - 22.0, 1.0, ACCENT);
            return;
        }

        // Spell hotbar, bottom-centre: one numbered slot per spell.
        let n = SPELL_BAR.len();
        let size = 46.0f32.min((sw - 40.0) / n as f32 - 5.0);
        let gap = 5.0;
        let total = n as f32 * size + (n - 1) as f32 * gap;
        let x0 = (sw - total) * 0.5;
        let y = sh - size - 18.0;
        const KEYS: [&str; 16] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "Q", "R", "G", "P"];
        for (k, sp) in SPELL_BAR.iter().enumerate() {
            let x = x0 + k as f32 * (size + gap);
            if self.hot(x, y, size, size) && self.pressed {
                game.spell = k;
                self.clicked = true;
            }
            let sel = k == game.spell;
            draw_rectangle(x, y, size, size, PANEL);
            if sel {
                draw_rectangle(x, y, size, size, Color::new(1.0, 0.71, 0.29, 0.12));
            }
            draw_rectangle_lines(x, y, size, size, if sel { 2.0 } else { 1.0 }, if sel { ACCENT } else { LINE });
            let (rows, main, acc) = sp.icon();
            let s = ((size - 14.0) / 7.0).floor().max(2.0);
            let off = (size - s * 7.0) * 0.5;
            let afford = game.player.mana >= sp.mana();
            self.icon(&rows, main, acc, x + off, y + off + 2.0, s, if afford { 1.0 } else { 0.3 });
            self.font.draw(KEYS[k], x + 4.0, y + 4.0, 1.0, if sel { ACCENT } else { MUTED });
        }
        let sp = game.current_spell();
        self.text_center(&format!("{}  -  {}", sp.name().to_uppercase(), sp.desc().to_uppercase()), sw * 0.5, y - 22.0, 2.0, TEXT);
    }
}

impl Ui {
    /// A pixel planet: continents and seas, lit from the top-left, with an atmosphere rim.
    pub fn planet_sphere(&self, p: &crate::planets::Planet, cx: f32, cy: f32, r: f32, px: f32, time: f32) {
        let land = {
            let b = p.top.base_color();
            [(b[0] as f32 * p.tint[0]).min(255.0) as u8, (b[1] as f32 * p.tint[1]).min(255.0) as u8, (b[2] as f32 * p.tint[2]).min(255.0) as u8]
        };
        let sea = p.sea.map(|m| m.base_color());
        let n = (r / px).ceil() as i32;
        for j in -n..=n {
            for i in -n..=n {
                let (dx, dy) = (i as f32 * px, j as f32 * px);
                let d = (dx * dx + dy * dy).sqrt();
                if d > r {
                    if d < r + px * 1.5 {
                        let c = p.sky_horizon;
                        draw_rectangle(cx + dx, cy + dy, px, px, Color::from_rgba(c[0], c[1], c[2], 110));
                    }
                    continue;
                }
                let (u, v) = (dx / r, dy / r);
                let z = (1.0 - u * u - v * v).max(0.0).sqrt();
                let spin = time * 0.15 + p.gx as f32 * 0.7;
                let lon = u.atan2(z) + spin;
                let lat = v;
                let nse = crate::noise::fbm(lon * 2.2 + 10.0, lat * 2.5 + p.gy as f32, 3, (p.gx * 31 + p.gy) as u32);
                let mut c = match sea {
                    Some(s) if nse < 0.5 => s,
                    _ => land,
                };
                if lat.abs() > 0.82 && p.kind != crate::planets::Kind::Lava {
                    c = [236, 240, 248];
                }
                let light = (0.35 + 0.65 * (z * 0.7 - u * 0.35 - v * 0.35 + 0.3)).clamp(0.12, 1.15);
                let col = Color::from_rgba((c[0] as f32 * light).min(255.0) as u8, (c[1] as f32 * light).min(255.0) as u8, (c[2] as f32 * light).min(255.0) as u8, 255);
                draw_rectangle(cx + dx, cy + dy, px, px, col);
            }
        }
    }

    /// Choose a destination: planets orbit on a tilted 3D ring; the front one is chosen.
    pub fn planet_picker(&mut self, game: &Game, list: &[crate::planets::Planet], time: f32) -> Option<usize> {
        let (sw, sh) = (screen_width(), screen_height());
        let n = list.len().max(1) as f32;
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.01, 0.0, 0.03, 0.93));
        // Starfield.
        for k in 0..260 {
            let h = crate::noise::hash3(k, 3, 11);
            let (x, y) = ((h % 1000) as f32 / 1000.0 * sw, ((h >> 10) % 1000) as f32 / 1000.0 * sh);
            let tw = 0.4 + 0.6 * ((time * 2.0 + k as f32).sin() * 0.5 + 0.5);
            let sz = if h % 9 == 0 { 3.0 } else { 2.0 };
            draw_rectangle((x / 2.0).floor() * 2.0, (y / 2.0).floor() * 2.0, sz, sz, Color::new(0.85, 0.88, 1.0, 0.5 * tw));
        }
        // Input: rotate with keys or the wheel.
        let wheel = mouse_wheel().1;
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) || wheel > 0.0 {
            self.carousel_target -= 1.0;
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) || wheel < 0.0 {
            self.carousel_target += 1.0;
        }
        let dt = get_frame_time().min(0.05);
        self.carousel += (self.carousel_target - self.carousel) * (dt * 7.0).min(1.0);
        let front = self.carousel_target.round().rem_euclid(n) as usize;
        let (cx, cy) = (sw * 0.5, sh * 0.44);
        let radius = sw * 0.36;
        let mut items: Vec<(f32, usize, f32, f32, f32)> = (0..list.len())
            .map(|i| {
                // Wrap around the ring, spaced so neighbours don't overlap.
                let mut d = (i as f32 - self.carousel).rem_euclid(n);
                if d > n * 0.5 {
                    d -= n;
                }
                let a = d * 0.42;
                let z = if a.abs() > 2.4 { -1.0 } else { a.cos() };
                let depth = ((z + 1.0) * 0.5).powi(3);
                let x = cx + a.sin() * radius;
                let y = cy - (1.0 - z) * sh * 0.08;
                (z, i, x, y, 10.0 + 88.0 * depth)
            })
            .collect();
        items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        // Orbit line.
        for k in 0..120 {
            let a = k as f32 / 120.0 * std::f32::consts::TAU;
            let (x, y) = (cx + a.sin() * radius, cy - (1.0 - a.cos()) * sh * 0.09);
            draw_rectangle(x, y + 2.0, 2.0, 2.0, Color::new(0.7, 0.6, 1.0, 0.12 + 0.12 * a.cos().max(0.0)));
        }
        let mut picked = None;
        let mut clicked_item: Option<usize> = None;
        for &(z, i, x, y, r) in &items {
            if z < -0.7 {
                continue;
            }
            let p = &list[i];
            let px = if r > 60.0 { 4.0 } else { 3.0 };
            self.planet_sphere(p, x, y, r, px, time);
            let shade = ((1.0 - z) * 0.35).clamp(0.0, 0.65);
            if shade > 0.02 {
                draw_circle(x, y, r + 1.0, Color::new(0.01, 0.0, 0.03, shade));
            }
            if i == front {
                let c = p.color();
                draw_circle_lines(x, y, r + 10.0 + (time * 3.0).sin() * 2.0, 2.0, Color::from_rgba(c[0], c[1], c[2], 160));
            } else if z > 0.0 {
                let label: String = p.name.chars().take(12).collect();
                self.text_center(&label, x, y + r + 10.0, 1.0, Color::new(0.75, 0.72, 0.7, z));
            }
            let dx = self.mouse.x - x;
            let dy = self.mouse.y - y;
            if dx * dx + dy * dy < r * r {
                clicked_item = Some(i);
            }
        }
        if self.pressed {
            if let Some(i) = clicked_item {
                if i == front {
                    picked = Some(i);
                } else {
                    let mut delta = (i as f32 - self.carousel_target.round()).rem_euclid(n);
                    if delta > n * 0.5 {
                        delta -= n;
                    }
                    self.carousel_target += delta;
                }
                self.clicked = true;
            }
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) || is_key_pressed(KeyCode::Space) {
            picked = Some(front);
        }
        // Header and the front planet's details.
        self.text_center("CHOOSE A DESTINATION", cx, 28.0, 3.0, ACCENT);
        let p = &list[front];
        let c = p.color();
        let y0 = sh * 0.7;
        self.text_center(&p.name, cx, y0, 5.0, Color::from_rgba(c[0], c[1], c[2], 255));
        self.text_center(p.kind.name(), cx, y0 + 48.0, 2.0, TEXT);
        let here = *p == game.planet;
        let sea = match p.sea {
            Some(crate::materials::Material::Lava) => "LAVA SEAS",
            Some(crate::materials::Material::Acid) => "ACID SEAS",
            Some(_) => "OCEANS",
            None => "NO SEAS",
        };
        let info = if here {
            "YOU ARE HERE".to_string()
        } else {
            format!("{:.1} LIGHT YEARS   -   GRAVITY {:.1}   -   {}   -   {} CREATURES", game.planet.light_years(p), p.gravity, sea, if p.is_earth() { 24 } else { p.creatures.len() })
        };
        self.text_center(&info, cx, y0 + 76.0, 1.0, if here { Color::from_rgba(140, 230, 150, 255) } else { MUTED });
        if *p == game.dest {
            self.text_center("PORTAL WAND IS TUNED HERE", cx, y0 + 96.0, 1.0, ACCENT);
        }
        self.text_center("<  A        CLICK OR ENTER TO CHOOSE        D  >", cx, sh - 34.0, 1.0, MUTED);
        picked
    }

    /// Chat history (fades out when closed) and the input line when open.
    /// `input` is (typed text, text the suggestions come from).
    pub fn chat(&self, game: &Game, input: Option<(&str, &str)>, time: f32, sel: usize) {
        let sh = screen_height();
        let x = 20.0;
        let open = input.is_some();
        let base = sh - 146.0;
        let lines: Vec<_> = game.chat.iter().rev().filter(|l| open || l.age < 10.0).take(if open { 16 } else { 6 }).collect();
        for (k, l) in lines.iter().enumerate() {
            let y = base - k as f32 * 20.0;
            let a = if open { 1.0 } else { ((10.0 - l.age) / 1.5).clamp(0.0, 1.0) };
            let w = self.font.width(&l.text, 2.0) + 14.0;
            draw_rectangle(x - 7.0, y - 4.0, w, 20.0, Color::new(0.0, 0.0, 0.0, 0.45 * a));
            self.font.draw(&l.text, x, y, 2.0, Color::from_rgba(l.color[0], l.color[1], l.color[2], (a * 255.0) as u8));
        }
        if let Some((buf, src)) = input {
            let y = sh - 118.0;
            draw_rectangle(x - 7.0, y - 5.0, (screen_width() * 0.6).max(520.0), 22.0, Color::new(0.0, 0.0, 0.0, 0.65));
            let (start, opts) = crate::game::suggest(src);
            let sel = if opts.is_empty() { 0 } else { sel % opts.len() };
            // Ghost text: the rest of the highlighted suggestion.
            if let Some(o) = opts.get(sel).filter(|_| src == buf) {
                let typed = buf.len() - start;
                if o.len() > typed {
                    self.font.draw(&o[typed..], x + self.font.width(buf, 2.0) + 2.0, y, 2.0, Color::new(1.0, 1.0, 1.0, 0.35));
                }
            }
            let caret = if (time * 2.5) as i32 % 2 == 0 { "_" } else { "" };
            self.font.draw(&format!("{}{}", buf, caret), x, y, 2.0, TEXT);
            // Suggestion list above the input, aligned with the word being typed.
            let sx = x + self.font.width(&src[..start], 2.0) + if start > 0 { 12.0 } else { 0.0 };
            if !opts.is_empty() && !(opts.len() == 1 && opts[0].len() == src.len() - start) {
                let shown = opts.len().min(8);
                let first = if sel >= shown { sel + 1 - shown } else { 0 };
                let w = opts.iter().map(|o| self.font.width(o, 2.0)).fold(0.0f32, f32::max) + 16.0;
                for (k, o) in opts.iter().enumerate().skip(first).take(shown) {
                    let row = (k - first) as f32;
                    let oy = y - 26.0 - (shown as f32 - 1.0 - row) * 20.0;
                    let hot = k == sel;
                    draw_rectangle(sx - 8.0, oy - 4.0, w, 20.0, Color::new(0.0, 0.0, 0.0, if hot { 0.85 } else { 0.7 }));
                    self.font.draw(o, sx, oy, 2.0, if hot { Color::new(1.0, 0.92, 0.4, 1.0) } else { Color::new(0.75, 0.75, 0.75, 1.0) });
                }
            } else if let Some(u) = crate::game::usage(src) {
                draw_rectangle(x - 7.0, y - 30.0, self.font.width(u, 2.0) + 14.0, 20.0, Color::new(0.0, 0.0, 0.0, 0.7));
                self.font.draw(u, x, y - 26.0, 2.0, Color::new(0.75, 0.75, 0.75, 1.0));
            }
        }
    }
}
