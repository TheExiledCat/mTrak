use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use super::{DialogComponent, DialogValue, NumberBox, TextPrompt, label_spans};
use crate::tui::{
    app::AppState,
    framework::{keymap::KeyHint, widget::MtrakWidget},
};

const LABELS: [&str; 4] = ["R", "G", "B", "Hex"];
const HEX_PART: usize = 3;
/// A channel's label, value and arrows, e.g. `R: 8D ↕`.
const CHANNEL_WIDTH: u16 = 9;

/// An RGB color: a number box per channel on one line and a hex field below them, kept in step
/// with each other. Tab moves through the four fields.
pub struct ColorPicker {
    id: &'static str,
    color: Color,
    channels: [NumberBox; 3],
    hex: TextPrompt,
    part: usize,
    focused: bool,
}

impl ColorPicker {
    pub fn new(id: &'static str) -> Self {
        let mut picker = Self {
            id,
            color: Color::Rgb(0, 0, 0),
            channels: [
                NumberBox::new("", "", 2).steps(),
                NumberBox::new("", "", 2).steps(),
                NumberBox::new("", "", 2).steps(),
            ],
            hex: TextPrompt::new("", ""),
            part: 0,
            focused: false,
        };
        picker.sync_fields();
        return picker;
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self.sync_fields();
        return self;
    }

    fn sync_fields(&mut self) {
        let rgb = to_rgb(self.color);
        for (channel, value) in self.channels.iter_mut().zip(rgb) {
            channel.set(value as i32);
        }
        self.hex.text = to_hex(rgb);
    }

    fn set_part(&mut self, part: usize) {
        self.set_part_focused(false);
        self.part = part;
        self.set_part_focused(true);
    }

    fn set_part_focused(&mut self, focused: bool) {
        if self.part == HEX_PART {
            self.hex.set_focused(focused);
            // drop a half-typed value
            self.hex.text = to_hex(to_rgb(self.color));
        } else {
            self.channels[self.part].set_focused(focused);
        }
    }

    fn render_label(&self, state: &AppState, part: usize, area: Rect, buf: &mut Buffer) {
        let focused = self.focused && self.part == part;
        Line::from(label_spans(state, LABELS[part], focused).to_vec()).render(area, buf);
    }

    fn handle_hex_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        let Event::Key(key) = event else {
            return false;
        };
        let takes = match key.code {
            KeyCode::Char(c) => (c.is_ascii_hexdigit() || c == '#') && self.hex.text.len() < 7,
            KeyCode::Backspace => true,
            _ => false,
        };
        if !takes || !self.hex.handle_event(state, event) {
            return false;
        }
        if let Some(rgb) = parse_hex(&self.hex.text) {
            self.color = Color::Rgb(rgb[0], rgb[1], rgb[2]);
            for (channel, value) in self.channels.iter_mut().zip(rgb) {
                channel.set(value as i32);
            }
        }
        return true;
    }
}

impl MtrakWidget for ColorPicker {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Release {
                return false;
            }
        }
        if self.part == HEX_PART {
            return self.handle_hex_event(state, event);
        }
        if !self.channels[self.part].handle_event(state, event) {
            return false;
        }
        let [r, g, b] = self.channels.each_ref().map(|c| c.number() as u8);
        self.color = Color::Rgb(r, g, b);
        self.hex.text = to_hex([r, g, b]);
        return true;
    }

    fn key_hints(&self, state: &AppState, hints: &mut Vec<KeyHint>) {
        if self.focused && self.part != HEX_PART {
            self.channels[self.part].key_hints(state, hints);
        }
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let [channel_row, hex_row] = Layout::vertical([Constraint::Length(1); 2]).areas(area);
        let channel_areas = Layout::horizontal([Constraint::Length(CHANNEL_WIDTH); 3])
            .spacing(1)
            .split(channel_row);
        for (part, channel_area) in channel_areas.iter().enumerate() {
            let [label_area, field_area] =
                Layout::horizontal([Constraint::Length(3), Constraint::Fill(1)])
                    .areas(*channel_area);
            self.render_label(state, part, label_area, buf);
            self.channels[part].render(state, field_area, buf);
        }
        let [label_area, hex_area, swatch_area] = Layout::horizontal([
            Constraint::Length(5),
            Constraint::Length(9),
            Constraint::Fill(1),
        ])
        .areas(hex_row);
        self.render_label(state, HEX_PART, label_area, buf);
        self.hex.render(state, hex_area, buf);
        let swatch = "\u{2588}".repeat(swatch_area.width.min(6) as usize);
        Span::styled(swatch, Style::new().fg(self.color)).render(swatch_area, buf);
    }
}

impl DialogComponent for ColorPicker {
    fn get_height(&self) -> u8 {
        return 2;
    }
    fn focusable(&self) -> bool {
        return true;
    }
    fn set_focused(&mut self, focused: bool) {
        if focused {
            self.part = 0;
        }
        self.focused = focused;
        self.set_part_focused(focused);
    }
    fn subcomponents(&self) -> usize {
        return LABELS.len();
    }
    fn focus_subcomponent(&mut self, index: usize) {
        self.set_part(index.min(HEX_PART));
    }
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        return Some((self.id, DialogValue::Color(self.color)));
    }
    fn set_value(&mut self, value: DialogValue) {
        if let DialogValue::Color(color) = value {
            self.color = color;
            self.sync_fields();
        }
    }
}

fn to_hex([r, g, b]: [u8; 3]) -> String {
    return format!("#{r:02X}{g:02X}{b:02X}");
}

/// `#RRGGBB`, the `#` optional.
fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    return Some([channel(0)?, channel(2)?, channel(4)?]);
}

/// The RGB values of a color, using the xterm defaults for the ANSI colors, which terminals
/// are free to show differently.
fn to_rgb(color: Color) -> [u8; 3] {
    const ANSI: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 0, 0],
        [0, 205, 0],
        [205, 205, 0],
        [0, 0, 238],
        [205, 0, 205],
        [0, 205, 205],
        [229, 229, 229],
        [127, 127, 127],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [92, 92, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    let index = match color {
        Color::Rgb(r, g, b) => return [r, g, b],
        Color::Reset | Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
        Color::Indexed(i) => i,
    };
    return match index {
        0..=15 => ANSI[index as usize],
        16..=231 => {
            let level = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
            let i = index - 16;
            [level(i / 36), level(i / 6 % 6), level(i % 6)]
        }
        _ => [8 + (index - 232) * 10; 3],
    };
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

    use super::*;
    use crate::{
        data::project::Project,
        tui::{framework::dialog::Dialog, theme::Theme},
    };

    fn press(picker: &mut ColorPicker, state: &mut AppState, code: KeyCode) {
        picker.handle_event(state, &Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    fn state() -> AppState {
        return AppState::new(Project::empty(), Theme::monochrome()).unwrap();
    }

    #[test]
    fn channels_update_the_hex_field() {
        let mut state = state();
        let mut picker = ColorPicker::new("c").color(Color::Rgb(0x10, 0x20, 0x30));
        picker.set_focused(true);
        press(&mut picker, &mut state, KeyCode::Up);
        picker.focus_subcomponent(1);
        press(&mut picker, &mut state, KeyCode::Right);
        assert_eq!(picker.color, Color::Rgb(0x11, 0x30, 0x30));
        assert_eq!(picker.hex.text, "#113030");
    }

    #[test]
    fn typing_a_full_hex_value_updates_the_channels() {
        let mut state = state();
        let mut picker = ColorPicker::new("c");
        picker.set_focused(true);
        picker.focus_subcomponent(HEX_PART);
        for _ in 0..7 {
            press(&mut picker, &mut state, KeyCode::Backspace);
        }
        for c in "#ABCDEF".chars() {
            press(&mut picker, &mut state, KeyCode::Char(c));
        }
        assert_eq!(picker.color, Color::Rgb(0xAB, 0xCD, 0xEF));
        assert_eq!(picker.channels[1].number(), 0xCD);
    }

    #[test]
    fn a_half_typed_hex_value_is_dropped_on_leaving() {
        let mut state = state();
        let mut picker = ColorPicker::new("c").color(Color::Rgb(1, 2, 3));
        picker.set_focused(true);
        picker.focus_subcomponent(HEX_PART);
        press(&mut picker, &mut state, KeyCode::Backspace);
        picker.focus_subcomponent(2);
        assert_eq!(picker.hex.text, "#010203");
        assert_eq!(picker.color, Color::Rgb(1, 2, 3));
    }

    #[test]
    fn tab_walks_the_fields_before_leaving() {
        use crate::tui::framework::{core_extensions::Boxed, dialog::Checkbox};

        let mut dialog = Dialog::new(
            "",
            vec![
                ColorPicker::new("c").boxed(),
                Checkbox::new("x", "X").boxed(),
            ],
        );
        for expected in [
            (Some(0), 0),
            (Some(0), 1),
            (Some(0), 2),
            (Some(0), 3),
            (Some(1), 0),
        ] {
            dialog.move_focus(true);
            assert_eq!((dialog.focus, dialog.sub_focus), expected);
        }
        dialog.move_focus(false);
        assert_eq!((dialog.focus, dialog.sub_focus), (Some(0), 3));
    }

    #[test]
    fn ansi_colors_convert_to_rgb() {
        assert_eq!(to_rgb(Color::DarkGray), [127, 127, 127]);
        assert_eq!(to_rgb(Color::Indexed(196)), [255, 0, 0]);
        assert_eq!(to_rgb(Color::Indexed(232)), [8, 8, 8]);
    }
}
