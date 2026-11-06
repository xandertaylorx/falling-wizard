//! A single simulation cell: material id plus a few bytes of state.

use crate::materials::Material;
use crate::noise::Rng;

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub mat: Material,
    /// Remaining lifetime (fire, smoke) or dissolving power (acid).
    pub life: u8,
    /// Heat. Flammables ignite when it gets high; fire >200 means "burning fuel".
    pub temp: u8,
    /// Per-cell colour noise, carried with the cell as it moves.
    pub shade: u8,
    /// Falling speed accumulator for powders and liquids.
    pub vel: u8,
    /// Equals the world clock when the cell was already updated this step.
    pub clock: u8,
}

impl Cell {
    pub const EMPTY: Cell = Cell {
        mat: Material::Empty,
        life: 0,
        temp: 0,
        shade: 0,
        vel: 0,
        clock: 0,
    };

    pub fn new(mat: Material, rng: &mut Rng) -> Cell {
        Cell {
            mat,
            life: mat.initial_life(rng),
            temp: 0,
            shade: rng.next_u32() as u8,
            vel: 0,
            clock: 0,
        }
    }

    /// `fuel` fires are burning oil: long-lived, smoky, and they spawn flames above.
    pub fn fire(fuel: bool, rng: &mut Rng) -> Cell {
        Cell {
            mat: Material::Fire,
            life: if fuel { rng.range(45, 110) } else { rng.range(6, 20) } as u8,
            temp: if fuel { 255 } else { 150 },
            shade: rng.next_u32() as u8,
            vel: 0,
            clock: 0,
        }
    }

    pub fn smoke(rng: &mut Rng, lo: i32, hi: i32) -> Cell {
        Cell {
            mat: Material::Smoke,
            life: rng.range(lo, hi) as u8,
            temp: 0,
            shade: rng.next_u32() as u8,
            vel: 0,
            clock: 0,
        }
    }
}
