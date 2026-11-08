//! World saves on disk, one folder per world:
//!   ~/Library/Application Support/Falling Wizard/worlds/<world>/
//!     world.txt                  settings, where you are, open portals
//!     planet_<x>_<y>.bin         every chunk you've changed on that planet

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::cell::Cell;
use crate::materials::Material;
use crate::world::{SavedChunk, CHUNK};

const CU: usize = CHUNK as usize;

pub fn root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Library/Application Support/Falling Wizard/worlds")
}

/// A fresh folder for a new world, named after it.
pub fn new_dir(name: &str) -> PathBuf {
    let mut slug: String = name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        slug = "world".into();
    }
    let mut dir = root().join(&slug);
    let mut k = 2;
    while dir.exists() {
        dir = root().join(format!("{slug}-{k}"));
        k += 1;
    }
    dir
}

pub fn open_folder() {
    let _ = std::fs::create_dir_all(root());
    let _ = std::process::Command::new("open").arg(root()).spawn();
}

pub struct SaveInfo {
    pub dir: PathBuf,
    pub name: String,
    pub creative: bool,
    pub planet: String,
    pub played: u64,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Read world.txt into key/value pairs (repeated keys are kept in order).
pub fn read_meta(dir: &Path) -> Option<Vec<(String, String)>> {
    let text = std::fs::read_to_string(dir.join("world.txt")).ok()?;
    Some(text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect())
}

pub fn list() -> Vec<SaveInfo> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root()) else { return out };
    for e in entries.flatten() {
        let dir = e.path();
        let Some(meta) = read_meta(&dir) else { continue };
        let get = |k: &str| meta.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone()).unwrap_or_default();
        out.push(SaveInfo { name: get("name"), creative: get("creative") == "true", planet: get("planet_name"), played: get("played").parse().unwrap_or(0), dir });
    }
    out.sort_by(|a, b| b.played.cmp(&a.played));
    out
}

pub fn delete(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

pub fn ago(t: u64) -> String {
    let d = now().saturating_sub(t);
    match d {
        0..=59 => "JUST NOW".into(),
        60..=3599 => format!("{} MIN AGO", d / 60),
        3600..=86399 => format!("{} H AGO", d / 3600),
        _ => format!("{} DAYS AGO", d / 86400),
    }
}

fn planet_file(dir: &Path, gx: i32, gy: i32) -> PathBuf {
    dir.join(format!("planet_{gx}_{gy}.bin"))
}

pub fn write_chunks(dir: &Path, gx: i32, gy: i32, chunks: &[SavedChunk]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut buf = Vec::with_capacity(8 + chunks.len() * (8 + CU * CU * 7));
    buf.extend_from_slice(b"FWC1");
    buf.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    for ((cx, cy), cells, wall) in chunks {
        buf.extend_from_slice(&cx.to_le_bytes());
        buf.extend_from_slice(&cy.to_le_bytes());
        for c in cells {
            buf.extend_from_slice(&[c.mat as u8, c.life, c.temp, c.shade, c.vel, c.back as u8]);
        }
        buf.extend_from_slice(wall);
    }
    let tmp = planet_file(dir, gx, gy).with_extension("tmp");
    std::fs::File::create(&tmp)?.write_all(&buf)?;
    std::fs::rename(tmp, planet_file(dir, gx, gy))
}

fn mat(v: u8) -> Material {
    if v <= crate::materials::LAST_MATERIAL as u8 {
        // SAFETY: Material is repr(u8) with contiguous discriminants up to LAST_MATERIAL.
        unsafe { std::mem::transmute::<u8, Material>(v) }
    } else {
        Material::Empty
    }
}

pub fn read_chunks(dir: &Path, gx: i32, gy: i32) -> Vec<SavedChunk> {
    let mut out = Vec::new();
    let Ok(mut f) = std::fs::File::open(planet_file(dir, gx, gy)) else { return out };
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() || buf.len() < 8 || &buf[..4] != b"FWC1" {
        return out;
    }
    let n = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let per = 8 + CU * CU * 6 + CU * CU;
    let mut seen = HashMap::new();
    for k in 0..n {
        let o = 8 + k * per;
        if o + per > buf.len() {
            break;
        }
        let cx = i32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
        let cy = i32::from_le_bytes(buf[o + 4..o + 8].try_into().unwrap());
        let cells: Vec<Cell> = (0..CU * CU)
            .map(|i| {
                let b = &buf[o + 8 + i * 6..o + 14 + i * 6];
                Cell { mat: mat(b[0]), life: b[1], temp: b[2], shade: b[3], vel: b[4], clock: 0, back: mat(b[5]) }
            })
            .collect();
        let w0 = o + 8 + CU * CU * 6;
        let wall = buf[w0..w0 + CU * CU].to_vec();
        seen.insert((cx, cy), (cells, wall));
    }
    for (k, (c, w)) in seen {
        out.push((k, c, w));
    }
    out
}
