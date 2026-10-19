# Slime Cave: Game Plan

A 2D side-scrolling platformer and metroidvania based on episode 1 of *That Time I Got Reincarnated as a Slime*. You play as a newly reborn slime who wakes up in the **Sealed Cave**, meets the Storm Dragon Veldora, collects skills by eating things, and escapes the cave.

> Note: this is a personal fan project. The names and characters belong to Fuse, Kodansha and 8bit. Use your own art, and rename things if you ever publish it.

---

## 1. Core idea

**"You become stronger by eating the world."**

The slime starts weak, blind and slow. Every object or enemy it **Predates** (absorbs) can give a new skill. Those skills unlock new movement and combat options, and those open new parts of the cave. This is the same loop as Rimuru's arc in episode 1.

## 2. Story beats (episode 1, adapted into levels)

| # | Level | Story beat | What the player learns |
|---|-------|------------|------------------------|
| 0 | Prologue (text only) | Satoru Mikami is stabbed and dies. A voice announces new skills. | Intro text, title screen |
| 1 | **Darkness** | The slime wakes up blind. It can only feel what it touches. | Movement (slow crawl, small hop). The screen is almost black, and touching things reveals them. |
| 2 | **Grass and Water** | It eats Hipokute Grass and falls into an underground lake. | **Predator** (absorb). **Water jet**: shoot water to dash and propel yourself. |
| 3 | **Veldora's Chamber** | It meets Veldora, who is trapped by the Infinite Prison. They talk and become friends. Veldora teaches **Magic Sense**. They name each other (Rimuru Tempest). Rimuru eats Veldora to analyze the seal. | Mostly dialogue. **Magic Sense** gives a large light radius, so the player can finally see. The player gets the name "Rimuru". |
| 4 | **Cave Depths** | Rimuru explores, eats Magic Ore, and fights cave monsters. | Combat with **Water Blade**, plus skills absorbed from enemies (see section 4) |
| 5 | **Serpent's Den** (mini boss) | Fight the Tempest Serpent. | Boss pattern, using the absorbed skills together |
| 6 | **The Sealed Door** | Adventurers (Kabal, Eren, Gido) open the door. Rimuru sneaks past them. | Stealth section: avoid their lantern light cones |
| End | Exit | Rimuru leaves the cave into the forest, which leads into episode 2 with the goblins. | Ending screen, stats (items eaten, time) |

## 3. Player (slime) mechanics

| Ability | How it's unlocked | Control | Effect |
|---|---|---|---|
| Crawl and hop | Start | ← → / Space | Slow, squishy movement |
| **Predator** | Level 2 | `E` (hold) | Absorbs a nearby item or a defeated enemy. Stores items and learns skills. |
| **Great Sage** | Start | automatic | Narrator and tutorial voice ("Notice. Skill acquired…"). Can also show hints. |
| **Water jet** | Level 2 | `Shift` + direction | Dash or air boost. Uses stored water. |
| **Magic Sense** | Level 3 | passive | Turns the darkness into a visible light radius |
| **Water Blade** | Level 4 | `J` | Ranged cutting attack. Uses water. |
| **Mimicry** | After the first monster | `1–4` | Turns into an absorbed monster to use its skills |

**Resources:** HP, **Water** (refilled at pools), and **Magicules** (from Magic Ore, used for skills).

## 4. Enemies and the skills they give

| Enemy | Behaviour | Skill gained by Predating it |
|---|---|---|
| Giant Bat | Swoops down from the ceiling | **Ultrasonic Waves**: a stun pulse. Mimicking it also allows a short glide. |
| Black Spider | Hangs on a thread and shoots webs | **Sticky Thread / Steel Thread**: a grapple hook that swings from ceilings |
| Evil Centipede | Patrols the floor and breathes poison | **Paralyzing Breath**: a short-range stun cone |
| Armor Saurus | Charges at you, front side protected by armor | **Body Armor**: damage reduction, can break cracked walls |
| Tempest Serpent (boss) | Coils around the arena, bites, sprays poison | **Poisonous Breath**, **Heat Sense** |

Each skill opens a gated area. For example: Steel Thread lets you cross gaps, Body Armor lets you smash walls, and Ultrasonic Waves can shatter crystal barriers.

## 5. World layout

One connected cave map split into rooms, metroidvania style:

```
[1 Darkness] → [2 Lake] → [3 Veldora's Chamber]
                  ↓
           [4a Bat Caverns] ↔ [4b Spider Nest] ↔ [4c Ore Mines]
                  ↓                                  ↓ (needs Body Armor)
           [5 Serpent's Den] ───────────────→ [6 Sealed Door] → EXIT
```

Optional secrets: extra Magic Ore, hidden Hipokute Grass (healing), and a lore stone about the Hero who sealed Veldora.

## 6. Art and audio style

- **Pixel art** with 16×16 tiles. The slime is 16×16 with a squash-and-stretch animation.
- Palette: dark cave blues and purples, glowing teal ore, and a light-blue slime.
- **Lighting is a key mechanic**: before Magic Sense, the player only sees a tiny radius. After it, the radius is much larger.
- Free assets to start with: [Kenney.nl](https://kenney.nl) and itch.io cave tilesets. Make the sprites in Aseprite or LibreSprite.
- Sound: soft cave ambience, a "blip" for Great Sage messages, and a deep rumble for Veldora's voice.

## 7. Tech plan (Rust)

**Engine: [macroquad](https://github.com/not-fl3/macroquad).** It is simple and fast to iterate with, and it can build to desktop and to the web (WASM).
(Alternative: **Bevy**, if you want an ECS and expect the project to grow a lot.)

Crates:
- `macroquad`: window, rendering, input, audio
- `macroquad-tiled` or a custom loader: maps built in the **Tiled** editor
- `serde` + `ron`: data files for enemies, skills and dialogue

Project layout:

```
rust_games/slime_cave/
├── Cargo.toml
├── assets/
│   ├── sprites/   (slime.png, bat.png, ...)
│   ├── tiles/     (cave_tileset.png)
│   ├── maps/      (level1.json ... from Tiled)
│   ├── audio/
│   └── data/      (skills.ron, enemies.ron, dialogue.ron)
└── src/
    ├── main.rs        game loop, state machine
    ├── state.rs       Title / Playing / Dialogue / Paused / GameOver
    ├── player.rs      slime movement, physics, squash animation
    ├── skills.rs      Predator, skill registry, mimicry
    ├── enemy.rs       enemy AI (bat, spider, centipede, saurus)
    ├── boss.rs        Tempest Serpent
    ├── level.rs       tilemap loading, collisions, room transitions
    ├── lighting.rs    darkness / Magic Sense mask
    ├── dialogue.rs    Great Sage + Veldora text boxes
    ├── camera.rs      smooth follow, screen shake
    └── ui.rs          HP / Water / Magicule bars, skill bar
```

## 8. Milestones

| Milestone | Goal | "Done" when… |
|---|---|---|
| **M1: Blob moves** | Window, slime sprite, gravity, hop, tile collision | The slime moves and jumps around a test room |
| **M2: Darkness** | Lighting mask, Great Sage text box | Level 1 is playable in the dark |
| **M3: Predator** | Absorbing items, inventory, water jet dash | Level 2 is complete |
| **M4: Veldora** | Dialogue system, naming scene, Magic Sense | Level 3 story scene plays |
| **M5: Combat** | Water Blade, enemies, HP, skill gain from enemies | Level 4 rooms are playable |
| **M6: Boss** | Tempest Serpent fight | The boss can be beaten |
| **M7: Escape** | Stealth section, ending screen | The full game can be played start to finish |
| **M8: Polish** | Sound, particles, save points, WASM build | Ready to share with friends |

## 9. Stretch ideas

- A Great Sage "analysis" screen that shows details about each eaten item or monster
- A Veldora chat menu after you eat him, with jokes and hints
- Speedrun timer and 100% completion (everything eaten)
- Episode 2 sequel: the goblin village
