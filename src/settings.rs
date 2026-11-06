//! User settings, saved as `key=value` lines in ~/.falling_wizard.cfg.

use std::path::PathBuf;

#[derive(Clone, PartialEq)]
pub struct Settings {
    pub master: f32,
    pub effects: f32,
    pub ambience: f32,
    pub shake: bool,
    pub show_fps: bool,
    pub fullscreen: bool,
    pub hq_light: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { master: 0.8, effects: 0.8, ambience: 0.7, shake: true, show_fps: true, fullscreen: false, hq_light: true }
    }
}

fn path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".falling_wizard.cfg"))
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::default();
        let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let f = v.parse::<f32>().ok().map(|x| x.clamp(0.0, 1.0));
            let b = v == "true";
            match k {
                "master" => s.master = f.unwrap_or(s.master),
                "effects" => s.effects = f.unwrap_or(s.effects),
                "ambience" => s.ambience = f.unwrap_or(s.ambience),
                "shake" => s.shake = b,
                "show_fps" => s.show_fps = b,
                "fullscreen" => s.fullscreen = b,
                "hq_light" => s.hq_light = b,
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            let text = format!(
                "master={:.2}\neffects={:.2}\nambience={:.2}\nshake={}\nshow_fps={}\nfullscreen={}\nhq_light={}\n",
                self.master, self.effects, self.ambience, self.shake, self.show_fps, self.fullscreen, self.hq_light
            );
            let _ = std::fs::write(p, text);
        }
    }
}
