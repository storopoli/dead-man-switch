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
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};

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
fn ui<B: Backend>(f: &mut Frame<B>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Percentage(50),
                Constraint::Max(5),
            ]
            .as_ref(),
        )
        .split(f.size());

    let block = legend_block();
    f.render_widget(block, chunks[0]);
    let block = ascii_block(ASCII_ART.as_ref());
    f.render_widget(block, chunks[1]);
    let block = timer_block();
    f.render_widget(block, chunks[2]);
}

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
        .block(Block::default().title("Instructions").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    block
}

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

fn timer_block() -> Gauge<'static> {
    Gauge::default().block(Block::default().title("Timer").borders(Borders::ALL))
}

/// Run the TUI.
///
/// This function will setup the terminal, run the main loop, and then
/// restore the terminal.
pub fn run() -> Result<()> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    loop {
        terminal.draw(ui)?;

        // Poll for events
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break, // Quit
                    KeyCode::Char('c') => todo!(),              // Check-In
                    KeyCode::Char('o') => todo!(),              // Options
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
