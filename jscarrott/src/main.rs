use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    io,
    rc::Rc,
};

mod generated_content;
mod sail;
mod sea;
use generated_content::{self as gc, PROFILE};

use ratzilla::{
    CanvasBackend, WebEventHandler, WebGl2Backend, WebRenderer,
    backend::{
        canvas::CanvasBackendOptions,
        webgl2::{FontAtlasConfig, WebGl2BackendOptions},
    },
    error::Error as RatzillaError,
    event::{KeyCode, MouseButton, MouseEventKind},
    event::{KeyEvent, MouseEvent},
    ratatui::{
        backend::{Backend, ClearType, WindowSize},
        buffer::Cell as BufferCell,
        layout::Size,
        prelude::*,
        widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    },
    web_sys::{
        self, WheelEvent,
        wasm_bindgen::{JsCast, JsValue, convert::FromWasmAbi, prelude::Closure},
    },
};
use unicode_width::UnicodeWidthStr;

/// Terminal font (loaded by index.html from the Fira Code CDN stylesheet).
const FONT_FAMILY: &str = "Fira Code";
const FONT_SIZE: f32 = 16.0;
/// Element the terminal canvas is mounted in; index.html sizes it to the viewport.
const ROOT_ID: &str = "terminal-root";

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

    /// URL fragment for the section (`#experience`); the welcome screen has none.
    fn slug(&self) -> Option<&'static str> {
        match self {
            Screen::Welcome => None,
            Screen::About => Some("about"),
            Screen::Experience => Some("experience"),
            Screen::Skills => Some("skills"),
            Screen::Education => Some("education"),
            Screen::Projects => Some("projects"),
        }
    }

    fn from_slug(slug: &str) -> Option<Screen> {
        Screen::all().into_iter().find(|s| s.slug() == Some(slug))
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
    helm: bool,  // full-screen sailboat demo, opened from an entry with `demo: sail`
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
            helm: false,
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
        self.helm = false;
    }

    fn open_entry(&mut self, index: usize) {
        if index < self.entries_len() {
            self.selected_entry = index;
            self.selected_bullet = 0;
            self.level = Level::Detail;
            self.scroll = 0;
            self.helm = false;
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

    /// One step down (`forward`) or up, shared by ↑/↓ and the mouse wheel: moves
    /// the menu or list selection, or scrolls a reading view.
    fn step(&mut self, forward: bool, scroll_max: u16) {
        if self.helm {
            return;
        }
        if self.screen == Screen::Welcome {
            if forward {
                self.next_menu();
            } else {
                self.prev_menu();
            }
        } else if self.is_scroll_view() {
            self.scroll = if forward {
                (self.scroll + 1).min(scroll_max)
            } else {
                self.scroll.saturating_sub(1)
            };
        } else {
            self.step_selection(forward);
        }
    }

    /// True on the highlights view of an entry that has the sailboat demo.
    fn has_demo(&self) -> bool {
        self.level == Level::Detail && self.current_entry().is_some_and(|e| e.demo == "sail")
    }

    fn open_helm(&mut self) {
        if self.has_demo() {
            self.helm = true;
        }
    }

    /// The deep link for the current view, without the leading `#`:
    /// `experience`, `experience/razorsecure`, or `experience/razorsecure/2`
    /// (1-based highlight), plus `projects/<demo entry>/helm` for the sailboat
    /// demo. Empty on the welcome screen.
    fn route(&self) -> String {
        let Some(screen) = self.screen.slug() else {
            return String::new();
        };
        match (self.current_entry(), self.level) {
            (Some(e), Level::Detail) if self.helm => format!("{screen}/{}/helm", e.slug),
            (Some(e), Level::Detail) => format!("{screen}/{}", e.slug),
            (Some(e), Level::Focus) => format!("{screen}/{}/{}", e.slug, self.selected_bullet + 1),
            _ => screen.to_string(),
        }
    }

    /// Navigate to a deep link, going as deep as the route stays valid (an
    /// unknown entry lands on the section's list rather than failing).
    fn apply_route(&mut self, route: &str) {
        self.go_home();
        let mut parts = route
            .trim_start_matches('#')
            .split('/')
            .filter(|p| !p.is_empty());
        let Some(screen) = parts.next().and_then(Screen::from_slug) else {
            return;
        };
        self.goto_section(screen);
        let Some(entries) = section_entries(screen) else {
            return;
        };
        let Some(i) = parts
            .next()
            .and_then(|slug| entries.iter().position(|e| e.slug == slug))
        else {
            return;
        };
        self.open_entry(i);
        match parts.next() {
            Some("helm") => self.open_helm(),
            Some(n) => {
                if let Some(n) = n.parse::<usize>().ok().filter(|n| *n >= 1) {
                    self.open_bullet(n - 1);
                }
            }
            None => {}
        }
    }

    /// Step back one level: focus -> detail -> list -> home.
    fn back(&mut self) {
        if self.helm {
            self.helm = false;
        } else if self.is_section() {
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

    /// Move the selection to whatever the pointer is over. Only single-row
    /// items follow the pointer: multi-line highlights can be partly clipped,
    /// and selecting one would scroll the list out from under the cursor.
    fn hover(&mut self, action: ClickAction) {
        match action {
            ClickAction::Goto(s) => {
                if let Some(i) = Screen::all().iter().position(|x| *x == s) {
                    self.selected_menu = i;
                }
            }
            ClickAction::OpenEntry(i) => self.selected_entry = i,
            ClickAction::OpenBullet(_) | ClickAction::OpenHelm => {}
        }
    }

    fn activate(&mut self, action: ClickAction) {
        match action {
            ClickAction::Goto(s) => self.goto_section(s),
            ClickAction::OpenEntry(i) => self.open_entry(i),
            ClickAction::OpenBullet(i) => self.open_bullet(i),
            ClickAction::OpenHelm => self.open_helm(),
        }
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
    OpenHelm,
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

fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    // In "plain" view the static HTML CV is shown, so don't start the terminal at
    // all — this keeps the requestAnimationFrame render loop from running on
    // phones / low-power devices where the canvas UI is a poor experience.
    if view_mode().as_deref() == Some("plain") {
        return;
    }

    // The WebGL2 backend rasterises glyphs from the page font on demand, so wait
    // for Fira Code first — otherwise the cell size is measured (and glyphs are
    // cached) using the browser's fallback monospace font.
    wasm_bindgen_futures::spawn_local(async {
        load_font().await;
        start().expect("failed to start terminal");
    });
}

async fn load_font() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let promise = document
        .fonts()
        .load(&format!("{FONT_SIZE}px \"{FONT_FAMILY}\""));
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Create the backend and hand it to the app. WebGL2 is ratzilla's recommended
/// backend (GPU-rendered, full emoji support, clickable URLs); Canvas 2D is kept
/// as a fallback for browsers without WebGL2.
fn start() -> io::Result<()> {
    let webgl = WebGl2Backend::new_with_options(
        WebGl2BackendOptions::new()
            .grid_id(ROOT_ID)
            .font_atlas_config(FontAtlasConfig::dynamic(&[FONT_FAMILY], FONT_SIZE))
            .canvas_padding_color(NORD0)
            // Let index.html's CSS size the canvas to the viewport; the backend
            // then re-measures it every frame, so window resizes just work.
            .disable_auto_css_resize()
            .enable_hyperlinks(),
    );
    match webgl {
        Ok(backend) => run(Terminal::new(WideGlyphFix::new(backend))?),
        Err(e) => {
            web_sys::console::warn_1(&format!("WebGL2 unavailable ({e}); using canvas").into());
            let backend =
                CanvasBackend::new_with_options(CanvasBackendOptions::new().grid_id(ROOT_ID))?;
            run(Terminal::new(WideGlyphFix::new(backend))?)
        }
    }
}

/// Works around ghosting between ratatui 0.30 and ratzilla's WebGL2 backend.
/// beamterm draws a wide glyph (emoji) as two half-glyphs, the right half in
/// the next cell, but when a narrow symbol later replaces the wide one,
/// ratatui's diff doesn't resend that next cell (real terminals clear it
/// themselves) — so half an emoji lingers on screen. This remembers where wide
/// glyphs were drawn and also blanks the trailing cell when one is replaced.
struct WideGlyphFix<B> {
    inner: B,
    wide: HashSet<(u16, u16)>,
}

impl<B> WideGlyphFix<B> {
    fn new(inner: B) -> Self {
        Self {
            inner,
            wide: HashSet::new(),
        }
    }
}

impl<B: Backend> Backend for WideGlyphFix<B> {
    type Error = B::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a BufferCell)>,
    {
        let cols = self.inner.size()?.width;
        let content: Vec<_> = content.collect();
        // Blank cells to send, each right after the cell that displaced a
        // wide glyph, so a genuine update to the trailing cell later in the
        // same batch still wins.
        let blanks: Vec<Option<BufferCell>> = content
            .iter()
            .map(|&(x, y, cell)| {
                if cell.symbol().width() >= 2 {
                    self.wide.insert((x, y));
                    None
                } else if self.wide.remove(&(x, y)) && x + 1 < cols {
                    let mut blank = BufferCell::EMPTY;
                    blank.set_style(cell.style());
                    Some(blank)
                } else {
                    None
                }
            })
            .collect();
        let merged = content
            .iter()
            .zip(&blanks)
            .flat_map(|(&(x, y, cell), blank)| {
                std::iter::once((x, y, cell)).chain(blank.as_ref().map(|b| (x + 1, y, b)))
            });
        self.inner.draw(merged)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.wide.clear();
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.wide.clear();
        self.inner.clear_region(clear_type)
    }

    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error> {
        self.inner.append_lines(n)
    }
    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.hide_cursor()
    }
    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.show_cursor()
    }
    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.inner.get_cursor_position()
    }
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        self.inner.set_cursor_position(position)
    }
    fn size(&self) -> Result<Size, Self::Error> {
        self.inner.size()
    }
    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.inner.window_size()
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }
}

impl<B: WebEventHandler> WebEventHandler for WideGlyphFix<B> {
    fn on_mouse_event<F>(&mut self, callback: F) -> Result<(), RatzillaError>
    where
        F: FnMut(MouseEvent) + 'static,
    {
        self.inner.on_mouse_event(callback)
    }
    fn clear_mouse_events(&mut self) {
        self.inner.clear_mouse_events()
    }
    fn on_key_event<F>(&mut self, callback: F) -> Result<(), RatzillaError>
    where
        F: FnMut(KeyEvent) + 'static,
    {
        self.inner.on_key_event(callback)
    }
    fn clear_key_events(&mut self) {
        self.inner.clear_key_events()
    }
}

/// Give the terminal canvas keyboard focus; ratzilla listens for keys on the
/// canvas itself rather than the whole document.
fn focus_terminal() {
    if let Some(canvas) = terminal_canvas() {
        let _ = canvas.focus();
    }
}

fn terminal_canvas() -> Option<web_sys::HtmlElement> {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| {
            d.query_selector(&format!("#{ROOT_ID} canvas"))
                .ok()
                .flatten()
        })
        .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
}

/// Keys only reach ratzilla while the canvas has focus, and the browser can
/// move focus to `<body>` behind our back (e.g. a fragment navigation from the
/// address bar resets it after our `popstate` handler runs). When nothing else
/// has focus, take it back and forward the key to the canvas.
fn forward_stray_keys(event: web_sys::KeyboardEvent) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let stray = document
        .active_element()
        .is_none_or(|el| matches!(el.tag_name().as_str(), "BODY" | "HTML"));
    let Some(canvas) = terminal_canvas().filter(|_| stray) else {
        return;
    };
    event.prevent_default();
    let _ = canvas.focus();
    let init = web_sys::KeyboardEventInit::new();
    init.set_key(&event.key());
    init.set_code(&event.code());
    init.set_shift_key(event.shift_key());
    init.set_ctrl_key(event.ctrl_key());
    init.set_alt_key(event.alt_key());
    // Not bubbling: ratzilla listens on the canvas itself, and a bubbling copy
    // would re-enter this (non-reentrant) window listener.
    if let Ok(copy) = web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init) {
        let _ = canvas.dispatch_event(&copy);
    }
}

fn current_hash() -> String {
    web_sys::window()
        .and_then(|w| w.location().hash().ok())
        .unwrap_or_default()
}

/// Record the current view in the URL. Each navigation becomes a history
/// entry, so the browser's back button steps back through the CV; selection
/// changes don't alter the route and so don't add entries.
fn push_route(app: &App) {
    set_route(app, true);
}

fn set_route(app: &App, push: bool) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let route = app.route();
    if current_hash().trim_start_matches('#') == route {
        return;
    }
    let url = if route.is_empty() {
        // Drop the fragment entirely rather than leaving a bare `#`.
        let loc = window.location();
        loc.pathname().unwrap_or_default() + &loc.search().unwrap_or_default()
    } else {
        format!("#{route}")
    };
    if let Ok(history) = window.history() {
        let _ = if push {
            history.push_state_with_url(&JsValue::NULL, "", Some(&url))
        } else {
            history.replace_state_with_url(&JsValue::NULL, "", Some(&url))
        };
    }
}

/// Attach a DOM event listener for the lifetime of the page.
fn listen<E: FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    event: &str,
    handler: impl FnMut(E) + 'static,
) {
    let closure = Closure::<dyn FnMut(E)>::new(handler);
    let _ = target.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
    closure.forget();
}

/// Mouse-wheel / trackpad scrolling, which neither ratzilla backend reports.
/// Deltas are accumulated and converted into the same steps as ↑/↓, so a
/// trackpad's stream of small deltas and a wheel's few large ones feel alike.
fn wheel_handler(app: Rc<RefCell<App>>, scroll_max: Rc<Cell<u16>>) -> impl FnMut(WheelEvent) {
    const LINE_PX: f64 = 20.0;
    let mut pending = 0.0f64;
    move |ev: WheelEvent| {
        ev.prevent_default();
        let mut app = app.borrow_mut();
        let delta = match ev.delta_mode() {
            WheelEvent::DOM_DELTA_LINE => ev.delta_y() * LINE_PX,
            WheelEvent::DOM_DELTA_PAGE => ev.delta_y() * LINE_PX * 20.0,
            _ => ev.delta_y(),
        };
        // Reading views scroll a line per step; menus and lists move a whole
        // item, so they need a bigger push per step.
        let per_step = if app.is_scroll_view() {
            LINE_PX
        } else {
            3.0 * LINE_PX
        };
        pending += delta;
        while pending.abs() >= per_step {
            app.step(pending > 0.0, scroll_max.get());
            pending -= per_step.copysign(pending);
        }
    }
}

/// Frame timing for the animations, from the browser's monotonic clock.
struct Clock {
    performance: Option<web_sys::Performance>,
    start: f64,
    last: Cell<f64>,
    reduced_motion: bool,
}

impl Clock {
    fn new() -> Self {
        let window = web_sys::window();
        let performance = window.as_ref().and_then(|w| w.performance());
        let now = performance.as_ref().map_or(0.0, |p| p.now());
        let reduced_motion = window
            .and_then(|w| {
                w.match_media("(prefers-reduced-motion: reduce)")
                    .ok()
                    .flatten()
            })
            .is_some_and(|m| m.matches());
        Self {
            performance,
            start: now,
            last: Cell::new(now),
            reduced_motion,
        }
    }

    /// Seconds since the last frame (capped, so a backgrounded tab doesn't make
    /// the boat jump on return) and seconds since start.
    fn tick(&self) -> (f64, f64) {
        let now = self.performance.as_ref().map_or(0.0, |p| p.now());
        let dt = ((now - self.last.replace(now)) / 1000.0).clamp(0.0, 0.1);
        (dt, (now - self.start) / 1000.0)
    }
}

fn run<B>(mut terminal: Terminal<B>) -> io::Result<()>
where
    B: Backend + WebEventHandler + 'static,
{
    let app = Rc::new(RefCell::new(App::new()));
    let regions: Rc<Regions> = Rc::new(RefCell::new(Vec::new()));
    // Maximum scroll offset for the current view, computed each render so the
    // key handler can clamp downward scrolling at the bottom of the content.
    let scroll_max: Rc<Cell<u16>> = Rc::new(Cell::new(0));
    // The sailboat demo keeps sailing whichever screen is showing, so the
    // preview is mid-passage rather than restarting each time it's opened.
    let sim = Rc::new(RefCell::new(sail::Sim::new()));

    // Open whatever the URL points at (normalising a stale or mistyped link to
    // the view actually shown), and follow the browser's back/forward —
    // re-focusing the terminal, which history navigation can blur.
    app.borrow_mut().apply_route(&current_hash());
    set_route(&app.borrow(), false);
    listen(&web_sys::window().expect("window"), "popstate", {
        let app = app.clone();
        move |_: web_sys::Event| {
            app.borrow_mut().apply_route(&current_hash());
            focus_terminal();
        }
    });

    if let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(ROOT_ID))
    {
        listen(
            &root,
            "wheel",
            wheel_handler(app.clone(), scroll_max.clone()),
        );
    }

    terminal.on_key_event({
        let app = app.clone();
        let scroll_max = scroll_max.clone();
        let sim = sim.clone();
        move |key_event| {
            let mut app = app.borrow_mut();
            if app.helm {
                let mut sim = sim.borrow_mut();
                // Shift for fine helm adjustments.
                let step = if key_event.shift { 2.0 } else { 10.0 };
                match key_event.code {
                    KeyCode::Left | KeyCode::Char('h' | 'a' | 'H' | 'A') => sim.steer(-step),
                    KeyCode::Right | KeyCode::Char('l' | 'd' | 'L' | 'D') => sim.steer(step),
                    KeyCode::Char(' ') | KeyCode::Enter => sim.toggle_autopilot(),
                    KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => app.back(),
                    _ => {}
                }
                push_route(&app);
                return;
            }
            match key_event.code {
                KeyCode::Char('d') => app.open_helm(),
                KeyCode::Char('q') => app.go_home(),
                KeyCode::Esc | KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                    app.back()
                }
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
                KeyCode::Down | KeyCode::Char('j') => app.step(true, scroll_max.get()),
                KeyCode::Up | KeyCode::Char('k') => app.step(false, scroll_max.get()),
                KeyCode::PageDown if app.is_scroll_view() => {
                    app.scroll = (app.scroll + 10).min(scroll_max.get());
                }
                KeyCode::PageUp if app.is_scroll_view() => {
                    app.scroll = app.scroll.saturating_sub(10);
                }
                KeyCode::Home if app.is_scroll_view() => app.scroll = 0,
                KeyCode::End if app.is_scroll_view() => app.scroll = scroll_max.get(),
                KeyCode::Char('1') => app.goto_section(Screen::About),
                KeyCode::Char('2') => app.goto_section(Screen::Experience),
                KeyCode::Char('3') => app.goto_section(Screen::Skills),
                KeyCode::Char('4') => app.goto_section(Screen::Education),
                KeyCode::Char('5') => app.goto_section(Screen::Projects),
                _ => {}
            }
            push_route(&app);
        }
    })?;

    // Mouse events arrive in terminal grid coordinates, so they can be
    // hit-tested directly against the regions recorded during the last render.
    terminal.on_mouse_event({
        let app = app.clone();
        let regions = regions.clone();
        move |ev| {
            let regions = regions.borrow();
            let pos = Position::new(ev.col, ev.row);
            let hit = regions
                .iter()
                .find(|(rect, _)| rect.contains(pos))
                .map(|(_, action)| *action);
            let mut app = app.borrow_mut();
            match ev.kind {
                MouseEventKind::Moved => {
                    if let Some(action) = hit {
                        app.hover(action);
                    }
                }
                // ButtonUp rather than SingleClick: the WebGL2 backend only
                // reports raw button transitions.
                MouseEventKind::ButtonUp(MouseButton::Left) => {
                    match hit {
                        Some(action) => app.activate(action),
                        // A click on a screen with no interactive regions
                        // (focus / About / Skills) steps back one level.
                        // (The helm screen has none either, but a stray click
                        // there shouldn't drop you out of the demo.)
                        None if regions.is_empty() && !app.helm => app.back(),
                        None => {}
                    }
                    push_route(&app);
                }
                _ => {}
            }
        }
    })?;

    focus_terminal();
    listen(
        &web_sys::window().expect("window"),
        "keydown",
        forward_stray_keys,
    );

    let clock = Clock::new();
    terminal.draw_web(move |f| {
        let (dt, t) = clock.tick();
        sim.borrow_mut().step(dt);
        // With reduced motion requested, the sea holds still (the sailboat
        // demo still sails: it's something the visitor chose to open).
        let sea_t = if clock.reduced_motion { 0.0 } else { t };
        ui(
            f,
            &app.borrow(),
            &regions,
            &scroll_max,
            &sim.borrow(),
            sea_t,
        );
    });

    Ok(())
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
    lines.iter().fold(0u16, |acc, l| {
        acc.saturating_add(wrap_count(&line_text(l), width))
    })
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
    panel(f, area);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((scroll.min(max), 0))
            .style(Style::default().fg(NORD6)),
        area,
    );
}

// Rendering -------------------------------------------------------------------

fn ui(
    f: &mut Frame<'_>,
    app: &App,
    regions: &Regions,
    scroll_max: &Cell<u16>,
    sim: &sail::Sim,
    t: f64,
) {
    regions.borrow_mut().clear();
    scroll_max.set(0);

    // The sea fills the screen; content panels are drawn opaque over it.
    let area = f.area();
    sea::render(f.buffer_mut(), area, t);

    if app.helm {
        render_helm(f, app, sim);
        return;
    }
    match app.screen {
        Screen::Welcome => render_welcome(f, app, regions),
        Screen::About => render_about(f, app, scroll_max),
        Screen::Skills => render_skills(f, app, scroll_max),
        _ => match app.level {
            Level::List => render_list(f, app, regions),
            Level::Detail => render_detail(f, app, regions, sim),
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

/// Blank `area` to the plain background, so a panel drawn there sits on top of
/// the sea rather than letting it show through around the text.
fn panel(f: &mut Frame<'_>, area: Rect) {
    f.render_widget(Clear, area);
    f.render_widget(Block::default().style(Style::default().bg(NORD0)), area);
}

fn title_bar(f: &mut Frame<'_>, area: Rect, title: &str) {
    panel(f, area);
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

fn link(url: String) -> Span<'static> {
    Span::styled(
        url,
        Style::default()
            .fg(FROST)
            .add_modifier(Modifier::UNDERLINED),
    )
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
        // Full https:// URLs so the WebGL2 backend makes them clickable.
        Line::from(vec![
            Span::raw(format!("📧 {}  🌐 ", PROFILE.email)),
            link(format!("https://{}", PROFILE.homepage)),
            Span::raw("  💼 "),
            link(format!("https://{}", PROFILE.github)),
        ]),
        Line::from(format!("📱 {}  📍 {}", PROFILE.phone, PROFILE.location)),
        Line::from(""),
    ])
    // Explicit base colour: unstyled spans would otherwise keep the sea's dim fg.
    .style(Style::default().fg(NORD6));

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
    panel(f, menu_area);
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
                Span::styled(
                    e.title,
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  ·  {}", e.org), Style::default().fg(NORD4)),
                Span::styled(format!("   {}", e.date), Style::default().fg(NORD3)),
            ]))
        })
        .collect();

    panel(f, list_area);
    let (block, _) = list_block(
        regions,
        list_area,
        "Select a role",
        entries.len(),
        ClickAction::OpenEntry,
    );

    let mut state = ListState::default();
    state.select(Some(
        app.selected_entry.min(entries.len().saturating_sub(1)),
    ));
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().fg(NORD0).bg(FROST))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, list_area, &mut state);

    footer(f, footer_area, "↑↓ Select • Enter/→ Open role • Esc Back");
}

/// Level 2: the selected role's highlights, each shown in full (bold lead plus
/// the wrapped detail) as a selectable, scrollable list.
fn render_detail(f: &mut Frame<'_>, app: &App, regions: &Regions, sim: &sail::Sim) {
    let Some(entry) = app.current_entry() else {
        return;
    };
    let accent = accent_color(entry.accent);
    let [title_area, body_area, footer_area] = content_layout(f.area());
    title_bar(
        f,
        title_area,
        &format!("{} {} · {}", entry.emoji, entry.title, entry.org),
    );

    let content = body_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    // An entry with the sailboat demo shares the body with a live nav chart:
    // side by side when there's room, otherwise the chart below.
    let (list_area, chart_area) = if app.has_demo() {
        let [list, chart] = if content.width >= 100 {
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
                .spacing(1)
                .areas(content)
        } else {
            Layout::vertical([Constraint::Fill(1), Constraint::Length(14)]).areas(content)
        };
        (list, Some(chart))
    } else {
        (content, None)
    };
    if let Some(chart) = chart_area {
        panel(f, chart);
        sail::render(
            f,
            chart,
            sim,
            " ⛵ Live nav · click or press D to take the helm ",
            true,
        );
        regions.borrow_mut().push((chart, ClickAction::OpenHelm));
    }
    panel(f, list_area);

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
                    Line::from(b.lead)
                        .style(Style::default().fg(accent).add_modifier(Modifier::BOLD)),
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
    state.select(Some(
        app.selected_bullet
            .min(entry.bullets.len().saturating_sub(1)),
    ));
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
        regs.push((
            Rect::new(inner.x, y, inner.width, h),
            ClickAction::OpenBullet(i),
        ));
        y += heights[i];
    }

    let hint = if app.has_demo() {
        "↑↓ Select • Enter/→ Read full • D Take the helm • ← Back"
    } else {
        "↑↓ Select • Enter/→ Read full • ← Back to roles"
    };
    footer(f, footer_area, hint);
}

/// Full-screen sailboat demo: the visitor steers, or hands back to the autopilot.
fn render_helm(f: &mut Frame<'_>, app: &App, sim: &sail::Sim) {
    let [title_area, body_area, footer_area] = content_layout(f.area());
    let name = app.current_entry().map_or("Sailboat", |e| e.org);
    title_bar(f, title_area, &format!("⛵ {name} · at the helm"));
    let chart = body_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    panel(f, chart);
    sail::render(f, chart, sim, " Nav chart ", false);
    footer(
        f,
        footer_area,
        "←/→ Steer (Shift: fine) • Space Autopilot • Esc Back",
    );
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
    panel(f, layout[0]);
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
        lines.push(
            Line::from(cat.name).style(Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        );
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
