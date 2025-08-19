use std::{
    process::exit,
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
};

use crossterm::event::KeyEvent;
use ratatui::crossterm::event::{self, Event, KeyCode};

use super::constants;

pub struct InputHandler {
    handle: Option<JoinHandle<()>>,
    reciever: mpsc::Receiver<event::Event>,
}
impl InputHandler {
    pub fn new() -> Self {
        let (transmitter, reciever) = mpsc::channel();
        let handle = thread::spawn(move || {
            loop {
                if let Ok(event) = event::read() {
                    if transmitter.send(event.clone()).is_err() {
                        break;
                    }
                    if let Event::Key(key_event) = event {
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
    pub fn read_event(&self) -> Option<Event> {
        return self.reciever.try_recv().ok();
    }
}
impl Drop for InputHandler {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.join();
        }
    }
}
