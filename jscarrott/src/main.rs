use std::{cell::RefCell, io, rc::Rc};

use ratzilla::{
    ratatui::{
        prelude::*,
        widgets::{Block, Clear},
    },
    CanvasBackend, DomBackend, WebRenderer,
};
use tachyonfx::{
    fx, CenteredShrink, Duration, Effect, EffectRenderer, EffectTimer, Interpolation, Motion,
    Shader,
};

#[derive(Copy, Debug, Clone)]
enum Screen {
    Main,
    Title,
}

struct App {
    screen: Screen,
}

fn main() -> io::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    let backend = DomBackend::new()?;
    let terminal = Terminal::new(backend)?;
    let mut effect = fx::sequence(&[
        // first we "sweep in" the text from the left, before reversing the effect
        fx::ping_pong(fx::sweep_in(
            Motion::LeftToRight,
            10,
            0,
            Color::DarkGray,
            EffectTimer::from_ms(5000, Interpolation::QuadIn),
        )),
        // then we coalesce the text back to its original state
        // (note that EffectTimers can be constructed from a tuple of duration and interpolation)
        fx::coalesce((2000, Interpolation::SineOut)),
    ]);
    let app = Rc::new(RefCell::new(App {
        screen: Screen::Title,
    }));
    terminal.on_key_event({
        let app = app.clone();
        move |key_event| {
            let mut app = app.borrow_mut();
            app.screen = Screen::Main;
            println!("Set {:?}", app.screen)
        }
    });

    terminal.draw_web(move |f| ui(f, &mut effect, app.clone()));

    Ok(())
}

fn ui(f: &mut Frame<'_>, effect: &mut Effect, app: Rc<RefCell<App>>) {
    let screen = app.borrow().screen;
    match screen {
        Screen::Title => render_title(f, effect, app),
        Screen::Main => render_main(f, effect, app),
    }
}

fn render_title(f: &mut Frame<'_>, effect: &mut Effect, app: Rc<RefCell<App>>) {
    Clear.render(f.area(), f.buffer_mut());
    Block::default()
        .style(Style::default().bg(Color::Black))
        .render(f.area(), f.buffer_mut());

    let area = f.area().inner_centered(25, 2);
    let main_text = Text::from(vec![
        Line::from("Welcome to my personal website"),
        Line::from("<Press any key to continue>"),
    ]);
    f.render_widget(main_text.light_magenta().centered(), area);
    if effect.running() {
        f.render_effect(effect, area, Duration::from_millis(400));
    }
}
fn render_main(f: &mut Frame<'_>, effect: &mut Effect, app: Rc<RefCell<App>>) {
    Clear.render(f.area(), f.buffer_mut());
    Block::default()
        .style(Style::default().bg(Color::Black))
        .render(f.area(), f.buffer_mut());

    let area = f.area().inner_centered(25, 2);
    let main_text = Text::from(vec![Line::from("Main Page")]);
    f.render_widget(main_text.light_magenta().centered(), area);
    if effect.running() {
        f.render_effect(effect, area, Duration::from_millis(400));
    }
}
