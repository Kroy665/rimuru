//! An autopilot that plays the whole game through the same input interface as
//! a human: it plans movement with the solver, fights with simple reactive
//! rules, eats things to gain skills, and sneaks past the adventurers.

use crate::enemies::{lantern_sees, Enemy, Kind};
use crate::game::{Game, Scene};
use crate::level::{Level, Tile, TS};
use crate::player::{Body, Input};
use crate::solver::{at_exit, exits, search, Goal};
use std::collections::VecDeque;

pub struct Bot {
    plan: VecDeque<Input>,
    room: usize,
    last_hp: i32,
    toggle: bool,
    pub log: Vec<String>,
    pub failed_searches: u32,
    wait: u32,
}

impl Default for Bot {
    fn default() -> Self {
        Self::new()
    }
}

fn tiles_of(lv: &Level, t: Tile) -> Vec<(f32, f32)> {
    let mut v = Vec::new();
    for y in 0..lv.h {
        for x in 0..lv.w {
            if lv.get(x, y) == t {
                v.push((x as f32 * TS + 8.0, y as f32 * TS + 8.0));
            }
        }
    }
    v
}

impl Bot {
    pub fn new() -> Bot {
        Bot {
            plan: VecDeque::new(),
            room: usize::MAX,
            last_hp: 0,
            toggle: false,
            log: Vec::new(),
            failed_searches: 0,
            wait: 0,
        }
    }

    /// Press a button on alternate frames so every press is a fresh edge.
    fn tap(&mut self, inp: Input) -> Input {
        self.toggle = !self.toggle;
        if self.toggle {
            inp
        } else {
            Input::default()
        }
    }

    pub fn next(&mut self, g: &Game) -> Input {
        match g.scene {
            Scene::Title | Scene::Prologue => return self.tap(Input { jump: true, ..Default::default() }),
            Scene::Ending => return Input::default(),
            Scene::Play => {}
        }
        if g.fade_out > 0 || g.dead_t > 0 {
            self.plan.clear();
            return Input::default();
        }
        if !g.dialogue.is_empty() {
            self.plan.clear();
            return self.tap(Input { jump: true, ..Default::default() });
        }
        if g.hp < self.last_hp || g.room != self.room {
            self.plan.clear();
        }
        self.last_hp = g.hp;
        self.room = g.room;
        if let Some(i) = self.plan.pop_front() {
            return i;
        }
        if self.wait > 0 {
            self.wait -= 1;
            return Input::default();
        }
        self.decide(g)
    }

    fn go(
        &mut self,
        g: &Game,
        what: &str,
        targets: Vec<(f32, f32)>,
        reached: &dyn Fn(&Body, &Level) -> bool,
    ) -> Input {
        let goal = Goal { targets, reached, hazard: None, timed: false };
        self.run_search(g, what, &goal, 400_000)
    }

    fn run_search(&mut self, g: &Game, what: &str, goal: &Goal, budget: usize) -> Input {
        match search(&g.lv, &g.skills, g.body.clone(), g.prev, goal, budget) {
            Ok(s) => {
                self.plan = s.inputs.into();
                self.plan.pop_front().unwrap_or_default()
            }
            Err(f) => {
                self.failed_searches += 1;
                self.log.push(format!(
                    "room {} t={}: no path for '{}' from ({:.0},{:.0}) best ({:.0},{:.0})",
                    g.room, g.stats.frames, what, g.body.x, g.body.y, f.best.0, f.best.1
                ));
                self.wait = 30;
                Input::default()
            }
        }
    }

    /// Try a cheap search to the exit; on success the plan is loaded.
    fn quick_exit(&mut self, g: &Game) -> bool {
        let goal = Goal { targets: exits(&g.lv), reached: &|b, l| at_exit(b, l), hazard: None, timed: false };
        match search(&g.lv, &g.skills, g.body.clone(), g.prev, &goal, 60_000) {
            Ok(s) if !s.inputs.is_empty() => {
                self.plan = s.inputs.into();
                true
            }
            _ => false,
        }
    }

    fn go_exit(&mut self, g: &Game) -> Input {
        self.go(g, "exit", exits(&g.lv), &|b, l| at_exit(b, l))
    }

    fn go_near(&mut self, g: &Game, what: &str, x: f32, y: f32, r: f32) -> Input {
        self.go(g, what, vec![(x, y)], &move |b, _| (b.cx() - x).abs() < r && (b.cy() - y).abs() < r + 6.0 && (b.on_ground || b.in_water))
    }

    fn eat_corpse(&mut self, g: &Game, e: &Enemy) -> Input {
        let (cx, cy) = (g.body.cx(), g.body.cy());
        if (e.x - cx).abs() < 20.0 && (e.y - cy).abs() < 20.0 {
            return self.tap(Input { predate: true, ..Default::default() });
        }
        self.go_near(g, "corpse", e.x, e.y, 14.0)
    }

    fn refill(&mut self, g: &Game) -> Input {
        let water = tiles_of(&g.lv, Tile::Water);
        self.go(g, "refill water", water, &|b, _| b.in_water && b.water > 90.0)
    }

    fn decide(&mut self, g: &Game) -> Input {
        let sk = &g.skills;
        let corpse = |k: Kind| g.enemies.iter().find(|e| e.dead && e.kind == k).cloned();
        let alive = |k: Kind| {
            let (cx, cy) = (g.body.cx(), g.body.cy());
            g.enemies
                .iter()
                .filter(|e| e.alive() && e.kind == k)
                .min_by(|a, b| {
                    let da = (a.x - cx).hypot(a.y - cy);
                    let db = (b.x - cx).hypot(b.y - cy);
                    da.partial_cmp(&db).unwrap()
                })
                .cloned()
        };
        match g.room {
            1 if !sk.water_jet => {
                if g.body.in_water {
                    return self.tap(Input { predate: true, ..Default::default() });
                }
                let water = tiles_of(&g.lv, Tile::Water);
                self.go(g, "lake", water, &|b, _| b.in_water)
            }
            2 if g.veldora.is_some() => {
                let (vx, vy) = g.veldora.unwrap();
                if !g.flags.veldora_ready {
                    let spot = trigger_cells(&g.lv, 2);
                    return self.go(g, "talk to Veldora", spot, &|b, l| {
                        l.triggers_near(b.x, b.y, crate::player::PW, crate::player::PH).contains(&2)
                    });
                }
                let (cx, cy) = (g.body.cx(), g.body.cy());
                if (vx - cx).abs() < 80.0 && (vy - cy).abs() < 80.0 {
                    return self.tap(Input { predate: true, ..Default::default() });
                }
                self.go_near(g, "Veldora", vx - 70.0, vy - 6.0, 10.0)
            }
            3 if !sk.ultrasonic => match corpse(Kind::Bat) {
                Some(e) => self.eat_corpse(g, &e),
                None => match alive(Kind::Bat) {
                    Some(e) => self.fight(g, &e),
                    None => self.go_exit(g),
                },
            },
            3 if !tiles_of(&g.lv, Tile::Crystal).is_empty() && !self.quick_exit(g) => {
                let cr = tiles_of(&g.lv, Tile::Crystal);
                let (cx, cy) = (g.body.cx(), g.body.cy());
                let near = cr.iter().any(|c| (c.0 - cx).hypot(c.1 - cy) < 60.0);
                if near && g.body.on_ground {
                    return self.tap(Input { sonic: true, ..Default::default() });
                }
                self.go(g, "crystal", cr.clone(), &move |b, _| b.on_ground && cr.iter().any(|c| (c.0 - b.cx()).hypot(c.1 - b.cy()) < 55.0))
            }
            4 if !sk.steel_thread => match corpse(Kind::Spider) {
                Some(e) => self.eat_corpse(g, &e),
                None => match alive(Kind::Spider) {
                    Some(e) => self.fight(g, &e),
                    None => self.go_exit(g),
                },
            },
            5 if !sk.body_armor => match corpse(Kind::Saurus) {
                Some(e) => self.eat_corpse(g, &e),
                None => {
                    // Clear centipedes that are in the way first.
                    if let Some(c) = alive(Kind::Centipede)
                        && (c.x - g.body.cx()).abs() < 150.0 && (c.y - g.body.cy()).abs() < 30.0 {
                            return self.fight(g, &c);
                        }
                    match alive(Kind::Saurus) {
                        Some(e) => self.fight(g, &e),
                        None => self.go_exit(g),
                    }
                }
            },
            5 if !tiles_of(&g.lv, Tile::Cracked).is_empty() && !self.quick_exit(g) => {
                let cr = tiles_of(&g.lv, Tile::Cracked);
                let (cx, cy) = (g.body.cx(), g.body.cy());
                if let Some(c) = cr.iter().find(|c| (c.0 - cx).abs() < 30.0 && (c.1 - cy).abs() < 10.0)
                    && g.body.on_ground && g.body.water >= 20.0 {
                        let right = c.0 > cx;
                        return self.tap(Input { jet: true, right, left: !right, ..Default::default() });
                    }
                if g.body.water < 20.0 {
                    return self.refill(g);
                }
                // Stand at floor level next to the wall.
                let low: Vec<(f32, f32)> = cr.iter().copied().filter(|c| !g.lv.solid_at(c.0 - 16.0, c.1) && g.lv.solid_at(c.0 - 16.0, c.1 + 16.0)).collect();
                self.go(g, "cracked wall", low.clone(), &move |b, _| {
                    b.on_ground && low.iter().any(|c| (c.0 - b.cx()).abs() < 28.0 && (c.1 - b.cy()).abs() < 8.0)
                })
            }
            6 if !g.flags.boss_eaten => match corpse(Kind::Serpent) {
                Some(e) => self.eat_corpse(g, &e),
                None => match alive(Kind::Serpent) {
                    Some(e) if e.awake => self.fight_boss(g, &e),
                    _ => {
                        let spot = trigger_cells(&g.lv, 1);
                        self.go(g, "boss trigger", spot, &|b, l| {
                            l.triggers_near(b.x, b.y, crate::player::PW, crate::player::PH).contains(&1)
                        })
                    }
                },
            },
            7 => self.sneak(g),
            _ => match self.plan.pop_front() {
                Some(i) => i,
                None => self.go_exit(g),
            },
        }
    }

    /// Generic ranged fight: line up and fire Water Blades.
    fn fight(&mut self, g: &Game, e: &Enemy) -> Input {
        let b = &g.body;
        let (cx, cy) = (b.cx(), b.cy());
        let (dx, dy) = (e.x - cx, e.y - cy);
        if b.water < 4.0 {
            return self.refill(g);
        }
        let mut inp = Input::default();
        let can_fire = g.blade_cd == 0 && !g.prev.blade;
        // Armor Saurus: stun first so the front is open.
        if e.kind == Kind::Saurus && e.stun == 0 && dx.abs() < 70.0 && dy.abs() < 30.0 && g.sonic_cd == 0 && !g.prev.sonic {
            return Input { sonic: true, ..Default::default() };
        }
        if e.kind == Kind::Saurus && e.stun == 0 && e.state == 2 && dx.abs() < 90.0 {
            // Charging at us: hop over it.
            return Input { jump: true, left: dx > 0.0, right: dx < 0.0, ..Default::default() };
        }
        if dx.abs() < 7.0 && dy < -10.0 && dy > -190.0 {
            if can_fire {
                inp.blade = true;
                inp.up = true;
            }
            return inp;
        }
        let facing_ok = dx * b.facing > 0.0;
        if dy.abs() < 10.0 && dx.abs() < 170.0 && g.lv.clear_line(cx, cy, e.x, e.y) {
            if !facing_ok {
                inp.right = dx > 0.0;
                inp.left = dx < 0.0;
            } else if can_fire {
                inp.blade = true;
            }
            if dx.abs() < 26.0 {
                // Too close: back off.
                inp.right = dx < 0.0;
                inp.left = dx > 0.0;
            }
            return inp;
        }
        match e.kind {
            Kind::Bat if e.state != 0 => {
                // It is coming to us; just face it.
                inp.right = dx > 0.0 && !facing_ok;
                inp.left = dx < 0.0 && !facing_ok;
                inp
            }
            Kind::Bat | Kind::Spider => {
                // Stand underneath and shoot upward.
                let below_y = e.home.1 + 160.0;
                self.go(g, "under target", vec![(e.x, below_y)], &{
                    let ex = e.x;
                    move |b: &Body, _: &Level| b.on_ground && (b.cx() - ex).abs() < 5.0
                })
            }
            _ => {
                let ey = e.y;
                let wants: Vec<(f32, f32)> = [-110.0, -70.0, 70.0, 110.0].iter().map(|o| (e.x + o, ey)).collect();
                let ws = wants.clone();
                self.go(g, "line up", wants, &move |b: &Body, _: &Level| {
                    b.on_ground && (b.cy() - ey).abs() < 9.0 && ws.iter().any(|w| (b.cx() - w.0).abs() < 22.0)
                })
            }
        }
    }

    fn fight_boss(&mut self, g: &Game, e: &Enemy) -> Input {
        let b = &g.body;
        let (cx, cy) = (b.cx(), b.cy());
        let (dx, dy) = (e.x - cx, e.y - cy);
        let dist = dx.hypot(dy);
        let mut inp = Input::default();
        // Body segments sliding into us: hop.
        let seg_close = e.segments().any(|(sx, sy)| (sx - cx).abs() < 30.0 && (sy - cy).abs() < 20.0);
        if (dist < 70.0 || (e.state == 1 && dist < 140.0)) && b.water >= 20.0 && g.body.jet_t == 0 && !g.prev.jet {
            // Jet up and away.
            inp.jet = true;
            inp.up = true;
            inp.right = dx < 0.0;
            inp.left = dx > 0.0;
            return inp;
        }
        if seg_close && b.on_ground {
            inp.jump = true;
            return inp;
        }
        if b.water < 8.0 {
            return self.refill(g);
        }
        if dist < 80.0 && g.sonic_cd == 0 && !g.prev.sonic {
            inp.sonic = true;
            return inp;
        }
        let facing_ok = dx * b.facing > 0.0;
        if !facing_ok {
            inp.right = dx > 0.0;
            inp.left = dx < 0.0;
            return inp;
        }
        if dy.abs() < 16.0 && g.blade_cd == 0 && !g.prev.blade {
            inp.blade = true;
            return inp;
        }
        // Keep some distance.
        if dist < 110.0 {
            inp.right = dx < 0.0;
            inp.left = dx > 0.0;
            if !g.lv.solid_at(cx + if dx < 0.0 { 20.0 } else { -20.0 }, cy) {
                return inp;
            }
            inp.jump = b.on_ground;
        }
        inp
    }

    /// Plan to the exit while never entering a lantern cone, simulating the
    /// adventurers' patrols forward in time.
    fn sneak(&mut self, g: &Game) -> Input {
        let mut sims: Vec<Enemy> = g.enemies.iter().filter(|e| matches!(e.kind, Kind::Adventurer(_))).cloned().collect();
        let horizon = 4000;
        let mut table: Vec<Vec<(f32, f32, f32)>> = Vec::with_capacity(horizon);
        for _ in 0..horizon {
            for s in &mut sims {
                s.update(&g.lv, -9999.0, -9999.0);
            }
            table.push(sims.iter().map(|s| {
                let (lx, ly) = s.lantern();
                (lx, ly, s.facing)
            }).collect());
        }
        let lv = g.lv.clone();
        let hazard = move |f: usize, b: &Body| {
            let row = &table[(f - 1).min(horizon - 1)];
            row.iter().any(|&(lx, ly, fc)| {
                [(0.0, 0.0), (4.0, 0.0), (-4.0, 0.0)]
                    .iter()
                    .any(|(ox, oy)| lantern_sees(&lv, lx, ly, fc, b.cx() + ox, b.cy() + oy))
            })
        };
        // Hop between high perches (ledge tops well above lantern height), then to the exit.
        let perches = perches(&g.lv, 150.0);
        let cx = g.body.cx();
        let next = perches.iter().find(|p| p.0 > cx + 24.0).copied();
        let exit_goal = Goal { targets: exits(&g.lv), reached: &|b, l| at_exit(b, l), hazard: Some(&hazard), timed: true };
        let t = std::time::Instant::now();
        let mut r = search(&g.lv, &g.skills, g.body.clone(), g.prev, &exit_goal, 40_000);
        if r.is_err()
            && let Some((px, py, half)) = next {
                let goal = Goal {
                    targets: vec![(px, py)],
                    reached: &move |b: &Body, _: &Level| b.on_ground && (b.cx() - px).abs() < half && (b.cy() - py).abs() < 8.0,
                    hazard: Some(&hazard),
                    timed: true,
                };
                r = search(&g.lv, &g.skills, g.body.clone(), g.prev, &goal, 150_000);
            }
        if std::env::var("BOT_DEBUG").is_ok() {
            match &r {
                Ok(s) => eprintln!("sneak to {next:?}: ok, {} frames, in {:?}", s.inputs.len(), t.elapsed()),
                Err(f) => eprintln!("sneak to {next:?}: FAIL best {:?} explored {} in {:?}", f.best, f.explored, t.elapsed()),
            }
        }
        match r {
            Ok(s) => {
                self.plan = s.inputs.into();
                self.plan.pop_front().unwrap_or_default()
            }
            Err(_) => {
                // Not safe right now: wait on this perch for the patrol to move on.
                self.wait = 20;
                Input::default()
            }
        }
    }
}

/// Standing spots on ledge tops at or above `max_y`: (centre x, body centre y, half width).
fn perches(lv: &Level, max_y: f32) -> Vec<(f32, f32, f32)> {
    let mut v = Vec::new();
    for y in 1..lv.h {
        let mut x = 0;
        while x < lv.w {
            let top = |x: i32| lv.get(x, y).solid() && !lv.get(x, y - 1).solid() && !lv.get(x, y - 2).solid() && lv.get(x, y - 1) != Tile::Exit;
            if top(x) {
                let s = x;
                while x < lv.w && top(x) {
                    x += 1;
                }
                let (x0, x1) = (s as f32 * TS, x as f32 * TS);
                let cy = y as f32 * TS - 5.0;
                if cy <= max_y && cy > 96.0 {
                    v.push(((x0 + x1) / 2.0, cy, (x1 - x0) / 2.0 - 6.0));
                }
            } else {
                x += 1;
            }
        }
    }
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    v
}

fn trigger_cells(lv: &Level, id: u8) -> Vec<(f32, f32)> {
    let mut v = Vec::new();
    for y in 0..lv.h {
        for x in 0..lv.w {
            if lv.trigger(x, y) == id {
                v.push((x as f32 * TS + 8.0, y as f32 * TS + 8.0));
            }
        }
    }
    v
}
