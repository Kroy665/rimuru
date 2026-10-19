//! Developer tools: room solver, screenshots, map previews.
use slime_cave::level::Level;
use slime_cave::player::Body;
use slime_cave::rooms::ROOMS;
use slime_cave::game::{Game, Scene};
use slime_cave::gfx::Frame;
use slime_cave::player::Input;
use slime_cave::{skills_for_room, solver};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("solve") => {
            let only: Option<usize> = args.get(2).and_then(|s| s.parse().ok());
            for (i, room) in ROOMS.iter().enumerate() {
                if only.is_some_and(|o| o != i) {
                    continue;
                }
                let lv = Level::parse(room.map);
                for leaving in [false, true] {
                    let sk = skills_for_room(i, leaving);
                    let start = Body::new(lv.spawn.0, lv.spawn.1);
                    let t = std::time::Instant::now();
                    match solver::solve(&lv, &sk, start, 3_000_000) {
                        Ok(s) => println!(
                            "room {i} {:<18} leaving={leaving:<5} SOLVED in {} frames ({} states, {:?})",
                            room.name, s.inputs.len(), s.explored, t.elapsed()
                        ),
                        Err(f) => println!(
                            "room {i} {:<18} leaving={leaving:<5} unsolved ({} states, best x={:.0} y={:.0} tile {},{}, {:?})",
                            room.name, f.explored, f.best.0, f.best.1, (f.best.0/16.0) as i32, (f.best.1/16.0) as i32, t.elapsed()
                        ),
                    }
                }
            }
        }
        Some("maps") => {
            for (i, room) in ROOMS.iter().enumerate() {
                let rows: Vec<&str> = room.map.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
                let widths: Vec<usize> = rows.iter().map(|r| r.len()).collect();
                println!("room {i} {}: {}x{} widths {:?}", room.name, widths.iter().max().unwrap(), rows.len(),
                    if widths.iter().all(|w| *w == widths[0]) { "uniform".to_string() } else { format!("{widths:?}") });
            }
        }
        Some("shots") => shots(args.get(2).map(|s| s.as_str()).unwrap_or("shots")),
        Some("play") => {
            let room: usize = args[2].parse().unwrap();
            let out = args.get(3).cloned();
            let every: u64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(4);
            play(room, out, every);
        }
        Some("bot") => {
            let out = args.get(2).cloned();
            let every: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(3);
            bot(out, every);
        }
        _ => eprintln!("usage: dev solve [room] | maps | shots [dir] | play <room> [dir] [every]"),
    }
}

fn save(g: &Game, path: &str) {
    let mut f = Frame::new();
    g.render(&mut f);
    f.save_png(path).unwrap();
}

fn run(g: &mut Game, frames: usize, inp: Input) {
    for _ in 0..frames {
        g.update(inp);
        g.sfx.clear();
    }
}

fn shots(dir: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let mut g = Game::new();
    run(&mut g, 40, Input::default());
    save(&g, &format!("{dir}/title.png"));
    let tap = Input { jump: true, ..Default::default() };
    run(&mut g, 1, tap);
    run(&mut g, 60, Input::default());
    save(&g, &format!("{dir}/prologue.png"));
    for i in 0..ROOMS.len() {
        let mut g = Game::start_at(i);
        g.auto_dialogue = true;
        run(&mut g, 90, Input::default());
        save(&g, &format!("{dir}/room{i}.png"));
    }
    // Veldora, lit, mid-conversation.
    let mut g = Game::start_at(2);
    g.skills.magic_sense = true;
    g.body.x = 300.0;
    run(&mut g, 200, Input::default());
    g.dialogue.push_back(slime_cave::game::Beat::Say(slime_cave::rooms::Who::Veldora, "KWAHAHAHA! A slime wandering this deep? I am Veldora, the Storm Dragon!".into()));
    run(&mut g, 100, Input::default());
    save(&g, &format!("{dir}/veldora.png"));
    // Pause screen.
    let mut g = Game::start_at(4);
    g.auto_dialogue = true;
    run(&mut g, 30, Input::default());
    g.paused = true;
    save(&g, &format!("{dir}/pause.png"));
    // A dialogue box.
    let mut g = Game::start_at(0);
    run(&mut g, 120, Input::default());
    save(&g, &format!("{dir}/dialogue.png"));
    let mut g = Game::start_at(7);
    g.scene = Scene::Ending;
    run(&mut g, 200, Input::default());
    save(&g, &format!("{dir}/ending.png"));
    println!("saved to {dir}/");
}

/// Let the solver drive the real game in one room (enemies included).
fn play(room: usize, out: Option<String>, every: u64) {
    let mut g = Game::start_at(room);
    g.auto_dialogue = true;
    run(&mut g, 2, Input::default());
    let sk = skills_for_room(room, true);
    g.skills = sk.clone();
    let sol = match solver::solve(&g.lv, &sk, g.body.clone(), 3_000_000) {
        Ok(s) => s,
        Err(f) => {
            println!("no solution (best {:?})", f.best);
            return;
        }
    };
    if let Some(d) = &out {
        std::fs::create_dir_all(d).unwrap();
    }
    let mut n = 0;
    let mut i = 0usize;
    let mut k = 0usize;
    while k < sol.inputs.len() && i < 20_000 {
        i += 1;
        // Only consume solver inputs on frames where physics will run.
        let live = g.dialogue.is_empty() && g.dead_t == 0;
        let inp = if live { k += 1; sol.inputs[k - 1] } else { Input::default() };
        g.update(inp);
        g.sfx.clear();
        if let Some(d) = &out
            && (i as u64).is_multiple_of(every) {
                save(&g, &format!("{d}/f{n:05}.png"));
                n += 1;
            }
        if g.room != room || g.fade_out > 0 {
            break;
        }
    }
    run(&mut g, 40, Input::default());
    println!(
        "room {room}: {} frames, now in room {} (scene {:?}), hp {}/{}, deaths {}, spotted {}",
        sol.inputs.len(), g.room, g.scene, g.hp, g.max_hp, g.stats.deaths, g.stats.spotted
    );
}

/// Play the whole game with the autopilot, from the title screen to the ending.
fn bot(out: Option<String>, every: u64) {
    use slime_cave::bot::Bot;
    let mut g = match std::env::var("BOT_ROOM").ok().and_then(|s| s.parse().ok()) {
        Some(r) => Game::start_at(r),
        None => Game::new(),
    };
    let mut bot = Bot::new();
    if let Some(d) = &out {
        std::fs::create_dir_all(d).unwrap();
    }
    let mut room = usize::MAX;
    let mut room_start = 0u64;
    let mut n = 0;
    let t0 = std::time::Instant::now();
    for frame in 0..60 * 60 * 30u64 {
        if std::env::var("BOT_TRACE").is_ok() && frame % 60 == 0 {
            eprintln!("f{frame} scene {:?} room {} pos ({:.0},{:.0}) dlg {} hp {}", g.scene, g.room, g.body.x, g.body.y, g.dialogue.len(), g.hp);
        }
        let logged = bot.log.len();
        let inp = bot.next(&g);
        for l in &bot.log[logged..] {
            println!("  log: {l}");
        }
        g.update(inp);
        g.sfx.clear();
        if g.scene == Scene::Play && g.room != room {
            if room != usize::MAX {
                println!("  cleared room {room} {:<18} in {:>5} frames, deaths so far {}", ROOMS[room].name, frame - room_start, g.stats.deaths);
            }
            room = g.room;
            room_start = frame;
        }
        if let Some(d) = &out
            && frame % every == 0 {
                save(&g, &format!("{d}/f{n:06}.png"));
                n += 1;
            }
        if g.scene == Scene::Ending {
            println!("  cleared room {room} {:<18} in {:>5} frames", ROOMS[room].name, frame - room_start);
            run(&mut g, 200, Input::default());
            if let Some(d) = &out {
                for _ in 0..40 {
                    save(&g, &format!("{d}/f{n:06}.png"));
                    n += 1;
                }
            }
            break;
        }
        if frame - room_start > 60 * 60 * 4 {
            println!("  STUCK in room {room} for 4 minutes of game time");
            break;
        }
    }
    let s = &g.stats;
    println!(
        "RESULT scene={:?} room={} time={}s deaths={} spotted={} ore={} eaten={} failed_searches={} (real {:?})",
        g.scene, g.room, s.frames / 60, s.deaths, s.spotted, s.ore, s.eaten, bot.failed_searches, t0.elapsed()
    );
}
