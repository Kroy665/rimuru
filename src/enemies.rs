//! Cave monsters, the Tempest Serpent boss, adventurers and projectiles.

use crate::level::{Level, Tile, TS};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Bat,
    Spider,
    Centipede,
    Saurus,
    Serpent,
    Adventurer(u8),
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Bat => "Giant Bat",
            Kind::Spider => "Black Spider",
            Kind::Centipede => "Evil Centipede",
            Kind::Saurus => "Armor Saurus",
            Kind::Serpent => "Tempest Serpent",
            Kind::Adventurer(0) => "Kabal",
            Kind::Adventurer(1) => "Eren",
            Kind::Adventurer(_) => "Gido",
        }
    }
    /// Half extents of the hitbox.
    pub fn half(self) -> (f32, f32) {
        match self {
            Kind::Bat => (7.0, 5.0),
            Kind::Spider => (7.0, 6.0),
            Kind::Centipede => (14.0, 5.0),
            Kind::Saurus => (15.0, 10.0),
            Kind::Serpent => (14.0, 11.0),
            Kind::Adventurer(_) => (6.0, 14.0),
        }
    }
    pub fn max_hp(self) -> i32 {
        match self {
            Kind::Bat => 2,
            Kind::Spider => 3,
            Kind::Centipede => 3,
            Kind::Saurus => 6,
            Kind::Serpent => 18,
            Kind::Adventurer(_) => 999,
        }
    }
    pub fn contact_damage(self) -> i32 {
        match self {
            Kind::Bat | Kind::Spider | Kind::Centipede => 1,
            Kind::Saurus | Kind::Serpent => 2,
            Kind::Adventurer(_) => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub home: (f32, f32),
    pub hp: i32,
    pub state: u8,
    pub t: i32,
    pub facing: f32,
    pub stun: i32,
    pub hurt: i32,
    pub dead: bool,
    /// Index into the room's spawn list (for remembering eaten corpses).
    pub id: usize,
    /// Serpent body trail (head positions, newest first).
    pub trail: Vec<(f32, f32)>,
    pub awake: bool,
    pub phase: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjKind {
    Blade,
    Web,
    Poison,
}

#[derive(Clone, Debug)]
pub struct Proj {
    pub kind: ProjKind,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: i32,
}

/// Things enemies want the game to do this frame.
#[derive(Default)]
pub struct AiOut {
    pub shots: Vec<Proj>,
    pub shake: f32,
    pub roar: bool,
}

impl Enemy {
    pub fn spawn(ch: char, x: f32, bottom: f32, id: usize, adv_index: u8) -> Option<Enemy> {
        let kind = match ch {
            'b' => Kind::Bat,
            's' => Kind::Spider,
            'c' => Kind::Centipede,
            'a' => Kind::Saurus,
            'S' => Kind::Serpent,
            'k' => Kind::Adventurer(adv_index),
            _ => return None,
        };
        let (_, hh) = kind.half();
        let y = match kind {
            // Bats and spiders hang from the ceiling of their cell.
            Kind::Bat => bottom - TS + hh,
            Kind::Spider => bottom - TS + hh,
            _ => bottom - hh,
        };
        Some(Enemy {
            kind,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            home: (x, y),
            hp: kind.max_hp(),
            state: 0,
            t: 0,
            facing: -1.0,
            stun: 0,
            hurt: 0,
            dead: false,
            id,
            trail: vec![(x, y); 90],
            awake: false,
            phase: 0,
        })
    }

    pub fn alive(&self) -> bool {
        !self.dead
    }

    /// Axis-aligned overlap with a rectangle given by its top-left and size.
    pub fn overlaps(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let (hw, hh) = self.kind.half();
        self.x - hw < x + w && self.x + hw > x && self.y - hh < y + h && self.y + hh > y
    }

    /// Serpent body segments (excluding the head).
    pub fn segments(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        (1..=11).map(move |i| self.trail[(i * 7).min(self.trail.len() - 1)])
    }

    pub fn update(&mut self, lv: &Level, px: f32, py: f32) -> AiOut {
        let mut out = AiOut::default();
        self.t += 1;
        self.hurt = (self.hurt - 1).max(0);
        if self.dead {
            // Corpses drop to the floor.
            if !matches!(self.kind, Kind::Serpent) {
                self.vy = (self.vy + 0.3).min(5.0);
                let (_, hh) = self.kind.half();
                if !lv.solid_at(self.x, self.y + hh + self.vy) {
                    self.y += self.vy;
                } else {
                    self.vy = 0.0;
                }
            }
            return out;
        }
        if self.stun > 0 {
            self.stun -= 1;
            return out;
        }
        let dx = px - self.x;
        let dy = py - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let sees = |e: &Enemy, range: f32| dist < range && lv.clear_line(e.x, e.y, px, py);

        match self.kind {
            Kind::Bat => match self.state {
                0 => {
                    self.x = self.home.0;
                    self.y = self.home.1 + (self.t as f32 * 0.05).sin();
                    if self.t > 40 && sees(self, 190.0) {
                        self.state = 1;
                        self.t = 0;
                        let l = dist.max(1.0);
                        self.vx = dx / l * 2.2;
                        self.vy = dy / l * 2.2;
                    }
                }
                1 => {
                    let wob = (self.t as f32 * 0.3).sin() * 0.8;
                    self.try_move(lv, self.vx, self.vy + wob);
                    if self.t > 70 {
                        self.state = 2;
                        self.t = 0;
                    }
                }
                _ => {
                    let (hx, hy) = (self.home.0 - self.x, self.home.1 - self.y);
                    let l = (hx * hx + hy * hy).sqrt();
                    if l < 2.0 {
                        self.state = 0;
                        self.t = 0;
                    } else {
                        self.x += hx / l * 1.6;
                        self.y += hy / l * 1.6;
                    }
                }
            },
            Kind::Spider => {
                self.x = self.home.0;
                self.y = self.home.1 + 34.0 + (self.t as f32 * 0.03).sin() * 22.0;
                self.facing = dx.signum();
                if self.t % 110 == 0 && sees(self, 170.0) {
                    let l = dist.max(1.0);
                    out.shots.push(Proj {
                        kind: ProjKind::Web,
                        x: self.x,
                        y: self.y,
                        vx: dx / l * 2.6,
                        vy: dy / l * 2.6,
                        life: 100,
                    });
                }
            }
            Kind::Centipede => {
                self.walk(lv, 0.7);
            }
            Kind::Saurus => match self.state {
                0 => {
                    self.walk(lv, 0.5);
                    let ahead = dx * self.facing > 0.0;
                    if ahead && dx.abs() < 150.0 && dy.abs() < 28.0 && sees(self, 160.0) {
                        self.state = 1;
                        self.t = 0;
                    }
                }
                1 => {
                    if self.t > 30 {
                        self.state = 2;
                        self.t = 0;
                    }
                }
                2 => {
                    let moved = self.walk(lv, 3.4);
                    if !moved || self.t > 80 {
                        self.state = 3;
                        self.t = 0;
                        out.shake = 3.0;
                    }
                }
                _ => {
                    if self.t > 60 {
                        self.state = 0;
                        self.t = 0;
                        self.facing = if dx > 0.0 { 1.0 } else { -1.0 };
                    }
                }
            },
            Kind::Serpent => self.serpent(lv, px, py, &mut out),
            Kind::Adventurer(_) => match self.state {
                0 => {
                    if !self.walk(lv, 0.55) {
                        self.state = 1;
                        self.t = 0;
                    }
                }
                _ => {
                    if self.t > 100 {
                        self.state = 0;
                        self.t = 0;
                    }
                }
            },
        }
        out
    }

    fn try_move(&mut self, lv: &Level, vx: f32, vy: f32) {
        let (hw, hh) = self.kind.half();
        if !lv.rect_solid(self.x + vx - hw, self.y - hh, hw * 2.0, hh * 2.0) {
            self.x += vx;
        } else {
            self.vx = -self.vx;
        }
        if !lv.rect_solid(self.x - hw, self.y + vy - hh, hw * 2.0, hh * 2.0) {
            self.y += vy;
        } else {
            self.vy = -self.vy;
        }
    }

    /// Ground patrol: walks until a wall or ledge, then turns. Returns false when it turned.
    fn walk(&mut self, lv: &Level, speed: f32) -> bool {
        let (hw, hh) = self.kind.half();
        let nx = self.x + self.facing * speed;
        let front = nx + self.facing * hw;
        let blocked = lv.rect_solid(nx - hw, self.y - hh, hw * 2.0, hh * 2.0 - 1.0);
        let floor_ahead = lv.solid_at(front, self.y + hh + 2.0);
        let hazard = lv.tile_at(front, self.y + hh - 2.0) == Tile::Spikes;
        if blocked || !floor_ahead || hazard {
            self.facing = -self.facing;
            return false;
        }
        self.x = nx;
        true
    }

    fn serpent(&mut self, lv: &Level, px: f32, py: f32, out: &mut AiOut) {
        if !self.awake {
            return;
        }
        let fast = self.hp <= self.kind.max_hp() / 2;
        if fast && self.phase == 0 {
            self.phase = 1;
            out.roar = true;
        }
        let floor_y = self.home.1;
        let spd = if fast { 1.9 } else { 1.3 };
        let dx = px - self.x;
        match self.state {
            // Slither towards the player along the floor.
            0 => {
                self.facing = dx.signum();
                self.vx = self.vx * 0.9 + self.facing * spd * 0.1;
                self.y += (floor_y - self.y) * 0.1;
                let limit = if fast { 90 } else { 130 };
                if self.t > limit {
                    self.state = if self.t % 2 == 0 && dx.abs() < 170.0 { 1 } else { 4 };
                    self.t = 0;
                }
            }
            // Rear up (telegraph).
            1 => {
                self.vx *= 0.85;
                self.y += (floor_y - 46.0 - self.y) * 0.08;
                if self.t > if fast { 26 } else { 40 } {
                    self.state = 2;
                    self.t = 0;
                    let (tx, ty) = (px - self.x, py - self.y);
                    let l = (tx * tx + ty * ty).sqrt().max(1.0);
                    self.vx = tx / l * 5.2;
                    self.vy = ty / l * 5.2;
                    out.roar = true;
                }
            }
            // Lunge.
            2 => {
                self.y += self.vy;
                if self.t > 22 || self.y > floor_y {
                    self.state = 3;
                    self.t = 0;
                    out.shake = 4.0;
                }
            }
            // Recover (vulnerable, slow).
            3 => {
                self.vx *= 0.8;
                self.vy = 0.0;
                self.y += (floor_y - self.y) * 0.15;
                if self.t > 45 {
                    self.state = 0;
                    self.t = 0;
                }
            }
            // Spit poison.
            _ => {
                self.vx *= 0.85;
                self.y += (floor_y - 30.0 - self.y) * 0.1;
                let shots = if fast { 5 } else { 3 };
                if self.t % 16 == 0 && self.t / 16 <= shots && self.t > 0 {
                    let k = (self.t / 16) as f32;
                    let dir = dx.signum();
                    out.shots.push(Proj {
                        kind: ProjKind::Poison,
                        x: self.x + dir * 10.0,
                        y: self.y - 4.0,
                        vx: dir * (1.4 + k * 0.6) + dx / 260.0,
                        vy: -4.0,
                        life: 160,
                    });
                }
                if self.t > 16 * (shots + 1) + 10 {
                    self.state = 0;
                    self.t = 0;
                }
            }
        }
        // Horizontal motion with walls.
        let (hw, _) = self.kind.half();
        let nx = self.x + self.vx;
        if lv.solid_at(nx + hw * self.vx.signum(), self.y) {
            self.vx = -self.vx * 0.3;
        } else {
            self.x = nx.clamp(2.0 * TS + hw, lv.px_w() - 2.0 * TS - hw);
        }
        self.y = self.y.min(floor_y);
        self.trail.insert(0, (self.x, self.y));
        self.trail.truncate(90);
    }

    /// Damage this enemy. Returns false if the hit was blocked.
    pub fn damage(&mut self, amount: i32, from_dir: f32) -> bool {
        if self.dead || matches!(self.kind, Kind::Adventurer(_)) {
            return false;
        }
        // Armor Saurus shrugs off hits to its armored front unless stunned.
        if self.kind == Kind::Saurus && self.stun == 0 && from_dir * self.facing < 0.0 {
            return false;
        }
        self.hp -= amount;
        self.hurt = 8;
        if self.kind == Kind::Serpent && !self.awake {
            self.awake = true;
        }
        if self.hp <= 0 {
            self.dead = true;
            self.vy = 0.0;
        }
        true
    }

    /// Lantern cone check for adventurers.
    pub fn spots(&self, lv: &Level, px: f32, py: f32) -> bool {
        if !matches!(self.kind, Kind::Adventurer(_)) {
            return false;
        }
        let (lx, ly) = self.lantern();
        lantern_sees(lv, lx, ly, self.facing, px, py)
    }

    pub fn lantern(&self) -> (f32, f32) {
        (self.x + self.facing * 6.0, self.y - 2.0)
    }
}

pub const LANTERN_RANGE: f32 = 110.0;

pub fn lantern_sees(lv: &Level, lx: f32, ly: f32, facing: f32, px: f32, py: f32) -> bool {
    let (dx, dy) = (px - lx, py - ly);
    let d = (dx * dx + dy * dy).sqrt();
    if d > LANTERN_RANGE || dx * facing <= 0.0 {
        return false;
    }
    let ang = (dy / d).asin().abs();
    ang < LANTERN_HALF_ANGLE && lv.clear_line(lx, ly, px, py)
}
pub const LANTERN_HALF_ANGLE: f32 = 0.42;

impl Proj {
    pub fn step(&mut self, lv: &Level) -> bool {
        if self.kind == ProjKind::Poison {
            self.vy += 0.18;
        }
        self.x += self.vx;
        self.y += self.vy;
        self.life -= 1;
        self.life > 0 && !lv.solid_at(self.x, self.y)
    }
    pub fn radius(&self) -> f32 {
        match self.kind {
            ProjKind::Blade => 5.0,
            ProjKind::Web => 4.0,
            ProjKind::Poison => 4.0,
        }
    }
}
