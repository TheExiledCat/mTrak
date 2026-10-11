use std::path::PathBuf;

use ratatui::{
    crossterm::event::{KeyCode, KeyModifiers},
    layout::{Constraint, Layout},
    widgets::Padding,
};

use crate::{
    error::Result,
    tui::{
        app::AppState,
        constants, files,
        framework::{
            core_extensions::Boxed,
            dialog::{Dialog, PopupLevel},
            keymap::{Key, KeyActions, KeyHint, Keymap},
            widget::MtrakWidget,
            widgets::{container::Container, footer::Footer},
        },
        views::{file_browser, header::Header, timeline::TimeLineView},
    },
};

/// Actions available everywhere in the main view, regardless of which child has focus.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MainAction {
    Quit,
    Save,
    SaveAs,
    PlayPattern,
    PlaySong,
}

pub struct MainView {
    header: Header,
    content: Container,
    footer: Footer,
    keys: Keymap<MainAction>,
}
impl MainView {
    pub fn new(state: &AppState) -> Self {
        return Self {
            header: Header::new(),
            content: Container::new(TimeLineView::new(state).boxed(), Padding::ZERO),
            footer: Footer::new(),
            keys: Keymap::new()
                .bind(Key::plain(KeyCode::Enter), MainAction::PlaySong, "Play SNG")
                .bind(
                    Key::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
                    MainAction::PlayPattern,
                    "Play PTN",
                )
                .bind(Key::ctrl('s'), MainAction::Save, "Save")
                .bind(
                    Key::new(KeyCode::Char('s'), KeyModifiers::ALT),
                    MainAction::SaveAs,
                    "Save As",
                )
                .bind(
                    Key::new(constants::EXIT_KEY, KeyModifiers::CONTROL),
                    MainAction::Quit,
                    "Quit",
                ),
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
            MainAction::Quit => state.request_exit(),
            MainAction::Save => match state.project_path.clone() {
                Some(path) => state.save_project(path)?,
                None => request_save_as(state),
            },
            MainAction::SaveAs => request_save_as(state),
            MainAction::PlayPattern => state.play_pattern(state.active_pattern),
            MainAction::PlaySong => state.play_song(state.active_sequence_index),
        }
        return Ok(());
    }
}
fn request_save_as(state: &mut AppState) {
    let start_directory = state
        .project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .or_else(files::get_projects_directory_root)
        .unwrap_or_else(|| PathBuf::from("."));
    state.request_save_file(
        |state, file_path| {
            let name = file_path.file_name().unwrap_or_default().to_string_lossy();
            let message = format!("Saved {}", name);
            state.save_project(file_path)?;
            state.open_dialog(Dialog::popup(PopupLevel::Info, "Saved", message));
            return Ok(());
        },
        start_directory,
        "Save project",
        vec!["*".to_owned() + file_browser::MTRAK_FILE_EXTENSION],
    );
}

impl MtrakWidget for MainView {
    fn update(&mut self, state: &mut AppState) {
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

    fn key_hints(&self, state: &AppState, hints: &mut Vec<KeyHint>) {
        hints.extend(self.keys.hint_for(MainAction::PlaySong));
        hints.extend(self.keys.hint_for(MainAction::PlayPattern));
        self.content.key_hints(state, hints);
        self.header.key_hints(state, hints);
        hints.extend(self.keys.hint_for(MainAction::Save));
        hints.extend(self.keys.hint_for(MainAction::SaveAs));
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

        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Min(17),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);
        self.header.render(state, header_area, buf);
        self.content.render(state, content_area, buf);
        self.footer.render(state, footer_area, buf);
    }
}
