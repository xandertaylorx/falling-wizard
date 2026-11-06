//! A single simulation cell: material id plus a few bytes of state.

use crate::materials::Material;
use crate::noise::Rng;

/// Fire `temp` encodings.
pub const FIRE_FREE: u8 = 150;
pub const FIRE_FUEL: u8 = 240;
pub const FIRE_FLOWING: u8 = 255;

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub mat: Material,
    /// Remaining lifetime (fire, gases), dissolving power (acid) or growth left (sprouts).
    pub life: u8,
    /// Heat for flammables; for fire, which kind of fire it is.
    pub temp: u8,
    /// Per-cell colour noise, carried with the cell as it moves.
    pub shade: u8,
    /// Falling speed accumulator for powders and liquids.
    pub vel: u8,
    /// Equals the world clock when the cell was already updated this step.
    pub clock: u8,
}

impl Cell {
    pub const EMPTY: Cell = Cell { mat: Material::Empty, life: 0, temp: 0, shade: 0, vel: 0, clock: 0 };

    pub fn new(mat: Material, rng: &mut Rng) -> Cell {
        Cell { mat, life: mat.initial_life(rng), temp: 0, shade: rng.next_u32() as u8, vel: 0, clock: 0 }
    }

    /// A free flame that flickers upwards and dies quickly.
    pub fn fire(rng: &mut Rng) -> Cell {
        Cell { mat: Material::Fire, life: rng.range(6, 20) as u8, temp: FIRE_FREE, shade: rng.next_u32() as u8, vel: 0, clock: 0 }
    }

    /// Fire consuming a flammable material.
    pub fn burning(m: Material, rng: &mut Rng) -> Cell {
        let (life, temp) = match m.burn() {
            Some(b) => (rng.range(b.lo, b.hi), if b.falls { FIRE_FLOWING } else { FIRE_FUEL }),
            None => (rng.range(6, 20), FIRE_FREE),
        };
        Cell { mat: Material::Fire, life: life.clamp(1, 255) as u8, temp, shade: rng.next_u32() as u8, vel: 0, clock: 0 }
    }

    pub fn gas(mat: Material, rng: &mut Rng, lo: i32, hi: i32) -> Cell {
        Cell { mat, life: rng.range(lo, hi) as u8, temp: 0, shade: rng.next_u32() as u8, vel: 0, clock: 0 }
    }
}
