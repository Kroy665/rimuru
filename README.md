# Slime Cave

A 2D pixel-art platformer based on episode 1 of *That Time I Got Reincarnated as a Slime*. You wake up as a blind slime in the Sealed Cave, eat everything to gain skills, befriend (and swallow) the Storm Dragon Veldora, beat the Tempest Serpent, and sneak past the adventurers at the sealed door.

Fan project. No art or audio assets: every pixel and sound is generated in code.

## See it first

`media/autopilot_playthrough.mp4` is a recording of the autopilot finishing the whole game. `media/screens/` has a screenshot of every scene.

## Run

```bash
cargo run --release
```

F11 switches to fullscreen. The window can be resized and the image scales to fit.

## Controls

| Action | Keys |
|---|---|
| Move | A / D or ← → |
| Hop / swim / let go of thread | Space, W, ↑ or Z |
| Predate (eat) | E or Q |
| Water Jet (needs stored water) | Shift or X, aimed with the direction keys |
| Water Blade | J or C (hold ↑ to fire upward) |
| Ultrasonic Waves | K or V |
| Steel Thread | L or B (hold ↑ to aim straight up). On the thread: ↑/↓ to climb, ←/→ to swing |
| Advance dialogue | Space, Enter or E |

## Rooms

1. **The Darkness**: no sight. Things you touch stay visible. Eat the Hipokute grass.
2. **Underground Lake**: eat the water to unlock **Water Jet** and get out of the lake.
3. **Veldora's Chamber**: Veldora gives you **Magic Sense** and a name, then asks you to eat him. That unlocks **Water Blade**.
4. **Bat Caverns**: eat a bat to get **Ultrasonic Waves**, which shatters the crystal barrier.
5. **Spider Nest**: eat a spider to get **Steel Thread**, then swing over the spike pit.
6. **Ore Mines**: stun the Armor Saurus (its front is armoured), eat it to get **Body Armor**, then jet through the cracked wall.
7. **Serpent's Den**: boss fight. Its head is the weak point. Watch for it rearing up before it lunges.
8. **The Sealed Door**: stealth. Stay out of the adventurers' lantern light.

Every 3 magic ore you eat adds one point of maximum vitality.

## Developer tools

```bash
cargo run --release --bin dev solve        # prove every room is beatable, and that each skill gate holds
cargo run --release --bin dev bot [dir]    # autopilot plays the whole game, title to ending (optionally dumps frames)
cargo run --release --bin dev shots [dir]  # screenshots of every scene
cargo run --release --bin dev play <room>  # solver drives one room inside the real game
cargo test --release                       # quick checks: rooms parse and are beatable
cargo test --release -- --ignored          # slow: skill gates hold + full autopilot playthrough
```

The game renders into a 480×270 software framebuffer (`src/gfx.rs`). The window only uploads it as a texture, so the tools can run headless and save PNGs.

`src/solver.rs` is an A* search over the real slime physics (`src/player.rs`). It is used for the room checks, and the autopilot (`src/bot.rs`) uses it to plan movement. The autopilot also simulates the adventurers' patrols forward in time, so it can plan a route that is never lit by a lantern.

## Layout

```
src/
  main.rs      window, input, sound playback (macroquad)
  game.rs      game state, story, combat, predation
  render.rs    tiles, sprites, lighting, HUD, dialogue
  player.rs    slime physics (shared with the solver)
  enemies.rs   bat, spider, centipede, armor saurus, serpent boss, adventurers
  rooms.rs     ASCII room maps and story text
  level.rs     tile map + collision
  gfx.rs       software framebuffer, font, PNG output
  sound.rs     synthesised sound effects
  solver.rs    A* over the physics
  bot.rs       autopilot
  bin/dev.rs   developer tools
```
