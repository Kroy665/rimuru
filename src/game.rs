//! Game state, story flow and per-frame simulation.

use crate::enemies::{Enemy, Kind, Proj, ProjKind};
use crate::gfx::{pal, Color, H, W};
use crate::level::{Level, Tile, TS};
use crate::player::{Body, Input, Skills, PH, PW, WATER_MAX};
use crate::rooms::{Who, ROOMS};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    Title,
    Prologue,
    Play,
    Ending,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Jump,
    Land,
    Jet,
    Splash,
    Blade,
    Hit,
    Tink,
    Hurt,
    Eat,
    Sonic,
    Thread,
    Notice,
    Break,
    Roar,
    Spotted,
    Talk,
    Door,
}

#[derive(Clone, Debug)]
pub enum Act {
    MagicSense,
    Named,
    VeldoraReady,
    Caught,
    Sage(String),
    WakeBoss,
}

#[derive(Clone, Debug)]
pub enum Beat {
    Say(Who, String),
    Do(Act),
}

pub struct Toast {
    pub text: String,
    pub t: i32,
    pub dur: i32,
}

pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: i32,
    pub max: i32,
    pub c: Color,
    pub size: f32,
    pub grav: f32,
}

#[derive(Default, Clone)]
pub struct Stats {
    pub frames: u64,
    pub deaths: u32,
    pub ore: u32,
    pub grass: u32,
    pub spotted: u32,
    pub eaten: u32,
}

#[derive(Default, Clone)]
pub struct Flags {
    pub named: bool,
    pub veldora_ready: bool,
    pub veldora_eaten: bool,
    pub lake_hint: bool,
    pub boss_eaten: bool,
    pub first_grass: bool,
}

pub const PROLOGUE: &[&str] = &[
    "Satoru Mikami, 37. An ordinary salaryman in Tokyo.",
    "One evening, a robber's knife ended that ordinary life.",
    "As the world faded, a calm, mechanical voice spoke in the dark...",
    "<<Confirmed. Constructing a new body... Resistances acquired... Unique skills acquired...>>",
    "And somewhere far away, deep inside a sealed cave, something small and squishy woke up.",
];

pub struct Game {
    pub scene: Scene,
    pub scene_t: i32,
    pub prologue_line: usize,
    pub room: usize,
    pub lv: Level,
    pub body: Body,
    pub skills: Skills,
    pub hp: i32,
    pub max_hp: i32,
    pub invuln: i32,
    pub magicules: u32,
    pub enemies: Vec<Enemy>,
    pub projs: Vec<Proj>,
    pub parts: Vec<Particle>,
    pub toasts: VecDeque<Toast>,
    pub dialogue: VecDeque<Beat>,
    pub typed: usize,
    pub cam: (f32, f32),
    pub shake: f32,
    pub revealed: Vec<bool>,
    pub fired: HashSet<(usize, u8)>,
    pub collected: HashSet<(usize, i32, i32)>,
    pub eaten: HashSet<(usize, usize)>,
    pub flags: Flags,
    pub fade_out: i32,
    pub fade_in: i32,
    pub banner: i32,
    pub dead_t: i32,
    pub prev: Input,
    pub blade_cd: i32,
    pub sonic_cd: i32,
    pub sonic: Option<(f32, f32, i32)>,
    pub safe: (f32, f32),
    pub squash: f32,
    pub eat_anim: i32,
    pub veldora: Option<(f32, f32)>,
    pub veldora_fade: i32,
    pub stats: Stats,
    pub sfx: Vec<Sfx>,
    pub t: u64,
    pub rng: u32,
    /// For tools: skip dialogue automatically.
    pub auto_dialogue: bool,
    pub paused: bool,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn new() -> Game {
        let lv = Level::parse(ROOMS[0].map);
        let body = Body::new(lv.spawn.0, lv.spawn.1);
        Game {
            scene: Scene::Title,
            scene_t: 0,
            prologue_line: 0,
            room: 0,
            revealed: vec![false; (lv.w * lv.h) as usize],
            safe: (body.x, body.y),
            lv,
            body,
            skills: Skills { predator: true, ..Default::default() },
            hp: 5,
            max_hp: 5,
            invuln: 0,
            magicules: 0,
            enemies: Vec::new(),
            projs: Vec::new(),
            parts: Vec::new(),
            toasts: VecDeque::new(),
            dialogue: VecDeque::new(),
            typed: 0,
            cam: (0.0, 0.0),
            shake: 0.0,
            fired: HashSet::new(),
            collected: HashSet::new(),
            eaten: HashSet::new(),
            flags: Flags::default(),
            fade_out: 0,
            fade_in: 0,
            banner: 0,
            dead_t: 0,
            prev: Input::default(),
            blade_cd: 0,
            sonic_cd: 0,
            sonic: None,
            squash: 0.0,
            eat_anim: 0,
            veldora: None,
            veldora_fade: 0,
            stats: Stats::default(),
            sfx: Vec::new(),
            t: 0,
            rng: 0x2545F491,
            auto_dialogue: false,
            paused: false,
        }
    }

    /// Jump straight into a room with the skills you'd normally have there (dev / tests).
    pub fn start_at(room: usize) -> Game {
        let mut g = Game::new();
        g.scene = Scene::Play;
        g.skills = crate::skills_for_room(room, false);
        g.flags.named = room >= 3;
        g.flags.veldora_eaten = room >= 3;
        g.flags.lake_hint = room >= 2;
        g.body.water = if g.skills.water_jet { WATER_MAX } else { 0.0 };
        g.load_room(room, true);
        g
    }

    pub fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng % 10000) as f32 / 10000.0
    }

    pub fn dark(&self) -> bool {
        ROOMS[self.room].dark && !self.skills.magic_sense
    }

    pub fn who_name(&self, w: Who) -> &'static str {
        if w == Who::Rimuru && !self.flags.named {
            "???"
        } else {
            w.name()
        }
    }

    fn sage(&mut self, text: impl Into<String>) {
        let text = format!("Notice. {}", text.into().trim_start_matches("Notice. "));
        let dur = 100 + text.len() as i32 * 2;
        self.toasts.push_back(Toast { text, t: 0, dur });
        self.sfx.push(Sfx::Notice);
    }

    fn say(&mut self, who: Who, text: &str) {
        if who == Who::Sage {
            self.sage(text);
        } else {
            self.dialogue.push_back(Beat::Say(who, text.to_string()));
        }
    }

    pub fn load_room(&mut self, room: usize, entering: bool) {
        self.room = room;
        let def = &ROOMS[room];
        self.lv = Level::parse(def.map);
        for &(r, x, y) in &self.collected {
            if r == room {
                self.lv.set(x, y, Tile::Empty);
            }
        }
        if (room == 2 && self.flags.veldora_eaten) || (room == 6 && self.flags.boss_eaten) {
            self.lv.open_doors();
        }
        self.body = Body {
            water: self.body.water,
            ..Body::new(self.lv.spawn.0, self.lv.spawn.1)
        };
        self.safe = (self.body.x, self.body.y);
        self.enemies.clear();
        self.veldora = None;
        let mut adv = 0;
        for (id, s) in self.lv.spawns.clone().iter().enumerate() {
            if s.kind == 'V' {
                if !self.flags.veldora_eaten {
                    self.veldora = Some((s.x, s.y));
                }
                continue;
            }
            if self.eaten.contains(&(room, id)) {
                continue;
            }
            if let Some(e) = Enemy::spawn(s.kind, s.x, s.y, id, adv) {
                if matches!(e.kind, Kind::Adventurer(_)) {
                    adv += 1;
                }
                self.enemies.push(e);
            }
        }
        self.projs.clear();
        self.revealed = vec![false; (self.lv.w * self.lv.h) as usize];
        self.hp = self.max_hp;
        self.invuln = 0;
        self.sonic = None;
        self.cam = self.cam_target();
        self.fade_in = 30;
        if entering {
            // Old news from the previous room.
            self.toasts.clear();
            self.banner = 180;
            for &(who, text) in def.on_enter {
                self.say(who, text);
            }
        }
    }

    fn cam_target(&self) -> (f32, f32) {
        let mut tx = self.body.cx() - W as f32 / 2.0 + self.body.facing * 24.0;
        if let (Some((vx, _)), false) = (self.veldora, self.dialogue.is_empty()) {
            // Frame both Rimuru and Veldora while they talk.
            tx = (self.body.cx() + vx) / 2.0 - W as f32 / 2.0;
        }
        let ty = self.body.cy() - H as f32 / 2.0 - 10.0;
        let clamp = |v: f32, size: f32, view: f32| {
            if size <= view {
                (size - view) / 2.0
            } else {
                v.clamp(0.0, size - view)
            }
        };
        (clamp(tx, self.lv.px_w(), W as f32), clamp(ty, self.lv.px_h(), H as f32))
    }

    pub fn burst(&mut self, x: f32, y: f32, n: usize, c: Color, speed: f32, grav: f32) {
        for _ in 0..n {
            let a = self.rand() * std::f32::consts::TAU;
            let s = speed * (0.3 + self.rand() * 0.7);
            let life = 18 + (self.rand() * 20.0) as i32;
            let size = 1.0 + self.rand() * 2.0;
            self.parts.push(Particle {
                x,
                y,
                vx: a.cos() * s,
                vy: a.sin() * s,
                life,
                max: life,
                c,
                size,
                grav,
            });
        }
    }

    pub fn update(&mut self, inp: Input) {
        self.t += 1;
        self.scene_t += 1;
        let prev = self.prev;
        let confirm = (inp.jump && !prev.jump) || (inp.start && !prev.start) || (inp.predate && !prev.predate);
        match self.scene {
            Scene::Title => {
                if confirm && self.scene_t > 20 {
                    self.scene = Scene::Prologue;
                    self.scene_t = 0;
                    self.prologue_line = 0;
                }
            }
            Scene::Prologue => {
                if (confirm || self.scene_t > 300) && self.scene_t > 15 {
                    self.prologue_line += 1;
                    self.scene_t = 0;
                    if self.prologue_line >= PROLOGUE.len() {
                        self.scene = Scene::Play;
                        self.load_room(0, true);
                    }
                }
            }
            Scene::Play => self.play(inp, prev, confirm),
            Scene::Ending => {
                if confirm && self.scene_t > 120 {
                    *self = Game::new();
                }
            }
        }
        self.prev = inp;
    }

    fn play(&mut self, inp: Input, prev: Input, confirm: bool) {
        if inp.start && !prev.start && self.dialogue.is_empty() {
            self.paused = !self.paused;
        }
        if self.paused {
            return;
        }
        self.stats.frames += 1;
        self.update_particles();
        let backlog = self.toasts.len() as i32;
        if let Some(t) = self.toasts.front_mut() {
            // Hurry along when more messages are waiting.
            t.t += if backlog > 1 { 2 } else { 1 };
            if t.t > t.dur {
                self.toasts.pop_front();
            }
        }
        self.banner = (self.banner - 1).max(0);
        self.shake *= 0.85;

        if self.fade_out > 0 {
            self.fade_out -= 1;
            if self.fade_out == 0 {
                if self.room + 1 >= ROOMS.len() {
                    self.scene = Scene::Ending;
                    self.scene_t = 0;
                } else {
                    self.load_room(self.room + 1, true);
                }
            }
            return;
        }
        self.fade_in = (self.fade_in - 1).max(0);

        if self.dead_t > 0 {
            self.dead_t -= 1;
            if self.dead_t == 0 {
                self.load_room(self.room, false);
                self.sage("Body reconstituted. Resuming from the room entrance.");
            }
            return;
        }

        // Dialogue pauses the world.
        if let Some(beat) = self.dialogue.front().cloned() {
            match beat {
                Beat::Do(act) => {
                    self.dialogue.pop_front();
                    self.run_act(act);
                }
                Beat::Say(_, text) => {
                    let len = text.chars().count();
                    if self.typed < len {
                        self.typed += 1;
                        if self.typed.is_multiple_of(4) {
                            self.sfx.push(Sfx::Talk);
                        }
                    }
                    if confirm || self.auto_dialogue {
                        if self.typed < len && !self.auto_dialogue {
                            self.typed = len;
                        } else {
                            self.dialogue.pop_front();
                            self.typed = 0;
                        }
                    }
                }
            }
            self.camera();
            return;
        }

        self.invuln = (self.invuln - 1).max(0);
        self.blade_cd = (self.blade_cd - 1).max(0);
        self.sonic_cd = (self.sonic_cd - 1).max(0);
        self.eat_anim = (self.eat_anim - 1).max(0);
        self.veldora_fade = (self.veldora_fade - 1).max(0);

        let ev = self.body.step(inp, prev, &mut self.lv, &self.skills);
        let (cx, cy) = (self.body.cx(), self.body.cy());
        let bottom = self.body.y + PH;
        if ev.jumped {
            self.squash = -0.35;
            self.sfx.push(Sfx::Jump);
        }
        if let Some(v) = ev.landed
            && v > 2.0 {
                self.squash = (v / 6.0).min(1.0) * 0.5;
                self.sfx.push(Sfx::Land);
                self.burst(cx, bottom, 4, pal::ROCK_HI, 1.0, 0.1);
            }
        if ev.jetted {
            self.sfx.push(Sfx::Jet);
            self.squash = -0.3;
        }
        if self.body.jet_t > 0 && self.t.is_multiple_of(2) {
            let (jx, jy) = (-self.body.jet_dx * 2.5, -self.body.jet_dy * 2.5);
            for _ in 0..2 {
                let (rx, ry) = (self.rand() - 0.5, self.rand() - 0.5);
                self.parts.push(Particle {
                    x: cx,
                    y: cy,
                    vx: jx + rx,
                    vy: jy + ry,
                    life: 16,
                    max: 16,
                    c: pal::WATER_HI,
                    size: 2.0,
                    grav: 0.1,
                });
            }
        }
        if ev.splashed {
            self.sfx.push(Sfx::Splash);
            self.burst(cx, cy, 10, pal::WATER_HI, 2.0, 0.15);
        }
        if !ev.broke.is_empty() {
            self.sfx.push(Sfx::Break);
            self.shake = 5.0;
            for (x, y) in ev.broke {
                self.burst(x as f32 * TS + 8.0, y as f32 * TS + 8.0, 10, pal::ROCK_HI, 2.5, 0.2);
            }
        }
        if ev.rope_attached {
            self.sfx.push(Sfx::Thread);
        }
        self.squash *= 0.8;

        // Darkness: touch reveals.
        if self.dark() {
            for (x, y) in Level::cells(self.body.x - 4.0, self.body.y - 4.0, PW + 8.0, PH + 8.0) {
                if x >= 0 && y >= 0 && x < self.lv.w && y < self.lv.h {
                    self.revealed[(y * self.lv.w + x) as usize] = true;
                }
            }
        }

        self.check_triggers();
        if self.room == 1 && self.body.in_water && !self.flags.lake_hint {
            self.flags.lake_hint = true;
            self.say(Who::Rimuru, "Water! I'm sinking... but I don't need to breathe? Handy.");
            self.sage("A large volume of water detected. Press E to [Predate] it.");
        }

        // Hazards.
        if self.lv.rect_spiked(self.body.x, self.body.y, PW, PH) {
            self.hurt(1, cx + self.body.facing);
            if self.hp > 0 {
                self.body.x = self.safe.0;
                self.body.y = self.safe.1;
                self.body.vx = 0.0;
                self.body.vy = 0.0;
                self.body.rope = None;
            }
        } else if self.body.on_ground {
            let near_spikes = Level::cells(self.body.x - 16.0, self.body.y, PW + 32.0, PH + 8.0)
                .any(|(x, y)| self.lv.get(x, y) == Tile::Spikes);
            if !near_spikes {
                self.safe = (self.body.x, self.body.y);
            }
        }

        // Exit.
        if crate::solver::at_exit(&self.body, &self.lv) {
            self.fade_out = 30;
            self.sfx.push(Sfx::Door);
            return;
        }

        self.actions(inp, prev);
        self.update_enemies();
        self.update_projectiles();
        self.camera();
    }

    fn camera(&mut self) {
        let t = self.cam_target();
        self.cam.0 += (t.0 - self.cam.0) * 0.12;
        self.cam.1 += (t.1 - self.cam.1) * 0.12;
    }

    fn check_triggers(&mut self) {
        let cells = self.lv.triggers_near(self.body.x, self.body.y, PW, PH);
        for id in cells {
            if !self.fired.insert((self.room, id)) {
                continue;
            }
            for &(tid, who, text) in ROOMS[self.room].triggers {
                if tid == id {
                    self.say(who, text);
                }
            }
            match (self.room, id) {
                (2, 1) => {
                    self.say(Who::Rimuru, "W-who's there?! I can't see anything!");
                    self.say(Who::Veldora, "Come closer, little one. I won't bite. ...Probably.");
                }
                (2, 2) => self.veldora_talk(),
                (6, 1) => {
                    self.dialogue.push_back(Beat::Do(Act::WakeBoss));
                }
                _ => {}
            }
        }
    }

    fn veldora_talk(&mut self) {
        use Who::*;
        let lines: &[(Who, &str)] = &[
            (Veldora, "KWAHAHAHA! A slime wandering this deep? I am Veldora, the Storm Dragon! One of the four True Dragons!"),
            (Rimuru, "A d-dragon?! Please don't eat me! I'm mostly water!"),
            (Veldora, "Hmph. Nobody is eating anybody. Three hundred years ago a Hero sealed me here with the [Infinite Prison]. I am... a little bored."),
            (Veldora, "You cannot see, can you? I shall teach you [Magic Sense]. Feel the magicules flowing all around you."),
        ];
        for &(w, t) in lines {
            self.say(w, t);
        }
        self.dialogue.push_back(Beat::Do(Act::MagicSense));
        let lines: &[(Who, &str)] = &[
            (Rimuru, "Whoa... I can see! The cave... the crystals... and you are HUGE."),
            (Veldora, "Kwahaha! You are the first to talk to me in centuries. Let us be friends! And friends give each other names."),
            (Veldora, "From today you are RIMURU. In return, choose a family name we shall share."),
            (Rimuru, "Storm Dragon... storm... How about TEMPEST? Veldora Tempest and Rimuru Tempest."),
            (Veldora, "TEMPEST! A splendid, thunderous name!"),
        ];
        for &(w, t) in lines {
            self.say(w, t);
        }
        self.dialogue.push_back(Beat::Do(Act::Named));
        let lines: &[(Who, &str)] = &[
            (Veldora, "Now, Rimuru. Your [Predator] can swallow anything. Swallow me, seal and all. Inside you, we can both work on breaking the prison."),
            (Rimuru, "You trust me that much? ...Alright. Hold still, Veldora!"),
        ];
        for &(w, t) in lines {
            self.say(w, t);
        }
        self.dialogue.push_back(Beat::Do(Act::VeldoraReady));
    }

    fn run_act(&mut self, act: Act) {
        match act {
            Act::MagicSense => {
                self.skills.magic_sense = true;
                self.sage("Skill [Magic Sense] acquired. Visual information now available.");
                if let Some((x, y)) = self.veldora {
                    self.burst(x, y - 40.0, 40, pal::GOLD, 3.0, 0.0);
                }
            }
            Act::Named => {
                self.flags.named = true;
                self.sage("Name [Rimuru Tempest] confirmed. Magicule capacity greatly increased.");
                self.max_hp += 1;
                self.hp = self.max_hp;
            }
            Act::VeldoraReady => {
                self.flags.veldora_ready = true;
                self.sage("Stand next to Veldora and press E to [Predate].");
            }
            Act::Caught => {
                self.stats.spotted += 1;
                self.load_room(self.room, false);
                self.sage("Retreated to a safe position. Stay out of the lantern light.");
            }
            Act::Sage(t) => self.sage(t),
            Act::WakeBoss => {
                for e in &mut self.enemies {
                    if e.kind == Kind::Serpent {
                        e.awake = true;
                    }
                }
                self.shake = 6.0;
                self.sfx.push(Sfx::Roar);
                self.say(Who::Sage, "Tempest Serpent. Its head is the weak point. Watch for the rear-up before it lunges.");
            }
        }
    }

    fn actions(&mut self, inp: Input, prev: Input) {
        let (cx, cy) = (self.body.cx(), self.body.cy());
        if inp.blade && !prev.blade && self.skills.water_blade && self.blade_cd == 0 {
            if self.body.water >= 4.0 {
                self.body.water -= 4.0;
                self.blade_cd = 12;
                let f = self.body.facing;
                let p = if inp.up {
                    Proj { kind: ProjKind::Blade, x: cx, y: cy - 8.0, vx: 0.0, vy: -6.0, life: 32 }
                } else {
                    Proj { kind: ProjKind::Blade, x: cx + f * 8.0, y: cy, vx: f * 6.0, vy: 0.0, life: 32 }
                };
                self.projs.push(p);
                self.sfx.push(Sfx::Blade);
            } else if self.toasts.is_empty() {
                self.sage("Insufficient water. Refill by touching water.");
            }
        }
        if inp.sonic && !prev.sonic && self.skills.ultrasonic && self.sonic_cd == 0 {
            self.sonic_cd = 60;
            self.sonic = Some((cx, cy, 0));
            self.sfx.push(Sfx::Sonic);
            const R: f32 = 80.0;
            let (x0, y0) = (((cx - R) / TS) as i32, ((cy - R) / TS) as i32);
            let (x1, y1) = (((cx + R) / TS) as i32, ((cy + R) / TS) as i32);
            for ty in y0..=y1 {
                for tx in x0..=x1 {
                    let (tcx, tcy) = (tx as f32 * TS + 8.0, ty as f32 * TS + 8.0);
                    if self.lv.get(tx, ty) == Tile::Crystal && (tcx - cx).hypot(tcy - cy) < R + 8.0 {
                        self.lv.set(tx, ty, Tile::Empty);
                        self.burst(tcx, tcy, 8, pal::CRYSTAL, 2.5, 0.15);
                        self.sfx.push(Sfx::Break);
                    }
                }
            }
            let mut hits = Vec::new();
            for (i, e) in self.enemies.iter_mut().enumerate() {
                if e.alive() && !matches!(e.kind, Kind::Adventurer(_)) && (e.x - cx).hypot(e.y - cy) < R {
                    e.stun = if e.kind == Kind::Serpent { 50 } else { 120 };
                    e.damage(1, (e.x - cx).signum());
                    hits.push(i);
                }
            }
            for i in hits {
                let (x, y) = (self.enemies[i].x, self.enemies[i].y);
                self.burst(x, y, 6, pal::WHITE, 1.5, 0.0);
            }
        }
        if inp.predate && !prev.predate {
            self.predate();
        }
    }

    fn predate(&mut self) {
        let (cx, cy) = (self.body.cx(), self.body.cy());
        self.eat_anim = 16;
        // Veldora.
        if let Some((vx, vy)) = self.veldora
            && self.flags.veldora_ready && (vx - cx).abs() < 90.0 && (vy - cy).abs() < 90.0 {
                self.veldora = None;
                self.veldora_fade = 90;
                self.flags.veldora_eaten = true;
                self.stats.eaten += 1;
                self.sfx.push(Sfx::Eat);
                self.shake = 8.0;
                self.burst(vx, vy - 40.0, 80, pal::GOLD, 4.0, 0.0);
                self.sage("Storm Dragon Veldora and the [Infinite Prison] have been predated. Analysis has begun.");
                self.sage("Skill [Water Blade] unlocked: press J to fire compressed water. Costs water.");
                self.skills.water_blade = true;
                self.lv.open_doors();
                self.sfx.push(Sfx::Door);
                return;
            }
        // Corpses.
        if let Some(i) = self
            .enemies
            .iter()
            .position(|e| e.dead && (e.x - cx).abs() < 26.0 && (e.y - cy).abs() < 26.0)
        {
            let e = self.enemies.remove(i);
            self.eaten.insert((self.room, e.id));
            self.stats.eaten += 1;
            self.sfx.push(Sfx::Eat);
            self.burst(e.x, e.y, 16, pal::SLIME_HI, 2.0, 0.0);
            self.hp = (self.hp + 1).min(self.max_hp);
            let gained = match e.kind {
                Kind::Bat if !self.skills.ultrasonic => {
                    self.skills.ultrasonic = true;
                    Some("[Ultrasonic Waves]: press K to stun foes and shatter crystal.")
                }
                Kind::Spider if !self.skills.steel_thread => {
                    self.skills.steel_thread = true;
                    Some("[Steel Thread]: press L to shoot thread (hold Up to aim straight up). Up/Down to climb, Space to let go.")
                }
                Kind::Centipede if !self.skills.poison_resist => {
                    self.skills.poison_resist = true;
                    Some("[Poison Resistance]: poison damage reduced.")
                }
                Kind::Saurus if !self.skills.body_armor => {
                    self.skills.body_armor = true;
                    Some("[Body Armor]: water-jet into cracked walls to smash them. Damage taken reduced.")
                }
                Kind::Serpent => {
                    self.skills.heat_sense = true;
                    self.flags.boss_eaten = true;
                    self.lv.open_doors();
                    self.sfx.push(Sfx::Door);
                    Some("[Heat Sense] and [Poisonous Breath] acquired. The way forward is open.")
                }
                _ => None,
            };
            match gained {
                Some(t) => self.sage(format!("{} predated. Skill {}", e.kind.name(), t)),
                None => self.sage(format!("{} predated. Analysis complete. No new skills.", e.kind.name())),
            }
            return;
        }
        // Ore and grass tiles.
        let mut best: Option<(i32, i32, Tile, f32)> = None;
        for (x, y) in Level::cells(cx - 20.0, cy - 20.0, 40.0, 40.0) {
            let t = self.lv.get(x, y);
            if matches!(t, Tile::Ore | Tile::Grass) {
                let d = (x as f32 * TS + 8.0 - cx).hypot(y as f32 * TS + 8.0 - cy);
                if best.is_none_or(|b| d < b.3) {
                    best = Some((x, y, t, d));
                }
            }
        }
        if let Some((x, y, t, _)) = best {
            self.lv.set(x, y, Tile::Empty);
            self.collected.insert((self.room, x, y));
            self.sfx.push(Sfx::Eat);
            let (px, py) = (x as f32 * TS + 8.0, y as f32 * TS + 8.0);
            if t == Tile::Ore {
                self.burst(px, py, 14, pal::ORE, 2.0, 0.0);
                self.magicules += 1;
                self.stats.ore += 1;
                if self.magicules.is_multiple_of(3) {
                    self.max_hp += 1;
                    self.hp = self.max_hp;
                    self.sage("Magicules absorbed. Maximum vitality increased!");
                } else if self.stats.ore == 1 {
                    self.sage("Magic ore predated. Every 3 ore increases maximum vitality.");
                }
            } else {
                self.burst(px, py, 14, pal::GRASS, 2.0, 0.0);
                self.hp = self.max_hp;
                self.stats.grass += 1;
                if !self.flags.first_grass {
                    self.flags.first_grass = true;
                    self.sage("Hipokute Grass predated. Analysis: a medicinal herb. Vitality restored.");
                }
            }
            return;
        }
        // Water.
        if self.body.in_water || self.lv.rect_has(self.body.x - 4.0, self.body.y, PW + 8.0, PH + 6.0, Tile::Water) {
            self.body.water = WATER_MAX;
            self.sfx.push(Sfx::Eat);
            self.burst(cx, cy, 12, pal::WATER_HI, 1.5, 0.0);
            if !self.skills.water_jet {
                self.skills.water_jet = true;
                self.sage("Water stored in the stomach. Expel it under pressure: press Shift (+ direction) to [Water Jet].");
                self.say(Who::Rimuru, "Like a squid! Let's blast out of this lake.");
            }
        }
    }

    pub fn hurt(&mut self, mut dmg: i32, from_x: f32) {
        if self.invuln > 0 || self.dead_t > 0 || dmg <= 0 {
            return;
        }
        if self.skills.body_armor && dmg > 1 {
            dmg -= 1;
        }
        self.hp -= dmg;
        self.invuln = 70;
        self.shake = 4.0;
        self.sfx.push(Sfx::Hurt);
        let dir = if self.body.cx() < from_x { -1.0 } else { 1.0 };
        self.body.vx = dir * 3.0;
        self.body.vy = -3.0;
        self.body.jet_t = 0;
        self.body.rope = None;
        let (cx, cy) = (self.body.cx(), self.body.cy());
        self.burst(cx, cy, 10, pal::SLIME, 2.0, 0.1);
        if self.hp <= 0 {
            self.hp = 0;
            self.dead_t = 70;
            self.stats.deaths += 1;
            self.burst(cx, cy, 30, pal::SLIME, 3.0, 0.15);
        }
    }

    fn update_enemies(&mut self) {
        let (cx, cy) = (self.body.cx(), self.body.cy());
        let mut shots = Vec::new();
        let mut contact: Option<(i32, f32)> = None;
        let mut caught: Option<Kind> = None;
        for e in &mut self.enemies {
            let out = e.update(&self.lv, cx, cy);
            shots.extend(out.shots);
            if out.shake > 0.0 {
                self.shake = self.shake.max(out.shake);
            }
            if out.roar {
                self.sfx.push(Sfx::Roar);
            }
            if !e.alive() {
                continue;
            }
            if e.spots(&self.lv, cx, cy) {
                caught = Some(e.kind);
            }
            let dmg = e.kind.contact_damage();
            if dmg > 0 && e.stun == 0 && e.overlaps(self.body.x, self.body.y, PW, PH) {
                contact = Some((dmg, e.x));
            }
            if e.kind == Kind::Serpent && e.awake {
                for (sx, sy) in e.segments() {
                    if (sx - cx).abs() < 12.0 && (sy - cy).abs() < 11.0 {
                        contact = Some((1, sx));
                    }
                }
            }
        }
        self.projs.extend(shots);
        if let Some((d, x)) = contact {
            self.hurt(d, x);
        }
        if let Some(Kind::Adventurer(i)) = caught
            && self.dialogue.is_empty() {
                self.sfx.push(Sfx::Spotted);
                let (who, line) = match i {
                    0 => (Who::Kabal, "Hm? A slime? ...Why is it glowing? Eren, Gido, look sharp!"),
                    1 => (Who::Eren, "Kyaa! A slime! Kabal, it's right there!"),
                    _ => (Who::Gido, "Heh, just a slime. Lemme poke it... hey, come back here!"),
                };
                self.say(who, line);
                self.dialogue.push_back(Beat::Do(Act::Caught));
            }
    }

    fn update_projectiles(&mut self) {
        let (bx, by) = (self.body.x, self.body.y);
        let mut i = 0;
        while i < self.projs.len() {
            let mut p = self.projs[i].clone();
            let mut keep = p.step(&self.lv);
            let r = p.radius();
            match p.kind {
                ProjKind::Blade => {
                    for e in &mut self.enemies {
                        if !keep {
                            break;
                        }
                        if e.alive() && !matches!(e.kind, Kind::Adventurer(_)) && e.overlaps(p.x - r, p.y - r, r * 2.0, r * 2.0)
                        {
                            keep = false;
                            if e.damage(1, p.vx.signum()) {
                                self.sfx.push(Sfx::Hit);
                                let c = if e.dead { pal::GOLD } else { pal::WHITE };
                                let (ex, ey) = (e.x, e.y);
                                self.parts.push(Particle { x: ex, y: ey, vx: 0.0, vy: -0.5, life: 10, max: 10, c, size: 4.0, grav: 0.0 });
                            } else {
                                self.sfx.push(Sfx::Tink);
                            }
                        }
                    }
                    if !keep {
                        self.burst(p.x, p.y, 6, pal::WATER_HI, 1.5, 0.1);
                    }
                }
                ProjKind::Web | ProjKind::Poison => {
                    let hit = p.x + r > bx && p.x - r < bx + PW && p.y + r > by && p.y - r < by + PH;
                    if hit {
                        keep = false;
                        if p.kind == ProjKind::Web {
                            self.body.slowed = 100;
                            self.sfx.push(Sfx::Hit);
                        } else {
                            let d = if self.skills.poison_resist { 1 } else { 2 };
                            self.hurt(d, p.x);
                        }
                    }
                    if !keep {
                        let c = if p.kind == ProjKind::Web { pal::WHITE } else { pal::POISON };
                        self.burst(p.x, p.y, 6, c, 1.2, 0.1);
                    }
                }
            }
            if keep {
                self.projs[i] = p;
                i += 1;
            } else {
                self.projs.remove(i);
            }
        }
        if let Some((x, y, t)) = self.sonic {
            self.sonic = if t < 18 { Some((x, y, t + 1)) } else { None };
        }
    }

    fn update_particles(&mut self) {
        for p in &mut self.parts {
            p.x += p.vx;
            p.y += p.vy;
            p.vy += p.grav;
            p.vx *= 0.96;
            p.life -= 1;
        }
        self.parts.retain(|p| p.life > 0);
    }
}
