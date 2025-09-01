use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{self},
    thread::{self, JoinHandle},
};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use super::{app::AppState, constants};

pub type HandleFunction = fn(Rc<RefCell<AppState>>, &KeyEvent) -> bool;
pub struct InputHandler {
    handle: Option<JoinHandle<()>>,
    reciever: mpsc::Receiver<event::KeyEvent>,
    keymaps: Vec<HandleFunction>,
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
                            if KeyCode::Char(char.to_ascii_lowercase()) == constants::EXIT_KEY
                                && key_event.modifiers.contains(KeyModifiers::CONTROL)
                            {
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
            keymaps: vec![],
        };
    }
    pub fn read_event(&self) -> Option<KeyEvent> {
        return self.reciever.try_recv().ok();
    }
    /// Handle the key event and return true if the event was handled by a handler
    pub fn handle_event(&self, state: Rc<RefCell<AppState>>, event: KeyEvent) -> bool {
        for handler in &self.keymaps {
            if handler(state.clone(), &event) {
                return true;
            }
        }

        return false;
    }
    pub fn register_handler(&mut self, handler: HandleFunction) {
        self.keymaps.push(handler);
    }
}
impl Drop for InputHandler {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
