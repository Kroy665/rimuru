//! Draws the game into a [`Frame`].

use crate::enemies::{Enemy, Kind, ProjKind, LANTERN_HALF_ANGLE, LANTERN_RANGE};
use crate::game::{Beat, Game, Scene, PROLOGUE};
use crate::gfx::{bayer, hash, pal, wrap, Color, Frame, H, W};
use crate::level::{Tile, TSI};
use crate::player::{PH, WATER_MAX};
use crate::rooms::ROOMS;

struct Light {
    x: f32,
    y: f32,
    r: f32,
    k: f32,
    /// Optional cone: (dir x, half angle).
    cone: Option<(f32, f32)>,
}

impl Game {
    pub fn render(&self, f: &mut Frame) {
        match self.scene {
            Scene::Title => self.draw_title(f),
            Scene::Prologue => self.draw_prologue(f),
            Scene::Play => self.draw_play(f),
            Scene::Ending => self.draw_ending(f),
        }
    }

    fn draw_play(&self, f: &mut Frame) {
        let sh = if self.shake > 0.3 {
            ((self.t as f32 * 2.3).sin() * self.shake, (self.t as f32 * 3.1).cos() * self.shake)
        } else {
            (0.0, 0.0)
        };
        let ox = (self.cam.0 + sh.0).round() as i32;
        let oy = (self.cam.1 + sh.1).round() as i32;

        self.draw_background(f, ox, oy);
        self.draw_tiles(f, ox, oy, false);
        if let Some((vx, vy)) = self.veldora {
            draw_veldora(f, vx - ox as f32, vy - oy as f32, self.t, 1.0);
        } else if self.veldora_fade > 0 {
            // Swirl into the slime.
            let k = self.veldora_fade as f32 / 90.0;
            let (x, y) = (self.body.cx() - ox as f32, self.body.cy() - oy as f32);
            for i in 0..24 {
                let a = i as f32 * 0.8 + self.t as f32 * 0.2;
                let r = 90.0 * k * (1.0 + (i % 3) as f32 * 0.3);
                f.circle(x + a.cos() * r, y + a.sin() * r * 0.7, 2.0 + 3.0 * k, pal::GOLD);
            }
        }
        for e in &self.enemies {
            draw_enemy(f, e, ox, oy, self.t);
        }
        self.draw_slime(f, ox, oy);
        for p in &self.projs {
            let (x, y) = (p.x - ox as f32, p.y - oy as f32);
            match p.kind {
                ProjKind::Blade if p.vx == 0.0 => {
                    for i in -5..=5 {
                        let fi = i as f32;
                        let by = y + (2.5 - fi * fi * 0.12) * p.vy.signum();
                        f.set((x + fi) as i32, by as i32, pal::WHITE);
                        f.set((x + fi) as i32, (by + 1.0) as i32, pal::WATER_HI);
                    }
                }
                ProjKind::Blade => {
                    let d = p.vx.signum();
                    for i in -5..=5 {
                        let fi = i as f32;
                        let bx = x + d * (2.5 - fi * fi * 0.12);
                        f.set(bx as i32, (y + fi) as i32, pal::WHITE);
                        f.set((bx - d) as i32, (y + fi) as i32, pal::WATER_HI);
                    }
                }
                ProjKind::Web => {
                    f.line(x - 3.0, y - 3.0, x + 3.0, y + 3.0, pal::WHITE);
                    f.line(x - 3.0, y + 3.0, x + 3.0, y - 3.0, pal::WHITE);
                    f.line(x - 4.0, y, x + 4.0, y, pal::GREY);
                }
                ProjKind::Poison => {
                    f.circle(x, y, 4.0, pal::POISON);
                    f.circle(x - 1.0, y - 1.0, 1.5, Color::hex(0xe6ffb0));
                }
            }
        }
        for p in &self.parts {
            let s = (p.size * (p.life as f32 / p.max as f32).max(0.3)).ceil() as i32;
            f.rect((p.x - ox as f32) as i32, (p.y - oy as f32) as i32, s, s, p.c);
        }
        if let Some((x, y, t)) = self.sonic {
            let r = 6.0 + t as f32 * 4.2;
            f.ring(x - ox as f32, y - oy as f32, r, 2.0, pal::WHITE);
            f.ring(x - ox as f32, y - oy as f32, r * 0.7, 1.0, pal::CRYSTAL);
        }
        self.draw_tiles(f, ox, oy, true);
        self.apply_lighting(f, ox, oy);
        self.draw_hud(f);
        self.draw_messages(f);

        let fade = if self.fade_out > 0 {
            1.0 - self.fade_out as f32 / 30.0
        } else if self.dead_t > 0 {
            1.0 - self.dead_t as f32 / 70.0
        } else {
            self.fade_in as f32 / 30.0
        };
        if fade > 0.0 {
            let c = if self.room + 1 >= ROOMS.len() && self.fade_out > 0 { pal::WHITE } else { pal::BLACK };
            f.rect_blend(0, 0, W, H, c, fade);
        }
        if self.paused {
            f.rect_blend(0, 0, W, H, pal::BLACK, 0.6);
            f.text2_center("PAUSED", W / 2, 90, pal::WHITE);
            small_text_center(f, "Enter / Esc to resume", W / 2, 120, pal::GREY);
            let controls = [
                "Move: A/D or arrows   Hop: Space/W   Predate: E",
                "Water Jet: Shift+dir   Blade: J (Up+J aims up)",
                "Sonic: K   Thread: L (Up/Down climb, Space lets go)",
            ];
            for (i, c) in controls.iter().enumerate() {
                small_text_center(f, c, W / 2, 150 + i as i32 * 12, pal::WHITE);
            }
        }
        if self.dead_t > 0 && self.dead_t < 50 {
            f.text_center("Body lost... reconstituting", W / 2, H / 2, pal::SLIME_HI);
        }
    }

    fn draw_background(&self, f: &mut Frame, ox: i32, oy: i32) {
        let dark = self.dark();
        let top = if dark { pal::BLACK } else { pal::CAVE_BG };
        let far = if dark { pal::BLACK } else { pal::CAVE_BG2 };
        let px = ox as f32 * 0.4;
        let py = oy as f32 * 0.4;
        for x in 0..W {
            let wx = x as f32 + px;
            let stal = 30.0 + (wx * 0.05).sin() * 14.0 + (wx * 0.13).sin() * 8.0 + ((wx * 0.31).sin() * 6.0).max(0.0) * 3.0;
            let mite = 40.0 + (wx * 0.04 + 2.0).sin() * 16.0 + (wx * 0.17).cos() * 7.0;
            for y in 0..H {
                let wy = y as f32 + py;
                let base = top.lerp(pal::BLACK, y as f32 / H as f32 * 0.5);
                let c = if wy < stal || wy > 260.0 - mite + (oy as f32 * 0.1) {
                    far
                } else {
                    base
                };
                f.px[(y * W + x) as usize] = c;
            }
        }
    }

    fn draw_tiles(&self, f: &mut Frame, ox: i32, oy: i32, front: bool) {
        let tx0 = (ox / TSI - 1).max(0);
        let ty0 = (oy / TSI - 1).max(0);
        let tx1 = ((ox + W) / TSI + 1).min(self.lv.w - 1);
        let ty1 = ((oy + H) / TSI + 1).min(self.lv.h - 1);
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let t = self.lv.get(tx, ty);
                let x = tx * TSI - ox;
                let y = ty * TSI - oy;
                if front {
                    if t == Tile::Water {
                        let surface = self.lv.get(tx, ty - 1) != Tile::Water;
                        f.rect_blend(x, y, TSI, TSI, pal::WATER, 0.55);
                        if surface {
                            for i in 0..TSI {
                                let wy = ((self.t as f32 * 0.08 + (x + i + ox) as f32 * 0.4).sin() * 1.2).round() as i32;
                                f.set(x + i, y + 1 + wy, pal::WATER_HI);
                            }
                        }
                    }
                    continue;
                }
                match t {
                    Tile::Rock | Tile::Cracked => self.draw_rock(f, tx, ty, x, y, t == Tile::Cracked),
                    Tile::Crystal => {
                        f.rect(x, y, TSI, TSI, Color::hex(0x5b2d9e));
                        let h = hash(tx, ty);
                        f.tri([(x as f32 + 2.0, y as f32 + 16.0), (x as f32 + 6.0, y as f32 + 1.0), (x as f32 + 10.0, y as f32 + 16.0)], pal::CRYSTAL);
                        f.tri([(x as f32 + 7.0, y as f32 + 16.0), (x as f32 + 12.0, y as f32 + 4.0 + (h % 4) as f32), (x as f32 + 15.0, y as f32 + 16.0)], Color::hex(0xa66cf0));
                        f.line(x as f32 + 5.0, y as f32 + 4.0, x as f32 + 5.0, y as f32 + 12.0, pal::WHITE);
                    }
                    Tile::Spikes => {
                        for i in 0..3 {
                            let bx = x as f32 + i as f32 * 5.5;
                            f.tri([(bx, y as f32 + 16.0), (bx + 2.7, y as f32 + 6.0), (bx + 5.4, y as f32 + 16.0)], pal::GREY);
                            f.line(bx + 2.7, y as f32 + 7.0, bx + 2.7, y as f32 + 15.0, pal::WHITE);
                        }
                    }
                    Tile::Ore => {
                        let pulse = ((self.t as f32 * 0.08 + tx as f32).sin() * 0.5 + 0.5) * 0.4;
                        let c = pal::ORE.lerp(pal::WHITE, pulse);
                        f.tri([(x as f32 + 2.0, y as f32 + 16.0), (x as f32 + 5.0, y as f32 + 6.0), (x as f32 + 8.0, y as f32 + 16.0)], c);
                        f.tri([(x as f32 + 6.0, y as f32 + 16.0), (x as f32 + 10.0, y as f32 + 3.0), (x as f32 + 14.0, y as f32 + 16.0)], pal::ORE);
                        f.line(x as f32 + 10.0, y as f32 + 6.0, x as f32 + 10.0, y as f32 + 13.0, pal::WHITE);
                    }
                    Tile::Grass => {
                        for i in 0..5 {
                            let bx = x as f32 + 2.0 + i as f32 * 3.0;
                            let sway = (self.t as f32 * 0.05 + i as f32).sin();
                            f.line(bx, y as f32 + 16.0, bx + sway, y as f32 + 7.0 + (i % 2) as f32 * 3.0, pal::GRASS);
                        }
                        f.circle(x as f32 + 5.0, y as f32 + 8.0, 1.5, pal::WHITE);
                        f.circle(x as f32 + 11.0, y as f32 + 10.0, 1.5, pal::GOLD);
                    }
                    Tile::Door => {
                        f.rect(x, y, TSI, TSI, Color::hex(0x2a2440));
                        f.rect(x, y, 2, TSI, pal::ROCK_HI);
                        let glow = ((self.t as f32 * 0.05).sin() * 0.5 + 0.5) * 0.6 + 0.4;
                        let c = pal::ROCK.lerp(pal::GOLD, glow);
                        f.rect(x + 6, y + 4, 4, 2, c);
                        f.rect(x + 7, y + 6, 2, 6, c);
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw_rock(&self, f: &mut Frame, tx: i32, ty: i32, x: i32, y: i32, cracked: bool) {
        let s = |dx: i32, dy: i32| self.lv.get(tx + dx, ty + dy).solid();
        let interior = s(0, -1) && s(0, 1) && s(-1, 0) && s(1, 0);
        let base = if interior { pal::ROCK_LO } else { pal::ROCK };
        f.rect(x, y, TSI, TSI, base);
        let h = hash(tx, ty);
        for i in 0..4 {
            let hx = (h >> (i * 4)) as i32 & 15;
            let hy = (h >> (i * 4 + 16)) as i32 & 15;
            f.set(x + hx, y + hy, if interior { pal::ROCK } else { pal::ROCK_LO });
        }
        if !s(0, -1) {
            f.rect(x, y, TSI, 2, pal::ROCK_HI);
            if h.is_multiple_of(5) {
                f.rect(x + (h % 11) as i32, y - 1, 3, 1, Color::hex(0x4f7a6a));
            }
        }
        if !s(0, 1) {
            f.rect(x, y + TSI - 2, TSI, 2, pal::ROCK_LO);
            if h.is_multiple_of(3) {
                let sx = x as f32 + 3.0 + (h % 9) as f32;
                f.tri([(sx, y as f32 + 16.0), (sx + 2.0, y as f32 + 22.0 + (h % 5) as f32), (sx + 4.0, y as f32 + 16.0)], pal::ROCK);
            }
        }
        if !s(-1, 0) {
            f.rect(x, y, 1, TSI, pal::ROCK_HI);
        }
        if !s(1, 0) {
            f.rect(x + TSI - 1, y, 1, TSI, pal::ROCK_LO);
        }
        if cracked {
            f.rect(x, y, TSI, TSI, Color::hex(0x4a3c5c));
            let c = pal::BLACK;
            f.line(x as f32 + 3.0, y as f32, x as f32 + 7.0, y as f32 + 7.0, c);
            f.line(x as f32 + 7.0, y as f32 + 7.0, x as f32 + 4.0, y as f32 + 15.0, c);
            f.line(x as f32 + 7.0, y as f32 + 7.0, x as f32 + 14.0, y as f32 + 10.0, c);
            f.rect(x, y, TSI, 1, Color::hex(0x6a5a80));
        }
    }

    fn draw_slime(&self, f: &mut Frame, ox: i32, oy: i32) {
        if self.dead_t > 0 {
            return;
        }
        if self.invuln > 0 && (self.invuln / 4) % 2 == 0 {
            return;
        }
        let b = &self.body;
        let (cx, bottom) = (b.cx() - ox as f32, b.y + PH - oy as f32);
        if let Some(r) = b.rope {
            f.line(cx, b.cy() - oy as f32, r.ax - ox as f32, r.ay - oy as f32, Color::hex(0xdedcf0));
        }
        let wob = (self.t as f32 * 0.15).sin() * 0.04;
        let sq = self.squash + wob;
        let w = 8.5 * (1.0 + sq * 0.6);
        let h = 7.0 * (1.0 - sq * 0.6);
        let cy = bottom - h + 0.5;
        f.ellipse(cx, cy + 1.0, w, h, pal::SLIME_LO);
        f.ellipse(cx, cy, w - 0.5, h - 1.0, pal::SLIME);
        f.ellipse(cx - w * 0.35, cy - h * 0.35, w * 0.3, h * 0.25, pal::SLIME_HI);
        let fx = b.facing * 2.0;
        if self.eat_anim > 0 {
            let o = (self.eat_anim as f32 / 16.0 * std::f32::consts::PI).sin();
            f.ellipse(cx + fx * 1.5, cy + 1.0, 3.0 + o * 3.0, 1.0 + o * 3.0, Color::hex(0x103a60));
        }
        let blink = self.t % 180 < 6;
        let ey = (cy - 1.0).round() as i32;
        for ex in [cx - 3.0 + fx, cx + 3.0 + fx] {
            if blink {
                f.rect(ex as i32, ey + 1, 2, 1, Color::hex(0x10284a));
            } else {
                f.rect(ex as i32, ey - 1, 1, 3, Color::hex(0x10284a));
            }
        }
        if self.body.slowed > 0 {
            f.line(cx - w, cy - 2.0, cx + w, cy + 3.0, pal::WHITE);
            f.line(cx - w, cy + 3.0, cx + w, cy - 2.0, pal::WHITE);
        }
    }

    fn lights(&self, ox: i32, oy: i32) -> Vec<Light> {
        let mut v = Vec::new();
        let (sx, sy) = (self.body.cx() - ox as f32, self.body.cy() - oy as f32);
        if self.dark() {
            v.push(Light { x: sx, y: sy, r: 44.0, k: 0.9, cone: None });
        } else {
            v.push(Light { x: sx, y: sy, r: 150.0, k: 0.75, cone: None });
        }
        let tx0 = (ox / TSI - 3).max(0);
        let ty0 = (oy / TSI - 3).max(0);
        let tx1 = ((ox + W) / TSI + 3).min(self.lv.w - 1);
        let ty1 = ((oy + H) / TSI + 3).min(self.lv.h - 1);
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let (x, y) = ((tx * TSI + 8 - ox) as f32, (ty * TSI + 8 - oy) as f32);
                match self.lv.get(tx, ty) {
                    Tile::Ore => v.push(Light { x, y, r: 36.0, k: 0.7, cone: None }),
                    Tile::Crystal => v.push(Light { x, y, r: 26.0, k: 0.5, cone: None }),
                    Tile::Grass => v.push(Light { x, y, r: 18.0, k: 0.35, cone: None }),
                    Tile::Exit => v.push(Light { x, y, r: 70.0, k: 0.6, cone: None }),
                    Tile::Water if self.lv.get(tx, ty - 1) != Tile::Water && (tx + ty) % 3 == 0 => {
                        v.push(Light { x, y, r: 28.0, k: 0.35, cone: None })
                    }
                    _ => {}
                }
            }
        }
        if let Some((vx, vy)) = self.veldora {
            let k = if self.dark() { 0.25 } else { 0.8 };
            v.push(Light { x: vx - ox as f32, y: vy - 40.0 - oy as f32, r: 130.0, k, cone: None });
        }
        for e in &self.enemies {
            if let Kind::Adventurer(_) = e.kind {
                let (lx, ly) = e.lantern();
                let (lx, ly) = (lx - ox as f32, ly - oy as f32);
                v.push(Light { x: lx, y: ly, r: LANTERN_RANGE, k: 1.0, cone: Some((e.facing, LANTERN_HALF_ANGLE)) });
                v.push(Light { x: lx, y: ly, r: 24.0, k: 0.8, cone: None });
            }
        }
        for p in &self.projs {
            if p.kind == ProjKind::Poison {
                v.push(Light { x: p.x - ox as f32, y: p.y - oy as f32, r: 18.0, k: 0.5, cone: None });
            }
        }
        if let Some((x, y, t)) = self.sonic {
            v.push(Light { x: x - ox as f32, y: y - oy as f32, r: 20.0 + t as f32 * 4.0, k: 0.4, cone: None });
        }
        v
    }

    fn apply_lighting(&self, f: &mut Frame, ox: i32, oy: i32) {
        let dark = self.dark();
        let ambient: f32 = if dark { 0.0 } else if self.room == 7 { 0.42 } else { 0.6 };
        let mut lm = vec![ambient; (W * H) as usize];
        if dark {
            // Remembered touch.
            for ty in 0..self.lv.h {
                for tx in 0..self.lv.w {
                    if self.revealed[(ty * self.lv.w + tx) as usize] {
                        let (x0, y0) = (tx * TSI - ox, ty * TSI - oy);
                        for y in y0.max(0)..(y0 + TSI).min(H) {
                            for x in x0.max(0)..(x0 + TSI).min(W) {
                                let i = (y * W + x) as usize;
                                lm[i] = lm[i].max(0.45);
                            }
                        }
                    }
                }
            }
        }
        for l in self.lights(ox, oy) {
            let x0 = ((l.x - l.r) as i32).max(0);
            let x1 = ((l.x + l.r) as i32).min(W - 1);
            let y0 = ((l.y - l.r) as i32).max(0);
            let y1 = ((l.y + l.r) as i32).min(H - 1);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (dx, dy) = (x as f32 + 0.5 - l.x, y as f32 + 0.5 - l.y);
                    let d2 = dx * dx + dy * dy;
                    if d2 > l.r * l.r {
                        continue;
                    }
                    let d = d2.sqrt();
                    if let Some((dir, half)) = l.cone
                        && (dx * dir <= 0.0 || (dy / d.max(0.01)).asin().abs() > half) {
                            continue;
                        }
                    let fall = 1.0 - d / l.r;
                    let i = (y * W + x) as usize;
                    lm[i] += fall * fall.sqrt() * l.k;
                }
            }
        }
        for y in 0..H {
            for x in 0..W {
                let i = (y * W + x) as usize;
                let l = lm[i].min(1.15);
                // Quantise into bands with ordered dithering for a pixel-art look.
                let q = ((l * 8.0 + bayer(x, y) - 0.5).round() / 8.0).max(0.0);
                f.px[i] = f.px[i].scale(q);
            }
        }
    }

    fn draw_hud(&self, f: &mut Frame) {
        // Vitality drops.
        for i in 0..self.max_hp {
            let x = 8.0 + i as f32 * 12.0;
            let full = i < self.hp;
            let c = if full { pal::SLIME } else { Color::hex(0x2a2a40) };
            f.circle(x + 4.0, 13.0, 4.0, c);
            f.tri([(x + 0.5, 12.0), (x + 4.0, 5.0), (x + 7.5, 12.0)], c);
            if full {
                f.set(x as i32 + 2, 11, pal::SLIME_HI);
            }
        }
        if self.skills.water_jet {
            f.rect(8, 21, 62, 6, pal::UI_BG);
            let w = (60.0 * self.body.water / WATER_MAX) as i32;
            f.rect(9, 22, w, 4, pal::WATER_HI);
            f.text_shadow("H2O", 74, 20, pal::WATER_HI);
        }
        if self.magicules > 0 {
            let s = format!("{}", self.magicules);
            f.tri([(10.0, 39.0), (13.0, 31.0), (16.0, 39.0)], pal::ORE);
            f.text_shadow(&s, 20, 32, pal::ORE);
        }
        // Skill bar.
        let sk = &self.skills;
        let list: [(bool, &str, &str); 7] = [
            (sk.predator, "E", "Predate"),
            (sk.water_jet, "Sh", "Jet"),
            (sk.water_blade, "J", "Blade"),
            (sk.ultrasonic, "K", "Sonic"),
            (sk.steel_thread, "L", "Thread"),
            (sk.body_armor, "-", "Armor"),
            (sk.magic_sense, "-", "Sense"),
        ];
        let mut x = 6;
        for (have, key, name) in list {
            if !have {
                continue;
            }
            let w = (name.len() as i32 + 3) * 6 + 8;
            f.rect_blend(x, H - 14, w, 12, pal::UI_BG, 0.7);
            small_text(f, key, x + 3, H - 11, pal::GOLD);
            small_text(f, name, x + 3 + (key.len() as i32 + 1) * 6, H - 11, pal::WHITE);
            x += w + 3;
        }
        // Predate prompt.
        if self.dialogue.is_empty() && self.can_predate_something() && !(self.t / 20).is_multiple_of(3) {
            let (sx, sy) = (self.body.cx() - self.cam.0, self.body.y - self.cam.1 - 14.0);
            small_text(f, "[E]", sx as i32 - 9, sy as i32, pal::GOLD);
        }
        if self.banner > 0 && !self.paused {
            let a = (self.banner as f32 / 40.0).min(1.0);
            let name = ROOMS[self.room].name;
            let y = 104;
            f.rect_blend(0, y - 8, W, 30, pal::BLACK, 0.5 * a);
            let c = pal::BLACK.lerp(pal::WHITE, a);
            f.text2_center(name, W / 2, y, c);
        }
    }

    pub fn can_predate_something(&self) -> bool {
        let (cx, cy) = (self.body.cx(), self.body.cy());
        if let Some((vx, vy)) = self.veldora
            && self.flags.veldora_ready && (vx - cx).abs() < 90.0 && (vy - cy).abs() < 90.0 {
                return true;
            }
        if self.enemies.iter().any(|e| e.dead && (e.x - cx).abs() < 26.0 && (e.y - cy).abs() < 26.0) {
            return true;
        }
        crate::level::Level::cells(cx - 20.0, cy - 20.0, 40.0, 40.0)
            .any(|(x, y)| matches!(self.lv.get(x, y), Tile::Ore | Tile::Grass))
            || (self.body.in_water && self.body.water < WATER_MAX - 1.0)
    }

    fn draw_messages(&self, f: &mut Frame) {
        if let Some(t) = self.toasts.front() {
            let lines = wrap(&t.text, 52);
            let h = lines.len() as i32 * 10 + 18;
            let a = (t.t as f32 / 10.0).min(1.0).min((t.dur - t.t) as f32 / 15.0);
            let y = 36 + ((1.0 - a) * -30.0) as i32;
            let x = W / 2 - 220;
            f.rect_blend(x, y, 440, h, pal::UI_BG, 0.88);
            f.rect_outline(x, y, 440, h, pal::GOLD.scale(0.8));
            small_text(f, "<< Great Sage >>", x + 8, y + 4, pal::GOLD);
            for (i, l) in lines.iter().enumerate() {
                f.text_shadow(l, x + 8, y + 14 + i as i32 * 10, pal::WHITE);
            }
        }
        if let Some(Beat::Say(who, text)) = self.dialogue.front() {
            let shown: String = text.chars().take(self.typed).collect();
            let lines = wrap(&shown, 54);
            let (x, y, w, h) = (12, H - 84, W - 24, 70);
            f.rect_blend(x, y, w, h, pal::UI_BG, 0.92);
            f.rect_outline(x, y, w, h, pal::UI_EDGE);
            let name = self.who_name(*who);
            let nc = match who {
                crate::rooms::Who::Veldora => pal::GOLD,
                crate::rooms::Who::Rimuru => pal::SLIME,
                _ => pal::RED,
            };
            let nw = name.len() as i32 * 8 + 12;
            f.rect(x + 8, y - 6, nw, 12, pal::UI_BG);
            f.rect_outline(x + 8, y - 6, nw, 12, nc);
            f.text(name, x + 14, y - 4, nc);
            for (i, l) in lines.iter().take(5).enumerate() {
                f.text_shadow(l, x + 10, y + 12 + i as i32 * 11, pal::WHITE);
            }
            if self.typed >= text.chars().count() && (self.t / 20).is_multiple_of(2) {
                f.tri([(x as f32 + w as f32 - 18.0, y as f32 + h as f32 - 12.0), (x as f32 + w as f32 - 10.0, y as f32 + h as f32 - 12.0), (x as f32 + w as f32 - 14.0, y as f32 + h as f32 - 7.0)], pal::GOLD);
            }
        }
    }

    fn draw_title(&self, f: &mut Frame) {
        f.clear(pal::CAVE_BG);
        for y in 0..H {
            for x in 0..W {
                let wx = x as f32 + self.t as f32 * 0.2;
                let stal = 40.0 + (wx * 0.05).sin() * 18.0 + (wx * 0.13).sin() * 9.0;
                let mite = 50.0 + (wx * 0.04 + 2.0).sin() * 16.0;
                if (y as f32) < stal || (y as f32) > H as f32 - mite {
                    f.px[(y * W + x) as usize] = pal::CAVE_BG2;
                }
                if (y as f32) < stal * 0.6 || (y as f32) > H as f32 - mite * 0.55 {
                    f.px[(y * W + x) as usize] = pal::ROCK_LO;
                }
            }
        }
        for i in 0..14 {
            let x = (hash(i, 7) % W as u32) as f32;
            let y = (hash(i, 9) % H as u32) as f32;
            let p = ((self.t as f32 * 0.05 + i as f32).sin() * 0.5 + 0.5) * 2.0;
            f.circle(x, y, 1.0 + p, pal::ORE.scale(0.6 + p * 0.2));
        }
        f.text2_center("SLIME CAVE", W / 2, 46, pal::SLIME_HI);
        f.text_center("That Time I Got Reincarnated as a Slime", W / 2, 72, pal::WHITE);
        f.text_center("- a fan game of episode 1 -", W / 2, 84, pal::GREY);
        // Bouncing slime.
        let bounce = ((self.t as f32 * 0.08).sin()).abs();
        let sq = (1.0 - bounce) * 0.4;
        let (cx, base) = (W as f32 / 2.0, 150.0);
        let w = 26.0 * (1.0 + sq);
        let h = 20.0 * (1.0 - sq);
        let cy = base - bounce * 26.0 - h;
        f.ellipse(cx, base + 2.0, 22.0 * (1.0 - bounce * 0.5), 3.0, pal::BLACK);
        f.ellipse(cx, cy + 2.0, w, h, pal::SLIME_LO);
        f.ellipse(cx, cy, w - 1.0, h - 2.0, pal::SLIME);
        f.ellipse(cx - w * 0.35, cy - h * 0.4, w * 0.28, h * 0.22, pal::SLIME_HI);
        f.rect(cx as i32 - 9, cy as i32 - 4, 2, 7, Color::hex(0x10284a));
        f.rect(cx as i32 + 7, cy as i32 - 4, 2, 7, Color::hex(0x10284a));
        if (self.t / 30).is_multiple_of(2) {
            f.text_center("Press ENTER or SPACE", W / 2, 182, pal::GOLD);
        }
        let controls = [
            "Move: A/D or arrows   Hop: Space/W   Predate: E",
            "Water Jet: Shift+dir   Blade: J   Sonic: K   Thread: L",
        ];
        for (i, c) in controls.iter().enumerate() {
            small_text_center(f, c, W / 2, 222 + i as i32 * 10, pal::GREY);
        }
        small_text_center(f, "Fan project. Characters (c) Fuse / Kodansha. Not for sale.", W / 2, 256, Color::hex(0x5a5680));
    }

    fn draw_prologue(&self, f: &mut Frame) {
        f.clear(pal::BLACK);
        let i = self.prologue_line.min(PROLOGUE.len() - 1);
        let a = (self.scene_t as f32 / 40.0).min(1.0);
        let c = pal::BLACK.lerp(if i == 3 { pal::GOLD } else { pal::WHITE }, a);
        let lines = wrap(PROLOGUE[i], 46);
        let y0 = H / 2 - lines.len() as i32 * 6;
        for (k, l) in lines.iter().enumerate() {
            f.text_center(l, W / 2, y0 + k as i32 * 12, c);
        }
        if i == 1 {
            // A single red streak.
            let w = (self.scene_t * 6).min(200);
            f.rect(W / 2 - w / 2, y0 + 34, w, 1, pal::RED.scale(a));
        }
        small_text_center(f, "Space: continue", W / 2, H - 16, pal::GREY.scale(0.6));
    }

    fn draw_ending(&self, f: &mut Frame) {
        for y in 0..H {
            let c = Color::hex(0xfff3c4).lerp(Color::hex(0x7ec8e3), y as f32 / H as f32);
            f.rect(0, y, W, 1, c);
        }
        // Forest silhouette.
        for x in 0..W {
            let h = 50.0 + (x as f32 * 0.07).sin() * 10.0 + ((x as f32 * 0.31).sin() * 8.0).abs();
            f.rect(x, H - h as i32, 1, h as i32, Color::hex(0x1f5a3a));
            let h2 = 25.0 + (x as f32 * 0.11 + 1.0).sin() * 8.0;
            f.rect(x, H - h2 as i32, 1, h2 as i32, Color::hex(0x143d27));
        }
        let a = (self.scene_t as f32 / 60.0).min(1.0);
        let c = Color::hex(0xfff3c4).lerp(Color::hex(0x1b1640), a);
        f.text2_center("THE END...?", W / 2, 30, c);
        let lines = [
            "Rimuru squeezes past the great sealed door.",
            "Sunlight. Wind. The Forest of Jura stretches ahead.",
            "Somewhere inside, Veldora is already complaining.",
        ];
        for (i, l) in lines.iter().enumerate() {
            f.text_center(l, W / 2, 62 + i as i32 * 12, c);
        }
        let s = &self.stats;
        let secs = s.frames / 60;
        let stats = [
            format!("Time        {:02}:{:02}", secs / 60, secs % 60),
            format!("Magic ore   {}", s.ore),
            format!("Herbs       {}", s.grass),
            format!("Predated    {}", s.eaten),
            format!("Deaths      {}", s.deaths),
            format!("Spotted     {}", s.spotted),
        ];
        if self.scene_t > 60 {
            f.rect_blend(W / 2 - 90, 108, 180, 82, pal::UI_BG, 0.8);
            for (i, l) in stats.iter().enumerate() {
                f.text(l, W / 2 - 80, 114 + i as i32 * 12, pal::WHITE);
            }
        }
        // Rimuru on a hill.
        let (cx, base) = (W as f32 / 2.0, H as f32 - 52.0);
        f.ellipse(cx, base - 7.0, 10.0, 8.0, pal::SLIME_LO);
        f.ellipse(cx, base - 8.0, 9.5, 7.0, pal::SLIME);
        f.ellipse(cx - 3.0, base - 11.0, 3.0, 2.0, pal::SLIME_HI);
        f.rect(cx as i32 - 4, base as i32 - 10, 1, 3, Color::hex(0x10284a));
        f.rect(cx as i32 + 3, base as i32 - 10, 1, 3, Color::hex(0x10284a));
        if self.scene_t > 120 && (self.t / 30).is_multiple_of(2) {
            f.text_center("To be continued: the goblin village.  [Enter]", W / 2, 200, pal::GOLD);
        }
    }
}

/// 6px-advance text using the 8x8 font (cropped), for compact UI.
pub fn small_text(f: &mut Frame, s: &str, x: i32, y: i32, c: Color) {
    use font8x8::UnicodeFonts;
    let mut cx = x;
    for ch in s.chars() {
        if let Some(g) = font8x8::BASIC_FONTS.get(ch) {
            for (row, bits) in g.iter().enumerate() {
                for col in 0..7 {
                    if bits & (1 << col) != 0 {
                        f.set(cx + col, y + row as i32, c);
                    }
                }
            }
        }
        cx += 6;
    }
}

pub fn small_text_center(f: &mut Frame, s: &str, cx: i32, y: i32, c: Color) {
    let w = s.chars().count() as i32 * 6;
    small_text(f, s, cx - w / 2, y, c);
}

fn draw_veldora(f: &mut Frame, x: f32, y: f32, t: u64, a: f32) {
    let t = t as f32;
    let body = Color::hex(0x1d2a3a).scale(a.max(0.2));
    let scale = Color::hex(0x2e4560);
    // Wings.
    f.tri([(x - 20.0, y - 60.0), (x - 110.0, y - 120.0), (x - 70.0, y - 30.0)], Color::hex(0x16202c));
    f.tri([(x + 20.0, y - 60.0), (x + 100.0, y - 125.0), (x + 70.0, y - 30.0)], Color::hex(0x16202c));
    // Tail and body.
    f.ellipse(x + 50.0, y - 10.0, 40.0, 10.0, body);
    f.ellipse(x, y - 30.0, 52.0, 30.0, body);
    f.ellipse(x - 5.0, y - 38.0, 36.0, 14.0, scale);
    // Neck and head.
    f.tri([(x - 30.0, y - 40.0), (x - 50.0, y - 85.0), (x - 10.0, y - 50.0)], body);
    f.ellipse(x - 52.0, y - 90.0, 20.0, 13.0, body);
    f.tri([(x - 66.0, y - 92.0), (x - 92.0, y - 84.0), (x - 66.0, y - 82.0)], body);
    f.tri([(x - 45.0, y - 100.0), (x - 30.0, y - 128.0), (x - 38.0, y - 98.0)], pal::GREY);
    f.tri([(x - 55.0, y - 101.0), (x - 50.0, y - 126.0), (x - 47.0, y - 100.0)], pal::GREY);
    let blink = (t as i32) % 240 < 8;
    if !blink {
        f.ellipse(x - 60.0, y - 92.0, 4.0, 2.5, pal::GOLD);
        f.rect((x - 60.0) as i32, (y - 94.0) as i32, 1, 4, pal::BLACK);
    }
    // Claws.
    f.ellipse(x - 30.0, y - 3.0, 12.0, 4.0, body);
    f.ellipse(x + 25.0, y - 3.0, 12.0, 4.0, body);
    // Infinite Prison.
    let r = 120.0 + (t * 0.04).sin() * 3.0;
    for i in 0..64 {
        let ang = i as f32 / 64.0 * std::f32::consts::TAU + t * 0.004;
        let (px, py) = (x - 10.0 + ang.cos() * r, y - 40.0 + ang.sin() * r * 0.62);
        if py < y + 2.0 {
            let c = if i % 8 == 0 { pal::GOLD } else { pal::PURPLE };
            f.rect(px as i32, py as i32, 2, 2, c);
        }
    }
    for i in 0..6 {
        let ang = i as f32 / 6.0 * std::f32::consts::TAU - t * 0.01;
        let (px, py) = (x - 10.0 + ang.cos() * r * 0.8, y - 40.0 + ang.sin() * r * 0.5);
        if py < y {
            f.rect_outline(px as i32 - 3, py as i32 - 3, 7, 7, pal::GOLD);
            f.set(px as i32, py as i32, pal::WHITE);
        }
    }
}

fn draw_enemy(f: &mut Frame, e: &Enemy, ox: i32, oy: i32, t: u64) {
    let (x, y) = (e.x - ox as f32, e.y - oy as f32);
    if x < -140.0 || x > W as f32 + 140.0 || y < -60.0 || y > H as f32 + 60.0 {
        return;
    }
    let flash = e.hurt > 0 && e.hurt % 2 == 0;
    let tint = |c: Color| -> Color {
        if flash {
            pal::WHITE
        } else if e.dead {
            c.lerp(Color::hex(0x555060), 0.6)
        } else {
            c
        }
    };
    let tf = t as f32;
    let fc = e.facing;
    match e.kind {
        Kind::Bat => {
            if e.dead {
                f.ellipse(x, y + 2.0, 6.0, 3.0, tint(Color::hex(0x4a2f5c)));
                return;
            }
            let flap = if e.state == 0 { 0.0 } else { (tf * 0.5).sin() * 5.0 };
            let body = tint(Color::hex(0x4a2f5c));
            f.tri([(x - 2.0, y), (x - 13.0, y - 4.0 + flap), (x - 6.0, y + 5.0)], tint(Color::hex(0x2d1a3d)));
            f.tri([(x + 2.0, y), (x + 13.0, y - 4.0 + flap), (x + 6.0, y + 5.0)], tint(Color::hex(0x2d1a3d)));
            f.ellipse(x, y, 5.0, 5.0, body);
            f.tri([(x - 4.0, y - 3.0), (x - 3.0, y - 8.0), (x - 1.0, y - 4.0)], body);
            f.tri([(x + 4.0, y - 3.0), (x + 3.0, y - 8.0), (x + 1.0, y - 4.0)], body);
            f.set(x as i32 - 2, y as i32 - 1, pal::RED);
            f.set(x as i32 + 2, y as i32 - 1, pal::RED);
        }
        Kind::Spider => {
            if !e.dead {
                let top = e.home.1 - oy as f32 - 8.0;
                f.line(x, top, x, y - 5.0, Color::hex(0xb0aac8));
            }
            let body = tint(Color::hex(0x4a3f66));
            for i in 0..4 {
                let k = (tf * 0.2 + i as f32).sin() * 1.5;
                let ly = y - 2.0 + i as f32 * 2.0;
                f.line(x - 3.0, ly, x - 10.0, ly + 3.0 + k, body);
                f.line(x + 3.0, ly, x + 10.0, ly + 3.0 - k, body);
            }
            f.ellipse(x, y + 1.0, 6.0, 6.0, body);
            f.ellipse(x, y - 4.0, 3.5, 3.0, tint(Color::hex(0x6a5a90)));
            f.set(x as i32 - 2, y as i32 + 1, tint(Color::hex(0xe8485a)));
            f.set(x as i32 + 2, y as i32 + 1, tint(Color::hex(0xe8485a)));
            if !e.dead {
                for i in 0..3 {
                    f.set(x as i32 - 2 + i * 2, y as i32 - 5, pal::RED);
                }
            }
        }
        Kind::Centipede => {
            let body = tint(Color::hex(0x8a3b2e));
            for i in 0..6 {
                let sx = x - fc * (i as f32 * 4.5 - 10.0);
                let wob = if e.dead { 0.0 } else { (tf * 0.3 + i as f32).sin() };
                f.ellipse(sx, y + 1.0, 3.5, 3.5, body);
                f.line(sx, y + 3.0, sx - 2.0 + wob, y + 6.0, tint(Color::hex(0x3a1a14)));
                f.line(sx, y + 3.0, sx + 2.0 - wob, y + 6.0, tint(Color::hex(0x3a1a14)));
            }
            let hx = x + fc * 14.0;
            f.ellipse(hx, y, 4.5, 4.0, tint(Color::hex(0xb04c3a)));
            f.line(hx, y - 3.0, hx + fc * 5.0, y - 7.0, body);
            if !e.dead {
                f.set(hx as i32 + fc as i32 * 2, y as i32 - 1, pal::GOLD);
            }
        }
        Kind::Saurus => {
            let body = tint(Color::hex(0x6b5b3e));
            let armor = tint(Color::hex(0x9c9aa8));
            let shake = if e.state == 1 { ((tf * 1.5).sin() * 1.0).round() } else { 0.0 };
            let x = x + shake;
            f.ellipse(x - fc * 14.0, y + 4.0, 10.0, 4.0, body);
            f.ellipse(x, y + 1.0, 15.0, 9.0, body);
            for i in 0..4 {
                let sx = x - fc * 8.0 + fc * i as f32 * 5.0;
                f.tri([(sx - 3.0, y - 6.0), (sx, y - 12.0), (sx + 3.0, y - 6.0)], armor);
            }
            f.ellipse(x + fc * 15.0, y - 1.0, 8.0, 6.0, armor);
            f.rect((x + fc * 13.0) as i32 - 1, y as i32 + 6, 4, 4, body);
            f.rect((x - fc * 8.0) as i32 - 1, y as i32 + 6, 4, 4, body);
            if !e.dead {
                f.set((x + fc * 18.0) as i32, y as i32 - 3, if e.state >= 1 { pal::RED } else { pal::GOLD });
            }
            if e.stun > 0 {
                stars(f, x, y - 16.0, tf);
            }
        }
        Kind::Serpent => {
            let body = tint(Color::hex(0x2f6b4f));
            let belly = tint(Color::hex(0x8fbf6a));
            let segs: Vec<(f32, f32)> = e.segments().collect();
            for (i, (sx, sy)) in segs.iter().enumerate().rev() {
                let r = 11.0 - i as f32 * 0.45;
                let (sx, sy) = (sx - ox as f32, sy - oy as f32);
                f.circle(sx, sy, r, body);
                f.circle(sx, sy + 2.0, r * 0.5, belly);
                if i % 2 == 0 {
                    f.set(sx as i32, sy as i32 - (r as i32) + 2, Color::hex(0x5fd18c));
                }
            }
            let dir = if e.vx.abs() > 0.1 { e.vx.signum() } else { fc };
            f.ellipse(x, y, 15.0, 11.0, body);
            f.ellipse(x + dir * 10.0, y + 3.0, 10.0, 6.0, body);
            f.ellipse(x + dir * 5.0, y + 6.0, 9.0, 3.0, belly);
            f.tri([(x - dir * 4.0, y - 8.0), (x - dir * 12.0, y - 18.0), (x + dir * 2.0, y - 10.0)], Color::hex(0x1f4a36));
            if !e.dead {
                f.rect((x + dir * 7.0) as i32 - 1, y as i32 - 5, 4, 3, pal::GOLD);
                f.rect((x + dir * 8.0) as i32, y as i32 - 5, 1, 3, pal::BLACK);
                if e.state == 2 || e.state == 4 {
                    f.ellipse(x + dir * 16.0, y + 4.0, 5.0, 4.0, Color::hex(0x6b1020));
                }
                if (t / 10).is_multiple_of(3) {
                    f.line(x + dir * 19.0, y + 3.0, x + dir * 26.0, y + 4.0, pal::RED);
                }
            }
            if e.state == 1 && (t / 4).is_multiple_of(2) && !e.dead {
                f.text("!", x as i32 - 3, y as i32 - 30, pal::RED);
            }
            if e.stun > 0 {
                stars(f, x, y - 16.0, tf);
            }
            if e.awake && !e.dead {
                // Boss bar.
                let w = 200;
                let fill = (w as f32 * e.hp as f32 / e.kind.max_hp() as f32) as i32;
                f.rect(W / 2 - w / 2 - 1, 18, w + 2, 7, pal::BLACK);
                f.rect(W / 2 - w / 2, 19, fill, 5, pal::POISON);
                small_text_center(f, "TEMPEST SERPENT", W / 2, 28, pal::WHITE);
            }
        }
        Kind::Adventurer(i) => {
            let (cloth, hair) = match i {
                0 => (Color::hex(0x7a5232), Color::hex(0x3b2a1a)),
                1 => (Color::hex(0x3f6fb5), Color::hex(0xe6c36a)),
                _ => (Color::hex(0x4f6b3a), Color::hex(0x2a2a2a)),
            };
            let step = if e.state == 0 { ((tf * 0.2).sin() * 2.0).round() } else { 0.0 };
            f.rect(x as i32 - 4, y as i32 + 6, 3, 8 + step as i32, Color::hex(0x2a2238));
            f.rect(x as i32 + 1, y as i32 + 6, 3, 8 - step as i32, Color::hex(0x2a2238));
            f.rect(x as i32 - 5, y as i32 - 6, 10, 13, cloth);
            f.circle(x, y - 10.0, 4.5, Color::hex(0xf0c8a0));
            f.rect(x as i32 - 5, y as i32 - 15, 10, 4, hair);
            f.set((x + fc * 2.0) as i32, y as i32 - 10, pal::BLACK);
            if i == 0 {
                f.line(x - fc * 4.0, y - 8.0, x - fc * 9.0, y + 6.0, pal::GREY);
            }
            let (lx, ly) = e.lantern();
            f.rect((lx - ox as f32) as i32 - 2, (ly - oy as f32) as i32 - 2, 5, 5, pal::GOLD);
            f.rect_outline((lx - ox as f32) as i32 - 3, (ly - oy as f32) as i32 - 3, 7, 7, Color::hex(0x5a4a2a));
        }
    }
}

fn stars(f: &mut Frame, x: f32, y: f32, t: f32) {
    for i in 0..3 {
        let a = t * 0.15 + i as f32 * 2.1;
        f.rect((x + a.cos() * 9.0) as i32, (y + a.sin() * 3.0) as i32, 2, 2, pal::GOLD);
    }
}
