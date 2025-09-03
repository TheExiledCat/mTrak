use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent},
    prelude,
    style::Stylize,
    text::Line,
    widgets::{Widget, canvas::Rectangle},
};
pub type OnBindingFieldEnter = fn(&mut BindingField);
pub struct BindingField {
    pub kind: BindingFieldKind,
    pub on_enter: Option<OnBindingFieldEnter>,
    pub input: String,
    pub active: bool,
    pub label: String,
}
impl BindingField {
    pub fn new(kind: BindingFieldKind, label: String) -> Self {
        return Self {
            kind,
            on_enter: None,
            input: String::new(),
            active: false,
            label,
        };
    }

    pub fn open(&mut self, on_enter: OnBindingFieldEnter) {
        self.active = true;
        self.on_enter = Some(on_enter);
    }

    pub fn read_input(&mut self, event: &KeyEvent) {
        if !self.active {
            return;
        }
        match event.code {
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Enter => {
                if let Some(action) = self.on_enter {
                    self.active = false;
                    action(self);
                    self.input.clear();
                }
            }
            KeyCode::Char(char) => {
                self.input.push(char);
            }
            _ => (),
        }
    }
    pub fn widget(&self) -> BindingFieldWidget {
        return BindingFieldWidget {
            kind: self.kind.clone(),
            active: self.active,
            input: self.input.clone(),
            label: self.label.clone(),
        };
    }
}
pub struct BindingFieldWidget {
    kind: BindingFieldKind,
    active: bool,
    input: String,
    label: String,
}
impl Widget for BindingFieldWidget {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        match self.active {
            true => self.render_active(area, buf),
            false => self.render_inactive(area, buf),
        }
    }
}

impl BindingFieldWidget {
    fn render_active(self, area: prelude::Rect, buf: &mut prelude::Buffer) {}
    fn render_inactive(self, area: prelude::Rect, buf: &mut prelude::Buffer) {
        Line::raw(format!(
            "{}: {:>width$}",
            &self.label,
            self.input,
            width = area.width as usize - self.label.chars().count()
        ))
        .right_aligned()
        .black()
        .on_white()
        .render(area, buf);
    }
}
#[derive(Clone)]
pub enum BindingFieldKind {
    TEXT,
    INT,
    INT_HEX,
}
pub struct BindingFieldState {}
