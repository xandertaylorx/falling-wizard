//! Sound effects synthesised at startup and wrapped in in-memory WAV files.

use crate::noise::Rng;
use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, PlaySoundParams, Sound};
use std::f32::consts::TAU;

const RATE: u32 = 44_100;

pub struct Audio {
    whoosh: Sound,
    thump: Sound,
    crackle: Sound,
    crackle_vol: f32,
    applied_vol: f32,
}

impl Audio {
    /// Returns `None` if the audio device or decoder is unavailable; the game then runs silent.
    pub async fn new() -> Option<Audio> {
        let mut rng = Rng::new(7);
        let whoosh = load_sound_from_bytes(&wav(&synth_whoosh(&mut rng))).await.ok()?;
        let thump = load_sound_from_bytes(&wav(&synth_thump(&mut rng))).await.ok()?;
        let crackle = load_sound_from_bytes(&wav(&synth_crackle(&mut rng))).await.ok()?;
        play_sound(&crackle, PlaySoundParams { looped: true, volume: 0.0 });
        Some(Audio { whoosh, thump, crackle, crackle_vol: 0.0, applied_vol: 0.0 })
    }

    pub fn whoosh(&self) {
        play_sound(&self.whoosh, PlaySoundParams { looped: false, volume: 0.45 });
    }

    pub fn thump(&self) {
        play_sound(&self.thump, PlaySoundParams { looped: false, volume: 0.9 });
    }

    /// Fade the fire crackle loop with the amount of fire on screen.
    pub fn update(&mut self, fire_cells: usize, dt: f32) {
        let target = ((fire_cells as f32) / 400.0).sqrt().min(1.0) * 0.4;
        self.crackle_vol += (target - self.crackle_vol) * (4.0 * dt).min(1.0);
        if (self.crackle_vol - self.applied_vol).abs() > 0.01 {
            self.applied_vol = self.crackle_vol;
            set_sound_volume(&self.crackle, self.applied_vol);
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
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
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

/// One-pole low-pass coefficient for a cutoff in Hz.
fn lp_coef(hz: f32) -> f32 {
    1.0 - (-TAU * hz / RATE as f32).exp()
}

/// Band-passed noise with a sweeping centre, plus a faint rising tone.
fn synth_whoosh(rng: &mut Rng) -> Vec<f32> {
    let n = (RATE as f32 * 0.38) as usize;
    let (mut lo, mut hi, mut phase) = (0.0f32, 0.0f32, 0.0f32);
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let centre = 400.0 + 2600.0 * (t * std::f32::consts::PI).sin();
            let noise = rng.f32() * 2.0 - 1.0;
            hi += (noise - hi) * lp_coef(centre * 1.6);
            lo += (noise - lo) * lp_coef(centre * 0.6);
            let band = hi - lo;
            phase += TAU * (500.0 + 1200.0 * t) / RATE as f32;
            let env = (t / 0.08).min(1.0) * (1.0 - t).powf(1.6);
            (band * 1.8 + phase.sin() * 0.08) * env
        })
        .collect()
}

/// Pitch-dropping sine plus a low-passed noise burst.
fn synth_thump(rng: &mut Rng) -> Vec<f32> {
    let n = (RATE as f32 * 0.9) as usize;
    let (mut phase, mut lp) = (0.0f32, 0.0f32);
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = 38.0 + 110.0 * (-t * 14.0).exp();
            phase += TAU * f / RATE as f32;
            let body = phase.sin() * (-t * 4.5).exp();
            lp += (rng.f32() * 2.0 - 1.0 - lp) * lp_coef(900.0);
            let crack = lp * 2.2 * (-t * 16.0).exp();
            ((body * 1.1 + crack) * 1.3).tanh() * (1.0 - (t / 0.9).powi(4))
        })
        .collect()
}

/// A seamless loop of soft rumble and random pops.
fn synth_crackle(rng: &mut Rng) -> Vec<f32> {
    let n = RATE as usize * 3;
    let mut out = vec![0.0f32; n];
    let mut lp = 0.0f32;
    for s in out.iter_mut() {
        lp += (rng.f32() * 2.0 - 1.0 - lp) * lp_coef(300.0);
        *s = lp * 0.35;
    }
    let pops = 120;
    for _ in 0..pops {
        let at = (rng.f32() * n as f32) as usize;
        let amp = rng.rangef(0.15, 0.8);
        let len = rng.range(60, 260) as usize;
        let mut hp_prev = 0.0;
        for k in 0..len {
            let noise = rng.f32() * 2.0 - 1.0;
            let v = (noise - hp_prev) * 0.5; // crude high-pass: sharper click
            hp_prev = noise;
            let env = (-(k as f32) / (len as f32 * 0.25)).exp();
            let idx = (at + k) % n; // wrap so the loop stays seamless
            out[idx] += v * amp * env;
        }
    }
    out
}
