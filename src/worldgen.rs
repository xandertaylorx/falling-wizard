//! Procedural world: a noisy surface, layered caves and worm tunnels, sand pockets,
//! a water pool, a sealed oil pocket and a few acid pools.

use crate::cell::Cell;
use crate::materials::Material;
use crate::noise::{fbm, Rng};
use crate::world::{World, H, W};

/// Fills `world` and returns the wizard's spawn position (top-left of its box).
pub fn generate(world: &mut World, seed: u64) -> (f32, f32) {
    let s = seed as u32;
    let mut rng = Rng::new(seed ^ 0xC0FFEE);

    let surface: Vec<i32> = (0..W)
        .map(|x| 26 + ((fbm(x as f32 * 0.018, 0.5, 3, s) - 0.5) * 34.0) as i32)
        .collect();

    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let top = surface[x as usize];
            let depth = y - top;
            let (fx, fy) = (x as f32, y as f32);
            let mat = if depth < 0 {
                Material::Empty
            } else {
                // Horizontally stretched noise gives layered cave bands.
                let cave = fbm(fx * 0.03, fy * 0.065, 4, s.wrapping_add(1));
                let worm = (fbm(fx * 0.017, fy * 0.03, 3, s.wrapping_add(2)) - 0.5).abs();
                let open = (cave > 0.6 && depth > 8) || (worm < 0.028 && depth > 5);
                if open {
                    Material::Empty
                } else if depth < 3 || fbm(fx * 0.05, fy * 0.05, 3, s.wrapping_add(3)) > 0.66 {
                    Material::Sand
                } else {
                    Material::Stone
                }
            };
            let mut c = Cell::new(mat, &mut rng);
            if mat == Material::Stone {
                // Strata: wavy horizontal bands plus grain.
                let band = ((fy * 0.55 + fbm(fx * 0.02, fy * 0.02, 2, s.wrapping_add(4)) * 9.0).sin() * 0.5 + 0.5) * 120.0;
                c.shade = (band + rng.f32() * 80.0) as u8;
            }
            world.cells[World::idx(x, y)] = c;
        }
    }

    // Water pool: an open stone bowl.
    let (wx, wy) = (rng.range(205, 265), rng.range(95, 120));
    bowl(world, &mut rng, wx, wy, 26, 11, Material::Water, false);

    // Oil pocket: a sealed shell, waiting for a spark.
    let (ox, oy) = (rng.range(95, 150), rng.range(125, 150));
    bowl(world, &mut rng, ox, oy, 18, 7, Material::Oil, true);

    // A few acid pools scattered about.
    for _ in 0..3 {
        let (ax, ay) = (rng.range(20, 300), rng.range(70, 165));
        if (ax - wx).abs() < 40 && (ay - wy).abs() < 22 || (ax - ox).abs() < 30 && (ay - oy).abs() < 16 {
            continue;
        }
        let rx = rng.range(6, 10);
        bowl(world, &mut rng, ax, ay, rx, 4, Material::Acid, false);
    }

    // Bedrock floor.
    for x in 0..W as i32 {
        for y in H as i32 - 2..H as i32 {
            world.cells[World::idx(x, y)] = Cell::new(Material::Stone, &mut rng);
        }
    }

    // Spawn: clear a column above the surface near the left edge.
    let sx = 24;
    let sy = surface[sx as usize];
    for y in 0..sy {
        for x in sx - 3..sx + 6 {
            world.cells[World::idx(x, y)] = Cell::EMPTY;
        }
    }

    world.wake_all();
    (sx as f32, (sy - 9) as f32)
}

/// Carve an elliptical cavity with a stone shell and fill its lower part with `fill`.
/// `sealed` adds a lid so the contents are trapped until something breaks in.
#[allow(clippy::too_many_arguments)]
fn bowl(world: &mut World, rng: &mut Rng, cx: i32, cy: i32, rx: i32, ry: i32, fill: Material, sealed: bool) {
    let fill_from = if sealed { cy - ry + 2 } else { cy - ry / 3 };
    for dy in -(ry + 4)..=(ry + 4) {
        for dx in -(rx + 4)..=(rx + 4) {
            let (x, y) = (cx + dx, cy + dy);
            if !World::in_bounds(x, y) {
                continue;
            }
            let e = (dx as f32 / rx as f32).powi(2) + (dy as f32 / ry as f32).powi(2);
            let i = World::idx(x, y);
            if e <= 1.0 {
                let m = if y >= fill_from { fill } else { Material::Empty };
                world.cells[i] = Cell::new(m, rng);
            } else if e <= 1.6 && (sealed || dy >= 0) && world.cells[i].mat != Material::Stone {
                let mut c = Cell::new(Material::Stone, rng);
                c.shade = 60 + (rng.f32() * 50.0) as u8;
                world.cells[i] = c;
            }
        }
    }
}
