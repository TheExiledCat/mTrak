use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::buffer::Buffer;

static ASCII_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_ascii_mode(enabled: bool) {
    ASCII_MODE.store(enabled, Ordering::Relaxed);
}

pub fn ascii_mode() -> bool {
    return ASCII_MODE.load(Ordering::Relaxed);
}

pub fn pick(unicode: &'static str, ascii: &'static str) -> &'static str {
    return if ascii_mode() { ascii } else { unicode };
}

pub fn to_ascii(c: char) -> char {
    return match c {
        c if c.is_ascii() => c,
        '\u{2500}' | '\u{2501}' | '\u{2504}' | '\u{2505}' | '\u{2508}' | '\u{2509}'
        | '\u{254C}' | '\u{254D}' | '\u{2550}' | '\u{2574}' | '\u{2576}' | '\u{2578}'
        | '\u{257A}' | '\u{257C}' | '\u{257E}' => '-',
        '\u{2502}' | '\u{2503}' | '\u{2506}' | '\u{2507}' | '\u{250A}' | '\u{250B}'
        | '\u{254E}' | '\u{254F}' | '\u{2551}' | '\u{2575}' | '\u{2577}' | '\u{2579}'
        | '\u{257B}' | '\u{257D}' | '\u{257F}' => '|',
        '\u{2571}' => '/',
        '\u{2572}' => '\\',
        '\u{2573}' => 'X',
        '\u{2500}'..='\u{257F}' => '+',
        '\u{2580}'..='\u{259F}' => '#',
        '↑' | '▲' | '⇧' => '^',
        '↓' | '▼' => 'v',
        '←' | '◀' => '<',
        '→' | '▶' => '>',
        '•' | '·' => '.',
        '…' => '~',
        '±' => '+',
        _ => '?',
    };
}

pub fn asciify(buf: &mut Buffer) {
    for cell in buf.content.iter_mut() {
        let symbol = cell.symbol();
        if symbol.is_ascii() {
            continue;
        }
        let c = to_ascii(symbol.chars().next().unwrap_or(' '));
        cell.set_char(c);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{
        layout::Rect,
        widgets::{Block, Widget},
    };

    use super::*;

    #[test]
    fn borders_become_ascii() {
        let area = Rect::new(0, 0, 4, 3);
        let mut buf = Buffer::empty(area);
        Block::bordered().render(area, &mut buf);
        asciify(&mut buf);
        assert_eq!(buf, Buffer::with_lines(["+--+", "|  |", "+--+"]));
    }
}
