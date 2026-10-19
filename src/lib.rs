pub mod bot;
pub mod enemies;
pub mod game;
pub mod gfx;
pub mod level;
pub mod player;
pub mod render;
pub mod rooms;
pub mod solver;
pub mod sound;

use player::Skills;

/// Skills the player is guaranteed to have when entering room `i`, and the
/// skills they will have gained by the time they leave it.
pub fn skills_for_room(i: usize, leaving: bool) -> Skills {
    let stage = if leaving { i + 1 } else { i };
    Skills {
        predator: true,
        water_jet: stage >= 2,
        magic_sense: stage >= 3,
        water_blade: stage >= 3,
        ultrasonic: stage >= 4,
        steel_thread: stage >= 5,
        body_armor: stage >= 6,
        poison_resist: stage >= 6,
        heat_sense: stage >= 7,
    }
}
