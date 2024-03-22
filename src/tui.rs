//! This module contains the TUI implementation for the Dead Man's Switch.

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};

/// The ASCII art for the TUI's main block.
const ASCII_ART: [&str; 5] = [
    "██████  ███████  █████  ██████      ███    ███  █████  ███    ██ ███████     ███████ ██     ██ ██ ████████  ██████ ██   ██",
    "██   ██ ██      ██   ██ ██   ██     ████  ████ ██   ██ ████   ██ ██          ██      ██     ██ ██    ██    ██      ██   ██",
    "██   ██ █████   ███████ ██   ██     ██ ████ ██ ███████ ██ ██  ██ ███████     ███████ ██  █  ██ ██    ██    ██      ███████",
    "██   ██ ██      ██   ██ ██   ██     ██  ██  ██ ██   ██ ██  ██ ██      ██          ██ ██ ███ ██ ██    ██    ██      ██   ██",
    "██████  ███████ ██   ██ ██████      ██      ██ ██   ██ ██   ████ ███████     ███████  ███ ███  ██    ██     ██████ ██   ██",
];

/// The main UI function.
///
/// This function will render the UI.
/// It's a simple UI with 3 blocks.
fn ui<B: Backend>(f: &mut Frame<B>, config_state: &ConfigState) {
    let modal_area = create_modal_rect(f.size(), 1.2);
    // Draw the modal if config_state.0 is true
    if config_state.0 {
        modal_config(f, modal_area);
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Max(3),
                Constraint::Percentage(40),
                Constraint::Max(6),
                Constraint::Max(5),
            ]
            .as_ref(),
        )
        .split(f.size());

    let block = legend_block();
    f.render_widget(block, chunks[0]);
    let block = ascii_block(ASCII_ART.as_ref());
    f.render_widget(block, chunks[1]);
    let block = instructions_block();
    f.render_widget(block, chunks[2]);
    let block = timer_block();
    f.render_widget(block, chunks[3]);
}

/// Whether the config modal is open or not.
struct ConfigState(bool);

/// Toggle the config modal.
impl ConfigState {
    fn toggle(&mut self) {
        self.0 = !self.0;
    }
}

/// The modal for the config.
fn modal_config<B: Backend>(f: &mut Frame<B>, area: Rect) {
    let block = Block::default()
        .title("Config")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));

    f.render_widget(block, area);

    // Define the layout within the modal
    let inner_area = Layout::default()
        .direction(Direction::Vertical)
        .margin(1) // Add some margin within the modal
        .constraints(
            [
                Constraint::Length(3), // For a title or some text
                Constraint::Length(3), // For additional information or input
                Constraint::Min(1),    // For other content or a message
            ]
            .as_ref(),
        )
        .split(area);

    // Example widget 1: Title
    let title =
        Paragraph::new(Span::raw("Modal Title")).block(Block::default().borders(Borders::NONE));
    f.render_widget(title, inner_area[0]);

    // Example widget 2: Additional Information
    let info = Paragraph::new(Span::raw("Some important info"))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(info, inner_area[1]);

    // Example widget 3: Other Content
    let other_content = Paragraph::new(Span::raw("Other content goes here..."))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(other_content, inner_area[2]);
}

/// Create a modal [`Rect`] based on a relative width.
///
/// The modal will be centered on the screen.
fn create_modal_rect(size: Rect, relative_width: f32) -> Rect {
    let modal_width = (size.width as f32 / relative_width) as u16;
    let modal_height = (size.height as f32 / relative_width) as u16;

    // Center the modal
    let modal_x = (size.width - modal_width) / 2;
    let modal_y = (size.height - modal_height) / 2;

    Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    }
}

/// The legend block.
///
/// Contains the keys legend for the TUI.
fn legend_block() -> Paragraph<'static> {
    let text = vec![Spans::from(vec![
        Span::styled(
            "c",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Check-In"),
        Span::raw("    "),
        Span::styled(
            "o",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Options"),
        Span::raw("    "),
        Span::styled(
            "q/Esc",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Quit"),
    ])];
    let block = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().title("Keys").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    block
}

/// The Instructions block.
///
/// Contains the instructions for the TUI.
fn instructions_block() -> Paragraph<'static> {
    let text = vec![
        Spans::from(vec![
            Span::styled(
                "1. ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Open the Config with "),
            Span::styled(
                "o",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" and modify the settings."),
        ]),
        Spans::from(vec![
            Span::styled(
                "2. ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Check-In with ",
            ),
            Span::styled(
                "c",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                " within the warning time.",
            ),
        ]),
        Spans::from(vec![
            Span::styled(
                "3. ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                "Otherwise the Dead Man's Switch will be triggered and the message with optional attachment will be sent.",
            ),
        ]),
    ];
    let block = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Left)
        .block(Block::default().title("Instructions").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    block
}

/// The ASCII block.
///
/// Contains the ASCII art for the TUI.
fn ascii_block(content: &[&'static str]) -> Paragraph<'static> {
    let text: Vec<Spans<'_>> = content
        .iter()
        .map(|line| {
            Spans::from(Span::styled(
                *line,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();

    let block = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().title("").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    block
}

/// The timer block.
///
/// Contains a [`Gauge`] widget to display the timer.
/// The timer will be updated every second.
///
/// ## Notes
///
/// The timer will be green if is still counting the warning time.
/// Eventually, it will turn red when the warning time is done,
/// and start counting the dead man's switch timer.
fn timer_block() -> Gauge<'static> {
    Gauge::default()
        .percent(30)
        .ratio(0.3)
        .gauge_style(
            Style::default()
                .fg(Color::Green)
                .bg(Color::Black)
                .add_modifier(Modifier::ITALIC),
        )
        .label("Time Left: 1 day 12h 30m 10s")
        .block(Block::default().title("Timer").borders(Borders::ALL))
}

/// Run the TUI.
///
/// This function will setup the terminal, run the main loop, and then
/// restore the terminal.
pub fn run() -> Result<()> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        // Enter alternate screen
        EnterAlternateScreen,
        // Enable mouse capture
        EnableMouseCapture,
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Instantiate the ConfigState
    let mut config_state = ConfigState(false);

    // Main loop
    loop {
        terminal.draw(|f| ui(f, &config_state))?;

        // Poll for events
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,  // Quit
                    KeyCode::Char('c') => todo!(),               // Check-In
                    KeyCode::Char('o') => config_state.toggle(), // Options
                    _ => {}
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
