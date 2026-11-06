# Falling Wizard

A Noita-style falling-sand game in Rust and [macroquad](https://github.com/not-fl3/macroquad), set in an infinite, procedurally generated world in the spirit of Terraria and Minecraft.
There are no asset files: every pixel comes from the simulation, the font is drawn in code, and every sound is synthesised at startup.

```
cargo run --release
```

## The world

- **Infinite** in every direction. It is generated chunk by chunk (64×64 cells) as you explore. Chunks you changed are kept; untouched ones are regenerated identically from the seed.
- **Biomes:** forest (grass, oaks, flowers), desert (dunes, cacti) and tundra (snow, pines, ice, mountains).
- **Layers:** topsoil, then stone with caves that widen with depth, then basalt and huge caverns with lava lakes. It gets darker as you go down.
- **Underground:** coal, gold, sand and dirt pockets, gunpowder seams, water pools, sealed oil and gunpowder pockets, and acid pools.

## Materials

Sand, dirt, grass (spreads over exposed dirt), snow, ice, glass, wood, leaves, plants, seeds (grow into plants on soil), coal, gold, water, oil, acid, lava, fire, smoke, steam (condenses back to rain) and gunpowder (chain-explodes).
Lava ignites flammables, boils water into steam and cools into basalt, melts ice and turns sand into glass. Acid eats almost everything except glass.

## Spells

| Key | Spell | Notes |
|---|---|---|
| 1 | Spark Bolt | fast, small explosion |
| 2 | Fireball | arcing, sets everything alight |
| 3 | Bomb | bounces, big blast after a short fuse |
| 4 | Digging Beam | hold to bore through rock (collects gold) |
| 5 | Torrent | hold to spray water |
| 6 | Frost | freezes water, turns lava to basalt, puts out fire |
| 7 | Teleport | blink to where the bolt lands |
| 8 | Seeds of Life | scatter seeds that grow into plants |

Spells cost mana, which regenerates. Your own explosions can hurt you. Fire, lava, acid and long falls all do damage.

## Controls

| Input | Action |
|---|---|
| A / D or arrows | Move |
| W / Space | Jump; hold in the air to fly (flight refills on the ground) |
| Left mouse | Cast the selected spell |
| Right mouse | Dig |
| 1–8, mouse wheel, Q / E | Choose a spell |
| Tab | Sandbox mode: LMB paints, RMB erases, wheel sets brush size, 1–0 or Q/E choose material |
| Esc | Pause (settings, controls, main menu) |
| F11 | Fullscreen |

Settings (master/effects/ambience volume, screen shake, FPS counter, fullscreen) are saved to `~/.falling_wizard.cfg`.

## Layout

- `world.rs`: streaming chunk window, sleeping 16×16 sim chunks, all material rules
- `worldgen.rs`: biomes, terrain layers, caves, ores, pools and trees
- `materials.rs` and `cell.rs`: material properties and the per-cell state
- `player.rs`: wizard physics, health, mana, flight
- `spells.rs`: projectiles, beam and sprays
- `fx.rs`: particles, explosions, freezing, digging
- `render.rs`: camera view, sky with parallax mountains and clouds, light map and glow
- `ui.rs`: pixel font, widgets, HUD
- `audio.rs`: synthesised effects and ambience loops with volume channels
- `settings.rs`, `game.rs`, `main.rs`: settings, game state, screens and input

## Dev tools

- `cargo run --release -- --bench` runs a headless benchmark that flies across the world casting spells, plus material behaviour checks.
- `cargo run --release -- --autotest 15 [dir]` runs a scripted, muted playthrough that prints FPS stats and optionally saves screenshots.
- `--mute` starts the game without audio.
