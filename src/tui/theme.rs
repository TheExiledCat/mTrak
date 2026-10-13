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
const FT2_WARNING: Color = Color::Rgb(230, 190, 80);
const FT2_MARK: Color = Color::Rgb(38, 56, 80);

/// Named styles for everything the views draw, so they never hard-code colors.
/// The monochrome variant relies only on text modifiers, so every state stays visible
/// on terminals without color support.
pub struct Theme {
    /// the base layer behind every other surface, e.g. the app backdrop around the panels
    pub background: Style,
    /// a content layer drawn on top of the background, e.g. the pattern editor
    pub surface: Style,
    /// a title or section heading, e.g. a section title in the keybindings list
    pub heading: Style,
    /// regular text on the background, e.g. the label of a footer key hint
    pub text: Style,
    /// a small filled label, e.g. the key of a footer key hint
    pub badge: Style,
    /// the default border, e.g. around an info dialog
    pub border: Style,
    /// a border that stands out from `border`, e.g. around the header panels
    pub border_strong: Style,
    /// a border that blends into the surface, e.g. between the tracks of a pattern
    pub border_subtle: Style,
    /// a border signalling a warning, e.g. around a warning dialog
    pub border_warning: Style,
    /// a border signalling an error, e.g. around an error dialog
    pub border_danger: Style,
    /// a full-width line separating sections of content, e.g. the line where a pattern begins
    pub divider: Style,
    /// the line numbers beside a list, e.g. the row numbers of the pattern editor
    pub gutter: Style,
    /// a gutter line that marks a regular interval, e.g. every 4th row number marking the beat
    pub gutter_accent: Style,
    /// the gutter line of the selected item, e.g. the row number of the cursor row
    pub gutter_active: Style,
    /// a highlighted row outside the selection, e.g. the cursor row in the other tracks
    pub highlight: Style,
    /// a second highlighted row shown alongside `highlight`, e.g. the playing row while the row is locked
    pub highlight_secondary: Style,
    /// the selected item, e.g. the cell under the cursor
    pub selection: Style,
    /// `selection` while typing changes the content, e.g. the cursor cell in edit mode
    pub selection_danger: Style,
    /// a selected range that stays readable, e.g. a marked block
    pub selection_subtle: Style,
    /// a filled alert, e.g. the mode indicator while recording
    pub danger: Style,
    /// the single column being edited, patched on top of `selection`, e.g. the volume digit under the cursor
    pub cursor: Style,
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
    /// with VT support, and the 16-color theme on other Windows consoles and macOS terminals,
    /// since every macOS terminal supports at least 16 colors, unless the user opted
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
        if cfg!(any(windows, target_os = "macos")) {
            return Self::ansi16();
        }
        return Self::monochrome();
    }

    /// Modelled on FastTracker 2: blue-grey desktop, black pattern editor, a blue-grey bar
    /// on the selected row that turns red in edit mode.
    pub fn ft2() -> Self {
        return Self {
            background: Style::new().fg(FT2_TEXT).bg(FT2_DESKTOP),
            surface: Style::new().fg(FT2_PATTERN_TEXT).bg(FT2_PATTERN_BG),
            heading: Style::new().bold().fg(FT2_TEXT),
            text: Style::new().fg(FT2_TEXT),
            badge: Style::new().fg(FT2_PATTERN_BG).bg(FT2_DESKTOP_LIGHT),
            border: Style::new().fg(FT2_DESKTOP_LIGHT),
            border_strong: Style::new().fg(FT2_TEXT),
            border_subtle: Style::new().fg(FT2_DESKTOP),
            border_warning: Style::new().fg(FT2_WARNING),
            border_danger: Style::new().fg(FT2_EDIT),
            divider: Style::new().fg(FT2_DESKTOP_LIGHT),
            gutter: Style::new().fg(FT2_PATTERN_TEXT),
            gutter_accent: Style::new().bold().fg(FT2_TEXT),
            gutter_active: Style::new().fg(FT2_PATTERN_BG).bg(FT2_DESKTOP_LIGHT),
            highlight: Style::new().fg(FT2_TEXT).bg(FT2_DESKTOP),
            highlight_secondary: Style::new().fg(FT2_PATTERN_BG).bg(FT2_WARNING),
            selection: Style::new().fg(FT2_PATTERN_BG).bg(FT2_DESKTOP_LIGHT),
            selection_danger: Style::new().fg(FT2_PATTERN_BG).bg(FT2_EDIT),
            selection_subtle: Style::new().fg(FT2_TEXT).bg(FT2_MARK),
            danger: Style::new().fg(FT2_PATTERN_BG).bg(FT2_EDIT),
            cursor: Style::new().fg(FT2_PATTERN_BG).bg(FT2_TEXT),
        };
    }

    pub fn ansi16() -> Self {
        return Self {
            background: Style::new().fg(Color::White).bg(Color::Blue),
            surface: Style::new().fg(Color::Gray).bg(Color::Black),
            heading: Style::new().bold().fg(Color::White),
            text: Style::new().fg(Color::White),
            badge: Style::new().fg(Color::Black).bg(Color::Gray),
            border: Style::new().fg(Color::Gray),
            border_strong: Style::new().fg(Color::White),
            border_subtle: Style::new().fg(Color::Blue),
            border_warning: Style::new().fg(Color::Yellow),
            border_danger: Style::new().fg(Color::Red),
            divider: Style::new().fg(Color::Gray),
            gutter: Style::new().fg(Color::Gray),
            gutter_accent: Style::new().bold().fg(Color::White),
            gutter_active: Style::new().fg(Color::Black).bg(Color::Gray),
            highlight: Style::new().fg(Color::White).bg(Color::Blue),
            highlight_secondary: Style::new().fg(Color::Black).bg(Color::Yellow),
            selection: Style::new().fg(Color::Black).bg(Color::Gray),
            selection_danger: Style::new().fg(Color::Black).bg(Color::Red),
            selection_subtle: Style::new().fg(Color::White).bg(Color::DarkGray),
            danger: Style::new().fg(Color::Black).bg(Color::Red),
            cursor: Style::new().fg(Color::Black).bg(Color::White),
        };
    }

    pub fn monochrome() -> Self {
        return Self {
            background: Style::new(),
            surface: Style::new(),
            heading: Style::new().bold(),
            text: Style::new(),
            badge: Style::new().reversed(),
            border: Style::new().bold(),
            border_strong: Style::new(),
            border_subtle: Style::new(),
            border_warning: Style::new().bold().underlined(),
            border_danger: Style::new().bold().reversed(),
            divider: Style::new().bold(),
            gutter: Style::new(),
            gutter_accent: Style::new().bold(),
            gutter_active: Style::new().reversed(),
            highlight: Style::new().underlined(),
            highlight_secondary: Style::new().bold().underlined(),
            selection: Style::new().reversed(),
            selection_danger: Style::new().reversed().bold(),
            selection_subtle: Style::new().reversed().dim(),
            danger: Style::new().reversed().bold(),
            // undo the cell's reverse so the edited column stands out from the rest of the cell
            cursor: Style::new().not_reversed().bold().underlined(),
        };
    }
}
