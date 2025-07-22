use std::{
    io,
    thread::sleep,
    time::{Duration, Instant},
};

use data::pattern::Pattern;
use ratatui::{
    Terminal,
    crossterm::{
        event::{Event, KeyCode},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::Alignment,
    prelude::CrosstermBackend,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

pub mod data;
fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen);
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let start_time = Instant::now();
    loop {
        if Instant::now() > start_time + Duration::from_secs(5) {
            break;
        }
        terminal.draw(|t| {
            let size = t.size();

            let block = Block::default()
                .title("Mini Tracker")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL);

            let text = Paragraph::new("Pattern 01")
                .block(block)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Yellow));

            t.render_widget(text, size);
        });
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen);
    return Ok(());
}
