use std::time::{Duration, Instant};

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::tui::{
    app::AppState,
    framework::{keymap::KeyHint, widget::MtrakWidget},
};

/// How long the hints rest at either end before scrolling on.
const SCROLL_PAUSE: Duration = Duration::from_secs(2);
/// How long each column is shown while the hints scroll.
const SCROLL_STEP: Duration = Duration::from_millis(500);

pub struct Footer {
    hints: Vec<KeyHint>,
    /// When the current hints were first shown, which the scroll position is derived from.
    shown_since: Instant,
}
impl Footer {
    pub fn new() -> Self {
        return Self {
            hints: Vec::new(),
            shown_since: Instant::now(),
        };
    }

    /// Replaces the hints, restarting the scroll from the start when they changed.
    pub fn set_hints(&mut self, hints: Vec<KeyHint>) {
        let changed = hints.len() != self.hints.len()
            || hints
                .iter()
                .zip(&self.hints)
                .any(|(a, b)| a.key != b.key || a.label != b.label);
        if changed {
            self.shown_since = Instant::now();
        }
        self.hints = hints;
    }

    fn get_hint_line(&self, state: &AppState) -> Line<'static> {
        let spans = self.hints.iter().flat_map(|hint| {
            [
                Span::styled(format!(" {} ", hint.key), state.theme.key_hint_key),
                Span::styled(format!(" {}  ", hint.label), state.theme.key_hint_label),
            ]
        });
        Line::from_iter(spans)
    }
}

/// Columns to scroll hints that are `overflow` columns too wide, `elapsed` after they were
/// first shown: rest at the start, step one column at a time to the end, rest there, then
/// jump back to the start and repeat.
fn scroll_offset(elapsed: Duration, overflow: u16) -> u16 {
    if overflow == 0 {
        return 0;
    }
    let scroll = SCROLL_STEP * overflow as u32;
    let cycle = SCROLL_PAUSE * 2 + scroll;
    let t = Duration::from_nanos((elapsed.as_nanos() % cycle.as_nanos()) as u64);
    if t < SCROLL_PAUSE {
        return 0;
    }
    let t = t - SCROLL_PAUSE;
    if t >= scroll {
        return overflow;
    }
    return (t.as_nanos() / SCROLL_STEP.as_nanos()) as u16;
}

impl MtrakWidget for Footer {
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let line = self.get_hint_line(state);
        let overflow = (line.width() as u16).saturating_sub(area.width);
        let offset = scroll_offset(self.shown_since.elapsed(), overflow);
        Paragraph::new(line).scroll((0, offset)).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_that_fit_never_scroll() {
        assert_eq!(scroll_offset(SCROLL_PAUSE * 3, 0), 0);
    }

    #[test]
    fn scrolls_in_steps_between_pauses() {
        let overflow = 4;
        let scroll_start = SCROLL_PAUSE;
        let scroll_end = scroll_start + SCROLL_STEP * overflow as u32;
        let cycle = scroll_end + SCROLL_PAUSE;
        assert_eq!(scroll_offset(Duration::ZERO, overflow), 0);
        assert_eq!(
            scroll_offset(scroll_start - Duration::from_millis(1), overflow),
            0
        );
        assert_eq!(scroll_offset(scroll_start + SCROLL_STEP, overflow), 1);
        assert_eq!(scroll_offset(scroll_start + SCROLL_STEP * 3, overflow), 3);
        assert_eq!(scroll_offset(scroll_end, overflow), overflow);
        assert_eq!(
            scroll_offset(cycle - Duration::from_millis(1), overflow),
            overflow
        );
        // back at the start for the next round
        assert_eq!(scroll_offset(cycle, overflow), 0);
    }
}
