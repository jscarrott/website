//! A toy sailboat simulation and nav-chart widget, for the STDA Sailboat
//! Simulator project's demo.
//!
//! Not the real 6-DOF model: boat speed comes from a simple polar (speed as a
//! fraction of wind speed by true wind angle) with first-order acceleration. An
//! autopilot sails a waypoint loop, tacking when the next mark is upwind, and
//! the visitor can take the helm.
//!
//! Angles are compass degrees (0 = north/up, clockwise). The chart is a
//! `WORLD_W` × `WORLD_H` area with y pointing up, as in ratatui's `Canvas`.

use std::collections::VecDeque;

use ratzilla::ratatui::{
    prelude::*,
    symbols::Marker,
    widgets::{
        Block, Borders, Paragraph,
        canvas::{Canvas, Line as ChartLine, Points},
    },
};

use crate::{FROST, NORD0, NORD3, NORD4, TEAL};

const YELLOW: Color = Color::Rgb(235, 203, 139); // Nord13
const ROUTE: Color = Color::Rgb(76, 86, 106); // Nord3
const WAKE: Color = Color::Rgb(94, 129, 172); // Nord10

const WORLD_W: f64 = 100.0;
const WORLD_H: f64 = 50.0;
/// The route loop: a beat up to the first mark, then reaching and running legs.
const WAYPOINTS: [(f64, f64); 4] = [(18.0, 10.0), (52.0, 42.0), (88.0, 30.0), (72.0, 8.0)];
/// Chart units travelled per knot per second.
const UNITS_PER_KNOT_SEC: f64 = 0.6;
/// Closest a boat can usefully point to the wind.
const NO_GO: f64 = 45.0;
/// Rudder authority, degrees per second.
const TURN_RATE: f64 = 45.0;
/// Speed as a fraction of wind speed, by true wind angle.
const POLAR: [(f64, f64); 7] = [
    (30.0, 0.05),
    (40.0, 0.35),
    (52.0, 0.48),
    (90.0, 0.60),
    (110.0, 0.62),
    (150.0, 0.52),
    (180.0, 0.42),
];

pub struct Sim {
    x: f64,
    y: f64,
    heading: f64,
    /// Heading the rudder is steering toward.
    target: f64,
    speed: f64,
    wind_from: f64,
    wind_speed: f64,
    auto: bool,
    waypoint: usize,
    /// Current tack when beating upwind: +1 wind on the starboard side, -1 port.
    tack: f64,
    notice: Option<(&'static str, f64)>,
    wake: VecDeque<(f64, f64)>,
    wake_timer: f64,
    t: f64,
}

impl Sim {
    pub fn new() -> Self {
        Self {
            x: 66.0,
            y: 10.0,
            heading: 270.0,
            target: 270.0,
            speed: 4.0,
            wind_from: 20.0,
            wind_speed: 13.0,
            auto: true,
            waypoint: 0,
            tack: 1.0,
            notice: None,
            wake: VecDeque::new(),
            wake_timer: 0.0,
            t: 0.0,
        }
    }

    /// Advance the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f64) {
        self.t += dt;
        // A slow, gentle wind shift keeps the beats and tacks varied.
        self.wind_from = norm(20.0 + 25.0 * (self.t * 0.035).sin());
        self.wind_speed = 13.0 + 3.0 * (self.t * 0.11 + 1.0).sin();

        if self.auto {
            self.target = self.autopilot();
        }
        let max_turn = TURN_RATE * dt;
        self.heading =
            norm(self.heading + diff(self.target, self.heading).clamp(-max_turn, max_turn));

        let target_speed = self.wind_speed * polar(self.twa());
        self.speed += (target_speed - self.speed) * (1.0 - (-dt / 1.5).exp());

        let rad = self.heading.to_radians();
        self.x += rad.sin() * self.speed * UNITS_PER_KNOT_SEC * dt;
        self.y += rad.cos() * self.speed * UNITS_PER_KNOT_SEC * dt;

        let (wx, wy) = WAYPOINTS[self.waypoint];
        if self.auto && (wx - self.x).hypot(wy - self.y) < 2.5 {
            self.waypoint = (self.waypoint + 1) % WAYPOINTS.len();
        }

        let off_chart =
            !(-2.0..=WORLD_W + 2.0).contains(&self.x) || !(-2.0..=WORLD_H + 2.0).contains(&self.y);
        if off_chart && !self.auto {
            self.auto = true;
            self.notice = Some(("Off the chart, autopilot engaged", self.t + 3.0));
        }

        self.wake_timer += dt;
        if self.wake_timer >= 0.2 {
            self.wake_timer = 0.0;
            self.wake.push_back((self.x, self.y));
            if self.wake.len() > 90 {
                self.wake.pop_front();
            }
        }
    }

    /// Take the helm and turn the target heading by `degrees`.
    pub fn steer(&mut self, degrees: f64) {
        if self.auto {
            self.auto = false;
            self.target = self.heading;
        }
        self.target = norm(self.target + degrees);
        self.notice = None;
    }

    pub fn toggle_autopilot(&mut self) {
        self.auto = !self.auto;
        self.target = self.heading;
        self.notice = None;
    }

    /// Heading toward the active waypoint, or a close-hauled course on the
    /// current tack when the mark is inside the no-go zone. It tacks once the
    /// other tack points at least 20° closer to the mark, so it doesn't flip-flop.
    fn autopilot(&mut self) -> f64 {
        let (wx, wy) = WAYPOINTS[self.waypoint];
        let bearing = norm((wx - self.x).atan2(wy - self.y).to_degrees());
        let relative = diff(bearing, self.wind_from);
        if relative.abs() >= NO_GO {
            self.tack = relative.signum();
            return bearing;
        }
        let this_tack = norm(self.wind_from + self.tack * NO_GO);
        let other_tack = norm(self.wind_from - self.tack * NO_GO);
        if diff(bearing, other_tack).abs() + 20.0 < diff(bearing, this_tack).abs() {
            self.tack = -self.tack;
            other_tack
        } else {
            this_tack
        }
    }

    /// True wind angle, 0 (head to wind) to 180 (dead downwind).
    fn twa(&self) -> f64 {
        diff(self.heading, self.wind_from).abs()
    }

    fn point_of_sail(&self) -> &'static str {
        match self.twa() {
            a if a < 35.0 => "in irons",
            a if a < 60.0 => "close-hauled",
            a if a < 80.0 => "close reach",
            a if a < 110.0 => "beam reach",
            a if a < 150.0 => "broad reach",
            _ => "running",
        }
    }

    fn tack_name(&self) -> &'static str {
        if diff(self.wind_from, self.heading) > 0.0 {
            "stbd"
        } else {
            "port"
        }
    }
}

/// Normalise to `0..360`.
fn norm(deg: f64) -> f64 {
    deg.rem_euclid(360.0)
}

/// Signed smallest difference `a - b`, in `-180..180`.
fn diff(a: f64, b: f64) -> f64 {
    (a - b + 540.0).rem_euclid(360.0) - 180.0
}

fn polar(twa: f64) -> f64 {
    if twa <= POLAR[0].0 {
        return POLAR[0].1;
    }
    POLAR
        .windows(2)
        .find(|w| twa <= w[1].0)
        .map_or(POLAR[POLAR.len() - 1].1, |w| {
            let ((a0, s0), (a1, s1)) = (w[0], w[1]);
            s0 + (s1 - s0) * (twa - a0) / (a1 - a0)
        })
}

/// Eight-way arrow for a compass direction.
fn arrow(deg: f64) -> &'static str {
    const ARROWS: [&str; 8] = ["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];
    ARROWS[((norm(deg) + 22.5) / 45.0) as usize % 8]
}

/// Draw the nav chart with its HUD. `compact` is the inline preview in the
/// project view (one HUD line); otherwise a second line shows the helm mode.
pub fn render(f: &mut Frame<'_>, area: Rect, sim: &Sim, title: &str, compact: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TEAL))
        .title(title)
        .title_style(Style::default().fg(TEAL));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let hud_rows = if compact { 1 } else { 2 };
    let [chart_area, hud_area] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(hud_rows)]).areas(inner);

    let (x_bounds, y_bounds) = bounds(chart_area);
    let wake: Vec<(f64, f64)> = sim.wake.iter().copied().collect();
    let rad = sim.heading.to_radians();
    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .background_color(NORD0)
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .paint(|ctx| {
            for (i, &(x1, y1)) in WAYPOINTS.iter().enumerate() {
                let (x2, y2) = WAYPOINTS[(i + 1) % WAYPOINTS.len()];
                ctx.draw(&ChartLine::new(x1, y1, x2, y2, ROUTE));
            }
            ctx.layer();
            ctx.draw(&Points::new(&wake, WAKE));
            // Bow line shows the heading, since the boat glyph can't rotate.
            ctx.draw(&ChartLine::new(
                sim.x,
                sim.y,
                sim.x + rad.sin() * 5.0,
                sim.y + rad.cos() * 5.0,
                YELLOW,
            ));
            for (i, &(x, y)) in WAYPOINTS.iter().enumerate() {
                let label = if sim.auto && i == sim.waypoint {
                    Span::styled(format!("◆{}", i + 1), Style::default().fg(YELLOW).bold())
                } else {
                    Span::styled(format!("◇{}", i + 1), Style::default().fg(NORD3))
                };
                ctx.print(x, y, label);
            }
            ctx.print(
                x_bounds[0] + 1.0,
                y_bounds[1] - 1.0,
                Span::styled("N↑", Style::default().fg(NORD3)),
            );
            ctx.print(sim.x, sim.y, "⛵");
        });
    f.render_widget(canvas, chart_area);

    let label = Style::default().fg(NORD3);
    let value = Style::default().fg(NORD4);
    // Arrow points where the wind blows to, as on a weather map.
    let wind = format!("{} {:.0} kn", arrow(sim.wind_from + 180.0), sim.wind_speed);
    if compact {
        let hud = Line::from(vec![
            Span::styled(format!("{:03.0}°", sim.heading), value),
            Span::styled(" · ", label),
            Span::styled(format!("{:.1} kn", sim.speed), value),
            Span::styled(" · ", label),
            Span::styled(sim.point_of_sail(), Style::default().fg(FROST)),
            Span::styled(" · wind ", label),
            Span::styled(wind, value),
        ]);
        f.render_widget(Paragraph::new(hud).alignment(Alignment::Center), hud_area);
        return;
    }
    let lines = vec![
        Line::from(vec![
            Span::styled("HDG ", label),
            Span::styled(format!("{:03.0}°", sim.heading), value),
            Span::styled("  SOG ", label),
            Span::styled(format!("{:.1} kn", sim.speed), value),
            Span::styled("  TWA ", label),
            Span::styled(format!("{:.0}° {}", sim.twa(), sim.tack_name()), value),
            Span::styled(
                format!("  {}", sim.point_of_sail()),
                Style::default().fg(FROST),
            ),
            Span::styled("  WIND ", label),
            Span::styled(wind, value),
        ]),
        match sim.notice {
            Some((text, until)) if sim.t < until => Line::styled(text, Style::default().fg(YELLOW)),
            _ if sim.auto => Line::styled(
                format!("AUTOPILOT → waypoint {}", sim.waypoint + 1),
                Style::default().fg(TEAL),
            ),
            _ => Line::styled(
                format!("YOU HAVE THE HELM → steering {:03.0}°", sim.target),
                Style::default().fg(YELLOW).bold(),
            ),
        },
    ];
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), hud_area);
}

/// Chart bounds that show the whole world with a margin, widened along one
/// axis so the chart isn't stretched (a terminal cell is about twice as tall
/// as it is wide).
fn bounds(area: Rect) -> ([f64; 2], [f64; 2]) {
    const MARGIN: f64 = 3.0;
    let (w, h) = (WORLD_W + 2.0 * MARGIN, WORLD_H + 2.0 * MARGIN);
    let (cx, cy) = (WORLD_W / 2.0, WORLD_H / 2.0);
    let aspect = f64::from(area.width.max(1)) / (f64::from(area.height.max(1)) * 2.0);
    if aspect > w / h {
        let half = h * aspect / 2.0;
        ([cx - half, cx + half], [cy - h / 2.0, cy + h / 2.0])
    } else {
        let half = w / aspect / 2.0;
        ([cx - w / 2.0, cx + w / 2.0], [cy - half, cy + half])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_wraps() {
        assert_eq!(diff(10.0, 350.0), 20.0);
        assert_eq!(diff(350.0, 10.0), -20.0);
    }

    #[test]
    fn polar_interpolates_and_clamps() {
        assert_eq!(polar(0.0), 0.05);
        assert!((polar(71.0) - 0.54).abs() < 1e-9);
        assert_eq!(polar(180.0), 0.42);
    }

    #[test]
    fn autopilot_completes_the_loop() {
        let mut sim = Sim::new();
        let mut visited = 0;
        let mut last = sim.waypoint;
        for _ in 0..(600.0 / 0.05) as usize {
            sim.step(0.05);
            if sim.waypoint != last {
                visited += 1;
                last = sim.waypoint;
            }
        }
        assert!(
            visited >= WAYPOINTS.len(),
            "only rounded {visited} marks in 10 minutes"
        );
    }
}
