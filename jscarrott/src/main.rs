use std::{cell::RefCell, io, rc::Rc};

mod content;
use content::PROFILE;

use ratzilla::{
    event::KeyCode, ratatui::{
        prelude::*,
        widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    }, DomBackend, WebRenderer
};

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
        .style(Style::default().bg(Color::Rgb(46, 52, 64))) // Nord0 - polar night
        .render(f.area(), f.buffer_mut());

    match app_ref.screen {
        Screen::Welcome => render_welcome(f, &app_ref),
        Screen::About => render_about(f),
        Screen::Experience => render_experience(f),
        Screen::Skills => render_skills(f),
        Screen::Education => render_education(f),
        Screen::Projects => render_projects(f),
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

    // Header with Nord colors
    let header_text = Text::from(vec![
        Line::from(""),
        Line::from(PROFILE.name).style(Style::default().fg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD)), // Nord8 - frost
        Line::from(PROFILE.position).style(Style::default().fg(Color::Rgb(235, 203, 139))), // Nord13 - aurora yellow
        Line::from(""),
        Line::from(format!("📧 {}  🌐 {}  💼 {}", PROFILE.email, PROFILE.homepage, PROFILE.github)),
        Line::from(format!("📱 {}  📍 {}", PROFILE.phone, PROFILE.location)),
        Line::from(""),
    ]);
    
    let header_area = main_layout[0].inner(Margin { horizontal: 2, vertical: 1 });
    f.render_widget(header_text.centered(), header_area);

    // Menu
    let screens = Screen::all();
    let menu_items: Vec<ListItem> = screens
        .iter()
        .enumerate()
        .map(|(i, screen)| {
            let style = if i == app.selected_menu {
                Style::default().fg(Color::Rgb(46, 52, 64)).bg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD) // Nord0 on Nord8
            } else {
                Style::default().fg(Color::Rgb(236, 239, 244)) // Nord6 - snow storm
            };
            
            let number = format!("{}. ", i + 1);
            ListItem::new(Line::from(vec![
                Span::styled(number, Style::default().fg(Color::Rgb(235, 203, 139))), // Nord13 - aurora yellow
                Span::styled(screen.title(), style),
            ]))
        })
        .collect();

    let menu = List::new(menu_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(143, 188, 187))) // Nord7 - frost teal
                .title(" Navigation ")
                .title_style(Style::default().fg(Color::Rgb(143, 188, 187)).add_modifier(Modifier::BOLD)) // Nord7 - frost teal
        );

    let menu_area = main_layout[1].inner(Margin { horizontal: 4, vertical: 1 });
    f.render_widget(menu, menu_area);

    // Instructions
    let instructions = Text::from(vec![
        Line::from("↑↓ Navigate • Enter/Space Select • 1-5 Quick Jump • Q/Esc Home"),
    ]);
    
    let instructions_area = main_layout[2].inner(Margin { horizontal: 2, vertical: 0 });
    f.render_widget(
        instructions.centered().style(Style::default().fg(Color::Rgb(76, 86, 106))), // Nord3 - polar night
        instructions_area,
    );

}

fn render_about(f: &mut Frame<'_>) {
    let area = f.area();
    
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    // Title
    let title = Paragraph::new("About Me")
        .style(Style::default().fg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD)) // Nord8 - frost
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(136, 192, 208))));
    f.render_widget(title, layout[0]);

    // Content
    let content = Text::from(vec![
        Line::from(""),
        Line::from("I'm a passionate Senior Software Engineer with over 8 years of experience"),
        Line::from("specializing in systems programming, high-performance computing, and"),
        Line::from("infrastructure automation. Currently leading platform development at"),
        Line::from("Razorsecure, where I architect solutions for next-generation security"),
        Line::from("products."),
        Line::from(""),
        Line::from("My expertise spans from low-level systems programming in Rust and C++"),
        Line::from("to full-stack development with modern frameworks. I have extensive"),
        Line::from("experience with containerization, CI/CD pipelines, and cloud"),
        Line::from("infrastructure deployment."),
        Line::from(""),
        Line::from("Core Competencies:"),
        Line::from("• High-performance systems and network programming"),
        Line::from("• Embedded systems and microcontroller development"),
        Line::from("• DevOps and infrastructure automation"),
        Line::from("• Open source contribution and community building"),
        Line::from(""),
        Line::from("I'm passionate about solving complex technical challenges and"),
        Line::from("contributing to the open source community. When not coding, I enjoy"),
        Line::from("working on personal projects that address real-world problems."),
    ]);

    let content_area = layout[1].inner(Margin { horizontal: 4, vertical: 1 });
    f.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::Rgb(236, 239, 244))), // Nord6 - snow storm
        content_area,
    );

    // Navigation hint
    let nav_hint = Paragraph::new("Press Q or Esc to return to main menu")
        .style(Style::default().fg(Color::Rgb(76, 86, 106))) // Nord3 - polar night
        .alignment(Alignment::Center);
    f.render_widget(nav_hint, layout[2]);
}

fn render_experience(f: &mut Frame<'_>) {
    let area = f.area();
    
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    // Title
    let title = Paragraph::new("Professional Experience")
        .style(Style::default().fg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD)) // Nord8 - frost
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(136, 192, 208))));
    f.render_widget(title, layout[0]);

    // Content
    let content = Text::from(vec![
        Line::from(""),
        Line::from("🚀 Senior Rust Software Engineer @ Razorsecure").style(Style::default().fg(Color::Rgb(235, 203, 139)).add_modifier(Modifier::BOLD)), // Nord13 - aurora yellow
        Line::from("   December 2019 - Present | Remote").style(Style::default().fg(Color::Rgb(216, 222, 233))), // Nord4 - snow storm
        Line::from(""),
        Line::from("   • Platform Lead: Drive technical direction and roadmap for core product platform"),
        Line::from("   • High-Performance Networking: Developed zero-copy packet inspection library"),
        Line::from("     achieving sub-microsecond latency and 1M+ packets per second throughput"),
        Line::from("   • Infrastructure Architecture: Deployed and maintained air-gapped Kubernetes"),
        Line::from("     clusters for both on-premise and cloud environments"),
        Line::from("   • Security Engineering: Built L2-L7 firewall for embedded security gateways"),
        Line::from("   • Technical Migration: Led successful migration from Python to high-performance"),
        Line::from("     Rust client for flagship product"),
        Line::from("   • Microservices Management: Orchestrated 20+ microservices using monorepo"),
        Line::from("     build tools and modern CI/CD practices"),
        Line::from("   • Open Source: Contributed to Rust libraries for network monitoring and"),
        Line::from("     performance analysis"),
        Line::from(""),
        Line::from("⚙️  Software Engineer @ Helitune/Beran Instruments").style(Style::default().fg(Color::Rgb(163, 190, 140)).add_modifier(Modifier::BOLD)), // Nord14 - aurora green
        Line::from("   June 2015 - December 2019 | Torrington, North Devon").style(Style::default().fg(Color::Rgb(216, 222, 233))), // Nord4 - snow storm
        Line::from(""),
        Line::from("   • Systems Engineering: Developed next-generation protection and condition"),
        Line::from("     monitoring systems deployed at NASA Ames Research Center"),
        Line::from("   • Embedded Development: Built best-in-class Rotor Track and Balance systems"),
        Line::from("     meeting DO-178 Level C/D aviation standards"),
        Line::from("   • Architecture Design: Implemented modular C++ signal processing system"),
        Line::from("     reducing development time and improving system robustness"),
        Line::from("   • Full-Stack Development: Maintained large-scale C#/WPF applications"),
        Line::from("     (100,000+ lines) with TDD and BDD methodologies"),
        Line::from("   • Quality Assurance: Created automated testing framework saving hundreds"),
        Line::from("     of development hours and preventing production issues"),
        Line::from("   • DevOps Implementation: Led transition from TFS to Git, establishing"),
        Line::from("     code review workflows and CI/CD pipelines with TeamCity"),
        Line::from("   • Cross-Functional Leadership: Collaborated with multidisciplinary teams"),
        Line::from("     to deliver critical solutions under tight deadlines"),
    ]);

    let content_area = layout[1].inner(Margin { horizontal: 2, vertical: 1 });
    f.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White)),
        content_area,
    );

    // Navigation hint
    let nav_hint = Paragraph::new("Press Q or Esc to return to main menu")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(nav_hint, layout[2]);
}

fn render_skills(f: &mut Frame<'_>) {
    let area = f.area();
    
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    // Title
    let title = Paragraph::new("Technical Skills")
        .style(Style::default().fg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD)) // Nord8 - frost
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(136, 192, 208))));
    f.render_widget(title, layout[0]);

    // Content
    let content = Text::from(vec![
        Line::from(""),
        Line::from("🦀 Systems Programming").style(Style::default().fg(Color::Rgb(235, 203, 139)).add_modifier(Modifier::BOLD)), // Nord13 - aurora yellow
        Line::from("   Rust, C++, C - Low-level systems, embedded development, performance optimization"),
        Line::from(""),
        Line::from("🐍 Application Development").style(Style::default().fg(Color::Rgb(163, 190, 140)).add_modifier(Modifier::BOLD)), // Nord14 - aurora green
        Line::from("   Python, C#, F#, Java, PowerShell - Full-stack applications and automation"),
        Line::from(""),
        Line::from("🧠 Development Methodologies").style(Style::default().fg(Color::Rgb(180, 142, 173)).add_modifier(Modifier::BOLD)), // Nord15 - aurora purple
        Line::from("   Test-Driven Development, Behavior-Driven Development, Extreme Programming"),
        Line::from(""),
        Line::from("🗄️  Data & Storage").style(Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)), // Nord9 - frost blue
        Line::from("   SQL, Redis, QuestDB, SQL Server, PostgreSQL - Database design and optimization"),
        Line::from(""),
        Line::from("📦 Containerization & Orchestration").style(Style::default().fg(Color::Rgb(143, 188, 187)).add_modifier(Modifier::BOLD)), // Nord7 - frost teal
        Line::from("   Docker, Podman, Kubernetes, GKE, AKS - Cloud-native deployment and scaling"),
        Line::from(""),
        Line::from("🏗️  Infrastructure & DevOps").style(Style::default().fg(Color::Rgb(191, 97, 106)).add_modifier(Modifier::BOLD)), // Nord11 - aurora red
        Line::from("   Terraform, Datadog, Jaeger, GitHub Actions, FluxCD, ArgoCD"),
        Line::from(""),
        Line::from("🧪 Testing & Quality").style(Style::default().fg(Color::Rgb(208, 135, 112)).add_modifier(Modifier::BOLD)), // Nord12 - aurora orange
        Line::from("   Pytest, NUnit, GoogleTest, GoogleMock, MSTest, Isolate"),
        Line::from(""),
        Line::from("📋 Project Management").style(Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)), // Nord10 - frost dark blue
        Line::from("   GitHub Issues, JIRA, Confluence, Agile, SCRUM - Team collaboration and delivery"),
    ]);

    let content_area = layout[1].inner(Margin { horizontal: 2, vertical: 1 });
    f.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White)),
        content_area,
    );

    // Navigation hint
    let nav_hint = Paragraph::new("Press Q or Esc to return to main menu")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(nav_hint, layout[2]);
}

fn render_education(f: &mut Frame<'_>) {
    let area = f.area();
    
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    // Title
    let title = Paragraph::new("Education & Training")
        .style(Style::default().fg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD)) // Nord8 - frost
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(136, 192, 208))));
    f.render_widget(title, layout[0]);

    // Content
    let content = Text::from(vec![
        Line::from(""),
        Line::from("🎓 BSc. Computing and Psychology").style(Style::default().fg(Color::Rgb(235, 203, 139)).add_modifier(Modifier::BOLD)), // Nord13 - aurora yellow
        Line::from("   The Open University | 2011 - 2015"),
        Line::from("   • Comprehensive study of software development and user interface psychology"),
        Line::from("   • Understanding of both technical implementation and human-computer interaction"),
        Line::from("   • Foundation in cognitive psychology applied to software design"),
        Line::from(""),
        Line::from("⚡ C for Real-Time Developers").style(Style::default().fg(Color::Rgb(163, 190, 140)).add_modifier(Modifier::BOLD)), // Nord14 - aurora green
        Line::from("   Feabhas, Royal Wootton Bassett | 2016"),
        Line::from("   • Intensive 5-day course on low-level C programming for embedded systems"),
        Line::from("   • Real-time systems development with and without RTOS"),
        Line::from("   • Critical timing analysis and performance optimization techniques"),
        Line::from(""),
        Line::from("🔧 Advanced C++ Development").style(Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)), // Nord9 - frost blue
        Line::from("   Feabhas, Royal Wootton Bassett | 2016"),
        Line::from("   • Advanced C++ programming for microcontroller environments"),
        Line::from("   • Memory management, templates, and modern C++ features"),
        Line::from("   • Best practices for embedded systems and performance-critical applications"),
        Line::from(""),
        Line::from("📚 Continuous Professional Development"),
        Line::from("   • Active participation in Rust and systems programming communities"),
        Line::from("   • Regular attendance at technical conferences and workshops"),
        Line::from("   • Contribution to open source projects and knowledge sharing"),
        Line::from("   • Staying current with emerging technologies and industry trends"),
    ]);

    let content_area = layout[1].inner(Margin { horizontal: 2, vertical: 1 });
    f.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White)),
        content_area,
    );

    // Navigation hint
    let nav_hint = Paragraph::new("Press Q or Esc to return to main menu")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(nav_hint, layout[2]);
}

fn render_projects(f: &mut Frame<'_>) {
    let area = f.area();
    
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    // Title
    let title = Paragraph::new("Projects & Contributions")
        .style(Style::default().fg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD)) // Nord8 - frost
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(136, 192, 208))));
    f.render_widget(title, layout[0]);

    // Content
    let content = Text::from(vec![
        Line::from(""),
        Line::from("📖 Md-book Combiner").style(Style::default().fg(Color::Rgb(235, 203, 139)).add_modifier(Modifier::BOLD)), // Nord13 - aurora yellow
        Line::from("   Author | 2023 - Present | github.com/jscarrott"),
        Line::from(""),
        Line::from("   • High-value documentation tool for combining multiple mdbook repositories"),
        Line::from("   • Automated release pipeline for seamless deployment and distribution"),
        Line::from("   • Rust-based CLI tool with focus on developer experience and performance"),
        Line::from("   • Solves complex documentation workflow challenges for large projects"),
        Line::from(""),
        Line::from("🔧 Rust nRF52 Hardware Abstraction Layer").style(Style::default().fg(Color::Rgb(163, 190, 140)).add_modifier(Modifier::BOLD)), // Nord14 - aurora green
        Line::from("   Contributor | 2018 - Present | Open Source"),
        Line::from(""),
        Line::from("   • Pioneering development of Rust hardware abstraction for Nordic microcontrollers"),
        Line::from("   • First to successfully run Rust on the nRF52840 chip"),
        Line::from("   • Contributed to the growing Rust embedded ecosystem and community"),
        Line::from("   • Advanced embedded systems programming and cross-platform development"),
        Line::from(""),
        Line::from("⚙️  Shell Configuration Manager (PshConfigMan)").style(Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)), // Nord9 - frost blue
        Line::from("   Author | 2018 - Present | PowerShell Gallery"),
        Line::from(""),
        Line::from("   • Cross-platform shell configuration management for PowerShell and Zsh"),
        Line::from("   • Leverages distributed version control principles for configuration sync"),
        Line::from("   • Available as official PowerShell package on PowerShell Gallery"),
        Line::from("   • Streamlines development environment setup across multiple machines"),
        Line::from(""),
        Line::from("🌐 Personal Website").style(Style::default().fg(Color::Rgb(180, 142, 173)).add_modifier(Modifier::BOLD)), // Nord15 - aurora purple
        Line::from("   • Modern terminal-style interface built with Rust and Ratzilla framework"),
        Line::from("   • Demonstrates advanced Rust web development capabilities"),
        Line::from("   • Responsive design with Nord color theme and interactive navigation"),
        Line::from("   • Showcases systems programming expertise in web context"),
    ]);

    let content_area = layout[1].inner(Margin { horizontal: 2, vertical: 1 });
    f.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White)),
        content_area,
    );

    // Navigation hint
    let nav_hint = Paragraph::new("Press Q or Esc to return to main menu")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(nav_hint, layout[2]);
}
