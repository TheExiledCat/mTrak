use std::{env, path::Path};

use ratatui::style::{Color, Style, Stylize};

use crate::{
    data::config::ThemeChoice,
    error::Result,
    tui::files,
    util::toml::{self, ParseError},
};

/// The key in theme.toml naming the built-in palette that unset colors come from.
const BASE_KEY: &str = "base";

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
/// text in others, see [`Theme::from_palette`], and what each one is for in [`Palette::entries`].
pub struct Palette {
    pub backdrop: Color,
    pub muted: Color,
    pub disabled: Color,
    pub surface: Color,
    pub surface_text: Color,
    pub text: Color,
    pub danger: Color,
    pub warning: Color,
    pub success: Color,
    pub mark: Color,
}

impl Palette {
    /// Every color with its key in theme.toml and what it's used for.
    fn entries(&mut self) -> [(&'static str, &'static str, &mut Color); 10] {
        return [
            (
                "backdrop",
                "the app backdrop around the panels, also the cursor row bar and subtle borders",
                &mut self.backdrop,
            ),
            (
                "muted",
                "toned-down text and borders, also the fill behind the selection",
                &mut self.muted,
            ),
            (
                "disabled",
                "text and key fills for something unavailable, e.g. undo with nothing to undo",
                &mut self.disabled,
            ),
            (
                "surface",
                "the content layer, e.g. the pattern editor, also the text on filled labels",
                &mut self.surface,
            ),
            (
                "surface_text",
                "regular text on the surface",
                &mut self.surface_text,
            ),
            (
                "text",
                "regular text on the backdrop, also the fill under the cursor",
                &mut self.text,
            ),
            ("danger", "errors and edit mode", &mut self.danger),
            (
                "warning",
                "warnings and the playing row while the row is locked",
                &mut self.warning,
            ),
            (
                "success",
                "success, e.g. the About dialog border",
                &mut self.success,
            ),
            ("mark", "the fill behind a marked block", &mut self.mark),
        ];
    }

    pub fn load(path: &Path) -> Result<Self> {
        return toml::load_file(path, Self::from_toml);
    }

    pub fn from_toml(text: &str) -> std::result::Result<Self, ParseError> {
        let entries = toml::parse(text)?;
        let mut palette = match entries.iter().find(|e| e.key == BASE_KEY) {
            None => FT2_PALETTE,
            Some(base) => match base.value.as_str() {
                "ft2" => FT2_PALETTE,
                "ansi16" => ANSI16_PALETTE,
                _ => {
                    return Err(ParseError::new(
                        base.line,
                        "base must be \"ft2\" or \"ansi16\"",
                    ));
                }
            },
        };
        for entry in entries.iter().filter(|e| e.key != BASE_KEY) {
            let mut colors = palette.entries();
            let Some((_, _, color)) = colors.iter_mut().find(|(key, _, _)| *key == entry.key)
            else {
                return Err(ParseError::new(
                    entry.line,
                    format!("unknown color {}", entry.key),
                ));
            };
            **color = entry.value.parse().map_err(|_| {
                ParseError::new(entry.line, format!("{:?} is not a color", entry.value))
            })?;
        }
        return Ok(palette);
    }

    pub fn into_toml(mut self) -> String {
        let mut text = String::from(
            "# mTrak theme, used with --color custom or theme = \"custom\" in config.toml.\n\
             # Colors are hex (\"#49688C\") or ANSI names (\"Red\", \"DarkGray\", ...).\n\
             # Colors left out come from the palette named by base (\"ft2\" or \"ansi16\").\n\
             # Delete this file to get the FT2 colors back.\n",
        );
        for (key, description, color) in self.entries() {
            text.push_str(&format!("\n# {description}\n{key} = \"{color}\"\n"));
        }
        return text;
    }

    /// The theme file written when there isn't one yet.
    pub fn default_toml() -> String {
        return FT2_PALETTE.into_toml();
    }
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
    pub fn from_choice(choice: Option<ThemeChoice>) -> Result<Self> {
        return Ok(match choice {
            Some(ThemeChoice::Mono) => Self::monochrome(),
            Some(ThemeChoice::Ansi16) => Self::ansi16(),
            Some(ThemeChoice::Full) => Self::ft2(),
            Some(ThemeChoice::Custom) => Self::custom()?,
            None => Self::detect(),
        });
    }

    /// Built from theme.toml in the config folder.
    pub fn custom() -> Result<Self> {
        let palette = Palette::load(&files::get_theme_file_path()?)?;
        return Ok(Self::from_palette(&palette));
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn the_default_file_reads_back_as_ft2() {
        let palette = Palette::from_toml(&Palette::default_toml()).unwrap();
        assert_eq!(palette.backdrop, FT2_PALETTE.backdrop);
        assert_eq!(palette.mark, FT2_PALETTE.mark);
    }

    #[test]
    fn colors_left_out_come_from_the_base() {
        let palette = Palette::from_toml("base = \"ansi16\"\ndanger = \"#FF0000\"").unwrap();
        assert_eq!(palette.danger, Color::Rgb(255, 0, 0));
        assert_eq!(palette.backdrop, Color::Blue);
    }

    #[test]
    fn the_example_themes_load() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("example_themes");
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "toml") {
                Palette::load(&path).unwrap_or_else(|e| panic!("{}: {}", path.display(), e));
            }
        }
    }

    #[test]
    fn rejects_unknown_keys_and_colors() {
        assert_eq!(
            Palette::from_toml("text = \"Red\"\nback = \"Red\"")
                .err()
                .unwrap()
                .line,
            2
        );
        assert_eq!(
            Palette::from_toml("text = \"bright\"").err().unwrap().line,
            1
        );
        assert!(Palette::from_toml("base = \"pink\"").is_err());
    }
}
