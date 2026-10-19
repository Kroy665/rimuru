use slime_cave::bot::Bot;
use slime_cave::game::{Game, Scene};
use slime_cave::level::Level;
use slime_cave::player::Body;
use slime_cave::rooms::ROOMS;
use slime_cave::{skills_for_room, solver};

#[test]
fn every_room_parses() {
    for room in ROOMS {
        let lv = Level::parse(room.map);
        assert!(lv.w >= 30 && lv.h >= 17, "{} is smaller than the screen", room.name);
        assert!(!solver::exits(&lv).is_empty(), "{} has no exit", room.name);
    }
}

#[test]
fn every_room_is_beatable_with_its_skills() {
    for (i, room) in ROOMS.iter().enumerate() {
        let lv = Level::parse(room.map);
        let sk = skills_for_room(i, true);
        let r = solver::solve(&lv, &sk, Body::new(lv.spawn.0, lv.spawn.1), 500_000);
        assert!(r.is_ok(), "room {i} ({}) cannot be finished", room.name);
    }
}

/// The skill gained in a room must actually be needed to leave it.
/// Slow (exhaustive-ish search), so opt in with `--ignored`.
#[test]
#[ignore]
fn skill_gates_hold() {
    for i in [1, 3, 4, 5] {
        let lv = Level::parse(ROOMS[i].map);
        let sk = skills_for_room(i, false);
        let r = solver::solve(&lv, &sk, Body::new(lv.spawn.0, lv.spawn.1), 1_000_000);
        assert!(r.is_err(), "room {i} ({}) can be skipped without its skill", ROOMS[i].name);
    }
}

/// The autopilot plays from the title screen to the ending. Run with
/// `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn autopilot_finishes_the_game() {
    let mut g = Game::new();
    let mut bot = Bot::new();
    for _ in 0..60 * 60 * 20 {
        let inp = bot.next(&g);
        g.update(inp);
        g.sfx.clear();
        if g.scene == Scene::Ending {
            break;
        }
    }
    assert_eq!(g.scene, Scene::Ending, "stuck in room {} ({}); log: {:?}", g.room, ROOMS[g.room].name, bot.log);
    assert!(g.stats.deaths <= 3, "too many deaths: {}", g.stats.deaths);
    assert_eq!(g.stats.spotted, 0);
}
