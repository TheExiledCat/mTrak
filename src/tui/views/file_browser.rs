use std::{
    fs::{self, DirEntry},
    path::PathBuf,
};

use ratatui::{
    crossterm::event::{KeyCode, KeyModifiers},
    layout::{Constraint, Layout},
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::tui::{
    app::AppState,
    framework::{
        dialog::{Dialog, PopupLevel},
        keymap::{Key, KeyActions, Keymap},
        widget::MtrakWidget,
    },
    views::file_browser::FileBrowserMode::SaveFile,
};
pub const MTRAK_FILE_EXTENSION: &str = ".mtrak";
const ALL_EXTENSIONS_FILTER: &str = "*";
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileBrowserAction {
    Down,
    Up,
    Select,
    Confirm,
    Cancel,
}

enum FileBrowserMode {
    SaveFile { extension_filter: String },
    OpenFile { extension_filter: String },
}
pub type Submit = dyn FnOnce(&mut AppState, PathBuf);
pub struct FileBrowser {
    keymap: Keymap<FileBrowserAction>,
    title: String,
    default_path: PathBuf,
    mode: FileBrowserMode,
    is_open: bool,

    current_folder: PathBuf,
    current_folder_contents: Vec<DirEntry>,
    selected_file: Option<PathBuf>,

    selected_index: usize,
    scroll_offset: usize,

    submit_handler: Option<Box<Submit>>,
}

impl Default for FileBrowser {
    fn default() -> Self {
        return Self::new();
    }
}

impl FileBrowser {
    pub fn new() -> Self {
        use FileBrowserAction::*;
        let keymap = Keymap::new()
            .bind(Key::new(KeyCode::Down, KeyModifiers::NONE), Down, "Move")
            .bind(Key::new(KeyCode::Up, KeyModifiers::NONE), Up, "Move")
            .bind(Key::char(' '), Select, "Select")
            .bind(Key::new(KeyCode::Esc, KeyModifiers::NONE), Cancel, "Cancel")
            .bind(
                Key::new(KeyCode::Enter, KeyModifiers::NONE),
                Confirm,
                "Confirm",
            );
        let default_path = dirs::home_dir().unwrap();
        return Self {
            keymap,
            default_path: default_path.clone(),
            is_open: false,
            mode: SaveFile {
                extension_filter: ".mtrak".into(),
            },
            current_folder: default_path,
            title: String::new(),
            selected_file: None,
            selected_index: 0,
            scroll_offset: 0,
            submit_handler: None,
            current_folder_contents: vec![],
        };
    }

    pub fn save_file(
        &mut self,
        on_submit: Box<Submit>,
        title: impl Into<String>,
        file_filter: impl Into<String>,
    ) {
        self.mode = SaveFile {
            extension_filter: file_filter.into(),
        };
        self.is_open = true;
        self.submit_handler = Some(on_submit);
        self.title = title.into();

        self.load_contents();
    }

    pub fn is_open(&self) -> bool {
        return self.is_open;
    }
    fn load_contents(&mut self) {
        let mut entries: Vec<DirEntry> = fs::read_dir(&self.current_folder)
            .map(|entries| entries.filter_map(Result::ok).collect())
            .unwrap_or_default();
        entries.sort_by_cached_key(|e| {
            (
                e.path().is_file(),
                e.file_name().to_string_lossy().to_lowercase(),
            )
        });
        self.current_folder_contents = entries;
    }
    fn submit(&mut self, state: &mut AppState) {
        self.is_open = false;
        self.submit_handler.take().unwrap()(state, self.selected_file.take().unwrap_or("".into()));
    }
}
impl MtrakWidget for FileBrowser {
    fn key_hints(
        &self,
        _state: &AppState,
        _hints: &mut Vec<crate::tui::framework::keymap::KeyHint>,
    ) {
        self.keymap.hints(_hints);
    }
    fn handle_event(
        &mut self,
        _state: &mut AppState,
        _event: &ratatui::crossterm::event::Event,
    ) -> bool {
        return self.is_open && self.dispatch(_state, _event);
    }
    fn render(
        &mut self,
        state: &crate::tui::app::AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
        Paragraph::new(self.current_folder.to_string_lossy())
            .left_aligned()
            .style(state.theme.header_border)
            .reversed()
            .block(
                Block::bordered()
                    .borders(Borders::TOP)
                    .title_top(self.title.clone()),
            )
            .render(layout[0], buf);
        let current_folder_contents = &self.current_folder_contents;
        let lines: Vec<Line> = [Line::raw("..")]
            .into_iter()
            .chain(current_folder_contents.iter().map(|d| {
                Line::raw(format!(
                    "{}{}",
                    d.path()
                        .file_name()
                        .map(|s| s.to_string_lossy())
                        .unwrap_or("N/A".into()),
                    if !d.path().is_file() { "/" } else { "" }
                ))
            }))
            .enumerate()
            .map(|l| {
                let index = l.0;
                let line = l.1;
                let mut style = state.theme.pattern_area;
                if self.selected_index == index {
                    style = style.reversed();
                }

                line.style(style)
            })
            .collect();

        let visible = layout[1].height as usize;
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if visible > 0 && self.selected_index >= self.scroll_offset + visible {
            self.scroll_offset = self.selected_index + 1 - visible;
        }
        let total_lines = lines.len();
        let block = Block::new().style(state.theme.pattern_area);
        Paragraph::new(lines)
            .left_aligned()
            .scroll((self.scroll_offset as u16, 0))
            .block(block.clone())
            .render(layout[1], buf);
        if self.scroll_offset + visible < total_lines {
            Paragraph::new("▼").block(block).render(layout[2], buf);
        }
        Paragraph::new(format!(
            "Selected: {}",
            self.selected_file
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        ))
        .left_aligned()
        .render(layout[3], buf);

        let mut hints = vec![];
        self.key_hints(state, &mut hints);
        Paragraph::new(Line::from(
            hints
                .iter()
                .flat_map(|h| {
                    [
                        Span::raw(h.key.to_string()).style(state.theme.key_hint_key),
                        Span::raw(format!(" {} ", h.label)).style(state.theme.key_hint_label),
                    ]
                    .into_iter()
                    .collect::<Vec<Span>>()
                })
                .collect::<Vec<Span>>(),
        ))
        .render(*layout.last().unwrap(), buf);
    }
}

impl KeyActions for FileBrowser {
    type Action = FileBrowserAction;

    fn keymap(&self) -> &Keymap<Self::Action> {
        return &self.keymap;
    }

    fn perform(&mut self, action: Self::Action, state: &mut crate::tui::app::AppState) {
        match action {
            FileBrowserAction::Up => self.selected_index = self.selected_index.saturating_sub(1),
            FileBrowserAction::Down => {
                self.selected_index = self
                    .selected_index
                    .saturating_add(1)
                    .min(self.current_folder_contents.len())
            }
            FileBrowserAction::Select => {
                let path = match self.selected_index {
                    0 => self.current_folder.parent().map(|p| p.to_path_buf()),
                    i => self.current_folder_contents.get(i - 1).map(|e| e.path()),
                };
                match path {
                    Some(dir) if dir.is_dir() => {
                        self.selected_index = 0;
                        self.scroll_offset = 0;
                        self.current_folder = dir;
                        self.load_contents();
                    }
                    Some(file) => self.selected_file = Some(file),
                    None => {}
                }
            }
            FileBrowserAction::Confirm => {
                if self.selected_file.is_some() {
                    self.submit(state);
                } else {
                    state.open_dialog(Dialog::popup(
                        PopupLevel::Warning,
                        "File browser",
                        "No file selected",
                    ));
                }
            }
            FileBrowserAction::Cancel => {
                self.is_open = false;
                self.submit_handler = None;
            }
        }
    }
}
