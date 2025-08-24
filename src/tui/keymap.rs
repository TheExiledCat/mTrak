use std::{
    sync::mpsc::{self},
    thread::{self, JoinHandle},
};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent};

use super::constants;

pub struct InputHandler {
    handle: Option<JoinHandle<()>>,
    reciever: mpsc::Receiver<event::KeyEvent>,
}
impl InputHandler {
    pub fn new() -> Self {
        let (transmitter, reciever) = mpsc::channel::<KeyEvent>();
        let handle = thread::spawn(move || {
            loop {
                if let Ok(event) = event::read() {
                    if let Event::Key(key_event) = event {
                        if transmitter.send(key_event.clone()).is_err() {
                            break;
                        }
                        if let KeyCode::Char(char) = key_event.code {
                            if KeyCode::Char(char.to_ascii_lowercase()) == constants::EXIT_KEY {
                                break;
                            }
                        }
                    }
                }
            }
        });
        return Self {
            handle: Some(handle),
            reciever,
        };
    }
    pub fn read_event(&self) -> Option<KeyEvent> {
        return self.reciever.try_recv().ok();
    }
}
impl Drop for InputHandler {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
