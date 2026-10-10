use std::env;

use ratatui::style::{Color, Style, Stylize};

use crate::cli::ColorMode;

// FastTracker 2 palette: blue-grey desktop panels around a black pattern editor
const FT2_DESKTOP: Color = Color::Rgb(73, 104, 140);
const FT2_DESKTOP_LIGHT: Color = Color::Rgb(141, 170, 203);
const FT2_PATTERN_BG: Color = Color::Rgb(0, 0, 0);
const FT2_PATTERN_TEXT: Color = Color::Rgb(196, 214, 236);
const FT2_TEXT: Color = Color::Rgb(255, 255, 255);
const FT2_EDIT: Color = Color::Rgb(214, 82, 82);

/// Named styles for everything the views draw, so they never hard-code colors.
/// The monochrome variant relies only on text modifiers, so every state stays visible
/// on terminals without color support.
pub struct Theme {
    /// the background and text of the whole app, behind the panels
    pub desktop: Style,
    /// the background and text of the pattern editor
    pub pattern_area: Style,
    pub header_border: Style,
    pub header_title: Style,
    pub panel_border: Style,
    pub timeline_border: Style,
    pub track_border: Style,
    /// the line marking where a pattern's first row begins
    pub pattern_start: Style,
    pub row_number: Style,
    /// every 4th row number, marking the beat
    pub beat_row_number: Style,
    pub selected_row_number: Style,
    pub search: Style,
    /// the selected row in tracks other than the selected one
    pub selected_row: Style,
    pub selected_cell: Style,
    /// `selected_cell` while in edit mode, so it's obvious that typing changes the pattern
    pub editing_cell: Style,
    /// the mode indicator while recording
    pub recording: Style,
    /// the column being edited inside the selected cell, patched on top of `selected_cell`
    pub cursor: Style,
    pub pattern_border: Style,
    pub selected_pattern_border: Style,
    /// the key part of a footer hint, e.g. "Ctrl+S"
    pub key_hint_key: Style,
    /// the description part of a footer hint, e.g. "Save"
    pub key_hint_label: Style,
    pub dialog_border: Style,
}

impl Theme {
    pub fn from_color_mode(mode: Option<ColorMode>) -> Self {
        return match mode {
            Some(ColorMode::Mono) => Self::monochrome(),
            Some(ColorMode::Ansi16) => Self::ansi16(),
            Some(ColorMode::Full) => Self::ft2(),
            None => Self::detect(),
        };
    }

    /// Picks the FT2 theme when the terminal advertises 24-bit color, or is a Windows console
    /// with VT support, and the 16-color theme on other Windows consoles, unless the user opted
    /// out of color via `NO_COLOR` (https://no-color.org) or the terminal is `dumb`, falling
    /// back to the monochrome theme.
    pub fn detect() -> Self {
        let no_color = env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        let dumb_terminal = env::var("TERM").is_ok_and(|t| t == "dumb");
        if no_color || dumb_terminal {
            return Self::monochrome();
        }
        #[cfg(windows)]
        let full_color = ratatui::crossterm::ansi_support::supports_ansi();
        #[cfg(not(windows))]
        let full_color = env::var("COLORTERM").is_ok_and(|c| c == "truecolor" || c == "24bit");
        if full_color {
            return Self::ft2();
        }
        if cfg!(windows) {
            return Self::ansi16();
        }
        return Self::monochrome();
    }

    /// Modelled on FastTracker 2: blue-grey desktop, black pattern editor, a blue-grey bar
    /// on the selected row that turns red in edit mode.
    pub fn ft2() -> Self {
        return Self {
            desktop: Style::new().fg(FT2_TEXT).bg(FT2_DESKTOP),
            pattern_area: Style::new().fg(FT2_PATTERN_TEXT).bg(FT2_PATTERN_BG),
            header_border: Style::new().fg(FT2_TEXT),
            header_title: Style::new().bold().fg(FT2_TEXT),
            panel_border: Style::new().fg(FT2_DESKTOP_LIGHT),
            timeline_border: Style::new().fg(FT2_DESKTOP_LIGHT),
            track_border: Style::new().fg(FT2_DESKTOP),
            pattern_start: Style::new().fg(FT2_DESKTOP_LIGHT),
            row_number: Style::new().fg(FT2_PATTERN_TEXT),
            beat_row_number: Style::new().bold().fg(FT2_TEXT),
            selected_row_number: Style::new().fg(FT2_PATTERN_BG).bg(FT2_DESKTOP_LIGHT),
            search: Style::new().fg(FT2_PATTERN_BG).bg(FT2_DESKTOP_LIGHT),
            selected_row: Style::new().fg(FT2_TEXT).bg(FT2_DESKTOP),
            selected_cell: Style::new().fg(FT2_PATTERN_BG).bg(FT2_DESKTOP_LIGHT),
            editing_cell: Style::new().fg(FT2_PATTERN_BG).bg(FT2_EDIT),
            recording: Style::new().fg(FT2_PATTERN_BG).bg(FT2_EDIT),
            cursor: Style::new().fg(FT2_PATTERN_BG).bg(FT2_TEXT),
            pattern_border: Style::new().fg(FT2_DESKTOP_LIGHT),
            selected_pattern_border: Style::new().fg(FT2_TEXT),
            key_hint_key: Style::new().fg(FT2_PATTERN_BG).bg(FT2_DESKTOP_LIGHT),
            key_hint_label: Style::new().fg(FT2_TEXT),
            dialog_border: Style::new().fg(FT2_DESKTOP_LIGHT),
        };
    }

    pub fn ansi16() -> Self {
        return Self {
            desktop: Style::new().fg(Color::White).bg(Color::Blue),
            pattern_area: Style::new().fg(Color::Gray).bg(Color::Black),
            header_border: Style::new().fg(Color::White),
            header_title: Style::new().bold().fg(Color::White),
            panel_border: Style::new().fg(Color::Gray),
            timeline_border: Style::new().fg(Color::Gray),
            track_border: Style::new().fg(Color::Blue),
            pattern_start: Style::new().fg(Color::Gray),
            row_number: Style::new().fg(Color::Gray),
            beat_row_number: Style::new().bold().fg(Color::White),
            selected_row_number: Style::new().fg(Color::Black).bg(Color::Gray),
            search: Style::new().fg(Color::Black).bg(Color::Gray),
            selected_row: Style::new().fg(Color::White).bg(Color::Blue),
            selected_cell: Style::new().fg(Color::Black).bg(Color::Gray),
            editing_cell: Style::new().fg(Color::Black).bg(Color::Red),
            recording: Style::new().fg(Color::Black).bg(Color::Red),
            cursor: Style::new().fg(Color::Black).bg(Color::White),
            pattern_border: Style::new().fg(Color::Gray),
            selected_pattern_border: Style::new().fg(Color::White),
            key_hint_key: Style::new().fg(Color::Black).bg(Color::Gray),
            key_hint_label: Style::new().fg(Color::White),
            dialog_border: Style::new().fg(Color::Gray),
        };
    }

    pub fn monochrome() -> Self {
        return Self {
            desktop: Style::new(),
            pattern_area: Style::new(),
            header_border: Style::new(),
            header_title: Style::new().bold(),
            panel_border: Style::new(),
            timeline_border: Style::new(),
            track_border: Style::new(),
            pattern_start: Style::new().bold(),
            row_number: Style::new(),
            beat_row_number: Style::new().bold(),
            selected_row_number: Style::new().reversed(),
            search: Style::new().reversed(),
            selected_row: Style::new().underlined(),
            selected_cell: Style::new().reversed(),
            editing_cell: Style::new().reversed().bold(),
            recording: Style::new().reversed().bold(),
            // undo the cell's reverse so the edited column stands out from the rest of the cell
            cursor: Style::new().not_reversed().bold().underlined(),
            pattern_border: Style::new(),
            selected_pattern_border: Style::new().reversed(),
            key_hint_key: Style::new().reversed(),
            key_hint_label: Style::new(),
            dialog_border: Style::new().bold(),
        };
    }
}
