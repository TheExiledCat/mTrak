use std::{
    thread,
    time::{Duration, Instant},
};

use ratatui::{
    DefaultTerminal, Terminal,
    crossterm::event::{Event, KeyCode, KeyEvent},
    layout::Alignment,
    style::{Style, Stylize},
    widgets::{Block, Borders, Paragraph, Widget},
};

use super::{constants, keymap::InputHandler};

pub struct App {
    pub terminal: DefaultTerminal,
    pub input_handler: InputHandler,
    pub fps: u16,

    last_key: char,
}
impl App {
    pub fn new(terminal: DefaultTerminal, fps: u16) -> Self {
        return Self {
            terminal,
            input_handler: InputHandler::new(),
            fps,
            last_key: '0',
        };
    }
    pub fn draw(&mut self) -> bool {
        let mut render_next = true;
        let frame_time = Duration::from_secs_f64(1.0 / self.fps as f64);
        let start = Instant::now();
        self.terminal
            .draw(|f| {
                let area = f.area();
                let window = Block::new()
                    .title("Key test")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::all())
                    .border_style(Style::new().yellow());
                f.render_widget(&window, area);
                if let Some(event) = self.input_handler.read_event() {
                    if let Event::Key(key_event) = event {
                        if let KeyCode::Char(char) = key_event.code {
                            let char = char.to_ascii_lowercase();
                            self.last_key = char;
                        }
                    }
                }
                let key_press = Paragraph::new(self.last_key.to_string())
                    .block(window)
                    .centered();
                f.render_widget(key_press, area);
                if KeyCode::Char(self.last_key) == constants::EXIT_KEY {
                    render_next = false;
                    return;
                }
            })
            .unwrap();
        let elapsed = start.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
        return render_next;
    }
}
