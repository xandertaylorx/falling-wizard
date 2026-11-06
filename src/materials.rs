//! Material definitions and their static properties.

use crate::noise::Rng;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Material {
    Empty,
    Stone,
    Sand,
    Water,
    Oil,
    Fire,
    Smoke,
    Acid,
    /// Stone debris thrown out by explosions; behaves like a powder.
    Rubble,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Empty,
    Static,
    Powder,
    Liquid,
    Gas,
    Fire,
}

impl Material {
    #[inline]
    pub const fn kind(self) -> Kind {
        match self {
            Material::Empty => Kind::Empty,
            Material::Stone => Kind::Static,
            Material::Sand | Material::Rubble => Kind::Powder,
            Material::Water | Material::Oil | Material::Acid => Kind::Liquid,
            Material::Smoke => Kind::Gas,
            Material::Fire => Kind::Fire,
        }
    }

    /// Heavier things sink through lighter liquids.
    #[inline]
    pub const fn density(self) -> u8 {
        match self {
            Material::Empty => 0,
            Material::Smoke => 1,
            Material::Fire => 2,
            Material::Oil => 8,
            Material::Water => 10,
            Material::Acid => 12,
            Material::Sand => 20,
            Material::Rubble => 22,
            Material::Stone => 255,
        }
    }

    /// How many cells a liquid may slide sideways per step.
    #[inline]
    pub const fn dispersion(self) -> i32 {
        match self {
            Material::Water => 6,
            Material::Oil => 4,
            Material::Acid => 4,
            _ => 0,
        }
    }

    #[inline]
    pub const fn flammable(self) -> bool {
        matches!(self, Material::Oil)
    }

    #[inline]
    pub const fn dissolvable(self) -> bool {
        matches!(self, Material::Stone | Material::Sand | Material::Rubble)
    }

    /// Projectiles and particles fly through these.
    #[inline]
    pub const fn passable(self) -> bool {
        matches!(self, Material::Empty | Material::Smoke | Material::Fire)
    }

    /// The wizard collides with these.
    #[inline]
    pub const fn solid(self) -> bool {
        matches!(self.kind(), Kind::Static | Kind::Powder)
    }

    pub const fn base_color(self) -> [u8; 3] {
        match self {
            Material::Empty => [0, 0, 0],
            Material::Stone => [96, 88, 100],
            Material::Sand => [214, 180, 110],
            Material::Water => [38, 98, 200],
            Material::Oil => [58, 40, 44],
            Material::Fire => [255, 140, 30],
            Material::Smoke => [150, 146, 150],
            Material::Acid => [110, 232, 64],
            Material::Rubble => [124, 114, 112],
        }
    }

    /// How strongly the per-cell noise shade varies the colour.
    pub const fn variation(self) -> f32 {
        match self {
            Material::Stone => 0.45,
            Material::Sand => 0.28,
            Material::Rubble => 0.4,
            Material::Water => 0.14,
            Material::Oil => 0.3,
            Material::Acid => 0.22,
            _ => 0.0,
        }
    }

    pub fn initial_life(self, rng: &mut Rng) -> u8 {
        match self {
            Material::Smoke => rng.range(70, 150) as u8,
            Material::Fire => rng.range(6, 20) as u8,
            Material::Acid => 5,
            _ => 0,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Material::Empty => "Eraser",
            Material::Stone => "Stone",
            Material::Sand => "Sand",
            Material::Water => "Water",
            Material::Oil => "Oil",
            Material::Fire => "Fire",
            Material::Smoke => "Smoke",
            Material::Acid => "Acid",
            Material::Rubble => "Rubble",
        }
    }
}
