use std::path::PathBuf;

use ratatui::{
    crossterm::event::{KeyCode, KeyModifiers},
    layout::{Constraint, Layout},
    widgets::{Padding, Paragraph, Widget},
};

use crate::{
    data::config::TITLE,
    error::Result,
    tui::{
        app::AppState,
        constants, files,
        framework::{
            core_extensions::Boxed,
            dialog::{Dialog, InfoLevel},
            keymap::{Action, HelpSection, Key, KeyActions, KeyHint, Keymap},
            widget::MtrakWidget,
            widgets::{container::Container, footer::Footer},
        },
        views::{
            file_browser,
            header::{self, Header},
            timeline::TimeLineView,
        },
    },
    updater::VERSION,
};

/// Actions available everywhere in the main view, regardless of which child has focus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainAction {
    Quit,
    Save,
    SaveAs,
    PlayPattern,
    PlaySong,
    Panic,
    Undo,
    Redo,
    Help,
    Commands,
    File, //Open a file
}

impl Action for MainAction {}

pub struct MainView {
    header: Header,
    content: Container,
    footer: Footer,
    keys: Keymap<MainAction>,
}
impl MainView {
    pub fn new() -> Self {
        return Self {
            header: Header::new(),
            content: Container::new(TimeLineView::new().boxed(), Padding::ZERO),
            footer: Footer::new(),
            keys: Keymap::new("global")
                .bind(Key::plain(KeyCode::Enter), MainAction::PlaySong, "Play SNG")
                .bind(
                    Key::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
                    MainAction::PlayPattern,
                    "Play PTN",
                )
                .bind(Key::char('~'), MainAction::Panic, "Panic")
                .bind(Key::ctrl('z'), MainAction::Undo, "Undo")
                .bind(Key::ctrl('y'), MainAction::Redo, "Redo")
                .bind(Key::ctrl('s'), MainAction::Save, "Save")
                .bind(
                    Key::new(KeyCode::Char('s'), KeyModifiers::ALT),
                    MainAction::SaveAs,
                    "Save As",
                )
                .bind(
                    Key::new(KeyCode::Char('o'), KeyModifiers::ALT),
                    MainAction::File,
                    "File",
                )
                .bind(
                    Key::new(constants::EXIT_KEY, KeyModifiers::CONTROL),
                    MainAction::Quit,
                    "Quit",
                )
                .bind(Key::char('?'), MainAction::Help, "Keys")
                .bind(Key::char('/'), MainAction::Commands, "Cmds"),
        };
    }
}
impl KeyActions for MainView {
    type Action = MainAction;

    fn keymap(&self) -> &Keymap<MainAction> {
        return &self.keys;
    }

    fn perform(&mut self, action: MainAction, state: &mut AppState) -> Result<()> {
        match action {
            MainAction::Quit => confirm_discard(state, "Quit?", AppState::request_exit),
            MainAction::Save => match state.project_path.clone() {
                Some(path) => state.save_project(path)?,
                None => request_save_as(state),
            },
            MainAction::SaveAs => request_save_as(state),
            MainAction::PlayPattern => state.play_pattern(state.active_pattern),
            MainAction::PlaySong => state.play_song(state.active_sequence_index),
            MainAction::Panic => {
                state.engine.panic();
                state.notify("PANIC");
            }
            MainAction::Undo => {
                if !state.undo() {
                    state.notify("NO UNDO");
                }
            }
            MainAction::Redo => {
                if !state.redo() {
                    state.notify("NO REDO");
                }
            }
            MainAction::Help => state.request_help(),
            MainAction::Commands => state.request_commands(),
            MainAction::File => confirm_discard(state, "Open?", browse_to_open),
        }
        return Ok(());
    }

    fn describe(action: MainAction) -> &'static str {
        return match action {
            MainAction::Quit => "Quit",
            MainAction::Save => "Save project",
            MainAction::SaveAs => "Save project as",
            MainAction::PlayPattern => "Play the current pattern",
            MainAction::PlaySong => "Play the song from the current sequence position",
            MainAction::Panic => "Panic: stop and silence every channel, resetting controllers",
            MainAction::Undo => "Undo the last edit",
            MainAction::Redo => "Redo the last undone edit",
            MainAction::Help => "Show all keybindings",
            MainAction::Commands => "Show all pattern commands",
            MainAction::File => "Load a file",
        };
    }
}
fn confirm_discard(state: &mut AppState, title: &str, then: fn(&mut AppState)) {
    if state.is_saved() {
        then(state);
        return;
    }
    let dialog = Dialog::confirm(InfoLevel::Error, title, "Unsaved progress will be lost")
        .on_submit(move |state, _| {
            then(state);
            return Ok(());
        });
    state.open_dialog(dialog);
}
fn project_browse_directory(state: &AppState) -> PathBuf {
    return state
        .project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .or_else(files::get_projects_directory_root)
        .unwrap_or_else(|| PathBuf::from("."));
}
fn request_save_as(state: &mut AppState) {
    let start_directory = project_browse_directory(state);
    state.request_save_file(
        |state, file_path| {
            let name = file_path.file_name().unwrap_or_default().to_string_lossy();
            let message = format!("Saved {}", name);
            state.save_project(file_path)?;
            state.open_dialog(Dialog::popup(InfoLevel::Info, "Saved", message));
            return Ok(());
        },
        start_directory,
        "Save project",
        vec!["*".to_owned() + file_browser::MTRAK_FILE_EXTENSION],
    );
}
fn browse_to_open(state: &mut AppState) {
    let start_directory = project_browse_directory(state);
    state.request_open_file(
        |state, file_path| {
            state.request_project_load(file_path);
            return Ok(());
        },
        start_directory,
        "Open project",
        vec!["*".to_owned() + file_browser::MTRAK_FILE_EXTENSION],
    );
}

impl MtrakWidget for MainView {
    fn update(&mut self, state: &mut AppState) {
        let label = if state.is_saved() { "Save" } else { "*Save" };
        for binding in self.keys.default_bindings_mut() {
            if binding.action == MainAction::Save {
                binding.label = label;
            }
        }
        self.header.update(state);
        self.content.update(state);
    }
    fn handle_event(
        &mut self,
        state: &mut crate::tui::app::AppState,
        event: &ratatui::crossterm::event::Event,
    ) -> bool {
        // global keys are checked first so no child can swallow them
        return self.dispatch(state, event)
            || self.content.handle_event(state, event)
            || self.header.handle_event(state, event);
    }

    fn help(&self, sections: &mut Vec<HelpSection>) {
        sections.push(self.help_section("Global"));
        self.content.help(sections);
        self.header.help(sections);
    }

    fn key_hints(&self, state: &AppState, hints: &mut Vec<KeyHint>) {
        hints.extend(self.keys.hint_for(MainAction::Help));
        hints.extend(self.keys.hint_for(MainAction::Commands));
        self.content.key_hints(state, hints);
        hints.extend(
            self.keys
                .hint_for(MainAction::Undo)
                .map(|h| h.enabled(state.can_undo())),
        );
        hints.extend(
            self.keys
                .hint_for(MainAction::Redo)
                .map(|h| h.enabled(state.can_redo())),
        );
        hints.extend(self.keys.hint_for(MainAction::Save));
        hints.extend(self.keys.hint_for(MainAction::File));
        hints.extend(self.keys.hint_for(MainAction::Quit));
    }

    fn render(
        &mut self,
        state: &crate::tui::app::AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        // collected every frame so the footer follows mode changes, e.g. entering edit mode
        let mut hints = Vec::new();
        self.key_hints(state, &mut hints);
        self.footer.set_hints(hints);

        let header_height = if state.header_collapsed {
            Constraint::Length(header::COLLAPSED_HEIGHT)
        } else {
            Constraint::Min(17)
        };
        let [header_area, content_area, footer_area] =
            Layout::vertical([header_height, Constraint::Fill(1), Constraint::Length(1)])
                .areas(area);
        let version_text = format!("{} v{}", TITLE, VERSION);
        let [footer_area, version_area] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(version_text.chars().count() as u16),
        ])
        .areas(footer_area);
        self.header.render(state, header_area, buf);
        self.content.render(state, content_area, buf);
        self.footer.render(state, footer_area, buf);

        Paragraph::new(version_text)
            .right_aligned()
            .render(version_area, buf);
    }
}
