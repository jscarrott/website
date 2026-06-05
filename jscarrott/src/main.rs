use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
};

mod generated_content;
use generated_content::{self as gc, PROFILE};

use ratzilla::{
    backend::canvas::CanvasBackendOptions,
    event::{KeyCode, MouseButton, MouseEventKind},
    ratatui::{
        prelude::*,
        widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    },
    web_sys, CanvasBackend, WebRenderer,
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
        "red" => Color::Rgb(191, 97, 106),     // Nord11
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
    fn all() -> [Screen; 5] {
        [
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

    /// Heading shown at the top of the section.
    fn heading(&self) -> &'static str {
        match self {
            Screen::About => "About Me",
            Screen::Experience => "Professional Experience",
            Screen::Skills => "Technical Skills",
            Screen::Education => "Education & Training",
            Screen::Projects => "Projects & Contributions",
            Screen::Welcome => "Welcome",
        }
    }
}

/// The drill-down entries for a section, if it is entry-based.
fn section_entries(screen: Screen) -> Option<&'static [gc::Entry]> {
    match screen {
        Screen::Experience => Some(gc::EXPERIENCE),
        Screen::Education => Some(gc::EDUCATION),
        Screen::Projects => Some(gc::PROJECTS),
        _ => None,
    }
}

/// Depth within an entry-based section.
#[derive(Copy, Clone, PartialEq)]
enum Level {
    List,   // choose a role
    Detail, // choose a highlight (bullet) within the role
    Focus,  // read one highlight in full
}

struct App {
    screen: Screen,
    selected_menu: usize,   // highlighted item on the welcome menu
    selected_entry: usize,  // highlighted role in a section list
    selected_bullet: usize, // highlighted highlight in the detail view
    level: Level,
    scroll: u16, // vertical scroll offset for focus / long content
}

impl App {
    fn new() -> Self {
        Self {
            screen: Screen::Welcome,
            selected_menu: 0,
            selected_entry: 0,
            selected_bullet: 0,
            level: Level::List,
            scroll: 0,
        }
    }

    fn is_section(&self) -> bool {
        section_entries(self.screen).is_some()
    }

    fn entries_len(&self) -> usize {
        section_entries(self.screen).map_or(0, <[_]>::len)
    }

    fn current_entry(&self) -> Option<&'static gc::Entry> {
        section_entries(self.screen).and_then(|e| e.get(self.selected_entry))
    }

    fn bullets_len(&self) -> usize {
        self.current_entry().map_or(0, |e| e.bullets.len())
    }

    fn next_menu(&mut self) {
        self.selected_menu = (self.selected_menu + 1) % Screen::all().len();
    }

    fn prev_menu(&mut self) {
        let n = Screen::all().len();
        self.selected_menu = (self.selected_menu + n - 1) % n;
    }

    fn select_current_menu(&mut self) {
        self.goto_section(Screen::all()[self.selected_menu]);
    }

    fn goto_section(&mut self, screen: Screen) {
        self.screen = screen;
        if let Some(i) = Screen::all().iter().position(|s| *s == screen) {
            self.selected_menu = i;
        }
        self.selected_entry = 0;
        self.selected_bullet = 0;
        self.level = Level::List;
        self.scroll = 0;
    }

    fn open_entry(&mut self, index: usize) {
        if index < self.entries_len() {
            self.selected_entry = index;
            self.selected_bullet = 0;
            self.level = Level::Detail;
            self.scroll = 0;
        }
    }

    fn open_bullet(&mut self, index: usize) {
        if index < self.bullets_len() {
            self.selected_bullet = index;
            self.level = Level::Focus;
            self.scroll = 0;
        }
    }

    /// Step the selection within the active list (roles or highlights).
    fn step_selection(&mut self, forward: bool) {
        let n = match self.level {
            Level::List => self.entries_len(),
            Level::Detail => self.bullets_len(),
            Level::Focus => return,
        };
        if n == 0 {
            return;
        }
        let sel = match self.level {
            Level::List => &mut self.selected_entry,
            Level::Detail => &mut self.selected_bullet,
            Level::Focus => return,
        };
        *sel = if forward {
            (*sel + 1) % n
        } else {
            (*sel + n - 1) % n
        };
    }

    /// Step back one level: focus -> detail -> list -> home.
    fn back(&mut self) {
        if self.is_section() {
            match self.level {
                Level::Focus => {
                    self.level = Level::Detail;
                    self.scroll = 0;
                }
                Level::Detail => self.level = Level::List,
                Level::List => self.go_home(),
            }
        } else if self.screen != Screen::Welcome {
            self.go_home();
        }
    }

    fn go_home(&mut self) {
        *self = App::new();
    }

    /// True when the active view is a free-scrolling text view (focus / About /
    /// Skills) rather than a selectable list.
    fn is_scroll_view(&self) -> bool {
        if self.is_section() {
            self.level == Level::Focus
        } else {
            self.screen != Screen::Welcome
        }
    }
}

/// An interactive region recorded during render, hit-tested on mouse click.
#[derive(Copy, Clone)]
enum ClickAction {
    Goto(Screen),
    OpenEntry(usize),
    OpenBullet(usize),
}

type Regions = RefCell<Vec<(Rect, ClickAction)>>;

/// The active view, read from `<html data-view>` (set by the page's inline
/// script). In "plain" mode the static HTML CV is shown instead of the terminal.
fn view_mode() -> Option<String> {
    web_sys::window()?
        .document()?
        .document_element()?
        .get_attribute("data-view")
}

fn main() -> io::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    // In "plain" view the static HTML CV is shown, so don't start the terminal at
    // all — this keeps the requestAnimationFrame render loop from running on
    // phones / low-power devices where the canvas UI is a poor experience.
    if view_mode().as_deref() == Some("plain") {
        return Ok(());
    }

    // Canvas backend draws to a single <canvas> rather than one DOM element per
    // cell, which is dramatically faster than the DOM backend on large grids.
    // Mount it inside #terminal-root so the page can show/hide it per view.
    let backend = CanvasBackend::new_with_options(CanvasBackendOptions::new().grid_id("terminal-root"))?;
    let terminal = Terminal::new(backend)?;

    let app = Rc::new(RefCell::new(App::new()));
    let regions: Rc<Regions> = Rc::new(RefCell::new(Vec::new()));
    let grid_size: Rc<Cell<(u16, u16)>> = Rc::new(Cell::new((0, 0)));
    // Maximum scroll offset for the current view, computed each render so the
    // key handler can clamp downward scrolling at the bottom of the content.
    let scroll_max: Rc<Cell<u16>> = Rc::new(Cell::new(0));

    terminal.on_key_event({
        let app = app.clone();
        let scroll_max = scroll_max.clone();
        move |key_event| {
            let mut app = app.borrow_mut();
            match key_event.code {
                KeyCode::Char('q') => app.go_home(),
                KeyCode::Esc | KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => app.back(),
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
                    if app.screen == Screen::Welcome {
                        app.select_current_menu();
                    } else if app.is_section() {
                        match app.level {
                            Level::List => {
                                let i = app.selected_entry;
                                app.open_entry(i);
                            }
                            Level::Detail => {
                                let i = app.selected_bullet;
                                app.open_bullet(i);
                            }
                            Level::Focus => {}
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.screen == Screen::Welcome {
                        app.next_menu();
                    } else if app.is_scroll_view() {
                        app.scroll = (app.scroll + 1).min(scroll_max.get());
                    } else {
                        app.step_selection(true);
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.screen == Screen::Welcome {
                        app.prev_menu();
                    } else if app.is_scroll_view() {
                        app.scroll = app.scroll.saturating_sub(1);
                    } else {
                        app.step_selection(false);
                    }
                }
                KeyCode::Char('1') => app.goto_section(Screen::About),
                KeyCode::Char('2') => app.goto_section(Screen::Experience),
                KeyCode::Char('3') => app.goto_section(Screen::Skills),
                KeyCode::Char('4') => app.goto_section(Screen::Education),
                KeyCode::Char('5') => app.goto_section(Screen::Projects),
                _ => {}
            }
        }
    });

    terminal.on_mouse_event({
        let app = app.clone();
        let regions = regions.clone();
        let grid_size = grid_size.clone();
        move |ev| {
            if ev.event != MouseEventKind::Pressed || ev.button != MouseButton::Left {
                return;
            }
            let mut app = app.borrow_mut();
            if let Some((col, row)) = pixel_to_cell(ev.x, ev.y, grid_size.get()) {
                for (rect, action) in regions.borrow().iter() {
                    if rect.x <= col
                        && col < rect.x + rect.width
                        && rect.y <= row
                        && row < rect.y + rect.height
                    {
                        match action {
                            ClickAction::Goto(s) => app.goto_section(*s),
                            ClickAction::OpenEntry(i) => app.open_entry(*i),
                            ClickAction::OpenBullet(i) => app.open_bullet(*i),
                        }
                        return;
                    }
                }
            }
            // A click that misses every region on a screen with no interactive
            // regions (focus / About / Skills) steps back one level.
            if regions.borrow().is_empty() {
                app.back();
            }
        }
    });

    terminal.draw_web({
        let app = app.clone();
        let regions = regions.clone();
        let grid_size = grid_size.clone();
        let scroll_max = scroll_max.clone();
        move |f| {
            grid_size.set((f.area().width, f.area().height));
            ui(f, &app.borrow(), &regions, &scroll_max);
        }
    });

    Ok(())
}

/// Map a viewport pixel coordinate to a terminal cell using the canvas bounding
/// box and the current grid size (in cells). Returns `None` if outside the grid.
fn pixel_to_cell(x: u32, y: u32, (cols, rows): (u16, u16)) -> Option<(u16, u16)> {
    if cols == 0 || rows == 0 {
        return None;
    }
    let canvas = web_sys::window()?
        .document()?
        .query_selector("canvas")
        .ok()??;
    let rect = canvas.get_bounding_client_rect();
    let cell_w = rect.width() / cols as f64;
    let cell_h = rect.height() / rows as f64;
    if cell_w <= 0.0 || cell_h <= 0.0 {
        return None;
    }
    let col = ((x as f64 - rect.left()) / cell_w).floor();
    let row = ((y as f64 - rect.top()) / cell_h).floor();
    if col < 0.0 || row < 0.0 || col >= cols as f64 || row >= rows as f64 {
        return None;
    }
    Some((col as u16, row as u16))
}

// Scroll bookkeeping ----------------------------------------------------------

/// Plain text of a line (concatenated span contents), for wrap measurement.
fn line_text(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Word-wrap `text` to `width` columns, approximating ratatui's
/// `Wrap { trim: true }`. Words longer than the line are hard-broken.
fn wrap_text(text: &str, width: u16) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let width = width as usize;
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_len = 0usize; // char count of `cur`

    for word in text.split_whitespace() {
        let wlen = word.chars().count();
        if cur_len != 0 && cur_len + 1 + wlen > width {
            out.push(std::mem::take(&mut cur));
            cur_len = 0;
        }
        if cur_len == 0 {
            if wlen > width {
                let chars: Vec<char> = word.chars().collect();
                let mut start = 0;
                while chars.len() - start > width {
                    out.push(chars[start..start + width].iter().collect());
                    start += width;
                }
                cur = chars[start..].iter().collect();
                cur_len = chars.len() - start;
            } else {
                cur.push_str(word);
                cur_len = wlen;
            }
        } else {
            cur.push(' ');
            cur.push_str(word);
            cur_len += 1 + wlen;
        }
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

/// Number of rows a single source line occupies once word-wrapped to `width`.
fn wrap_count(text: &str, width: u16) -> u16 {
    wrap_text(text, width).len() as u16
}

/// Total wrapped height of a block of lines at the given width.
fn wrapped_height(lines: &[Line], width: u16) -> u16 {
    lines
        .iter()
        .fold(0u16, |acc, l| acc.saturating_add(wrap_count(&line_text(l), width)))
}

/// Render a scrollable paragraph, publishing the clamped max scroll so the key
/// handler can stop at the bottom. Returns nothing; writes into `area`.
fn render_scrollable(
    f: &mut Frame<'_>,
    area: Rect,
    lines: Vec<Line>,
    scroll: u16,
    scroll_max: &Cell<u16>,
) {
    let max = wrapped_height(&lines, area.width).saturating_sub(area.height);
    scroll_max.set(max);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((scroll.min(max), 0))
            .style(Style::default().fg(NORD6)),
        area,
    );
}

// Rendering -------------------------------------------------------------------

fn ui(f: &mut Frame<'_>, app: &App, regions: &Regions, scroll_max: &Cell<u16>) {
    regions.borrow_mut().clear();
    scroll_max.set(0);

    // Clear the screen with Nord polar night background
    Clear.render(f.area(), f.buffer_mut());
    Block::default()
        .style(Style::default().bg(NORD0))
        .render(f.area(), f.buffer_mut());

    match app.screen {
        Screen::Welcome => render_welcome(f, app, regions),
        Screen::About => render_about(f, app, scroll_max),
        Screen::Skills => render_skills(f, app, scroll_max),
        _ => match app.level {
            Level::List => render_list(f, app, regions),
            Level::Detail => render_detail(f, app, regions),
            Level::Focus => render_focus(f, app, scroll_max),
        },
    }
}

/// Three-row scaffold (title, body, footer) shared by every content screen.
fn content_layout(area: Rect) -> [Rect; 3] {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);
    [parts[0], parts[1], parts[2]]
}

fn title_bar(f: &mut Frame<'_>, area: Rect, title: &str) {
    let widget = Paragraph::new(title)
        .style(Style::default().fg(FROST).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(FROST)),
        );
    f.render_widget(widget, area);
}

fn footer(f: &mut Frame<'_>, area: Rect, hint: &str) {
    f.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(NORD3))
            .alignment(Alignment::Center),
        area,
    );
}

fn render_welcome(f: &mut Frame<'_>, app: &App, regions: &Regions) {
    let area = f.area();

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

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

    f.render_widget(
        header_text.centered(),
        main_layout[0].inner(Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );

    let screens = Screen::all();
    let menu_items: Vec<ListItem> = screens
        .iter()
        .enumerate()
        .map(|(i, screen)| {
            let number = format!(" {}. ", i + 1);
            ListItem::new(Line::from(vec![
                Span::styled(number, Style::default().fg(Color::Rgb(235, 203, 139))),
                Span::styled(screen.title(), Style::default().fg(NORD6)),
            ]))
        })
        .collect();

    let menu_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TEAL))
        .title(" Navigation ")
        .title_style(Style::default().fg(TEAL).add_modifier(Modifier::BOLD));

    let menu_area = main_layout[1].inner(Margin {
        horizontal: 4,
        vertical: 1,
    });

    let inner = menu_block.inner(menu_area);
    {
        let mut regs = regions.borrow_mut();
        for (i, screen) in screens.iter().enumerate() {
            if (i as u16) < inner.height {
                regs.push((
                    Rect::new(inner.x, inner.y + i as u16, inner.width, 1),
                    ClickAction::Goto(*screen),
                ));
            }
        }
    }

    let mut state = ListState::default();
    state.select(Some(app.selected_menu));
    let menu = List::new(menu_items)
        .block(menu_block)
        .highlight_style(
            Style::default()
                .fg(NORD0)
                .bg(FROST)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");
    f.render_stateful_widget(menu, menu_area, &mut state);

    footer(
        f,
        main_layout[2].inner(Margin {
            horizontal: 2,
            vertical: 0,
        }),
        "↑↓/Click Select • Enter Open • 1-5 Jump • Q Home",
    );
}

/// Records one clickable row per item and returns the block's inner rect.
fn list_block<'a>(
    regions: &Regions,
    area: Rect,
    title: &'a str,
    count: usize,
    action: impl Fn(usize) -> ClickAction,
) -> (Block<'a>, Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TEAL))
        .title(format!(" {title} "))
        .title_style(Style::default().fg(TEAL));
    let inner = block.inner(area);
    let mut regs = regions.borrow_mut();
    for i in 0..count {
        if (i as u16) < inner.height {
            regs.push((
                Rect::new(inner.x, inner.y + i as u16, inner.width, 1),
                action(i),
            ));
        }
    }
    (block, inner)
}

/// Level 1: list of roles in a section, with date column.
fn render_list(f: &mut Frame<'_>, app: &App, regions: &Regions) {
    let entries = section_entries(app.screen).unwrap_or(&[]);
    let [title_area, body_area, footer_area] = content_layout(f.area());
    title_bar(f, title_area, app.screen.heading());

    let list_area = body_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            let accent = accent_color(e.accent);
            ListItem::new(Line::from(vec![
                Span::raw(format!("{} ", e.emoji)),
                Span::styled(e.title, Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ·  {}", e.org), Style::default().fg(NORD4)),
                Span::styled(format!("   {}", e.date), Style::default().fg(NORD3)),
            ]))
        })
        .collect();

    let (block, _) = list_block(regions, list_area, "Select a role", entries.len(), ClickAction::OpenEntry);

    let mut state = ListState::default();
    state.select(Some(app.selected_entry.min(entries.len().saturating_sub(1))));
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().fg(NORD0).bg(FROST))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, list_area, &mut state);

    footer(f, footer_area, "↑↓ Select • Enter/→ Open role • Esc Back");
}

/// Level 2: the selected role's highlights, each shown in full (bold lead plus
/// the wrapped detail) as a selectable, scrollable list.
fn render_detail(f: &mut Frame<'_>, app: &App, regions: &Regions) {
    let Some(entry) = app.current_entry() else {
        return;
    };
    let accent = accent_color(entry.accent);
    let [title_area, body_area, footer_area] = content_layout(f.area());
    title_bar(f, title_area, &format!("{} {} · {}", entry.emoji, entry.title, entry.org));

    let list_area = body_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TEAL))
        .title(" Highlights ")
        .title_style(Style::default().fg(TEAL));
    let inner = block.inner(list_area);

    // Each item is multi-line, so reserve the highlight-symbol width and wrap.
    const SYMBOL_W: u16 = 2; // "▌ "
    let text_w = inner.width.saturating_sub(SYMBOL_W).max(1);

    let mut heights: Vec<u16> = Vec::with_capacity(entry.bullets.len());
    let items: Vec<ListItem> = entry
        .bullets
        .iter()
        .map(|b| {
            let mut lines: Vec<Line> = Vec::new();
            if !b.lead.is_empty() {
                lines.push(
                    Line::from(b.lead).style(Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                );
            }
            for wl in wrap_text(b.rest, text_w) {
                lines.push(Line::from(wl).style(Style::default().fg(NORD6)));
            }
            lines.push(Line::from("")); // spacer between highlights
            heights.push(lines.len() as u16);
            ListItem::new(lines)
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.selected_bullet.min(entry.bullets.len().saturating_sub(1))));
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
        .highlight_symbol("▌ ");
    f.render_stateful_widget(list, list_area, &mut state);

    // Map each visible item to a click region using the post-render scroll
    // offset, so clicks land on the right highlight even when the list scrolls.
    let bottom = inner.y + inner.height;
    let mut y = inner.y;
    let mut regs = regions.borrow_mut();
    for i in state.offset()..entry.bullets.len() {
        if y >= bottom {
            break;
        }
        let h = heights[i].min(bottom - y);
        regs.push((Rect::new(inner.x, y, inner.width, h), ClickAction::OpenBullet(i)));
        y += heights[i];
    }

    footer(f, footer_area, "↑↓ Select • Enter/→ Read full • ← Back to roles");
}

/// A centred reading column within `area`, capped at `max_width` so the text
/// stays comfortable on wide screens, then inset by the given margins.
fn centered_column(area: Rect, max_width: u16, horizontal: u16, vertical: u16) -> Rect {
    let w = area.width.min(max_width);
    let x = area.x + (area.width - w) / 2;
    Rect::new(x, area.y, w, area.height).inner(Margin {
        horizontal,
        vertical,
    })
}

/// Split text into sentences (on `". "`) so the focus view can space them out.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        cur.push(c);
        if c == '.' && chars.peek() == Some(&' ') {
            chars.next(); // swallow the separating space
            sentences.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        sentences.push(cur.trim().to_string());
    }
    if sentences.is_empty() {
        sentences.push(text.to_string());
    }
    sentences
}

/// Level 3: read one highlight in full — a spacious, scrollable reading view.
fn render_focus(f: &mut Frame<'_>, app: &App, scroll_max: &Cell<u16>) {
    let Some(entry) = app.current_entry() else {
        return;
    };
    let Some(bullet) = entry.bullets.get(app.selected_bullet) else {
        return;
    };
    let accent = accent_color(entry.accent);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(f.area());

    // Slim breadcrumb: which role this highlight belongs to.
    let crumb = Paragraph::new(format!(
        "{} {}  ·  {}  ·  {}",
        entry.emoji, entry.title, entry.org, entry.date
    ))
    .style(Style::default().fg(NORD4))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(accent)),
    );
    f.render_widget(crumb, layout[0]);

    // Expanded body: a wide centred column with the lead as a heading and the
    // detail broken into spaced-out sentences.
    let mut lines: Vec<Line> = vec![Line::from(""), Line::from("")];
    if !bullet.lead.is_empty() {
        lines.push(
            Line::from(bullet.lead)
                .style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
                .centered(),
        );
        lines.push(Line::from(""));
        lines.push(Line::from(""));
    }
    for sentence in split_sentences(bullet.rest) {
        lines.push(Line::from(sentence).style(Style::default().fg(NORD6)));
        lines.push(Line::from(""));
    }

    render_scrollable(
        f,
        centered_column(layout[1], 100, 2, 1),
        lines,
        app.scroll,
        scroll_max,
    );

    footer(f, layout[2], "↑↓ Scroll • ← / Esc Back to highlights");
}

fn render_about(f: &mut Frame<'_>, app: &App, scroll_max: &Cell<u16>) {
    let [title_area, body_area, footer_area] = content_layout(f.area());
    title_bar(f, title_area, app.screen.heading());

    let lines: Vec<Line> = gc::ABOUT.iter().map(|l| Line::from(*l)).collect();
    render_scrollable(
        f,
        body_area.inner(Margin {
            horizontal: 4,
            vertical: 1,
        }),
        lines,
        app.scroll,
        scroll_max,
    );

    footer(f, footer_area, "↑↓ Scroll • Esc Back");
}

fn render_skills(f: &mut Frame<'_>, app: &App, scroll_max: &Cell<u16>) {
    let [title_area, body_area, footer_area] = content_layout(f.area());
    title_bar(f, title_area, app.screen.heading());

    let palette = ["yellow", "green", "purple", "blue", "teal", "red", "orange"];
    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, cat) in gc::SKILLS.iter().enumerate() {
        let accent = accent_color(palette[i % palette.len()]);
        lines.push(Line::from(cat.name).style(Style::default().fg(accent).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(format!("   {}", cat.items)).style(Style::default().fg(NORD6)));
        lines.push(Line::from(""));
    }

    render_scrollable(
        f,
        body_area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        }),
        lines,
        app.scroll,
        scroll_max,
    );

    footer(f, footer_area, "↑↓ Scroll • Esc Back");
}
