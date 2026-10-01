//! Small animated ASCII toys, one per section, each with a click interaction:
//!
//! * About: a North Devon lighthouse sweeping its beam (click: foghorn)
//! * Experience: a train streaming network traffic to an on-train IDS (click:
//!   inject a malformed frame and watch it get flagged). The fault is a
//!   generic, made-up one: no real product's detection rules are depicted.
//! * Skills: Ferris the Rust crab calling out skills (click: jump)
//! * Education: Conway's Game of Life (click: drop a glider)
//!
//! Projects reuses the sailboat nav chart from `sail`.

use ratzilla::ratatui::{buffer::Buffer, prelude::*};

use crate::{NORD0, NORD3, NORD4, NORD6, TEAL, generated_content as gc, sail, sea};

const YELLOW: Color = Color::Rgb(235, 203, 139); // Nord13
const RED: Color = Color::Rgb(191, 97, 106); // Nord11
const GREEN: Color = Color::Rgb(163, 190, 140); // Nord14
const ORANGE: Color = Color::Rgb(208, 135, 112); // Nord12
const RUST: Color = Color::Rgb(222, 165, 132);

/// Which toy a click region belongs to.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Toy {
    Lighthouse,
    Train,
    Ferris,
    Life,
}

pub struct Toys {
    pub sail: sail::Sim,
    lighthouse: Lighthouse,
    train: Train,
    ferris: Ferris,
    life: Life,
    t: f64,
}

impl Toys {
    pub fn new() -> Self {
        Self {
            sail: sail::Sim::new(),
            lighthouse: Lighthouse::default(),
            train: Train::new(),
            ferris: Ferris::new(),
            life: Life::new(),
            t: 0.0,
        }
    }

    /// Advance everything. `ambient` is false under prefers-reduced-motion,
    /// which freezes the decorative toys (the sailboat, being opt-in, sails on).
    pub fn step(&mut self, dt: f64, ambient: bool) {
        self.sail.step(dt);
        if !ambient {
            return;
        }
        self.t += dt;
        self.train.step(dt, self.t);
        self.ferris.step(dt, self.t);
        self.life.step(dt);
    }

    /// A click at `(col, row)` relative to the toy's area.
    pub fn poke(&mut self, toy: Toy, col: u16, row: u16) {
        match toy {
            Toy::Lighthouse => self.lighthouse.foghorn_until = self.t + 1.8,
            Toy::Train => self.train.fault_queued = true,
            Toy::Ferris => self.ferris.poke(self.t),
            Toy::Life => self.life.glider(col, row),
        }
    }

    pub fn render(&mut self, toy: Toy, buf: &mut Buffer, area: Rect) {
        match toy {
            Toy::Lighthouse => self.lighthouse.render(buf, area, self.t),
            Toy::Train => self.train.render(buf, area, self.t),
            Toy::Ferris => self.ferris.render(buf, area, self.t),
            Toy::Life => self.life.render(buf, area),
        }
    }
}

/// Write `text` at `(x, y)` relative to `area`, one char per cell, clipped.
fn put(buf: &mut Buffer, area: Rect, x: i32, y: i32, text: &str, style: Style) {
    if y < 0 || y >= i32::from(area.height) {
        return;
    }
    for (i, ch) in text.chars().enumerate() {
        let cx = x + i as i32;
        if ch == '\0' || cx < 0 || cx >= i32::from(area.width) {
            continue;
        }
        let mut s = [0u8; 4];
        buf[(area.x + cx as u16, area.y + y as u16)]
            .set_symbol(ch.encode_utf8(&mut s))
            .set_style(style);
    }
}

fn fg(c: Color) -> Style {
    Style::default().fg(c).bg(NORD0)
}

/// Linear blend between two RGB colours (`k` = 0 gives `a`).
fn mix(a: Color, b: Color, k: f64) -> Color {
    let (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) = (a, b) else {
        return a;
    };
    let l = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * k.clamp(0.0, 1.0)) as u8;
    Color::Rgb(l(ar, br), l(ag, bg), l(ab, bb))
}

/// Deterministic per-cell noise in `0.0..1.0`.
fn hash(x: i32, y: i32) -> f64 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    f64::from(h & 0xFFFF) / 65536.0
}

/// Tiny xorshift PRNG; good enough for seeding toys.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn chance(&mut self, p: f64) -> bool {
        (self.next() % 10_000) as f64 / 10_000.0 < p
    }
}

// Lighthouse -------------------------------------------------------------------

#[derive(Default)]
struct Lighthouse {
    foghorn_until: f64,
}

impl Lighthouse {
    const ART: [&'static str; 9] = [
        "   ▄▄▄   ",
        "  ▐ ◉ ▌  ",
        "  ▀███▀  ",
        "   ███   ",
        "   ███   ",
        "   ███   ",
        "  ▐███▌  ",
        " ▄█████▄ ",
        "▟███████▙",
    ];

    fn render(&self, buf: &mut Buffer, area: Rect, t: f64) {
        let (w, h) = (i32::from(area.width), i32::from(area.height));
        // Night sky: sparse stars, each twinkling on its own phase.
        for y in 0..h - 3 {
            for x in 0..w {
                let n = hash(x, y);
                if n < 0.025 {
                    let lit = (t * (0.6 + n * 20.0) + n * 100.0).sin() > 0.2;
                    let (ch, c) = if n < 0.006 {
                        ("✦", NORD4)
                    } else {
                        ("·", NORD3)
                    };
                    put(
                        buf,
                        area,
                        x,
                        y,
                        ch,
                        fg(if lit { c } else { mix(c, NORD0, 0.6) }),
                    );
                }
            }
        }
        // Sea along the bottom two rows, borrowing the background's swell.
        let sea_area = Rect::new(
            area.x,
            area.y + area.height.saturating_sub(2),
            area.width,
            2.min(area.height),
        );
        sea::render(buf, sea_area, t * 1.5);

        let art_h = Self::ART.len() as i32;
        let (ax, ay) = (w / 2 - 4, h - 2 - art_h);
        for (i, line) in Self::ART.iter().enumerate() {
            let y = ay + i as i32;
            // Red and white bands down the tower.
            let colour = match i {
                0..=2 => NORD4,
                3 | 5 => RED,
                7 | 8 => NORD3,
                _ => NORD6,
            };
            put(buf, area, ax, y, line, fg(colour));
        }

        // The beam rotates; seen side-on it's a cone whose reach is the
        // projection of the rotation, and head-on the lamp flares instead.
        let theta = t * 1.1;
        let (lamp_x, lamp_y) = (ax + 4, ay + 1);
        let side = theta.cos();
        let reach = (f64::from(w / 2 - 6) * side.abs()) as i32;
        let dir = if side >= 0.0 { 1 } else { -1 };
        for d in 1..=reach {
            let x = lamp_x + dir * (2 + d);
            let fade = 0.15 + 0.75 * f64::from(d) / f64::from(reach.max(1));
            let spread = (d / 7).min(2);
            for dy in -spread..=spread {
                let (ch, k) = if dy == 0 {
                    ("━", fade)
                } else if dy.abs() == spread {
                    ("·", fade + 0.15)
                } else {
                    ("─", fade + 0.08)
                };
                put(buf, area, x, lamp_y + dy, ch, fg(mix(YELLOW, NORD0, k)));
            }
        }
        let facing = theta.sin() > 0.9;
        put(
            buf,
            area,
            lamp_x,
            lamp_y,
            if facing { "✺" } else { "◉" },
            fg(if facing { NORD6 } else { YELLOW }).bold(),
        );

        if t < self.foghorn_until {
            put(
                buf,
                area,
                (w / 2 - 6).max(0),
                1,
                "♪ BWAAAMP ♪",
                fg(NORD4).bold(),
            );
        }
    }
}

// Train + IDS ------------------------------------------------------------------

struct Packet {
    x: f64,
    car: i32,
    bad: bool,
}

/// One line of the IDS event log, newest first.
struct LogLine {
    text: String,
    bad: bool,
}

struct Train {
    x: f64,
    packets: Vec<Packet>,
    spawn_timer: f64,
    fault_queued: bool,
    seen: u32,
    faults: u32,
    alert_until: f64,
    log: std::collections::VecDeque<LogLine>,
    rng: Rng,
    width: i32,
}

impl Train {
    const CAR: [&'static str; 3] = ["┌──────┐", "│▫ ▫ ▫ │", "└◦────◦┘"];
    const LOCO: [&'static str; 3] = ["┌─────╮ ", "│▫ ▫  ╰╮", "└◦───◦─┘"];
    const CARS: i32 = 3;
    const LEN: i32 = Self::CARS * 9 + 8;
    const IDS_X: i32 = 8;

    fn new() -> Self {
        Self {
            x: 4.0,
            packets: Vec::new(),
            spawn_timer: 0.0,
            fault_queued: false,
            seen: 0,
            faults: 0,
            alert_until: 0.0,
            log: std::collections::VecDeque::new(),
            rng: Rng(0x2545_F491_4F6C_DD1D),
            width: 80,
        }
    }

    fn step(&mut self, dt: f64, t: f64) {
        self.x += 9.0 * dt;
        if self.x > f64::from(self.width) {
            self.x = -f64::from(Self::LEN);
        }
        // Each car publishes process data a few times a second.
        self.spawn_timer += dt;
        if self.spawn_timer > 0.22 {
            self.spawn_timer = 0.0;
            let car = (self.rng.next() % Self::CARS as u64) as i32;
            let px = self.x + f64::from(car * 9 + 4);
            let on_screen = px > f64::from(Self::IDS_X) && px < f64::from(self.width);
            let start = if on_screen {
                px
            } else {
                f64::from(self.width - 1)
            };
            let bad = std::mem::take(&mut self.fault_queued);
            if on_screen || bad {
                self.packets.push(Packet { x: start, car, bad });
            }
        }
        for p in &mut self.packets {
            p.x -= 28.0 * dt;
        }
        let ids = f64::from(Self::IDS_X);
        for p in self.packets.iter().filter(|p| p.x <= ids) {
            self.seen += 1;
            let line = if p.bad {
                self.faults += 1;
                self.alert_until = t + 3.0;
                LogLine {
                    text: format!(
                        "⚠ car {}  frame {:>5}  malformed  ALERT",
                        p.car + 1,
                        self.seen
                    ),
                    bad: true,
                }
            } else {
                LogLine {
                    text: format!("car {}  frame {:>5}  ok", p.car + 1, self.seen),
                    bad: false,
                }
            };
            self.log.push_front(line);
        }
        self.log.truncate(64);
        self.packets.retain(|p| p.x > ids);
    }

    fn render(&mut self, buf: &mut Buffer, area: Rect, t: f64) {
        self.width = i32::from(area.width);
        let h = i32::from(area.height);
        let track = h - 1;
        let train_top = track - 3;
        let bus = train_top - 1;

        // Status line.
        if t < self.alert_until {
            put(buf, area, 1, 0, "⚠ ALERT: malformed frame", fg(RED).bold());
        } else {
            put(buf, area, 1, 0, "✓ traffic nominal", fg(GREEN));
        }
        let counts = format!("frames {:>5}  ·  alerts {}", self.seen, self.faults);
        put(buf, area, 1, 1, &counts, fg(NORD3));

        // Event log in the space between the status and the train, fading
        // with age; alerts stay red.
        let log_rows = (bus - 3).max(0) as usize;
        for (i, line) in self.log.iter().take(log_rows).enumerate() {
            let colour = if line.bad {
                RED
            } else {
                mix(NORD3, NORD0, 0.15 + 0.7 * i as f64 / log_rows.max(1) as f64)
            };
            put(buf, area, 1, 3 + i as i32, &line.text, fg(colour));
        }

        // The IDS tap the packets flow into.
        let ids_colour = if t < self.alert_until && (t * 6.0).sin() > 0.0 {
            RED
        } else {
            TEAL
        };
        put(buf, area, 0, bus, "[IDS]◂", fg(ids_colour).bold());
        for x in Self::IDS_X..i32::from(area.width) {
            if x % 3 == 0 {
                put(buf, area, x, bus, "·", fg(mix(NORD3, NORD0, 0.5)));
            }
        }
        for p in &self.packets {
            let (ch, c) = if p.bad { ("◆", RED) } else { ("•", TEAL) };
            put(buf, area, p.x as i32, bus, ch, fg(c));
        }

        // Track with sleepers.
        for x in 0..i32::from(area.width) {
            put(
                buf,
                area,
                x,
                track,
                if x % 4 == 0 { "╪" } else { "═" },
                fg(NORD3),
            );
        }

        // Cars then locomotive, coupled on the middle row.
        let x0 = self.x as i32;
        for row in 0..3 {
            for car in 0..Self::CARS {
                let cx = x0 + car * 9;
                put(
                    buf,
                    area,
                    cx,
                    train_top + row,
                    Self::CAR[row as usize],
                    fg(NORD4),
                );
                if row == 1 {
                    put(buf, area, cx + 8, train_top + row, "═", fg(NORD3));
                }
            }
            put(
                buf,
                area,
                x0 + Self::CARS * 9,
                train_top + row,
                Self::LOCO[row as usize],
                fg(YELLOW),
            );
        }
    }
}

// Ferris -----------------------------------------------------------------------

struct Ferris {
    words: Vec<&'static str>,
    word: usize,
    say_until: f64,
    next_say: f64,
    jump_start: f64,
    x: f64,
}

impl Ferris {
    const BODY: [&'static str; 3] = ["    _~^~^~_    ", "\\) /  o o  \\ (/", "  '_   -   _'  "];
    const LEGS: [&'static str; 2] = ["  / '-----' \\  ", "  \\ '-----' /  "];

    fn new() -> Self {
        Self {
            words: skill_words(),
            word: 0,
            say_until: 0.0,
            next_say: 1.5,
            jump_start: -10.0,
            x: 0.0,
        }
    }

    fn step(&mut self, _dt: f64, t: f64) {
        if t >= self.next_say {
            self.say(t);
        }
        // Crabs walk sideways.
        self.x = 0.5 + 0.5 * (t * 0.5).sin();
    }

    fn say(&mut self, t: f64) {
        if !self.words.is_empty() {
            self.word = (self.word + 1) % self.words.len();
        }
        self.say_until = t + 2.5;
        self.next_say = t + 5.0;
    }

    fn poke(&mut self, t: f64) {
        self.jump_start = t;
        self.say(t);
    }

    fn render(&self, buf: &mut Buffer, area: Rect, t: f64) {
        let (w, h) = (i32::from(area.width), i32::from(area.height));
        let art_w = 15;
        let x = 1 + (self.x * f64::from((w - art_w - 2).max(0))) as i32;
        // A half-second hop after a click.
        let since = t - self.jump_start;
        let hop = if (0.0..0.5).contains(&since) {
            (since / 0.5 * std::f64::consts::PI).sin() * 3.0
        } else {
            0.0
        } as i32;
        let base = h - 5 - hop;

        // Sand.
        for sx in 0..w {
            let n = hash(sx, 7);
            put(
                buf,
                area,
                sx,
                h - 1,
                if n < 0.3 { "·" } else { "▁" },
                fg(mix(ORANGE, NORD0, 0.7)),
            );
        }

        let walking = (t * 0.5).cos().abs() > 0.15;
        let legs = Self::LEGS[usize::from(walking && (t * 6.0) as i64 % 2 == 0)];
        let blink = (t % 4.0) < 0.15;
        for (i, line) in Self::BODY.iter().chain(std::iter::once(&legs)).enumerate() {
            let line = if i == 1 && blink {
                line.replace("o o", "- -")
            } else {
                line.to_string()
            };
            put(buf, area, x, base + i as i32, &line, fg(RUST).bold());
        }

        if t < self.say_until
            && let Some(word) = self.words.get(self.word)
        {
            let bubble = format!(" {word}! ");
            let bw = bubble.chars().count() as i32;
            let bx = (x + art_w / 2 - bw / 2).clamp(0, (w - bw).max(0));
            let by = base - 3;
            let edge = "─".repeat(bw as usize);
            put(buf, area, bx, by, &format!("╭{edge}╮"), fg(NORD3));
            put(buf, area, bx, by + 1, "│", fg(NORD3));
            put(buf, area, bx + 1, by + 1, &bubble, fg(NORD6).bold());
            put(buf, area, bx + bw + 1, by + 1, "│", fg(NORD3));
            put(buf, area, bx, by + 2, &format!("╰{edge}╯"), fg(NORD3));
            put(buf, area, x + art_w / 2, by + 2, "┬", fg(NORD3));
        }
    }
}

/// Individual skills from the skills content: items split at top-level
/// commas with any parenthetical detail dropped ("Rust (Tokio, …)" → "Rust").
fn skill_words() -> Vec<&'static str> {
    let mut words = Vec::new();
    for cat in gc::SKILLS
        .iter()
        .filter(|c| !c.name.starts_with("Previously"))
    {
        let (mut depth, mut start) = (0, 0);
        let items = cat.items;
        for (i, ch) in items
            .char_indices()
            .chain(std::iter::once((items.len(), ',')))
        {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    let item = items[start..i].split(" (").next().unwrap_or("").trim();
                    if !item.is_empty() && item.len() <= 22 {
                        words.push(item);
                    }
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    words
}

// Game of Life -----------------------------------------------------------------

struct Life {
    w: usize,
    /// Rows of cells; each terminal row shows two using half blocks.
    h: usize,
    cells: Vec<bool>,
    prev2: Vec<bool>,
    prev1: Vec<bool>,
    stale: u32,
    timer: f64,
    age: f64,
    rng: Rng,
}

impl Life {
    fn new() -> Self {
        Self {
            w: 0,
            h: 0,
            cells: Vec::new(),
            prev2: Vec::new(),
            prev1: Vec::new(),
            stale: 0,
            timer: 0.0,
            age: 0.0,
            rng: Rng(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.seed();
    }

    fn seed(&mut self) {
        self.cells = (0..self.w * self.h)
            .map(|_| self.rng.chance(0.28))
            .collect();
        self.prev1.clear();
        self.prev2.clear();
        self.stale = 0;
        self.age = 0.0;
    }

    fn step(&mut self, dt: f64) {
        if self.w == 0 {
            return;
        }
        self.timer += dt;
        self.age += dt;
        if self.timer < 0.12 {
            return;
        }
        self.timer = 0.0;
        let (w, h) = (self.w as i64, self.h as i64);
        let next: Vec<bool> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| {
                // Wrap around the edges (a torus).
                let n = (-1..=1)
                    .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| (dx, dy) != (0, 0))
                    .filter(|&(dx, dy)| {
                        self.cells[(((y + dy + h) % h) * w + (x + dx + w) % w) as usize]
                    })
                    .count();
                let alive = self.cells[(y * w + x) as usize];
                n == 3 || (alive && n == 2)
            })
            .collect();
        // Reseed once it settles into stills or blinkers, or after a while.
        if next == self.prev1 || next == self.prev2 {
            self.stale += 1;
        } else {
            self.stale = 0;
        }
        self.prev2 = std::mem::replace(&mut self.prev1, std::mem::replace(&mut self.cells, next));
        if self.stale > 25 || self.age > 90.0 {
            self.seed();
        }
    }

    /// Drop a glider at a terminal cell, heading down-right.
    fn glider(&mut self, col: u16, row: u16) {
        if self.w < 3 || self.h < 3 {
            return;
        }
        let (cx, cy) = (usize::from(col), usize::from(row) * 2);
        for (dx, dy) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
            let (x, y) = ((cx + dx) % self.w, (cy + dy) % self.h);
            self.cells[y * self.w + x] = true;
        }
        self.stale = 0;
    }

    fn render(&mut self, buf: &mut Buffer, area: Rect) {
        let (w, h) = (usize::from(area.width), usize::from(area.height) * 2);
        if (w, h) != (self.w, self.h) {
            self.resize(w, h);
        }
        let style = fg(GREEN);
        for row in 0..area.height {
            for col in 0..area.width {
                let top = self.cells[usize::from(row) * 2 * w + usize::from(col)];
                let bottom = self.cells[(usize::from(row) * 2 + 1) * w + usize::from(col)];
                let ch = match (top, bottom) {
                    (true, true) => "█",
                    (true, false) => "▀",
                    (false, true) => "▄",
                    (false, false) => " ",
                };
                buf[(area.x + col, area.y + row)]
                    .set_symbol(ch)
                    .set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_words_split_at_top_level() {
        let words = skill_words();
        assert!(words.contains(&"Rust"));
        assert!(words.contains(&"C++"));
        assert!(!words.iter().any(|w| w.contains('(') || w.contains(')')));
    }

    #[test]
    fn blinker_oscillates() {
        let mut life = Life::new();
        life.w = 5;
        life.h = 5;
        life.cells = vec![false; 25];
        for x in 1..=3 {
            life.cells[2 * 5 + x] = true;
        }
        life.step(0.2);
        let vertical: Vec<usize> = (0..25).filter(|&i| life.cells[i]).collect();
        assert_eq!(vertical, vec![7, 12, 17]);
    }

    #[test]
    fn bad_frame_raises_an_alert() {
        let mut train = Train::new();
        train.width = 60;
        train.fault_queued = true;
        let mut t = 0.0;
        while t < 10.0 && train.faults == 0 {
            t += 0.05;
            train.step(0.05, t);
        }
        assert_eq!(train.faults, 1);
        assert!(train.alert_until > t);
    }
}
