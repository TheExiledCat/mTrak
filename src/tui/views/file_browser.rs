use std::{
    cell::RefCell,
    ffi::OsStr,
    fs::{self, DirEntry},
    path::PathBuf,
    rc::Rc,
};

use ratatui::{
    crossterm::event::{KeyCode, KeyModifiers},
    layout::{Constraint, Layout},
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::tui::{
    app::AppState,
    framework::{
        core_extensions::Boxed,
        dialog::{Dialog, PopupLevel, Select, TextPrompt},
        keymap::{Key, KeyActions, Keymap},
        widget::MtrakWidget,
        widgets::footer::Footer,
    },
    views::file_browser::FileBrowserMode::{OpenFile, SaveFile},
};
pub const MTRAK_FILE_EXTENSION: &str = ".mtrak";
const ALL_EXTENSIONS_FILTER: &str = "*";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileBrowserAction {
    Down,
    Up,
    Select,
    Confirm,
    NewDirectory,
    Cancel,
}

enum FileBrowserMode {
    SaveFile { extensions: Vec<String> },
    OpenFile { extensions: Vec<String> },
}

impl FileBrowserMode {
    fn extensions(&self) -> &[String] {
        return match self {
            SaveFile { extensions } | OpenFile { extensions } => extensions,
        };
    }
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
    pending_input: Rc<RefCell<Option<PendingInput>>>,
    footer: Footer,
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
            .bind(Key::ctrl('n'), NewDirectory, "Create")
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
                extensions: vec![ALL_EXTENSIONS_FILTER.into()],
            },
            current_folder: default_path,
            title: String::new(),
            selected_file: None,
            selected_index: 0,
            scroll_offset: 0,
            submit_handler: None,
            pending_input: Rc::new(RefCell::new(None)),
            footer: Footer::new(),
            current_folder_contents: vec![],
        };
    }
    //opens a save file picker, picks a directory to save the file in and a name for the file.
    pub fn save_file(
        &mut self,
        on_submit: Box<Submit>,
        start_directory: PathBuf,
        title: impl Into<String>,
        extensions: Vec<String>,
    ) {
        self.mode = SaveFile { extensions };
        self.is_open = true;
        self.submit_handler = Some(on_submit);
        self.title = title.into();
        let _ = fs::create_dir_all(&start_directory);
        self.current_folder = start_directory;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.selected_file = None;
        self.pending_input.replace(None);

        self.load_contents();
    }

    pub fn is_open(&self) -> bool {
        return self.is_open;
    }
    fn load_contents(&mut self) {
        let mut entries: Vec<DirEntry> = fs::read_dir(&self.current_folder)
            .map(|entries| entries.filter_map(Result::ok).collect())
            .unwrap_or_default();
        let extensions = self.mode.extensions();
        entries.retain(|e| e.path().is_dir() || matches_filter(&e.file_name(), extensions));
        entries.sort_by_cached_key(|e| {
            (
                e.path().is_file(),
                e.file_name().to_string_lossy().to_lowercase(),
            )
        });
        self.current_folder_contents = entries;
    }
    fn submit(&mut self, state: &mut AppState) {
        match &self.mode {
            SaveFile { extensions } => {
                if let Some(file) = self.selected_file.clone() {
                    self.finish(state, file);
                    return;
                }
                let options = extensions
                    .iter()
                    .map(|e| extension_label(e).to_owned())
                    .collect();
                let pending = self.pending_input.clone();
                let dialog = Dialog::new(
                    "Save file",
                    vec![
                        TextPrompt::new("name", "Name").boxed(),
                        Select::new("extension", options)
                            .label("Extension")
                            .max_visible(4)
                            .boxed(),
                    ],
                )
                .on_submit(move |_, result| {
                    pending.replace(Some(PendingInput::FileName {
                        name: result.text("name").unwrap_or_default().to_owned(),
                        extension: result.choice("extension").unwrap_or(0),
                    }));
                });
                state.open_dialog(dialog);
            }
            OpenFile { .. } => todo!(),
        };
    }
    fn finish(&mut self, state: &mut AppState, path: PathBuf) {
        self.is_open = false;
        if let Some(on_submit) = self.submit_handler.take() {
            on_submit(state, path);
        }
    }
    fn request_new_directory(&self, state: &mut AppState) {
        let pending = self.pending_input.clone();
        let dialog = Dialog::text_field("New directory", String::new(), "name").on_submit(
            move |_, result| {
                let name = result.text("name").unwrap_or_default().to_owned();
                pending.replace(Some(PendingInput::NewDirectory(name)));
            },
        );
        state.open_dialog(dialog);
    }
    fn create_directory(&mut self, state: &mut AppState, name: &str) {
        let path = self.current_folder.join(name);
        if let Err(e) = fs::create_dir(&path) {
            state.open_dialog(Dialog::popup(
                PopupLevel::Error,
                "New directory",
                e.to_string(),
            ));
            return;
        }
        self.load_contents();
        if let Some(i) = self
            .current_folder_contents
            .iter()
            .position(|e| e.path() == path)
        {
            self.selected_index = i + 1;
        }
    }
}

enum PendingInput {
    FileName { name: String, extension: usize },
    NewDirectory(String),
}

/// The extension part of a filter like `*.mtrak`, empty for `*`.
fn extension_of(filter: &str) -> &str {
    return filter.trim_start_matches('*');
}

fn extension_label(filter: &str) -> &str {
    let extension = extension_of(filter);
    if extension.is_empty() {
        return "Any";
    }
    return extension;
}

fn matches_filter(file_name: &OsStr, filters: &[String]) -> bool {
    let file_name = file_name.to_string_lossy().to_lowercase();
    return filters.iter().any(|filter| {
        filter == ALL_EXTENSIONS_FILTER || file_name.ends_with(&extension_of(filter).to_lowercase())
    });
}

fn with_extension(name: String, filter: &str) -> String {
    let extension = extension_of(filter);
    if extension.is_empty() || name.ends_with(extension) {
        return name;
    }
    return name + extension;
}

impl MtrakWidget for FileBrowser {
    fn update(&mut self, state: &mut AppState) {
        let Some(input) = self.pending_input.take() else {
            return;
        };
        match input {
            PendingInput::FileName { name, extension } => {
                let name = name.trim();
                if name.is_empty() {
                    return;
                }
                let filter = self
                    .mode
                    .extensions()
                    .get(extension)
                    .map(String::as_str)
                    .unwrap_or(ALL_EXTENSIONS_FILTER);
                let path = self
                    .current_folder
                    .join(with_extension(name.to_owned(), filter));
                self.finish(state, path);
            }
            PendingInput::NewDirectory(name) => {
                let name = name.trim();
                if !name.is_empty() {
                    self.create_directory(state, name);
                }
            }
        }
    }
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
        let selection_text = match &self.mode {
            SaveFile { .. } => self
                .selected_file
                .as_ref()
                .unwrap_or(&self.current_folder)
                .to_string_lossy()
                .to_string(),
            OpenFile { .. } => self
                .selected_file
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        };
        Paragraph::new(format!("Selected: {}", selection_text))
            .left_aligned()
            .render(layout[3], buf);

        let mut hints = vec![];
        self.key_hints(state, &mut hints);
        self.footer.set_hints(hints);
        self.footer.render(state, *layout.last().unwrap(), buf);
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
                        self.selected_file = None;
                        self.current_folder = dir;
                        self.load_contents();
                    }
                    Some(file) if self.selected_file.as_ref() == Some(&file) => {
                        self.selected_file = None
                    }
                    Some(file) => self.selected_file = Some(file),
                    None => {}
                }
            }
            FileBrowserAction::Confirm => {
                if matches!(self.mode, SaveFile { .. }) || self.selected_file.is_some() {
                    self.submit(state);
                } else {
                    state.open_dialog(Dialog::popup(
                        PopupLevel::Warning,
                        "File browser",
                        "No file selected",
                    ));
                }
            }
            FileBrowserAction::NewDirectory => self.request_new_directory(state),
            FileBrowserAction::Cancel => {
                self.is_open = false;
                self.submit_handler = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_extension_appends_missing_extension() {
        assert_eq!(with_extension("song".into(), "*.mtrak"), "song.mtrak");
        assert_eq!(with_extension("song.mtrak".into(), "*.mtrak"), "song.mtrak");
        assert_eq!(with_extension("song".into(), "*"), "song");
    }

    #[test]
    fn matches_filter_checks_every_extension() {
        let filters = vec!["*.mtrak".to_owned(), "*.mid".to_owned()];
        assert!(matches_filter(OsStr::new("song.mtrak"), &filters));
        assert!(matches_filter(OsStr::new("SONG.MTRAK"), &filters));
        assert!(matches_filter(OsStr::new("song.mid"), &filters));
        assert!(!matches_filter(OsStr::new("song.wav"), &filters));
        assert!(matches_filter(
            OsStr::new("song.wav"),
            &[ALL_EXTENSIONS_FILTER.to_owned()]
        ));
    }
}
