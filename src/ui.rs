//! Minimal immediate-mode UI with a built-in 5x7 pixel font, plus the in-game HUD.

use macroquad::prelude::*;

use crate::game::{Game, PALETTE};
use crate::player::{MAX_FLIGHT, MAX_HP, MAX_MANA};
use crate::render::{shade, View};
use crate::spells::Card;

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
}

impl Ui {
    pub fn new() -> Self {
        Self { font: Font::new(), mouse: Vec2::ZERO, pressed: false, down: false, next_id: 0, dragging: None, clicked: false, hovering: false }
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

    fn card_box(&mut self, card: Option<Card>, x: f32, y: f32, size: f32, sel: bool) -> bool {
        let hot = self.hot(x, y, size, size);
        draw_rectangle(x, y, size, size, PANEL);
        if let Some(c) = card {
            let (rows, main, acc) = c.icon();
            let s = ((size - 8.0) / 7.0).floor();
            let off = (size - s * 7.0) * 0.5;
            if c.is_mod() {
                draw_rectangle(x + 2.0, y + 2.0, size - 4.0, size - 4.0, Color::new(0.35, 0.25, 0.5, 0.35));
            }
            self.icon(&rows, main, acc, x + off, y + off, s, 1.0);
        }
        let border = if sel { ACCENT } else if hot { Color::new(1.0, 1.0, 1.0, 0.35) } else { LINE };
        draw_rectangle_lines(x, y, size, size, if sel { 2.0 } else { 1.0 }, border);
        hot
    }

    fn tooltip(&self, c: Card, x: f32, y: f32) {
        let lines = [c.name().to_uppercase(), c.desc().to_uppercase(), format!("MANA {}   {}", c.mana() as i32, if c.is_mod() { "MODIFIER" } else { "SPELL" })];
        let w = lines.iter().map(|l| self.font.width(l, 1.0)).fold(0.0f32, f32::max) + 20.0;
        let (x, y) = (x.min(screen_width() - w - 8.0), y - 48.0);
        self.panel(x, y, w, 44.0);
        self.text(&lines[0], x + 10.0, y + 8.0, 1.0, ACCENT);
        self.text(&lines[1], x + 10.0, y + 20.0, 1.0, TEXT);
        self.text(&lines[2], x + 10.0, y + 32.0, 1.0, MUTED);
    }

    pub fn hud(&mut self, game: &mut Game, view: &View, fps: f32, show_fps: bool) {
        let (sw, sh) = (screen_width(), screen_height());
        let p = &game.player;

        // Vitals, top-left.
        let bars = [
            (p.hp / MAX_HP, Color::new(0.88, 0.3, 0.3, 1.0)),
            (p.mana / MAX_MANA, if game.fizzle > 0.0 { Color::new(1.0, 1.0, 1.0, 1.0) } else { Color::new(0.38, 0.56, 1.0, 1.0) }),
            (p.flight / MAX_FLIGHT, Color::new(0.86, 0.86, 0.78, 1.0)),
        ];
        for (k, (v, col)) in bars.iter().enumerate() {
            let y = 20.0 + k as f32 * 12.0;
            draw_rectangle(20.0, y, 150.0, 5.0, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_rectangle(20.0, y, 150.0 * v.clamp(0.0, 1.0), 5.0, *col);
        }
        draw_rectangle(20.0, 60.0, 8.0, 8.0, Color::from_rgba(236, 190, 64, 255));
        draw_rectangle(22.0, 62.0, 2.0, 2.0, Color::from_rgba(255, 248, 210, 255));
        self.text(&format!("{}", p.gold), 34.0, 61.0, 1.0, TEXT);

        // Place, time and weather, top-right.
        let (px, py) = p.center();
        let surf = game.world.surface_at(px as i32);
        let depth = ((py - surf as f32) / 4.0).round() as i32;
        let place = if py < -520.0 {
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
        self.text_right(game.weather.label(), sw - 20.0, 40.0, 1.0, MUTED);
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

        // Wands, bottom-centre.
        let size = 30.0;
        let gap = 14.0;
        let widths: Vec<f32> = game.wands.iter().map(|w| w.slots.len() as f32 * (size + 2.0) + 12.0).collect();
        let total: f32 = widths.iter().sum::<f32>() + gap * (widths.len() - 1) as f32;
        let mut x = (sw - total) * 0.5;
        let y = sh - size - 26.0;
        for (wi, wand) in game.wands.iter().enumerate() {
            let w = widths[wi];
            let sel = wi == game.wand;
            if self.hot(x, y - 6.0, w, size + 12.0) && self.pressed {
                game.wand = wi;
                self.clicked = true;
            }
            draw_rectangle(x, y - 6.0, w, size + 12.0, Color::new(0.03, 0.03, 0.06, if sel { 0.85 } else { 0.55 }));
            draw_rectangle(x, y - 6.0, w, 2.0, Color::from_rgba(wand.tint[0], wand.tint[1], wand.tint[2], if sel { 255 } else { 90 }));
            self.font.draw(&format!("{}", wi + 1), x + 3.0, y - 2.0, 1.0, if sel { ACCENT } else { MUTED });
            for (k, slot) in wand.slots.iter().enumerate() {
                let sx = x + 8.0 + k as f32 * (size + 2.0);
                draw_rectangle(sx, y, size, size, Color::new(1.0, 1.0, 1.0, 0.04));
                if let Some(c) = slot {
                    let (rows, main, acc) = c.icon();
                    self.icon(&rows, main, acc, sx + 4.0, y + 4.0, 3.0, if sel { 1.0 } else { 0.55 });
                }
            }
            if sel {
                draw_rectangle_lines(x, y - 6.0, w, size + 12.0, 1.0, ACCENT);
            }
            x += w + gap;
        }
        self.text_center("TAB  WANDS & SPELLS", sw * 0.5, y - 22.0, 1.0, MUTED);
    }

    /// The wand editor: drag spell cards between your bag and wand slots.
    pub fn inventory(&mut self, game: &mut Game) {
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.02, 0.01, 0.04, 0.6));
        let (pw, ph) = (760.0f32.min(sw - 40.0), 520.0f32.min(sh - 40.0));
        let (x0, y0) = ((sw - pw) * 0.5, (sh - ph) * 0.5);
        self.panel(x0, y0, pw, ph);
        self.text("WANDS", x0 + 24.0, y0 + 20.0, 2.0, ACCENT);
        self.text_right("CLICK TO PICK UP AND PLACE  -  RIGHT CLICK RETURNS TO BAG  -  TAB TO CLOSE", x0 + pw - 24.0, y0 + 24.0, 1.0, MUTED);
        let size = 40.0;
        let mut tip: Option<(Card, f32, f32)> = None;
        let right = is_mouse_button_pressed(MouseButton::Right);
        let mut y = y0 + 52.0;
        for wi in 0..game.wands.len() {
            let tint = game.wands[wi].tint;
            draw_rectangle(x0 + 24.0, y + 4.0, 4.0, size - 8.0, Color::from_rgba(tint[0], tint[1], tint[2], 255));
            self.text(&format!("{}", wi + 1), x0 + 36.0, y + 14.0, 2.0, if wi == game.wand { ACCENT } else { TEXT });
            for k in 0..game.wands[wi].slots.len() {
                let sx = x0 + 60.0 + k as f32 * (size + 4.0);
                let card = game.wands[wi].slots[k];
                if self.card_box(card, sx, y, size, false) {
                    if let Some(c) = card {
                        tip = Some((c, sx, y));
                    }
                    if self.pressed {
                        let held = game.held.take();
                        game.held = game.wands[wi].slots[k].take();
                        game.wands[wi].slots[k] = held;
                        self.clicked = true;
                    } else if right {
                        if let Some(c) = game.wands[wi].slots[k].take() {
                            game.bag.push(c);
                            self.clicked = true;
                        }
                    }
                }
            }
            y += size + 10.0;
        }
        y += 8.0;
        self.text("SPELLS", x0 + 24.0, y, 2.0, ACCENT);
        y += 26.0;
        let cols = ((pw - 48.0) / (size + 4.0)).floor() as usize;
        let bag_y = y;
        let mut take: Option<usize> = None;
        for (i, c) in game.bag.iter().enumerate() {
            let (cx, cy) = (x0 + 24.0 + (i % cols) as f32 * (size + 4.0), y + (i / cols) as f32 * (size + 4.0));
            if self.card_box(Some(*c), cx, cy, size, false) {
                tip = Some((*c, cx, cy));
                if self.pressed {
                    take = Some(i);
                }
            }
        }
        let rows = (game.bag.len() + cols) / cols;
        let bag_rect = (x0 + 24.0, bag_y, cols as f32 * (size + 4.0), rows.max(1) as f32 * (size + 4.0));
        if let Some(i) = take {
            let c = game.bag.remove(i);
            if let Some(h) = game.held.replace(c) {
                game.bag.insert(i, h);
            }
            self.clicked = true;
        } else if self.pressed && game.held.is_some() && self.over(bag_rect.0, bag_rect.1, bag_rect.2, bag_rect.3 + size) {
            game.stow_held();
            self.clicked = true;
        }
        if right {
            game.stow_held();
        }
        if let Some((c, x, y)) = tip {
            if game.held.is_none() {
                self.tooltip(c, x, y);
            }
        }
        if let Some(c) = game.held {
            let (rows, main, acc) = c.icon();
            self.icon(&rows, main, acc, self.mouse.x - 14.0, self.mouse.y - 14.0, 4.0, 0.9);
        }
    }
}
