//! Breadth-first search over the real slime physics. Used to prove that every
//! room can be finished with the skills the player has by then, and that the
//! skill gates actually gate.

use crate::level::{Level, Tile};
use crate::player::{Body, Input, Skills, PH, PW};
use std::collections::HashSet;

pub const MACRO_FRAMES: usize = 6;

pub struct Solution {
    /// One input per frame.
    pub inputs: Vec<Input>,
    pub explored: usize,
}

pub struct Failure {
    pub explored: usize,
    /// Furthest right point reached, for debugging level design.
    pub best: (f32, f32),
}

/// Level as the solver sees it: breakables the player can break are gone.
pub fn solver_level(lv: &Level, sk: &Skills) -> Level {
    let mut l = lv.clone();
    l.open_doors();
    if sk.ultrasonic {
        l.replace_all(Tile::Crystal, Tile::Empty);
    }
    if sk.body_armor {
        l.replace_all(Tile::Cracked, Tile::Empty);
    }
    l
}

fn actions(sk: &Skills) -> Vec<Input> {
    let mut out = Vec::new();
    for d in 0..3 {
        let base = Input { left: d == 1, right: d == 2, ..Default::default() };
        out.push(base);
        out.push(Input { jump: true, ..base });
        if sk.water_jet {
            out.push(Input { jet: true, ..base });
            out.push(Input { jet: true, up: true, ..base });
        }
        if sk.steel_thread {
            out.push(Input { thread: true, ..base });
            out.push(Input { thread: true, up: true, ..base });
            out.push(Input { up: true, ..base });
            out.push(Input { down: true, ..base });
        }
    }
    out
}

fn key(b: &Body, depth: u32, timed: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ((b.x / 3.0).round() as i32).hash(&mut h);
    ((b.y / 3.0).round() as i32).hash(&mut h);
    (b.vx.round() as i32).hash(&mut h);
    (b.vy.round() as i32).hash(&mut h);
    b.on_ground.hash(&mut h);
    b.air_jets.hash(&mut h);
    b.jet_t.hash(&mut h);
    b.jumping.hash(&mut h);
    ((b.water / 20.0) as i32).hash(&mut h);
    if let Some(r) = b.rope {
        ((r.ax / 4.0) as i32, (r.ay / 4.0) as i32, (r.len / 4.0) as i32).hash(&mut h);
    }
    if timed {
        (depth / 3).hash(&mut h);
    }
    h.finish()
}

pub fn at_exit(b: &Body, lv: &Level) -> bool {
    lv.rect_has(b.x, b.y, PW, PH, Tile::Exit)
}

pub fn exits(lv: &Level) -> Vec<(f32, f32)> {
    let mut v = Vec::new();
    for y in 0..lv.h {
        for x in 0..lv.w {
            if lv.get(x, y) == Tile::Exit {
                v.push((x as f32 * 16.0 + 8.0, y as f32 * 16.0 + 8.0));
            }
        }
    }
    v
}

/// Weighted A*: cost is macro steps, heuristic is distance to the nearest exit.
pub fn solve(lv: &Level, sk: &Skills, start: Body, max_states: usize) -> Result<Solution, Failure> {
    let lv = solver_level(lv, sk);
    let goal = Goal {
        targets: exits(&lv),
        reached: &|b: &Body, l: &Level| at_exit(b, l),
        hazard: None,
        timed: false,
    };
    search(&lv, sk, start, Input::default(), &goal, max_states)
}

pub type Hazard<'a> = &'a dyn Fn(usize, &Body) -> bool;

pub struct Goal<'a> {
    /// Points the heuristic pulls towards.
    pub targets: Vec<(f32, f32)>,
    pub reached: &'a dyn Fn(&Body, &Level) -> bool,
    /// Extra failure condition, given frames since the search started.
    pub hazard: Option<Hazard<'a>>,
    /// Include time in the state key, so waiting can be part of a plan.
    pub timed: bool,
}

/// General search over the slime physics. The level is used as given.
pub fn search(
    lv: &Level,
    sk: &Skills,
    start: Body,
    prev: Input,
    goal: &Goal,
    max_states: usize,
) -> Result<Solution, Failure> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut lv = lv.clone();
    let mut acts = actions(sk);
    if goal.timed {
        // Waiting in place matters when dodging moving lights.
        acts.push(Input::default());
    }
    let h = |b: &Body| -> u32 {
        let d = goal
            .targets
            .iter()
            .map(|g| ((g.0 - b.cx()).powi(2) + (g.1 - b.cy()).powi(2)).sqrt())
            .fold(f32::MAX, f32::min);
        // ~12px per macro step at walking speed; weight it to search greedily.
        (d / 12.0 * 3.0) as u32
    };
    if (goal.reached)(&start, &lv) {
        return Ok(Solution { inputs: Vec::new(), explored: 0 });
    }
    // node: body, last held input, parent, action index, depth
    let mut nodes: Vec<(Body, Input, usize, usize, u32)> = vec![(start.clone(), prev, usize::MAX, 0, 0)];
    let mut open = BinaryHeap::new();
    open.push(Reverse((h(&start), 0usize)));
    let mut seen = HashSet::new();
    seen.insert(key(&start, 0, goal.timed));
    let mut best = (start.x, start.y);
    while let Some(Reverse((_, idx))) = open.pop() {
        if nodes.len() >= max_states {
            break;
        }
        let (body, prev, _, _, depth) = nodes[idx].clone();
        for (ai, act) in acts.iter().enumerate() {
            let mut b = body.clone();
            let mut p = prev;
            let mut dead = false;
            let mut done = false;
            for fi in 0..MACRO_FRAMES {
                b.step(*act, p, &mut lv, sk);
                p = *act;
                let frame = depth as usize * MACRO_FRAMES + fi + 1;
                if lv.rect_spiked(b.x, b.y, PW, PH) || goal.hazard.is_some_and(|hz| hz(frame, &b)) {
                    dead = true;
                    break;
                }
                if (goal.reached)(&b, &lv) {
                    done = true;
                    break;
                }
            }
            if dead {
                continue;
            }
            if done {
                nodes.push((b, p, idx, ai, depth + 1));
                let mut inputs = Vec::new();
                let mut i = nodes.len() - 1;
                while nodes[i].2 != usize::MAX {
                    for _ in 0..MACRO_FRAMES {
                        inputs.push(acts[nodes[i].3]);
                    }
                    i = nodes[i].2;
                }
                inputs.reverse();
                return Ok(Solution { inputs, explored: nodes.len() });
            }
            if seen.insert(key(&b, depth + 1, goal.timed)) {
                if b.x > best.0 {
                    best = (b.x, b.y);
                }
                let f = depth + 1 + h(&b);
                nodes.push((b, p, idx, ai, depth + 1));
                open.push(Reverse((f, nodes.len() - 1)));
            }
        }
    }
    Err(Failure { explored: nodes.len(), best })
}
