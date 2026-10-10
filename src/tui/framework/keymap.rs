use std::fmt::{self, Display};

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::tui::{app::AppState, glyphs};

pub const CONTROL_SYMBOL: char = '^';
pub const SHIFT_SYMBOL: &'static str = "⇧";
pub const ASCII_SHIFT_SYMBOL: &'static str = "S-";
pub const ALT_SYMBOL: &'static str = "A-";
/// A key plus its modifiers, e.g. Ctrl+S.
///
/// Terminals disagree on how they report Shift: some send `Char('S')`, others `Char('s')` with
/// SHIFT, and symbols like `?` may or may not carry SHIFT. `Key::new` normalises all of these
/// so a binding matches no matter which form the terminal sends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl Key {
    pub fn new(code: KeyCode, mut modifiers: KeyModifiers) -> Self {
        let code = match code {
            KeyCode::Char(c) if c.is_ascii_uppercase() => {
                modifiers.insert(KeyModifiers::SHIFT);
                KeyCode::Char(c.to_ascii_lowercase())
            }
            KeyCode::Char(c) if !c.is_ascii_alphabetic() => {
                modifiers.remove(KeyModifiers::SHIFT);
                code
            }
            KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => {
                modifiers.remove(KeyModifiers::SHIFT);
                KeyCode::BackTab
            }
            KeyCode::BackTab => {
                modifiers.remove(KeyModifiers::SHIFT);
                code
            }
            _ => code,
        };
        return Self { code, modifiers };
    }

    pub fn plain(code: KeyCode) -> Self {
        return Self::new(code, KeyModifiers::NONE);
    }

    pub fn char(c: char) -> Self {
        return Self::plain(KeyCode::Char(c));
    }

    pub fn ctrl(c: char) -> Self {
        return Self::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    }

    /// The number of an unmodified function key, e.g. 9 for F9.
    fn function_number(&self) -> Option<u8> {
        return match self.code {
            KeyCode::F(n) if self.modifiers.is_empty() => Some(n),
            _ => None,
        };
    }

    pub fn matches(&self, event: &KeyEvent) -> bool {
        // some platforms (e.g. Windows) also report key releases, which should not trigger actions
        if event.kind == KeyEventKind::Release {
            return false;
        }
        return *self == Key::new(event.code, event.modifiers);
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            write!(f, "{}", CONTROL_SYMBOL)?;
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            write!(f, "{}", ALT_SYMBOL)?;
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            write!(f, "{}", glyphs::pick(SHIFT_SYMBOL, ASCII_SHIFT_SYMBOL))?;
        }
        let has_modifier = self
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
        return match self.code {
            KeyCode::Char(' ') => write!(f, "Spc"),
            KeyCode::Char(c) if has_modifier => write!(f, "{}", c.to_ascii_uppercase()),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Up => write!(f, "{}", glyphs::pick("↑", "Up")),
            KeyCode::Down => write!(f, "{}", glyphs::pick("↓", "Dn")),
            KeyCode::Left => write!(f, "{}", glyphs::pick("←", "Lt")),
            KeyCode::Right => write!(f, "{}", glyphs::pick("→", "Rt")),
            KeyCode::F(n) => write!(f, "F{n}"),
            KeyCode::BackTab => write!(f, "{}Tab", glyphs::pick(SHIFT_SYMBOL, ASCII_SHIFT_SYMBOL)),
            KeyCode::PageUp => write!(f, "PgUp"),
            KeyCode::PageDown => write!(f, "PgDn"),
            KeyCode::Insert => write!(f, "Ins"),
            KeyCode::Delete => write!(f, "Del"),
            KeyCode::Backspace => write!(f, "Bksp"),
            other => write!(f, "{other:?}"),
        };
    }
}

/// A key with a short description, ready to show in a footer or next to a control.
pub struct KeyHint {
    pub key: String,
    pub label: &'static str,
}

pub struct Binding<A> {
    pub key: Key,
    pub action: A,
    pub label: &'static str,
}

/// Maps keys to a widget's actions. The same table drives event handling (`lookup`) and
/// what gets displayed (`hints`, `key_for`), so the two can never drift apart.
pub struct Keymap<A> {
    bindings: Vec<Binding<A>>,
}

impl<A: Copy + PartialEq> Keymap<A> {
    pub fn new() -> Self {
        return Self {
            bindings: Vec::new(),
        };
    }

    pub fn bind(mut self, key: Key, action: A, label: &'static str) -> Self {
        self.bindings.push(Binding { key, action, label });
        return self;
    }

    /// Every binding, in the order they were added.
    pub fn bindings(&self) -> &[Binding<A>] {
        return &self.bindings;
    }

    /// The action bound to the pressed key, if any.
    pub fn lookup(&self, event: &KeyEvent) -> Option<A> {
        return self
            .bindings
            .iter()
            .find(|b| b.key.matches(event))
            .map(|b| b.action);
    }

    /// The first key bound to `action`, for showing it next to the thing it controls.
    pub fn key_for(&self, action: A) -> Option<Key> {
        return self
            .bindings
            .iter()
            .find(|b| b.action == action)
            .map(|b| b.key);
    }

    pub fn hint_for(&self, action: A) -> Option<KeyHint> {
        return self
            .bindings
            .iter()
            .find(|b| b.action == action)
            .map(|b| KeyHint {
                key: b.key.to_string(),
                label: b.label,
            });
    }

    /// Appends a hint per label, merging the keys that share it, e.g. "↑/↓ Move row".
    pub fn hints(&self, hints: &mut Vec<KeyHint>) {
        let mut groups: Vec<(&'static str, Vec<Key>)> = Vec::new();
        for binding in &self.bindings {
            match groups.iter_mut().find(|(label, _)| *label == binding.label) {
                Some((_, keys)) => keys.push(binding.key),
                None => groups.push((binding.label, vec![binding.key])),
            }
        }
        hints.extend(groups.into_iter().map(|(label, keys)| KeyHint {
            key: join_keys(&keys),
            label,
        }));
    }
}

/// Joins keys with "/", collapsing runs of three or more consecutive function keys into a
/// range, e.g. "Fn9-12" instead of "F9/F10/F11/F12".
fn join_keys(keys: &[Key]) -> String {
    let mut parts = Vec::new();
    let mut start = 0;
    while start < keys.len() {
        let mut end = start;
        while let (Some(a), Some(b)) = (
            keys[end].function_number(),
            keys.get(end + 1).and_then(Key::function_number),
        ) {
            if b != a + 1 {
                break;
            }
            end += 1;
        }
        if end - start >= 2 {
            let first = keys[start].function_number().unwrap();
            let last = keys[end].function_number().unwrap();
            parts.push(format!("Fn{first}-{last}"));
        } else {
            parts.extend(keys[start..=end].iter().map(Key::to_string));
        }
        start = end + 1;
    }
    return parts.join("/");
}

/// Implemented by widgets that react to keys through a `Keymap`. Provides the shared glue
/// (event -> lookup -> perform) so each widget only has to say what its actions do.
pub trait KeyActions {
    type Action: Copy + PartialEq;

    /// The keymap that is currently active, e.g. a different one per mode.
    fn keymap(&self) -> &Keymap<Self::Action>;

    fn perform(&mut self, action: Self::Action, state: &mut AppState);

    /// Runs the action bound to `event`, returning whether it was handled.
    fn dispatch(&mut self, state: &mut AppState, event: &Event) -> bool {
        let Event::Key(key) = event else {
            return false;
        };
        let Some(action) = self.keymap().lookup(key) else {
            return false;
        };
        self.perform(action, state);
        return true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        return KeyEvent::new(code, modifiers);
    }

    #[test]
    fn shifted_letters_match_either_form() {
        let key = Key::char('S');
        assert!(key.matches(&press(KeyCode::Char('S'), KeyModifiers::NONE)));
        assert!(key.matches(&press(KeyCode::Char('S'), KeyModifiers::SHIFT)));
        assert!(key.matches(&press(KeyCode::Char('s'), KeyModifiers::SHIFT)));
        assert!(!key.matches(&press(KeyCode::Char('s'), KeyModifiers::NONE)));
    }

    #[test]
    fn symbols_ignore_shift() {
        let key = Key::char('?');
        assert!(key.matches(&press(KeyCode::Char('?'), KeyModifiers::SHIFT)));
        assert!(key.matches(&press(KeyCode::Char('?'), KeyModifiers::NONE)));
    }

    #[test]
    fn shift_tab_matches_either_form() {
        for key in [
            Key::plain(KeyCode::BackTab),
            Key::new(KeyCode::Tab, KeyModifiers::SHIFT),
        ] {
            assert!(key.matches(&press(KeyCode::BackTab, KeyModifiers::SHIFT)));
            assert!(key.matches(&press(KeyCode::BackTab, KeyModifiers::NONE)));
            assert!(!key.matches(&press(KeyCode::Tab, KeyModifiers::NONE)));
            assert_eq!(key.to_string(), "⇧Tab");
        }
    }

    #[test]
    fn releases_do_not_match() {
        let mut event = press(KeyCode::Char('q'), KeyModifiers::CONTROL);
        event.kind = KeyEventKind::Release;
        assert!(!Key::ctrl('q').matches(&event));
    }

    #[test]
    fn display() {
        assert_eq!(Key::ctrl('s').to_string(), "^S");
        assert_eq!(Key::char(' ').to_string(), "Spc");
        assert_eq!(Key::plain(KeyCode::Up).to_string(), "↑");
        assert_eq!(Key::plain(KeyCode::Enter).to_string(), "Enter");
    }

    #[test]
    fn hints_merge_keys_with_the_same_label() {
        let keymap = Keymap::new()
            .bind(Key::plain(KeyCode::Up), 0, "ROW")
            .bind(Key::plain(KeyCode::Down), 1, "ROW")
            .bind(Key::ctrl('s'), 2, "Save");
        let mut hints = Vec::new();
        keymap.hints(&mut hints);
        let shown: Vec<(String, &str)> = hints.into_iter().map(|h| (h.key, h.label)).collect();
        assert_eq!(
            shown,
            vec![("↑/↓".to_string(), "ROW"), ("^S".to_string(), "Save")]
        );
    }

    #[test]
    fn hints_collapse_function_key_runs() {
        let keymap = (9..=12)
            .fold(Keymap::new(), |keymap, n| {
                keymap.bind(Key::plain(KeyCode::F(n)), n, "JMP")
            })
            .bind(Key::plain(KeyCode::F(1)), 1, "Help")
            .bind(Key::plain(KeyCode::F(2)), 2, "Help");
        let mut hints = Vec::new();
        keymap.hints(&mut hints);
        let shown: Vec<(String, &str)> = hints.into_iter().map(|h| (h.key, h.label)).collect();
        assert_eq!(
            shown,
            vec![("Fn9-12".to_string(), "JMP"), ("F1/F2".to_string(), "Help")]
        );
    }

    #[test]
    fn hints_merge_any_number_of_keys() {
        let keymap = Keymap::new()
            .bind(Key::plain(KeyCode::Delete), 0, "Del")
            .bind(Key::plain(KeyCode::Backspace), 1, "Del")
            .bind(Key::new(KeyCode::Delete, KeyModifiers::SHIFT), 2, "Del")
            .bind(Key::new(KeyCode::Delete, KeyModifiers::ALT), 3, "Del");
        let mut hints = Vec::new();
        keymap.hints(&mut hints);
        let shown: Vec<(String, &str)> = hints.into_iter().map(|h| (h.key, h.label)).collect();
        assert_eq!(shown, vec![("Del/Bksp/⇧Del/A-Del".to_string(), "Del")]);
    }
}
