# Falling Wizard

A small falling-sand game in Rust and [macroquad](https://github.com/not-fl3/macroquad), with Noita-style mechanics.
There are no asset files: every pixel comes from the simulation and every sound is synthesised at startup.

```
cargo run --release
```

## Controls

| Input | Action |
|---|---|
| A / D or Left / Right | Walk |
| W or Up | Jump; hold in the air to levitate (uses mana, refills on the ground); swim up in liquids |
| Left mouse | Cast a spark bolt at the cursor (explodes on impact) |
| Right mouse | Dig a small hole at the cursor |
| Middle mouse or Space | Paint the selected material at the cursor |
| Mouse wheel | Brush size (1–20) |
| 1 Sand, 2 Water, 3 Oil, 4 Fire, 5 Acid, 6 Stone, 7 Smoke, 0 Eraser | Choose the brush material |
| R | Generate a new world |
| Esc | Quit |

## Layout

- `cell.rs`: a cell holds a material plus life, temperature, colour shade, fall speed and an "updated this step" clock.
- `materials.rs`: material ids and properties (kind, density, flammability, colours).
- `world.rs`: the 320×180 grid, chunked sleeping, update rules, rendering into one RGBA buffer.
- `worldgen.rs`: noise-based surface, layered caves and worm tunnels, sand pockets, water pool, oil pocket, acid pools.
- `player.rs`: wizard physics, collision and sprite.
- `fx.rs`: spark bolts, explosions, debris and spark particles, screen shake, the additive glow light map.
- `audio.rs`: synthesised whoosh, thump and a looping fire crackle written as in-memory WAVs.
- `main.rs`: game loop, fixed 60 Hz simulation step, input, HUD.

## Dev tools

- `cargo run --release -- --bench` runs 1800 headless steps with explosions and reports sim and render cost.
- `cargo run --release -- --autotest 10 [dir]` runs a scripted session (walking, casting, igniting oil), prints FPS stats, and optionally saves screenshots to `dir`.
