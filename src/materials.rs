//! Material definitions and their static properties.

use crate::noise::Rng;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Material {
    Empty,
    Stone,
    Basalt,
    Dirt,
    Grass,
    Sand,
    Snow,
    Ice,
    Glass,
    Wood,
    Leaves,
    Plant,
    /// Growing plant tip.
    Sprout,
    Seed,
    Coal,
    Gold,
    Water,
    Oil,
    Acid,
    Lava,
    Fire,
    Smoke,
    Steam,
    Gunpowder,
    /// Stone debris thrown out by explosions.
    Rubble,
    /// Loose rock in cave ceilings; collapses when disturbed.
    Gravel,
    /// Flammable cave gas that flashes into fire.
    Gas,
    /// Sand you sink into.
    Quicksand,
    Blood,
    Goo,
    Bone,
    MoonRock,
    Crystal,
    AlienGrass,
    Mushroom,
    Brick,
    Chest,
    Altar,
    Cloud,
    /// Portal structure stone (indestructible).
    PortalStone,
    /// Glowing runes on portal structures.
    Rune,
    /// An active portal's surface; its `temp` holds the destination's hue.
    Portal,
    /// Ground eaten by the void after a rift shatters.
    VoidRock,
    /// Glowing violet crystal that grows out of the void.
    VoidCrystal,
    /// Loose void ash.
    VoidDust,
}

/// The highest material id (for loading saves).
pub const LAST_MATERIAL: Material = Material::VoidDust;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Empty,
    Static,
    Powder,
    Liquid,
    Gas,
    Fire,
    /// Actively growing things.
    Life,
}

/// How a material burns once it reaches `ignite` heat.
pub struct Burn {
    pub ignite: u8,
    pub lo: i32,
    pub hi: i32,
    /// Burning liquid that keeps flowing while on fire.
    pub falls: bool,
}

use Material as M;

impl Material {
    #[inline]
    pub const fn kind(self) -> Kind {
        match self {
            M::Empty => Kind::Empty,
            M::Stone | M::Basalt | M::Dirt | M::Grass | M::Ice | M::Glass | M::Wood | M::Leaves | M::Plant | M::Coal | M::Gold
            | M::MoonRock | M::Crystal | M::AlienGrass | M::Mushroom | M::Brick | M::Chest | M::Altar | M::Cloud | M::PortalStone | M::Rune | M::Portal | M::VoidRock | M::VoidCrystal => Kind::Static,
            M::Sand | M::Snow | M::Seed | M::Gunpowder | M::Rubble | M::Gravel | M::Quicksand | M::Bone | M::VoidDust => Kind::Powder,
            M::Water | M::Oil | M::Acid | M::Lava | M::Blood | M::Goo => Kind::Liquid,
            M::Smoke | M::Steam | M::Gas => Kind::Gas,
            M::Fire => Kind::Fire,
            M::Sprout => Kind::Life,
        }
    }

    #[inline]
    pub const fn density(self) -> u8 {
        match self {
            M::Empty => 0,
            M::Smoke | M::Steam | M::Gas => 1,
            M::Blood => 11,
            M::Goo => 13,
            M::Bone => 17,
            M::Quicksand => 21,
            M::Gravel => 23,
            M::Fire => 2,
            M::Oil => 8,
            M::Water => 10,
            M::Acid => 12,
            M::Snow => 14,
            M::Lava => 15,
            M::Seed => 16,
            M::Gunpowder => 18,
            M::Sand => 20,
            M::Rubble => 22,
            M::VoidDust => 19,
            _ => 255,
        }
    }

    #[inline]
    pub const fn dispersion(self) -> i32 {
        match self {
            M::Water | M::Blood => 6,
            M::Oil | M::Acid => 4,
            M::Goo => 1,
            M::Lava => 2,
            _ => 0,
        }
    }

    pub const fn burn(self) -> Option<Burn> {
        match self {
            M::Oil => Some(Burn { ignite: 100, lo: 45, hi: 110, falls: true }),
            M::Wood => Some(Burn { ignite: 180, lo: 120, hi: 230, falls: false }),
            M::Leaves => Some(Burn { ignite: 50, lo: 12, hi: 35, falls: false }),
            M::Plant | M::Sprout => Some(Burn { ignite: 60, lo: 20, hi: 45, falls: false }),
            M::Grass => Some(Burn { ignite: 140, lo: 8, hi: 18, falls: false }),
            M::Coal => Some(Burn { ignite: 220, lo: 200, hi: 255, falls: false }),
            M::Seed => Some(Burn { ignite: 50, lo: 4, hi: 10, falls: false }),
            M::Gunpowder => Some(Burn { ignite: 20, lo: 1, hi: 2, falls: false }),
            M::Gas => Some(Burn { ignite: 10, lo: 3, hi: 9, falls: false }),
            M::Mushroom => Some(Burn { ignite: 70, lo: 20, hi: 40, falls: false }),
            M::AlienGrass => Some(Burn { ignite: 150, lo: 8, hi: 18, falls: false }),
            _ => None,
        }
    }

    #[inline]
    pub const fn flammable(self) -> bool {
        self.burn().is_some()
    }

    /// Background plant life (as opposed to background structures).
    pub const fn foliage(self) -> bool {
        matches!(self, M::Wood | M::Leaves | M::Plant | M::Mushroom)
    }

    /// Portal structures can't be destroyed.
    pub const fn indestructible(self) -> bool {
        matches!(self, M::PortalStone | M::Rune | M::Portal)
    }

    /// Relative speed at which acid eats this material (0 = immune).
    #[inline]
    pub const fn acid_rate(self) -> f32 {
        match self {
            M::Stone => 1.0,
            M::Basalt => 0.25,
            M::Gold => 0.5,
            M::Dirt | M::Grass | M::Sand | M::Snow | M::Ice | M::Rubble | M::Coal | M::Seed | M::Gunpowder | M::Gravel | M::Quicksand | M::Bone => 1.6,
            M::MoonRock | M::Brick => 0.6,
            M::VoidDust => 1.0,
            M::VoidRock => 0.2,
            M::AlienGrass | M::Mushroom => 2.0,
            M::Wood | M::Leaves | M::Plant | M::Sprout => 2.0,
            _ => 0.0,
        }
    }

    /// Spells and particles fly through these.
    #[inline]
    pub const fn passable(self) -> bool {
        matches!(self, M::Empty | M::Smoke | M::Steam | M::Gas | M::Fire | M::Leaves | M::Plant | M::Sprout | M::Mushroom | M::Portal)
    }

    /// The wizard collides with these (foliage is walk-through).
    #[inline]
    pub const fn solid(self) -> bool {
        matches!(self.kind(), Kind::Static | Kind::Powder) && !matches!(self, M::Leaves | M::Plant | M::Mushroom | M::Quicksand | M::Portal)
    }

    pub const fn base_color(self) -> [u8; 3] {
        match self {
            M::Empty => [0, 0, 0],
            M::Stone => [98, 92, 102],
            M::Basalt => [54, 50, 62],
            M::Dirt => [112, 78, 52],
            M::Grass => [78, 156, 62],
            M::Sand => [216, 184, 114],
            M::Snow => [232, 238, 246],
            M::Ice => [158, 208, 238],
            M::Glass => [196, 228, 238],
            M::Wood => [108, 72, 44],
            M::Leaves => [54, 132, 56],
            M::Plant => [72, 164, 62],
            M::Sprout => [150, 226, 96],
            M::Seed => [176, 144, 84],
            M::Coal => [38, 36, 42],
            M::Gold => [236, 190, 64],
            M::Water => [38, 98, 200],
            M::Oil => [58, 40, 44],
            M::Acid => [110, 232, 64],
            M::Lava => [255, 110, 20],
            M::Fire => [255, 140, 30],
            M::Smoke => [150, 146, 150],
            M::Steam => [214, 220, 230],
            M::Gunpowder => [72, 68, 76],
            M::Rubble => [124, 114, 112],
            M::Gravel => [110, 102, 98],
            M::Gas => [120, 170, 90],
            M::Quicksand => [196, 160, 96],
            M::Blood => [150, 20, 28],
            M::Goo => [90, 210, 110],
            M::Bone => [226, 220, 196],
            M::MoonRock => [150, 146, 170],
            M::Crystal => [140, 220, 255],
            M::AlienGrass => [190, 90, 230],
            M::Mushroom => [90, 230, 210],
            M::Brick => [120, 70, 60],
            M::Chest => [176, 120, 52],
            M::Altar => [230, 200, 120],
            M::Cloud => [246, 248, 255],
            M::PortalStone => [62, 52, 80],
            M::Rune => [150, 100, 255],
            M::Portal => [180, 120, 255],
            M::VoidRock => [46, 26, 70],
            M::VoidCrystal => [196, 110, 255],
            M::VoidDust => [112, 64, 160],
        }
    }

    pub const fn variation(self) -> f32 {
        match self {
            M::Stone => 0.45,
            M::Basalt => 0.5,
            M::Dirt => 0.35,
            M::Grass => 0.3,
            M::Sand => 0.28,
            M::Snow => 0.08,
            M::Ice => 0.15,
            M::Glass => 0.1,
            M::Wood => 0.4,
            M::Leaves => 0.5,
            M::Plant => 0.3,
            M::Sprout => 0.2,
            M::Seed => 0.3,
            M::Coal => 0.7,
            M::Gold => 0.5,
            M::Water => 0.14,
            M::Oil => 0.3,
            M::Acid => 0.22,
            M::Lava => 0.3,
            M::Gunpowder => 0.6,
            M::Rubble | M::Gravel => 0.4,
            M::Quicksand => 0.25,
            M::Blood | M::Goo => 0.2,
            M::Bone => 0.15,
            M::MoonRock => 0.35,
            M::Crystal => 0.3,
            M::AlienGrass => 0.3,
            M::Mushroom => 0.3,
            M::Brick => 0.3,
            M::Chest => 0.2,
            M::Altar => 0.2,
            M::Cloud => 0.06,
            M::PortalStone => 0.3,
            M::Rune => 0.2,
            M::VoidRock => 0.55,
            M::VoidCrystal => 0.3,
            M::VoidDust => 0.35,
            _ => 0.0,
        }
    }

    /// Light emitted, used by the lighting pass.
    pub const fn emissive(self) -> Option<[f32; 3]> {
        match self {
            M::Lava => Some([1.0, 0.42, 0.08]),
            M::Acid => Some([0.03, 0.11, 0.02]),
            M::Crystal => Some([0.25, 0.55, 0.8]),
            M::Mushroom => Some([0.15, 0.6, 0.55]),
            M::Altar => Some([0.9, 0.7, 0.3]),
            M::Rune => Some([0.6, 0.4, 1.4]),
            M::Goo => Some([0.04, 0.16, 0.06]),
            M::VoidCrystal => Some([0.7, 0.25, 1.3]),
            M::VoidRock => Some([0.04, 0.0, 0.08]),
            _ => None,
        }
    }

    pub fn initial_life(self, rng: &mut Rng) -> u8 {
        match self {
            M::Smoke => rng.range(70, 150) as u8,
            M::Steam => rng.range(40, 90) as u8,
            M::Gas => 255,
            M::Fire => rng.range(6, 20) as u8,
            M::Acid => 5,
            M::Sprout => rng.range(10, 28) as u8,
            _ => 0,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            M::Empty => "Eraser",
            M::Stone => "Stone",
            M::Basalt => "Basalt",
            M::Dirt => "Dirt",
            M::Grass => "Grass",
            M::Sand => "Sand",
            M::Snow => "Snow",
            M::Ice => "Ice",
            M::Glass => "Glass",
            M::Wood => "Wood",
            M::Leaves => "Leaves",
            M::Plant => "Plant",
            M::Sprout => "Sprout",
            M::Seed => "Seeds",
            M::Coal => "Coal",
            M::Gold => "Gold",
            M::Water => "Water",
            M::Oil => "Oil",
            M::Acid => "Acid",
            M::Lava => "Lava",
            M::Fire => "Fire",
            M::Smoke => "Smoke",
            M::Steam => "Steam",
            M::Gunpowder => "Gunpowder",
            M::Rubble => "Rubble",
            M::Gravel => "Gravel",
            M::Gas => "Gas",
            M::Quicksand => "Quicksand",
            M::Blood => "Blood",
            M::Goo => "Goo",
            M::Bone => "Bone",
            M::MoonRock => "Moon Rock",
            M::Crystal => "Crystal",
            M::AlienGrass => "Star Moss",
            M::Mushroom => "Glowshroom",
            M::Brick => "Brick",
            M::Chest => "Chest",
            M::Altar => "Altar",
            M::Cloud => "Cloud",
            M::PortalStone => "Portal Stone",
            M::Rune => "Rune",
            M::Portal => "Portal",
            M::VoidRock => "Void Rock",
            M::VoidCrystal => "Void Crystal",
            M::VoidDust => "Void Dust",
        }
    }
}
