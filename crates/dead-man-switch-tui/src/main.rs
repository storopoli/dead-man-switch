//! TUI implementation for the Dead Man's Switch.

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};
use thiserror::Error;

use dead_man_switch::{
    config::{config_path, load_or_initialize_config, ConfigError, Email},
    email::EmailError,
    timer::{Timer, TimerType},
};

/// The ASCII art for the TUI's main block.
const ASCII_ART: [&str; 5] = [
    "██████  ███████  █████  ██████      ███    ███  █████  ███    ██ ███████     ███████ ██     ██ ██ ████████  ██████ ██   ██",
    "██   ██ ██      ██   ██ ██   ██     ████  ████ ██   ██ ████   ██ ██          ██      ██     ██ ██    ██    ██      ██   ██",
    "██   ██ █████   ███████ ██   ██     ██ ████ ██ ███████ ██ ██  ██ ███████     ███████ ██  █  ██ ██    ██    ██      ███████",
    "██   ██ ██      ██   ██ ██   ██     ██  ██  ██ ██   ██ ██  ██ ██      ██          ██ ██ ███ ██ ██    ██    ██      ██   ██",
    "██████  ███████ ██   ██ ██████      ██      ██ ██   ██ ██   ████ ███████     ███████  ███ ███  ██    ██     ██████ ██   ██",
];

/// Wrapper around [`Timer`].
struct TuiTimer(Timer);

impl TuiTimer {
    /// Determines the gauge style based on the inner [`Timer`] state.
    fn gauge_style(&self) -> Style {
        let percent = self.0.remaining_percent();
        let style = if percent > 30 {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };
        Style::default().fg(style.fg.unwrap())
    }

    /// Determines the label style based on the inner [`Timer`] state.
    fn label_style(&self) -> Style {
        let percent = self.0.remaining_percent();

        let style = if percent > 30 {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        };
        Style::default().fg(style.fg.unwrap())
    }

    /// Determines the Widget title based on the inner [`Timer`] type.
    fn title(&self) -> String {
        match self.0.get_type() {
            TimerType::Warning => "Warning".to_string(),
            TimerType::DeadMan => "Dead Man's Switch".to_string(),
        }
    }

    /// Creates a new [`TuiTimer`] from a [`Timer`].
    fn new(timer: Timer) -> Self {
        TuiTimer(timer)
    }
}

/// The main UI function.
///
/// This function will render the UI.
/// It's a simple UI with 3 blocks.
fn ui(f: &mut Frame, config_path: &str, timer: &TuiTimer) {
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
        .split(f.area());

    let legend_widget = legend_block();
    f.render_widget(legend_widget, chunks[0]);

    let ascii_widget = ascii_block(ASCII_ART.as_ref());
    f.render_widget(ascii_widget, chunks[1]);

    let instructions_widget = instructions_block(config_path);
    f.render_widget(instructions_widget, chunks[2]);

    let gauge_title = timer.title();
    let gauge_style = timer.gauge_style();
    let label_style = timer.label_style();
    let label = timer.0.label();
    let current_percent = timer.0.remaining_percent();
    let timer_widget = timer_block(
        gauge_title,
        current_percent,
        label,
        gauge_style,
        label_style,
    );
    f.render_widget(timer_widget, chunks[3]);
}

/// The legend block.
///
/// Contains the keys legend for the TUI.
fn legend_block() -> Paragraph<'static> {
    let text = vec![Line::from(vec![
        Span::styled(
            "c",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":Check-In"),
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
fn instructions_block(config_path: &str) -> Paragraph<'static> {
    let text = vec![
        Line::from(vec![
            Span::styled(
                "1. ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Edit the Config at "),
            Span::styled(
                config_path.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" and modify the settings."),
        ]),
        Line::from(vec![
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
        Line::from(vec![
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
    let text: Vec<Line<'_>> = content
        .iter()
        .map(|line| {
            Line::from(Span::styled(
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
/// # Parameters
///
/// - `title`: The title for the timer.
/// - `current_percent`: The current percentage of the timer.
/// - `label`: The label for the timer.
/// - `gauge_style`: The [`Style`] for the timer.
///
/// # Notes
///
/// The timer will be green if is still counting the warning time.
/// Eventually, it will turn red when the warning time is done,
/// and start counting the dead man's switch timer.
fn timer_block(
    title: String,
    current_percent: u16,
    label: String,
    gauge_style: Style,
    label_style: Style,
) -> Gauge<'static> {
    let title = Span::styled(
        format!("Timer: {title}"),
        match current_percent {
            0..=30 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            _ => Style::default().fg(Color::Green),
        },
    );
    Gauge::default()
        .percent(current_percent)
        .gauge_style(gauge_style)
        .label(Span::styled(label, label_style))
        .block(Block::default().title(title).borders(Borders::ALL))
}

/// TUI Error type.
#[derive(Error, Debug)]
enum TuiError {
    /// IO Error.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// [`ConfigError`] blanket error conversion.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// [`EmailError`] blanket error conversion.
    #[error(transparent)]
    Email(#[from] EmailError),
}

/// Run the TUI.
///
/// This function will setup the terminal, run the main loop, and then
/// restore the terminal.
fn run() -> Result<(), TuiError> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Instantiate the Config
    let config = load_or_initialize_config()?;

    // Get config OS-agnostic path
    let config_path = config_path()?.to_string_lossy().to_string();

    // Create a new Timer
    let mut timer = TuiTimer::new(Timer::new(
        TimerType::Warning,
        Duration::from_secs(config.timer_warning),
    ));

    // Main loop
    loop {
        terminal.draw(|f| ui(f, &config_path, &timer))?;

        // Poll for events
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,   // Quit
                    KeyCode::Char('c') => timer.0.reset(&config), // Check-In
                    _ => {}
                }
            }
        }

        // Condition to exit the loop
        if timer.0.expired() {
            match timer.0.get_type() {
                TimerType::Warning => {
                    config.send_email(Email::Warning)?;
                }
                TimerType::DeadMan => {
                    config.send_email(Email::DeadMan)?;
                    break;
                }
            }
        }

        let elapsed = timer.0.elapsed();
        timer.0.update(elapsed, config.timer_dead_man);
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

/// The main function.
///
/// This function executes the main loop of the application
/// by calling the [`run`] function.
fn main() -> Result<(), TuiError> {
    run()?;
    Ok(())
}
