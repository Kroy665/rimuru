//! Tiny software renderer. The whole game draws into a low-res RGBA buffer,
//! which the window backend scales up and the headless tools save as PNG.

use font8x8::UnicodeFonts;

pub const W: i32 = 480;
pub const H: i32 = 270;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const fn hex(v: u32) -> Color {
        Color((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        Color(f(self.0, o.0), f(self.1, o.1), f(self.2, o.2))
    }
    pub fn scale(self, k: f32) -> Color {
        let f = |a: u8| (a as f32 * k).clamp(0.0, 255.0) as u8;
        Color(f(self.0), f(self.1), f(self.2))
    }
}

pub mod pal {
    use super::Color;
    pub const BLACK: Color = Color::hex(0x07060d);
    pub const CAVE_BG: Color = Color::hex(0x15122a);
    pub const CAVE_BG2: Color = Color::hex(0x1d1838);
    pub const ROCK: Color = Color::hex(0x3b3566);
    pub const ROCK_HI: Color = Color::hex(0x5a5291);
    pub const ROCK_LO: Color = Color::hex(0x272247);
    pub const WATER: Color = Color::hex(0x1f6fa8);
    pub const WATER_HI: Color = Color::hex(0x5cc8f0);
    pub const SLIME: Color = Color::hex(0x5fc4f5);
    pub const SLIME_HI: Color = Color::hex(0xc8f1ff);
    pub const SLIME_LO: Color = Color::hex(0x2c86c9);
    pub const ORE: Color = Color::hex(0x3ff0d0);
    pub const GRASS: Color = Color::hex(0x69d66b);
    pub const CRYSTAL: Color = Color::hex(0xc78bff);
    pub const WHITE: Color = Color::hex(0xf2f0ff);
    pub const GREY: Color = Color::hex(0x8f8aa8);
    pub const RED: Color = Color::hex(0xe8485a);
    pub const GOLD: Color = Color::hex(0xffd166);
    pub const PURPLE: Color = Color::hex(0x8a4fff);
    pub const POISON: Color = Color::hex(0x9be04a);
    pub const UI_BG: Color = Color::hex(0x0d0b1c);
    pub const UI_EDGE: Color = Color::hex(0x6c63c9);
}

pub struct Frame {
    pub px: Vec<Color>,
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    pub fn new() -> Frame {
        Frame { px: vec![pal::BLACK; (W * H) as usize] }
    }

    pub fn clear(&mut self, c: Color) {
        self.px.fill(c);
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Color) {
        if x >= 0 && y >= 0 && x < W && y < H {
            self.px[(y * W + x) as usize] = c;
        }
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Color {
        self.px[(y.clamp(0, H - 1) * W + x.clamp(0, W - 1)) as usize]
    }

    /// Alpha blend a single pixel.
    pub fn blend(&mut self, x: i32, y: i32, c: Color, a: f32) {
        if x >= 0 && y >= 0 && x < W && y < H {
            let i = (y * W + x) as usize;
            self.px[i] = self.px[i].lerp(c, a);
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(W), (y + h).min(H));
        if x1 <= x0 {
            return;
        }
        for yy in y0..y1 {
            let row = (yy * W) as usize;
            self.px[row + x0 as usize..row + x1 as usize].fill(c);
        }
    }

    pub fn rect_blend(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color, a: f32) {
        for yy in y.max(0)..(y + h).min(H) {
            for xx in x.max(0)..(x + w).min(W) {
                self.blend(xx, yy, c, a);
            }
        }
    }

    pub fn rect_outline(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        self.rect(x, y, w, 1, c);
        self.rect(x, y + h - 1, w, 1, c);
        self.rect(x, y, 1, h, c);
        self.rect(x + w - 1, y, 1, h, c);
    }

    /// Filled ellipse centred on (cx, cy).
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Color) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let y0 = (cy - ry).floor() as i32;
        let y1 = (cy + ry).ceil() as i32;
        for y in y0..=y1 {
            let dy = (y as f32 + 0.5 - cy) / ry;
            if dy.abs() > 1.0 {
                continue;
            }
            let half = rx * (1.0 - dy * dy).sqrt();
            let xa = (cx - half).round() as i32;
            let xb = (cx + half).round() as i32;
            self.rect(xa, y, xb - xa, 1, c);
        }
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Color) {
        self.ellipse(cx, cy, r, r, c);
    }

    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, thick: f32, c: Color) {
        let r0 = (r - thick).max(0.0);
        let x0 = (cx - r - 1.0) as i32;
        let x1 = (cx + r + 1.0) as i32;
        let y0 = (cy - r - 1.0) as i32;
        let y1 = (cy + r + 1.0) as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                if d <= r && d >= r0 {
                    self.set(x, y, c);
                }
            }
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).ceil().max(1.0) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            self.set(
                (x0 + (x1 - x0) * t).round() as i32,
                (y0 + (y1 - y0) * t).round() as i32,
                c,
            );
        }
    }

    pub fn tri(&mut self, p: [(f32, f32); 3], c: Color) {
        let minx = p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor() as i32;
        let maxx = p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil() as i32;
        let miny = p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor() as i32;
        let maxy = p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil() as i32;
        let edge = |a: (f32, f32), b: (f32, f32), x: f32, y: f32| {
            (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)
        };
        for y in miny.max(0)..=maxy.min(H - 1) {
            for x in minx.max(0)..=maxx.min(W - 1) {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let e0 = edge(p[0], p[1], fx, fy);
                let e1 = edge(p[1], p[2], fx, fy);
                let e2 = edge(p[2], p[0], fx, fy);
                if (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0)
                {
                    self.set(x, y, c);
                }
            }
        }
    }

    /// 8x8 bitmap text. Returns the width drawn.
    pub fn text(&mut self, s: &str, x: i32, y: i32, c: Color) -> i32 {
        let mut cx = x;
        for ch in s.chars() {
            if let Some(g) = font8x8::BASIC_FONTS.get(ch) {
                for (row, bits) in g.iter().enumerate() {
                    for col in 0..8 {
                        if bits & (1 << col) != 0 {
                            self.set(cx + col, y + row as i32, c);
                        }
                    }
                }
            }
            cx += 8;
        }
        cx - x
    }

    pub fn text_shadow(&mut self, s: &str, x: i32, y: i32, c: Color) {
        self.text(s, x + 1, y + 1, pal::BLACK);
        self.text(s, x, y, c);
    }

    pub fn text_center(&mut self, s: &str, cx: i32, y: i32, c: Color) {
        let w = s.chars().count() as i32 * 8;
        self.text_shadow(s, cx - w / 2, y, c);
    }

    /// Big text (2x scale).
    pub fn text2(&mut self, s: &str, x: i32, y: i32, c: Color) {
        let mut cx = x;
        for ch in s.chars() {
            if let Some(g) = font8x8::BASIC_FONTS.get(ch) {
                for (row, bits) in g.iter().enumerate() {
                    for col in 0..8 {
                        if bits & (1 << col) != 0 {
                            self.rect(cx + col * 2, y + row as i32 * 2, 2, 2, c);
                        }
                    }
                }
            }
            cx += 16;
        }
    }

    pub fn text2_center(&mut self, s: &str, cx: i32, y: i32, c: Color) {
        let w = s.chars().count() as i32 * 16;
        self.text2(s, cx - w / 2 + 2, y + 2, pal::BLACK);
        self.text2(s, cx - w / 2, y, c);
    }

    /// Write RGBA8 bytes (for texture upload / PNG).
    pub fn to_rgba(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.px.len() * 4);
        for c in &self.px {
            out.extend_from_slice(&[c.0, c.1, c.2, 255]);
        }
    }

    pub fn save_png(&self, path: &str) -> std::io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), W as u32, H as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(std::io::Error::other)?;
        let mut buf = Vec::new();
        self.to_rgba(&mut buf);
        writer.write_image_data(&buf).map_err(std::io::Error::other)?;
        Ok(())
    }
}

/// Wrap text into lines of at most `max` characters.
pub fn wrap(s: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for para in s.split('\n') {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            if !cur.is_empty() && cur.len() + 1 + word.len() > max {
                lines.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
        lines.push(cur);
    }
    lines
}

/// 4x4 Bayer matrix for ordered dithering (values 0..1).
pub fn bayer(x: i32, y: i32) -> f32 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[(y & 3) as usize][(x & 3) as usize] as f32 + 0.5) / 16.0
}

/// Cheap deterministic hash noise.
pub fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}
