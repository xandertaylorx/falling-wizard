//! Sound effects synthesised at startup and wrapped in in-memory WAV files.
//! Volumes: master x (effects | ambience).

use crate::noise::Rng;
use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, PlaySoundParams, Sound};
use std::f32::consts::{PI, TAU};

const RATE: u32 = 44_100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Whoosh,
    Fireball,
    Throw,
    Boom,
    Pop,
    Zap,
    Freeze,
    Splash,
    Dig,
    Hurt,
    Click,
    Tick,
    Hit,
    Chime,
    Dash,
    Thunder,
}

const SFX_COUNT: usize = 16;

impl Sfx {
    fn idx(self) -> usize {
        self as usize
    }
    /// (base volume, minimum seconds between plays)
    fn params(self) -> (f32, f32) {
        match self {
            Sfx::Whoosh => (0.4, 0.05),
            Sfx::Fireball => (0.5, 0.1),
            Sfx::Throw => (0.45, 0.1),
            Sfx::Boom => (0.9, 0.06),
            Sfx::Pop => (0.5, 0.05),
            Sfx::Zap => (0.45, 0.1),
            Sfx::Freeze => (0.5, 0.1),
            Sfx::Splash => (0.25, 0.09),
            Sfx::Dig => (0.25, 0.08),
            Sfx::Hurt => (0.5, 0.35),
            Sfx::Click => (0.35, 0.03),
            Sfx::Tick => (0.35, 0.06),
            Sfx::Hit => (0.35, 0.05),
            Sfx::Chime => (0.5, 0.2),
            Sfx::Dash => (0.35, 0.1),
            Sfx::Thunder => (1.0, 1.0),
        }
    }
}

pub struct Audio {
    sounds: Vec<Sound>,
    crackle: Sound,
    rumble: Sound,
    cooldown: [f32; SFX_COUNT],
    fire_vol: f32,
    lava_vol: f32,
    applied: (f32, f32),
    pub master: f32,
    pub effects: f32,
    pub ambience: f32,
}

impl Audio {
    /// Returns `None` if audio can't be initialised; the game then runs silent.
    pub async fn new() -> Option<Audio> {
        let mut rng = Rng::new(7);
        let synths: [fn(&mut Rng) -> Vec<f32>; SFX_COUNT] =
            [whoosh, fireball, throw, boom, pop, zap, freeze, splash, dig, hurt, click, tick, hit, chime, dash, thunder];
        let mut sounds = Vec::new();
        for f in synths {
            sounds.push(load_sound_from_bytes(&wav(&f(&mut rng))).await.ok()?);
        }
        let crackle = load_sound_from_bytes(&wav(&crackle(&mut rng))).await.ok()?;
        let rumble = load_sound_from_bytes(&wav(&rumble(&mut rng))).await.ok()?;
        play_sound(&crackle, PlaySoundParams { looped: true, volume: 0.0 });
        play_sound(&rumble, PlaySoundParams { looped: true, volume: 0.0 });
        Some(Audio {
            sounds,
            crackle,
            rumble,
            cooldown: [0.0; SFX_COUNT],
            fire_vol: 0.0,
            lava_vol: 0.0,
            applied: (0.0, 0.0),
            master: 0.8,
            effects: 0.8,
            ambience: 0.7,
        })
    }

    pub fn play(&mut self, s: Sfx) {
        let (base, gap) = s.params();
        if self.cooldown[s.idx()] > 0.0 {
            return;
        }
        let vol = base * self.master * self.effects;
        if vol < 0.002 {
            return;
        }
        self.cooldown[s.idx()] = gap;
        play_sound(&self.sounds[s.idx()], PlaySoundParams { looped: false, volume: vol });
    }

    /// Fade ambience loops with the amount of fire and lava on screen.
    pub fn update(&mut self, dt: f32, fire: usize, lava: usize) {
        for c in self.cooldown.iter_mut() {
            *c -= dt;
        }
        let k = (4.0 * dt).min(1.0);
        self.fire_vol += (((fire as f32) / 400.0).sqrt().min(1.0) * 0.45 - self.fire_vol) * k;
        self.lava_vol += (((lava as f32) / 600.0).sqrt().min(1.0) * 0.5 - self.lava_vol) * k;
        let amb = self.master * self.ambience;
        let (f, l) = (self.fire_vol * amb, self.lava_vol * amb);
        if (f - self.applied.0).abs() > 0.005 {
            set_sound_volume(&self.crackle, f);
            self.applied.0 = f;
        }
        if (l - self.applied.1).abs() > 0.005 {
            set_sound_volume(&self.rumble, l);
            self.applied.1 = l;
        }
    }
}

/// 16-bit mono PCM WAV.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    b
}

fn lp(hz: f32) -> f32 {
    1.0 - (-TAU * hz / RATE as f32).exp()
}

fn samples(secs: f32) -> usize {
    (RATE as f32 * secs) as usize
}

/// Band-passed noise sweep: the spark bolt.
fn whoosh(rng: &mut Rng) -> Vec<f32> {
    noise_sweep(rng, 0.36, 400.0, 2600.0, 0.08)
}

fn fireball(rng: &mut Rng) -> Vec<f32> {
    let mut v = noise_sweep(rng, 0.5, 200.0, 1200.0, 0.05);
    for (i, s) in v.iter_mut().enumerate() {
        if rng.chance(0.004) {
            *s += 0.5 * (1.0 - i as f32 / 22000.0).max(0.0);
        }
    }
    v
}

fn throw(rng: &mut Rng) -> Vec<f32> {
    noise_sweep(rng, 0.22, 900.0, 500.0, 0.02)
}

fn noise_sweep(rng: &mut Rng, secs: f32, f0: f32, f1: f32, attack: f32) -> Vec<f32> {
    let n = samples(secs);
    let (mut lo, mut hi) = (0.0f32, 0.0f32);
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let centre = f0 + (f1 - f0) * (t * PI).sin();
            let noise = rng.f32() * 2.0 - 1.0;
            hi += (noise - hi) * lp(centre * 1.6);
            lo += (noise - lo) * lp(centre * 0.6);
            let env = (t / attack).min(1.0) * (1.0 - t).powf(1.6);
            (hi - lo) * 1.8 * env
        })
        .collect()
}

/// Pitch-dropping sine plus a low-passed noise burst.
fn boom(rng: &mut Rng) -> Vec<f32> {
    thump(rng, 0.9, 38.0, 110.0, 900.0)
}

fn pop(rng: &mut Rng) -> Vec<f32> {
    thump(rng, 0.35, 70.0, 160.0, 2000.0)
}

fn thump(rng: &mut Rng, secs: f32, f_end: f32, f_add: f32, noise_hz: f32) -> Vec<f32> {
    let n = samples(secs);
    let (mut phase, mut l) = (0.0f32, 0.0f32);
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            phase += TAU * (f_end + f_add * (-t * 14.0).exp()) / RATE as f32;
            let body = phase.sin() * (-t * 4.5 / secs).exp();
            l += (rng.f32() * 2.0 - 1.0 - l) * lp(noise_hz);
            let crack = l * 2.2 * (-t * 16.0).exp();
            ((body * 1.1 + crack) * 1.3).tanh() * (1.0 - (t / secs).powi(4))
        })
        .collect()
}

/// Rising sine sweep with vibrato: teleport.
fn zap(_: &mut Rng) -> Vec<f32> {
    let n = samples(0.3);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            phase += TAU * (300.0 + 1500.0 * t * t + (t * 60.0).sin() * 40.0) / RATE as f32;
            phase.sin().signum() * 0.18 * (1.0 - t) * (t / 0.05).min(1.0)
        })
        .collect()
}

/// Shimmering high partials: frost.
fn freeze(rng: &mut Rng) -> Vec<f32> {
    let n = samples(0.6);
    let freqs = [1760.0, 2217.0, 2637.0, 3520.0];
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let env = (-t * 6.0).exp();
            let tone: f32 = freqs.iter().enumerate().map(|(k, f)| (TAU * f * t + k as f32).sin() * (1.0 + (t * (9.0 + k as f32)).sin()) * 0.5).sum();
            (tone * 0.06 + (rng.f32() - 0.5) * 0.08) * env
        })
        .collect()
}

fn splash(rng: &mut Rng) -> Vec<f32> {
    let n = samples(0.12);
    let mut l = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            l += (rng.f32() * 2.0 - 1.0 - l) * lp(2500.0);
            l * (1.0 - t).powi(2) * 0.9
        })
        .collect()
}

fn dig(rng: &mut Rng) -> Vec<f32> {
    let n = samples(0.1);
    let mut l = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            l += (rng.f32() * 2.0 - 1.0 - l) * lp(700.0);
            let grit = if (i / 300) % 2 == 0 { 1.0 } else { 0.5 };
            l * grit * (1.0 - t) * 1.4
        })
        .collect()
}

fn hurt(_: &mut Rng) -> Vec<f32> {
    let n = samples(0.18);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            phase += TAU * (220.0 - 120.0 * t) / RATE as f32;
            (phase.sin() * 3.0).tanh() * 0.25 * (1.0 - t)
        })
        .collect()
}

fn click(_: &mut Rng) -> Vec<f32> {
    let n = samples(0.05);
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            (TAU * 1200.0 * t).sin() * (-t * 90.0).exp() * 0.5
        })
        .collect()
}

fn tick(_: &mut Rng) -> Vec<f32> {
    let n = samples(0.06);
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            (TAU * 320.0 * t).sin() * (-t * 60.0).exp() * 0.6
        })
        .collect()
}

fn hit(rng: &mut Rng) -> Vec<f32> {
    let n = samples(0.08);
    let mut l = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            l += (rng.f32() * 2.0 - 1.0 - l) * lp(1800.0);
            (l * 1.5 + (TAU * 180.0 * i as f32 / RATE as f32).sin() * 0.3) * (1.0 - t).powi(2)
        })
        .collect()
}

/// Two bright bell partials: found something.
fn chime(_: &mut Rng) -> Vec<f32> {
    let n = samples(0.7);
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let a = if t < 0.09 { 880.0 } else { 1318.5 };
            let local = if t < 0.09 { t } else { t - 0.09 };
            ((TAU * a * t).sin() * 0.25 + (TAU * a * 2.0 * t).sin() * 0.08) * (-local * 5.0).exp()
        })
        .collect()
}

fn dash(rng: &mut Rng) -> Vec<f32> {
    noise_sweep(rng, 0.15, 1800.0, 700.0, 0.02)
}

fn thunder(rng: &mut Rng) -> Vec<f32> {
    let n = samples(2.2);
    let mut l = 0.0f32;
    let mut l2 = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            l += (rng.f32() * 2.0 - 1.0 - l) * lp(250.0);
            l2 += (rng.f32() * 2.0 - 1.0 - l2) * lp(3000.0);
            let crack = l2 * (-t * 14.0).exp();
            let roll = l * 3.0 * (-t * 1.6).exp() * (1.0 + (t * 9.0).sin() * 0.4);
            (crack + roll).tanh() * 0.9
        })
        .collect()
}

/// Seamless loop of soft rumble and random pops: fire.
fn crackle(rng: &mut Rng) -> Vec<f32> {
    let n = samples(3.0);
    let mut out = vec![0.0f32; n];
    let mut l = 0.0f32;
    for s in out.iter_mut() {
        l += (rng.f32() * 2.0 - 1.0 - l) * lp(300.0);
        *s = l * 0.35;
    }
    for _ in 0..120 {
        let at = (rng.f32() * n as f32) as usize;
        let amp = rng.rangef(0.15, 0.8);
        let len = rng.range(60, 260) as usize;
        let mut prev = 0.0;
        for k in 0..len {
            let noise = rng.f32() * 2.0 - 1.0;
            let v = (noise - prev) * 0.5;
            prev = noise;
            out[(at + k) % n] += v * amp * (-(k as f32) / (len as f32 * 0.25)).exp();
        }
    }
    out
}

/// Seamless loop of deep rumble and slow bubbles: lava.
fn rumble(rng: &mut Rng) -> Vec<f32> {
    let n = samples(4.0);
    let mut out = vec![0.0f32; n];
    let mut l = 0.0f32;
    for s in out.iter_mut() {
        l += (rng.f32() * 2.0 - 1.0 - l) * lp(90.0);
        *s = l * 1.6;
    }
    for _ in 0..14 {
        let at = (rng.f32() * n as f32) as usize;
        let len = rng.range(3000, 7000) as usize;
        let f0 = rng.rangef(90.0, 180.0);
        let mut phase = 0.0f32;
        for k in 0..len {
            let t = k as f32 / len as f32;
            phase += TAU * f0 * (1.0 + t * 0.8) / RATE as f32;
            out[(at + k) % n] += phase.sin() * 0.18 * (t * PI).sin();
        }
    }
    out
}
