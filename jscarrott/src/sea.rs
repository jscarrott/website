//! Animated sea-swell background, drawn under every screen.
//!
//! Two sine swells (a long one rolling left to right and a slower cross-swell)
//! are summed per cell; only cells near a crest get a glyph, so the sea reads as
//! sparse, drifting bands of `~` rather than a solid texture. Colours sit just
//! above the background so the content panels on top stay easy to read.

use ratzilla::ratatui::{buffer::Buffer, layout::Rect, style::Color};

use crate::NORD0;

const CREST: Color = Color::Rgb(67, 76, 94); // Nord2
const SWELL: Color = Color::Rgb(59, 66, 82); // Nord1

/// Fill `area` with the sea at time `t` (seconds).
pub fn render(buf: &mut Buffer, area: Rect, t: f64) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let (symbol, fg) = cell(x, y, t);
            buf[(x, y)].set_symbol(symbol).set_fg(fg).set_bg(NORD0);
        }
    }
}

fn cell(x: u16, y: u16, t: f64) -> (&'static str, Color) {
    let (xf, yf) = (f64::from(x), f64::from(y));
    // The row-dependent phase bends each crest so bands don't line up vertically.
    let swell = (xf * 0.16 - t * 0.9 + (yf * 0.55).sin() * 2.0).sin();
    let cross = (xf * 0.05 + yf * 0.8 + t * 0.35).sin();
    let height = swell * 0.7 + cross * 0.3;
    // Fixed per-cell noise thins out the lower parts of each band.
    let noise = hash(x, y);
    if height > 0.82 {
        ("~", CREST)
    } else if height > 0.6 && noise < 0.5 {
        ("~", SWELL)
    } else if height > 0.35 && noise < 0.1 {
        ("·", SWELL)
    } else {
        (" ", SWELL)
    }
}

/// Cheap deterministic per-cell noise in `0.0..1.0`.
fn hash(x: u16, y: u16) -> f64 {
    let mut h = u32::from(x).wrapping_mul(0x9E37_79B1) ^ u32::from(y).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    f64::from(h & 0xFFFF) / 65536.0
}
