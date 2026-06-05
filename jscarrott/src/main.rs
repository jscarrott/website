use std::{cell::RefCell, io, rc::Rc};

mod generated_content;
use generated_content::{self as gc, PROFILE};

use ratzilla::{
    event::KeyCode,
    ratatui::{
        prelude::*,
        widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    },
    DomBackend, WebRenderer,
};

// Nord palette ----------------------------------------------------------------
const NORD0: Color = Color::Rgb(46, 52, 64); // polar night (background)
const NORD3: Color = Color::Rgb(76, 86, 106); // polar night (muted hints)
const NORD4: Color = Color::Rgb(216, 222, 233); // snow storm (dim)
const NORD6: Color = Color::Rgb(236, 239, 244); // snow storm (body text)
const FROST: Color = Color::Rgb(136, 192, 208); // Nord8 (headings)
const TEAL: Color = Color::Rgb(143, 188, 187); // Nord7

/// Resolve a content `accent` name (set in the markdown frontmatter) to a Nord
/// aurora colour.
fn accent_color(name: &str) -> Color {
    match name {
        "yellow" => Color::Rgb(235, 203, 139), // Nord13
        "green" => Color::Rgb(163, 190, 140),  // Nord14
        "blue" => Color::Rgb(129, 161, 193),   // Nord9
        "purple" => Color::Rgb(180, 142, 173), // Nord15
        "teal" => TEAL,
        "red" => Color::Rgb(191, 97, 106),    // Nord11
        "orange" => Color::Rgb(208, 135, 112), // Nord12
        _ => FROST,
    }
}

#[derive(Copy, Debug, Clone, PartialEq)]
enum Screen {
    Welcome,
    About,
    Experience,
    Skills,
    Education,
    Projects,
}

impl Screen {
    fn all() -> Vec<Screen> {
        vec![
            Screen::About,
            Screen::Experience,
            Screen::Skills,
            Screen::Education,
            Screen::Projects,
        ]
    }

    fn title(&self) -> &'static str {
        match self {
            Screen::Welcome => "Welcome",
            Screen::About => "About",
            Screen::Experience => "Experience",
            Screen::Skills => "Skills",
            Screen::Education => "Education",
            Screen::Projects => "Projects",
        }
    }
}

struct App {
    screen: Screen,
    selected_menu: usize,
}

impl App {
    fn new() -> Self {
        Self {
            screen: Screen::Welcome,
            selected_menu: 0,
        }
    }

    fn next_menu(&mut self) {
        let screens = Screen::all();
        if self.selected_menu < screens.len() - 1 {
            self.selected_menu += 1;
        } else {
            self.selected_menu = 0;
        }
    }

    fn prev_menu(&mut self) {
        let screens = Screen::all();
        if self.selected_menu > 0 {
            self.selected_menu -= 1;
        } else {
            self.selected_menu = screens.len() - 1;
        }
    }

    fn select_current_menu(&mut self) {
        let screens = Screen::all();
        self.screen = screens[self.selected_menu];
    }

    fn go_home(&mut self) {
        self.screen = Screen::Welcome;
        self.selected_menu = 0;
    }
}

fn main() -> io::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    let backend = DomBackend::new()?;
    let terminal = Terminal::new(backend)?;

    let app = Rc::new(RefCell::new(App::new()));

    terminal.on_key_event({
        let app = app.clone();
        move |key_event| {
            let mut app = app.borrow_mut();
            match key_event.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    app.go_home();
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if app.screen == Screen::Welcome {
                        app.select_current_menu();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.screen == Screen::Welcome {
                        app.next_menu();
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.screen == Screen::Welcome {
                        app.prev_menu();
                    }
                }
                KeyCode::Char('1') => app.screen = Screen::About,
                KeyCode::Char('2') => app.screen = Screen::Experience,
                KeyCode::Char('3') => app.screen = Screen::Skills,
                KeyCode::Char('4') => app.screen = Screen::Education,
                KeyCode::Char('5') => app.screen = Screen::Projects,
                _ => {}
            }
        }
    });

    terminal.draw_web(move |f| ui(f, app.clone()));

    Ok(())
}

fn ui(f: &mut Frame<'_>, app: Rc<RefCell<App>>) {
    let app_ref = app.borrow();

    // Clear the screen with Nord polar night background
    Clear.render(f.area(), f.buffer_mut());
    Block::default()
        .style(Style::default().bg(NORD0))
        .render(f.area(), f.buffer_mut());

    match app_ref.screen {
        Screen::Welcome => render_welcome(f, &app_ref),
        Screen::About => render_about(f),
        Screen::Experience => render_entry_screen(f, "Professional Experience", gc::EXPERIENCE),
        Screen::Skills => render_skills(f),
        Screen::Education => render_entry_screen(f, "Education & Training", gc::EDUCATION),
        Screen::Projects => render_entry_screen(f, "Projects & Contributions", gc::PROJECTS),
    }
}

fn render_welcome(f: &mut Frame<'_>, app: &App) {
    let area = f.area();

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    // Header (sourced from the generated profile)
    let header_text = Text::from(vec![
        Line::from(""),
        Line::from(PROFILE.name).style(Style::default().fg(FROST).add_modifier(Modifier::BOLD)),
        Line::from(PROFILE.position).style(Style::default().fg(Color::Rgb(235, 203, 139))),
        Line::from(""),
        Line::from(format!(
            "📧 {}  🌐 {}  💼 {}",
            PROFILE.email, PROFILE.homepage, PROFILE.github
        )),
        Line::from(format!("📱 {}  📍 {}", PROFILE.phone, PROFILE.location)),
        Line::from(""),
    ]);

    let header_area = main_layout[0].inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    f.render_widget(header_text.centered(), header_area);

    // Menu
    let screens = Screen::all();
    let menu_items: Vec<ListItem> = screens
        .iter()
        .enumerate()
        .map(|(i, screen)| {
            let style = if i == app.selected_menu {
                Style::default()
                    .fg(NORD0)
                    .bg(FROST)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(NORD6)
            };

            let number = format!("{}. ", i + 1);
            ListItem::new(Line::from(vec![
                Span::styled(number, Style::default().fg(Color::Rgb(235, 203, 139))),
                Span::styled(screen.title(), style),
            ]))
        })
        .collect();

    let menu = List::new(menu_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(TEAL))
            .title(" Navigation ")
            .title_style(Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
    );

    let menu_area = main_layout[1].inner(Margin {
        horizontal: 4,
        vertical: 1,
    });
    f.render_widget(menu, menu_area);

    // Instructions
    let instructions = Text::from(vec![Line::from(
        "↑↓ Navigate • Enter/Space Select • 1-5 Quick Jump • Q/Esc Home",
    )]);

    let instructions_area = main_layout[2].inner(Margin {
        horizontal: 2,
        vertical: 0,
    });
    f.render_widget(
        instructions.centered().style(Style::default().fg(NORD3)),
        instructions_area,
    );
}

/// Shared chrome for a content screen: centered title, body, and a return hint.
fn screen_frame(f: &mut Frame<'_>, title: &str, body: Paragraph) {
    let area = f.area();

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    let title_widget = Paragraph::new(title)
        .style(Style::default().fg(FROST).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(FROST)),
        );
    f.render_widget(title_widget, layout[0]);

    let content_area = layout[1].inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    f.render_widget(body.wrap(Wrap { trim: true }), content_area);

    let nav_hint = Paragraph::new("Press Q or Esc to return to main menu")
        .style(Style::default().fg(NORD3))
        .alignment(Alignment::Center);
    f.render_widget(nav_hint, layout[2]);
}

/// Render an entry-based section (Experience, Education, Projects).
fn render_entry_screen(f: &mut Frame<'_>, title: &str, entries: &[gc::Entry]) {
    let body_style = Style::default().fg(NORD6);
    let date_style = Style::default().fg(NORD4);

    let mut lines: Vec<Line> = vec![Line::from("")];
    for e in entries {
        let accent = accent_color(e.accent);
        let header = if e.emoji.is_empty() {
            format!("{} @ {}", e.title, e.org)
        } else {
            format!("{} {} @ {}", e.emoji, e.title, e.org)
        };
        lines.push(Line::from(header).style(Style::default().fg(accent).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(format!("   {} | {}", e.date, e.location)).style(date_style));
        lines.push(Line::from(""));
        for b in e.bullets {
            if b.lead.is_empty() {
                lines.push(Line::from(format!("   • {}", b.rest)).style(body_style));
            } else {
                lines.push(Line::from(vec![
                    Span::styled("   • ", body_style),
                    Span::styled(b.lead, Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" {}", b.rest), body_style),
                ]));
            }
        }
        lines.push(Line::from(""));
    }

    screen_frame(f, title, Paragraph::new(lines));
}

fn render_skills(f: &mut Frame<'_>) {
    // Cycle the aurora palette across categories for visual variety.
    let palette = ["yellow", "green", "purple", "blue", "teal", "red", "orange"];
    let body_style = Style::default().fg(NORD6);

    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, cat) in gc::SKILLS.iter().enumerate() {
        let accent = accent_color(palette[i % palette.len()]);
        lines.push(Line::from(cat.name).style(Style::default().fg(accent).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(format!("   {}", cat.items)).style(body_style));
        lines.push(Line::from(""));
    }

    screen_frame(f, "Technical Skills", Paragraph::new(lines));
}

fn render_about(f: &mut Frame<'_>) {
    let lines: Vec<Line> = gc::ABOUT.iter().map(|l| Line::from(*l)).collect();
    screen_frame(
        f,
        "About Me",
        Paragraph::new(lines).style(Style::default().fg(NORD6)),
    );
}
