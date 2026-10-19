//! Tile maps: parsing from ASCII, collision queries.

pub const TS: f32 = 16.0;
pub const TSI: i32 = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Tile {
    Empty,
    Rock,
    Water,
    /// Breakable by an armored water-jet dash.
    Cracked,
    /// Breakable by Ultrasonic Waves.
    Crystal,
    Spikes,
    /// Magic ore, predate for magicules.
    Ore,
    /// Hipokute grass, predate to heal.
    Grass,
    /// Sealed until the room's goal is done.
    Door,
    Exit,
}

impl Tile {
    pub fn solid(self) -> bool {
        matches!(self, Tile::Rock | Tile::Cracked | Tile::Crystal | Tile::Door)
    }
}

#[derive(Clone, Debug)]
pub struct Spawn {
    pub kind: char,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone)]
pub struct Level {
    pub w: i32,
    pub h: i32,
    pub tiles: Vec<Tile>,
    /// Trigger id per cell (0 = none).
    pub triggers: Vec<u8>,
    pub spawn: (f32, f32),
    pub spawns: Vec<Spawn>,
}

impl Level {
    pub fn parse(src: &str) -> Level {
        let rows: Vec<&str> = src
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        let h = rows.len() as i32;
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let mut lv = Level {
            w,
            h,
            tiles: vec![Tile::Rock; (w * h) as usize],
            triggers: vec![0; (w * h) as usize],
            spawn: (0.0, 0.0),
            spawns: Vec::new(),
        };
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let (x, y) = (x as i32, y as i32);
                let i = (y * w + x) as usize;
                let cx = x as f32 * TS + TS / 2.0;
                let by = (y + 1) as f32 * TS; // bottom of the cell
                lv.tiles[i] = match ch {
                    '#' => Tile::Rock,
                    '~' => Tile::Water,
                    '=' => Tile::Cracked,
                    'C' => Tile::Crystal,
                    '^' => Tile::Spikes,
                    'o' => Tile::Ore,
                    'g' => Tile::Grass,
                    'D' => Tile::Door,
                    'X' => Tile::Exit,
                    _ => Tile::Empty,
                };
                match ch {
                    'P' => lv.spawn = (cx, by),
                    '1'..='9' => lv.triggers[i] = ch as u8 - b'0',
                    'b' | 's' | 'c' | 'a' | 'S' | 'V' | 'k' => {
                        lv.spawns.push(Spawn { kind: ch, x: cx, y: by })
                    }
                    // Enemy standing in water.
                    'w' => {
                        lv.tiles[i] = Tile::Water;
                    }
                    _ => {}
                }
            }
        }
        lv
    }

    pub fn px_w(&self) -> f32 {
        self.w as f32 * TS
    }
    pub fn px_h(&self) -> f32 {
        self.h as f32 * TS
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            Tile::Rock
        } else {
            self.tiles[(y * self.w + x) as usize]
        }
    }

    pub fn set(&mut self, x: i32, y: i32, t: Tile) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.tiles[(y * self.w + x) as usize] = t;
        }
    }

    pub fn trigger(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            0
        } else {
            self.triggers[(y * self.w + x) as usize]
        }
    }

    /// Story triggers act on a vertical band, so a mark a few rows above the
    /// floor still fires when walking underneath it.
    pub fn triggers_near(&self, x: f32, y: f32, w: f32, h: f32) -> Vec<u8> {
        let mut v: Vec<u8> = Self::cells(x, y - 48.0, w, h + 64.0)
            .map(|(cx, cy)| self.trigger(cx, cy))
            .filter(|&t| t != 0)
            .collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn tile_at(&self, px: f32, py: f32) -> Tile {
        self.get((px / TS).floor() as i32, (py / TS).floor() as i32)
    }

    pub fn solid_at(&self, px: f32, py: f32) -> bool {
        self.tile_at(px, py).solid()
    }

    /// Cells overlapped by a rectangle.
    pub fn cells(x: f32, y: f32, w: f32, h: f32) -> impl Iterator<Item = (i32, i32)> {
        let x0 = (x / TS).floor() as i32;
        let x1 = ((x + w - 0.001) / TS).floor() as i32;
        let y0 = (y / TS).floor() as i32;
        let y1 = ((y + h - 0.001) / TS).floor() as i32;
        (y0..=y1).flat_map(move |cy| (x0..=x1).map(move |cx| (cx, cy)))
    }

    pub fn rect_solid(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        Self::cells(x, y, w, h).any(|(cx, cy)| self.get(cx, cy).solid())
    }

    pub fn rect_has(&self, x: f32, y: f32, w: f32, h: f32, t: Tile) -> bool {
        Self::cells(x, y, w, h).any(|(cx, cy)| self.get(cx, cy) == t)
    }

    /// Spikes only hurt in the lower half of their cell.
    pub fn rect_spiked(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        Self::cells(x, y, w, h).any(|(cx, cy)| {
            self.get(cx, cy) == Tile::Spikes && y + h > cy as f32 * TS + 7.0
        })
    }

    /// March a ray until it hits a solid tile. Returns the hit point.
    pub fn raycast(&self, x: f32, y: f32, dx: f32, dy: f32, max: f32) -> Option<(f32, f32)> {
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (ux, uy) = (dx / len, dy / len);
        let mut d = 0.0;
        while d < max {
            let (px, py) = (x + ux * d, y + uy * d);
            if self.solid_at(px, py) {
                return Some((px, py));
            }
            d += 2.0;
        }
        None
    }

    /// Line of sight between two points (no solid tile in between).
    pub fn clear_line(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let dist = (dx * dx + dy * dy).sqrt();
        self.raycast(x0, y0, dx, dy, dist).is_none()
    }

    pub fn open_doors(&mut self) {
        for t in self.tiles.iter_mut() {
            if *t == Tile::Door {
                *t = Tile::Empty;
            }
        }
    }

    /// Replace every tile of one kind (used by the solver to model breakables).
    pub fn replace_all(&mut self, from: Tile, to: Tile) {
        for t in self.tiles.iter_mut() {
            if *t == from {
                *t = to;
            }
        }
    }
}
