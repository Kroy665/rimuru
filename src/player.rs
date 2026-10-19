//! Slime body physics. Deterministic and render-free so the solver can reuse it.

use crate::level::{Level, Tile, TS};

pub const PW: f32 = 12.0;
pub const PH: f32 = 10.0;
pub const WATER_MAX: f32 = 100.0;
pub const JET_COST: f32 = 20.0;
pub const JET_FRAMES: i32 = 10;
pub const JET_SPEED: f32 = 6.0;
pub const GRAVITY: f32 = 0.32;
pub const JUMP_V: f32 = 5.0;
pub const ROPE_MAX: f32 = 150.0;

/// Buttons held this frame. Edges are derived by comparing with the previous frame.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub struct Input {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub jump: bool,
    pub jet: bool,
    pub blade: bool,
    pub sonic: bool,
    pub thread: bool,
    pub predate: bool,
    pub start: bool,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Skills {
    pub predator: bool,
    pub water_jet: bool,
    pub magic_sense: bool,
    pub water_blade: bool,
    pub ultrasonic: bool,
    pub steel_thread: bool,
    pub body_armor: bool,
    pub poison_resist: bool,
    pub heat_sense: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rope {
    pub ax: f32,
    pub ay: f32,
    pub len: f32,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub facing: f32,
    pub on_ground: bool,
    pub in_water: bool,
    pub jet_t: i32,
    pub jet_dx: f32,
    pub jet_dy: f32,
    pub air_jets: i32,
    pub rope: Option<Rope>,
    pub coyote: i32,
    pub jump_buf: i32,
    pub jumping: bool,
    pub water: f32,
    /// Frames of web slowdown left.
    pub slowed: i32,
}

#[derive(Default, Debug)]
pub struct Events {
    pub jumped: bool,
    pub landed: Option<f32>,
    pub jetted: bool,
    pub broke: Vec<(i32, i32)>,
    pub rope_attached: bool,
    pub rope_released: bool,
    pub splashed: bool,
}

impl Body {
    /// Create a body standing with its feet at (cx, bottom).
    pub fn new(cx: f32, bottom: f32) -> Body {
        Body {
            x: cx - PW / 2.0,
            y: bottom - PH,
            vx: 0.0,
            vy: 0.0,
            facing: 1.0,
            on_ground: false,
            in_water: false,
            jet_t: 0,
            jet_dx: 0.0,
            jet_dy: 0.0,
            air_jets: 1,
            rope: None,
            coyote: 0,
            jump_buf: 0,
            jumping: false,
            water: 0.0,
            slowed: 0,
        }
    }

    pub fn cx(&self) -> f32 {
        self.x + PW / 2.0
    }
    pub fn cy(&self) -> f32 {
        self.y + PH / 2.0
    }

    pub fn step(&mut self, inp: Input, prev: Input, lv: &mut Level, sk: &Skills) -> Events {
        let mut ev = Events::default();
        let jump_pressed = inp.jump && !prev.jump;
        let was_water = self.in_water;
        self.in_water = lv.rect_has(self.x, self.y + PH * 0.4, PW, PH * 0.6, Tile::Water);
        if self.in_water && !was_water {
            ev.splashed = true;
        }
        if self.in_water {
            if sk.water_jet {
                self.water = (self.water + 2.0).min(WATER_MAX);
            }
            self.air_jets = 1;
        } else if sk.water_jet {
            self.water = (self.water + 0.05).min(WATER_MAX);
        }
        self.slowed = (self.slowed - 1).max(0);

        let dir = (inp.right as i32 - inp.left as i32) as f32;
        if dir != 0.0 && self.jet_t == 0 {
            self.facing = dir;
        }
        if jump_pressed {
            self.jump_buf = 6;
        } else {
            self.jump_buf = (self.jump_buf - 1).max(0);
        }
        if self.on_ground {
            self.coyote = 6;
            self.air_jets = 1;
        } else {
            self.coyote = (self.coyote - 1).max(0);
        }

        // Steel Thread: fire or release.
        if sk.steel_thread && inp.thread && !prev.thread {
            if self.rope.is_some() {
                self.rope = None;
                ev.rope_released = true;
            } else {
                let (dx, dy) = if inp.up { (0.0, -1.0) } else { (self.facing * 0.6, -1.0) };
                if let Some((hx, hy)) = lv.raycast(self.cx(), self.cy(), dx, dy, ROPE_MAX) {
                    let len = ((hx - self.cx()).powi(2) + (hy - self.cy()).powi(2)).sqrt();
                    self.rope = Some(Rope { ax: hx, ay: hy, len: len.max(16.0) });
                    self.jet_t = 0;
                    self.air_jets = 1;
                    self.jumping = false;
                    ev.rope_attached = true;
                }
            }
        }
        if let Some(r) = self.rope
            && !lv.solid_at(r.ax, r.ay) {
                self.rope = None;
            }

        // Water jet.
        let can_jet = self.on_ground || self.in_water || self.rope.is_some() || self.air_jets > 0;
        if sk.water_jet && inp.jet && !prev.jet && self.jet_t == 0 && self.water >= JET_COST && can_jet
        {
            let mut dx = dir;
            let dy = (inp.down as i32 - inp.up as i32) as f32;
            if dx == 0.0 && dy == 0.0 {
                dx = self.facing;
            }
            let l = (dx * dx + dy * dy).sqrt();
            self.jet_dx = dx / l;
            self.jet_dy = dy / l;
            self.jet_t = JET_FRAMES;
            self.water -= JET_COST;
            if !(self.on_ground || self.in_water || self.rope.is_some()) {
                self.air_jets -= 1;
            }
            self.rope = None;
            self.jumping = false;
            if dx != 0.0 {
                self.facing = dx.signum();
            }
            ev.jetted = true;
        }

        let jetting = self.jet_t > 0;
        if jetting {
            self.vx = self.jet_dx * JET_SPEED;
            self.vy = self.jet_dy * JET_SPEED;
            self.jet_t -= 1;
            if self.jet_t == 0 {
                self.vx *= 0.5;
                self.vy *= 0.5;
            }
        } else if let Some(mut r) = self.rope {
            self.vy += GRAVITY;
            self.vx += dir * 0.14;
            self.vx *= 0.995;
            if inp.up {
                r.len = (r.len - 1.5).max(16.0);
            }
            if inp.down {
                r.len = (r.len + 1.5).min(ROPE_MAX + 20.0);
            }
            self.rope = Some(r);
            if jump_pressed {
                self.rope = None;
                self.vy = self.vy.min(0.0) - 3.5;
                self.jump_buf = 0;
                ev.jumped = true;
                ev.rope_released = true;
            }
        } else {
            let (maxs, acc, fric) = if self.in_water {
                (1.4, 0.2, 0.12)
            } else if self.on_ground {
                (2.0, 0.35, 0.3)
            } else {
                (2.0, 0.22, 0.06)
            };
            let maxs = if self.slowed > 0 { maxs * 0.5 } else { maxs };
            if dir != 0.0 && self.vx * dir < maxs {
                self.vx += dir * acc;
                if self.vx * dir > maxs {
                    self.vx = dir * maxs;
                }
            } else if dir != 0.0 {
                // Faster than walking (after a jet): bleed speed off gently.
                let over = self.vx.abs() - maxs;
                self.vx -= self.vx.signum() * over.min(fric * 0.5);
            } else if self.vx.abs() > fric {
                self.vx -= self.vx.signum() * fric;
            } else {
                self.vx = 0.0;
            }

            let (g, maxf) = if self.in_water { (0.09, 1.2) } else { (GRAVITY, 6.0) };
            if self.vy < maxf {
                self.vy = (self.vy + g).min(maxf);
            } else {
                self.vy = (self.vy - 0.3).max(maxf);
            }

            if self.jump_buf > 0 {
                if self.in_water {
                    self.vy = -2.8;
                    self.jump_buf = 0;
                    ev.jumped = true;
                } else if self.coyote > 0 {
                    self.vy = if self.slowed > 0 { -JUMP_V * 0.75 } else { -JUMP_V };
                    self.coyote = 0;
                    self.jump_buf = 0;
                    self.jumping = true;
                    ev.jumped = true;
                }
            }
            if self.jumping && !inp.jump && self.vy < -2.0 {
                self.vy = -2.0;
            }
            if self.vy >= 0.0 {
                self.jumping = false;
            }
        }

        // Armored jets smash cracked walls they run into.
        if jetting && sk.body_armor {
            let (nx, ny) = (self.x + self.vx * 2.0, self.y + self.vy * 2.0);
            for (cx, cy) in Level::cells(nx - 2.0, ny - 2.0, PW + 4.0, PH + 4.0) {
                if lv.get(cx, cy) == Tile::Cracked {
                    lv.set(cx, cy, Tile::Empty);
                    ev.broke.push((cx, cy));
                }
            }
        }

        let was_ground = self.on_ground;
        let impact = self.vy;
        self.move_x(lv);
        self.move_y(lv);
        self.on_ground = self.vy >= 0.0 && lv.rect_solid(self.x, self.y + PH, PW, 0.5);
        if self.on_ground && !was_ground {
            ev.landed = Some(impact);
        }

        if let Some(r) = self.rope {
            self.constrain_rope(r, lv);
        }
        ev
    }

    fn move_x(&mut self, lv: &Level) {
        let nx = self.x + self.vx;
        if !lv.rect_solid(nx, self.y, PW, PH) {
            self.x = nx;
            return;
        }
        if self.vx > 0.0 {
            let col = ((nx + PW - 0.001) / TS).floor();
            self.x = (col * TS - PW).max(self.x);
        } else {
            let col = (nx / TS).floor();
            self.x = ((col + 1.0) * TS).min(self.x);
        }
        if lv.rect_solid(self.x, self.y, PW, PH) {
            // Should not happen; stay put rather than tunnel.
            self.x -= self.vx;
        }
        self.vx = 0.0;
        if self.jet_t > 0 && self.jet_dy.abs() < 0.1 {
            self.jet_t = 0;
        }
    }

    fn move_y(&mut self, lv: &Level) {
        let ny = self.y + self.vy;
        if !lv.rect_solid(self.x, ny, PW, PH) {
            self.y = ny;
            return;
        }
        if self.vy > 0.0 {
            let row = ((ny + PH - 0.001) / TS).floor();
            self.y = (row * TS - PH).max(self.y);
        } else {
            let row = (ny / TS).floor();
            self.y = ((row + 1.0) * TS).min(self.y);
            self.jumping = false;
        }
        self.vy = 0.0;
    }

    fn constrain_rope(&mut self, r: Rope, lv: &Level) {
        let (dx, dy) = (self.cx() - r.ax, self.cy() - r.ay);
        let d = (dx * dx + dy * dy).sqrt();
        if d <= r.len || d < 1e-3 {
            return;
        }
        let (nx, ny) = (dx / d, dy / d);
        let tx = r.ax + nx * r.len - PW / 2.0;
        let ty = r.ay + ny * r.len - PH / 2.0;
        if !lv.rect_solid(tx, ty, PW, PH) {
            self.x = tx;
            self.y = ty;
        } else if !lv.rect_solid(tx, self.y, PW, PH) {
            self.x = tx;
        } else if !lv.rect_solid(self.x, ty, PW, PH) {
            self.y = ty;
        }
        let vr = self.vx * nx + self.vy * ny;
        if vr > 0.0 {
            self.vx -= nx * vr;
            self.vy -= ny * vr;
        }
    }
}
