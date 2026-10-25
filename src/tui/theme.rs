use std::env;

use ratatui::style::{Color, Style, Stylize};

use crate::cli::ColorMode;

// FastTracker 2 palette: blue-grey desktop panels around a black pattern editor
const FT2_PALETTE: Palette = Palette {
    backdrop: Color::Rgb(73, 104, 140),
    muted: Color::Rgb(141, 170, 203),
    disabled: Color::Rgb(141, 170, 203),
    surface: Color::Rgb(0, 0, 0),
    surface_text: Color::Rgb(196, 214, 236),
    text: Color::Rgb(255, 255, 255),
    danger: Color::Rgb(214, 82, 82),
    warning: Color::Rgb(230, 190, 80),
    success: Color::Rgb(120, 200, 120),
    mark: Color::Rgb(38, 56, 80),
};

const ANSI16_PALETTE: Palette = Palette {
    backdrop: Color::Blue,
    muted: Color::Gray,
    disabled: Color::DarkGray,
    surface: Color::Black,
    surface_text: Color::Gray,
    text: Color::White,
    danger: Color::Red,
    warning: Color::Yellow,
    success: Color::Green,
    mark: Color::DarkGray,
};

/// The colors a color theme is built from. Each color is used as a fill in some styles and as
/// text in others, see [`Theme::from_palette`].
pub struct Palette {
    /// the app backdrop around the panels, also the cursor row bar and subtle borders
    pub backdrop: Color,
    /// toned-down text and borders, also the fill behind the selection
    pub muted: Color,
    /// text and key fills for something currently unavailable, e.g. the undo hint with
    /// nothing to undo
    pub disabled: Color,
    /// the content layer, e.g. the pattern editor, also the text on filled labels
    pub surface: Color,
    /// regular text on the surface
    pub surface_text: Color,
    /// regular text on the backdrop, also the fill under the cursor
    pub text: Color,
    /// errors and edit mode
    pub danger: Color,
    /// warnings and the playing row while the row is locked
    pub warning: Color,
    /// success, e.g. the About dialog border
    pub success: Color,
    /// the fill behind a marked block
    pub mark: Color,
}

/// Named styles for everything the views draw, so they never hard-code colors.
/// The color themes are built from a [`Palette`]; the monochrome variant relies only on text
/// modifiers, so every state stays visible on terminals without color support.
pub struct Theme {
    /// the base layer behind every other surface, e.g. the app backdrop around the panels
    pub background: Style,
    /// a content layer drawn on top of the background, e.g. the pattern editor
    pub surface: Style,
    /// text that stands out, e.g. a section title in the keybindings list or every 4th row
    /// number marking the beat
    pub heading: Style,
    /// regular text on the background, e.g. the label of a footer key hint, and the borders
    /// around the header panels
    pub text: Style,
    /// `text` for something currently unavailable, e.g. the label of the undo hint with nothing to undo
    pub text_disabled: Style,
    /// a solid bar in the text color, e.g. a panel title on a header border, the timer or a
    /// channel's level meter
    pub label: Style,
    /// a clickable link, e.g. the repository URL in the About dialog
    pub link: Style,
    /// `selection` for something currently unavailable, e.g. the key of the undo hint with nothing to undo
    pub selection_disabled: Style,
    /// the default border, e.g. around an info dialog, and full-width lines separating
    /// content, e.g. the line where a pattern begins
    pub border: Style,
    /// a border that blends into the surface, e.g. between the tracks of a pattern
    pub border_subtle: Style,
    /// a border signalling a warning, e.g. around a warning dialog
    pub border_warning: Style,
    /// a border signalling an error, e.g. around an error dialog
    pub border_danger: Style,
    /// a border signalling something good, e.g. around the About dialog
    pub border_success: Style,
    /// the line numbers beside a list, e.g. the row numbers of the pattern editor
    pub gutter: Style,
    /// a highlighted row outside the selection, e.g. the cursor row in the other tracks
    pub highlight: Style,
    /// a second highlighted row shown alongside `highlight`, e.g. the playing row while the row is locked
    pub highlight_secondary: Style,
    /// the selected item, e.g. the cell under the cursor or the row number of the cursor row,
    /// and small filled labels, e.g. the key of a footer key hint
    pub selection: Style,
    /// a selected range that stays readable, e.g. a marked block
    pub selection_subtle: Style,
    /// a filled alert, e.g. the mode indicator while recording or the cursor cell in edit mode
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
        return Self::from_palette(&FT2_PALETTE);
    }

    pub fn ansi16() -> Self {
        return Self::from_palette(&ANSI16_PALETTE);
    }

    pub fn from_palette(palette: &Palette) -> Self {
        return Self {
            background: Style::new().fg(palette.text).bg(palette.backdrop),
            surface: Style::new().fg(palette.surface_text).bg(palette.surface),
            heading: Style::new().bold().fg(palette.text),
            text: Style::new().fg(palette.text),
            text_disabled: Style::new().fg(palette.disabled),
            label: Style::new().fg(palette.backdrop).bg(palette.text),
            link: Style::new().underlined().fg(palette.text),
            selection_disabled: Style::new().fg(palette.backdrop).bg(palette.disabled),
            border: Style::new().fg(palette.muted),
            border_subtle: Style::new().fg(palette.backdrop),
            border_warning: Style::new().fg(palette.warning),
            border_danger: Style::new().fg(palette.danger),
            border_success: Style::new().fg(palette.success),
            gutter: Style::new().fg(palette.surface_text),
            highlight: Style::new().fg(palette.text).bg(palette.backdrop),
            highlight_secondary: Style::new().fg(palette.surface).bg(palette.warning),
            selection: Style::new().fg(palette.surface).bg(palette.muted),
            selection_subtle: Style::new().fg(palette.text).bg(palette.mark),
            danger: Style::new().fg(palette.surface).bg(palette.danger),
            cursor: Style::new().fg(palette.surface).bg(palette.text),
        };
    }

    /// Hand-picked rather than built from a palette, since it has no colors to share.
    pub fn monochrome() -> Self {
        return Self {
            background: Style::new(),
            surface: Style::new(),
            heading: Style::new().bold(),
            text: Style::new(),
            text_disabled: Style::new().dim(),
            label: Style::new().reversed(),
            link: Style::new().underlined(),
            selection_disabled: Style::new().reversed().dim(),
            border: Style::new().bold(),
            border_subtle: Style::new(),
            border_warning: Style::new().bold().underlined(),
            border_danger: Style::new().bold().reversed(),
            border_success: Style::new().bold(),
            gutter: Style::new(),
            highlight: Style::new().underlined(),
            highlight_secondary: Style::new().bold().underlined(),
            selection: Style::new().reversed(),
            selection_subtle: Style::new().reversed().dim(),
            danger: Style::new().reversed().bold(),
            // undo the cell's reverse so the edited column stands out from the rest of the cell
            cursor: Style::new().not_reversed().bold().underlined(),
        };
    }
}
