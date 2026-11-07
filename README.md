# Falling Wizard

A creative, Noita-style falling-sand sandbox in Rust and [macroquad](https://github.com/not-fl3/macroquad), set in an infinite, procedurally generated world in the spirit of Terraria and Minecraft.
There are no asset files: every pixel is simulated or drawn in code, the font is hand-made in code, and every sound is synthesised at startup.

```
cargo run --release
```

## The world

- **Infinite** in every direction, generated chunk by chunk as you explore.
- **Surface biomes:** forest (grass, oaks, flowers), desert (dunes, cacti, quicksand) and tundra (snow, pines, ice, mountains).
- **Going down:** caves that widen with depth, gravel that caves in, pockets of flammable gas, ruins holding chests and wand altars, and finally basalt caverns with lava lakes.
- **Going up:** cloud platforms and grassy sky isles. Higher still, the sky fades into starry space with a ringed planet, and floating astral islands of moon rock, glowing crystals and glowshrooms.
- **Weather:** a day/night cycle, rain that puts out fires and fills pools, and lightning storms that set trees alight.

## Creatures

22 creatures, each drawn in code with its own behaviour, spawning by biome, depth, altitude and time of day:
zombies, skeletons (throw bones), slimes, demon eyes that watch you, bats, fire imps, lions and tigers (they pounce), scorpions, snow wolves, yetis (throw snowballs), stone golems, lava salamanders, the burrowing Devourer worm, dragons that breathe fire, ghosts that pass through walls, star jellies, moon hoppers, void eyes, wisps, cloud sprites and mushlings.
They burst into blood, goo, bones or rubble, and drop gold.

## Spells

Every spell has its own numbered hotbar slot (1–9, 0, -, =) or use the mouse wheel:
Spark Bolt, Fireball, Bomb, Lightning, Frost, Acid Ball, Torrent, Seeds of Life, Teleport, Digging Beam, Oil Spray and **Black Hole** (warps space around it, tears up everything nearby including solid rock, creatures and you, then collapses and spits it all back out).

- **Pets:** cast Seeds of Life straight at a creature to tame it (up to three). Pets follow you, fight nearby creatures and wear a heart.
- **Ragdolls:** creatures you kill go flying and splatter where they land.
- **Volcanoes and geysers:** volcanoes erupt lava every so often, and geysers blast water into the air.
- **Chests and altars:** chests in ruins and on sky islands hold gold. Altars restore you and set your respawn point.

## Chat and commands

Press **T** to chat or **/** to type a command (Enter sends, Esc cancels, ↑ repeats the last one):

| Command | What it does |
|---|---|
| `/tp volcano` `geyser` `ruin` `altar` `island` `space` `caves` `lava` `forest` `desert` `tundra` `spawn` | Teleport to the nearest one |
| `/tp <x> <y>` | Teleport to coordinates |
| `/time set day\|noon\|night\|midnight\|<0-1>` | Change the time of day |
| `/weather clear\|rain\|storm` | Change the weather |
| `/summon <creature> [count]` | Spawn creatures (e.g. `/summon dragon`) |
| `/kill`, `/kill pets` | Remove creatures |
| `/fly`, `/god` | Infinite flight, can't be hurt |
| `/heal`, `/gold <n>`, `/pos`, `/seed`, `/clear`, `/help` | Utilities |

## Getting to space

Hold jump to fly. Flight refills on the ground and slowly while gliding, so you can climb in bursts or rest on clouds and sky islands.
The quick way is **Teleport (9)** aimed straight up: a bolt that hits nothing teleports you to where it stops, a few hundred cells. Two or three casts reach space.

## Movement

Run, jump, wall-slide and wall-jump, fly while holding jump, dash (Shift), glide (S), and swing on a grappling vine (F).

This is a creative game: there's no fall damage and no hunger. You can only be hurt by fire, lava, acid, explosions and creatures, and respawning is instant.

## Controls

| Input | Action |
|---|---|
| A / D | Move |
| W / Space | Jump, wall-jump, hold to fly |
| S | Glide |
| Shift | Dash |
| F | Grappling vine (press again to let go) |
| Left mouse | Cast the selected spell |
| Right mouse | Dig |
| 1–9, 0, -, = / mouse wheel | Choose a spell |
| T / / | Chat / commands |
| E | Open chest / use altar |
| B | Sandbox brush (paint any material; wheel = size, 1–0 / Z X = material) |
| Esc | Pause, settings, controls |
| F11 | Fullscreen |

Settings (volumes, screen shake, FPS counter, fullscreen, lighting quality) are saved to `~/.falling_wizard.cfg`.

## Rendering

The CPU writes a colour image and a light image (emission + opacity) for the screen. On the GPU, a ray-marching pass casts rays from every cell through the light image, so sky light pours into cave mouths, lava and crystals light their surroundings, and terrain casts real shadows. A blur pass smooths it, and a composite pass adds the wizard's light, bloom, tone mapping and a vignette.

## Layout

`world.rs` (streaming + simulation), `worldgen.rs` (biomes, layers, ruins, sky), `materials.rs`, `cell.rs`, `player.rs`, `spells.rs` (spells and projectiles), `mobs.rs`, `weather.rs`, `fx.rs`, `render.rs` (lighting shaders), `ui.rs` (pixel font, HUD), `audio.rs`, `settings.rs`, `game.rs`, `main.rs`.

## Dev tools

- `cargo run --release -- --bench` runs a headless benchmark that flies across the world casting spells, plus material behaviour checks.
- `cargo run --release -- --autotest 22 [dir]` runs a scripted, **muted** tour (surface, spells, hotbar, caves, lava, sky isles, space, night storm, another biome) that prints FPS stats and saves screenshots.
- `--mute` starts the game without audio.
