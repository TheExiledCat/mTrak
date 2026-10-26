use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Display, Write},
};

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::{
    error::Result,
    tui::{app::AppState, glyphs, keybinds},
};

#[cfg(target_os = "macos")]
pub const CONTROL_SYMBOL: &str = "⌃";
#[cfg(not(target_os = "macos"))]
pub const CONTROL_SYMBOL: &str = "^";
pub const ASCII_CONTROL_SYMBOL: &str = "^";
#[cfg(target_os = "macos")]
pub const ALT_SYMBOL: &str = "⌥";
#[cfg(not(target_os = "macos"))]
pub const ALT_SYMBOL: &str = "A-";
pub const ASCII_ALT_SYMBOL: &str = "A-";
pub const SHIFT_SYMBOL: &str = "⇧";
pub const ASCII_SHIFT_SYMBOL: &str = "S-";
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
    pub fn alt(c: char) -> Self {
        return Self::new(KeyCode::Char(c), KeyModifiers::ALT);
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

impl Key {
    fn write(&self, f: &mut impl Write, ascii: bool) -> fmt::Result {
        let pick = |unicode, plain| {
            if ascii {
                plain
            } else {
                glyphs::pick(unicode, plain)
            }
        };
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            write!(f, "{}", pick(CONTROL_SYMBOL, ASCII_CONTROL_SYMBOL))?;
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            write!(f, "{}", pick(ALT_SYMBOL, ASCII_ALT_SYMBOL))?;
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            write!(f, "{}", pick(SHIFT_SYMBOL, ASCII_SHIFT_SYMBOL))?;
        }
        let has_modifier = self
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
        return match self.code {
            KeyCode::Char(' ') => write!(f, "Spc"),
            KeyCode::Char(c) if has_modifier => write!(f, "{}", c.to_ascii_uppercase()),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Up => write!(f, "{}", pick("↑", "Up")),
            KeyCode::Down => write!(f, "{}", pick("↓", "Dn")),
            KeyCode::Left => write!(f, "{}", pick("←", "Lt")),
            KeyCode::Right => write!(f, "{}", pick("→", "Rt")),
            KeyCode::F(n) => write!(f, "F{n}"),
            KeyCode::BackTab => write!(f, "{}Tab", pick(SHIFT_SYMBOL, ASCII_SHIFT_SYMBOL)),
            KeyCode::PageUp => write!(f, "PgUp"),
            KeyCode::PageDown => write!(f, "PgDn"),
            KeyCode::Insert => write!(f, "Ins"),
            KeyCode::Delete => write!(f, "Del"),
            KeyCode::Backspace => write!(f, "Bksp"),
            other => write!(f, "{other:?}"),
        };
    }

    /// The key as written in keybinds.toml, e.g. `Z`, `^S`, `A-O` or `S-Tab`.
    pub fn to_text(self) -> String {
        if self.modifiers == KeyModifiers::SHIFT
            && let KeyCode::Char(c) = self.code
            && c.is_ascii_alphabetic()
        {
            return c.to_ascii_uppercase().to_string();
        }
        let mut text = String::new();
        let _ = self.write(&mut text, true);
        return text;
    }

    /// Reads `to_text`'s format. A prefix needs a key after it, so `^` alone is the caret key.
    pub fn parse(text: &str) -> Option<Self> {
        let mut modifiers = KeyModifiers::NONE;
        let mut rest = text;
        loop {
            let (modifier, after) = if let Some(after) = rest.strip_prefix(ASCII_CONTROL_SYMBOL) {
                (KeyModifiers::CONTROL, after)
            } else if let Some(after) = rest.strip_prefix(ASCII_ALT_SYMBOL) {
                (KeyModifiers::ALT, after)
            } else if let Some(after) = rest.strip_prefix(ASCII_SHIFT_SYMBOL) {
                (KeyModifiers::SHIFT, after)
            } else {
                break;
            };
            if after.is_empty() {
                break;
            }
            modifiers.insert(modifier);
            rest = after;
        }
        let code = match rest {
            "Spc" => KeyCode::Char(' '),
            "Up" => KeyCode::Up,
            "Dn" | "Down" => KeyCode::Down,
            "Lt" | "Left" => KeyCode::Left,
            "Rt" | "Right" => KeyCode::Right,
            "PgUp" => KeyCode::PageUp,
            "PgDn" => KeyCode::PageDown,
            "Ins" => KeyCode::Insert,
            "Del" => KeyCode::Delete,
            "Bksp" => KeyCode::Backspace,
            "Enter" => KeyCode::Enter,
            "Esc" => KeyCode::Esc,
            "Tab" => KeyCode::Tab,
            "Home" => KeyCode::Home,
            "End" => KeyCode::End,
            _ => {
                let mut chars = rest.chars();
                match (chars.next(), chars.next()) {
                    // ^S is Ctrl+S, not Ctrl+Shift+S
                    (Some(c), None) if !modifiers.is_empty() => {
                        KeyCode::Char(c.to_ascii_lowercase())
                    }
                    (Some(c), None) => KeyCode::Char(c),
                    _ => KeyCode::F(rest.strip_prefix('F')?.parse().ok()?),
                }
            }
        };
        return Some(Self::new(code, modifiers));
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return self.write(f, false);
    }
}

/// A key with a short description, ready to show in a footer or next to a control.
pub struct KeyHint {
    pub key: String,
    pub label: &'static str,
    pub enabled: bool,
}
impl KeyHint {
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        return self;
    }
}

pub struct HelpEntry {
    pub key: String,
    pub description: &'static str,
    /// Set when the entry can be rebound.
    pub binding: Option<BindingInfo>,
}

#[derive(Clone)]
pub struct BindingInfo {
    /// e.g. `pattern.toggle_edit`
    pub id: String,
    pub defaults: Vec<Key>,
}

pub trait Action: Copy + PartialEq + fmt::Debug {
    /// The variant name in snake_case. Override it for variants with values.
    fn id(self) -> String {
        return snake_case(&format!("{self:?}"));
    }
}

pub fn snake_case(name: &str) -> String {
    let mut text = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                text.push('_');
            }
            text.push(c.to_ascii_lowercase());
        } else {
            text.push(c);
        }
    }
    return text;
}

pub struct HelpSection {
    pub title: &'static str,
    pub entries: Vec<HelpEntry>,
}

impl HelpSection {
    pub fn new(title: &'static str) -> Self {
        return Self {
            title,
            entries: Vec::new(),
        };
    }

    pub fn entry(mut self, key: impl Into<String>, description: &'static str) -> Self {
        self.entries.push(HelpEntry {
            key: key.into(),
            description,
            binding: None,
        });
        return self;
    }

    pub fn entries(mut self, entries: Vec<HelpEntry>) -> Self {
        self.entries.extend(entries);
        return self;
    }
}

#[derive(Clone, Copy)]
pub struct Binding<A> {
    pub key: Key,
    pub action: A,
    pub label: &'static str,
}

/// Maps keys to a widget's actions. The same table drives event handling (`lookup`) and
/// what gets displayed (`hints`, `key_for`), so the two can never drift apart.
/// Keys from keybinds.toml replace the ones passed to `bind`.
pub struct Keymap<A> {
    /// Prefix of the action ids, e.g. `pattern`.
    scope: &'static str,
    bindings: Vec<Binding<A>>,
}

impl<A: Action> Keymap<A> {
    pub fn new(scope: &'static str) -> Self {
        return Self {
            scope,
            bindings: Vec::new(),
        };
    }

    pub fn bind(mut self, key: Key, action: A, label: &'static str) -> Self {
        self.bindings.push(Binding { key, action, label });
        return self;
    }

    pub fn id(&self, action: A) -> String {
        return format!("{}.{}", self.scope, action.id());
    }

    /// The bindings with keybinds.toml applied.
    pub fn bindings(&self) -> Cow<'_, [Binding<A>]> {
        return keybinds::with_overrides(|overrides| self.bindings_with(overrides));
    }

    fn bindings_with(&self, overrides: &HashMap<String, Vec<Key>>) -> Cow<'_, [Binding<A>]> {
        if overrides.is_empty() {
            return Cow::Borrowed(self.bindings.as_slice());
        }
        let mut bindings = Vec::new();
        let mut overridden: Vec<A> = Vec::new();
        for binding in &self.bindings {
            let Some(keys) = overrides.get(&self.id(binding.action)) else {
                bindings.push(*binding);
                continue;
            };
            if !overridden.contains(&binding.action) {
                overridden.push(binding.action);
                bindings.extend(keys.iter().map(|&key| Binding { key, ..*binding }));
            }
        }
        return Cow::Owned(bindings);
    }

    /// The defaults, e.g. to change a label.
    pub fn default_bindings_mut(&mut self) -> &mut [Binding<A>] {
        return &mut self.bindings;
    }

    /// The action bound to the pressed key, if any.
    pub fn lookup(&self, event: &KeyEvent) -> Option<A> {
        return self
            .bindings()
            .iter()
            .find(|b| b.key.matches(event))
            .map(|b| b.action);
    }

    /// The first key bound to `action`, for showing it next to the thing it controls.
    pub fn key_for(&self, action: A) -> Option<Key> {
        return self
            .bindings()
            .iter()
            .find(|b| b.action == action)
            .map(|b| b.key);
    }

    /// The key as shown on screen, or `--` when unbound.
    pub fn key_text(&self, action: A) -> String {
        return self
            .key_for(action)
            .map_or_else(|| NO_KEY.to_owned(), |key| key.to_string());
    }

    /// `key: label`, e.g. for the header buttons.
    pub fn labelled_key(&self, action: A) -> String {
        let label = self
            .bindings
            .iter()
            .find(|b| b.action == action)
            .map_or("", |b| b.label);
        return format!("{}: {label}", self.key_text(action));
    }

    pub fn hint_for(&self, action: A) -> Option<KeyHint> {
        return self
            .bindings()
            .iter()
            .find(|b| b.action == action)
            .map(|b| KeyHint {
                key: b.key.to_string(),
                label: b.label,
                enabled: true,
            });
    }

    /// Appends a hint per label, merging the keys that share it, e.g. "↑/↓ Move row".
    pub fn hints(&self, hints: &mut Vec<KeyHint>) {
        let mut groups: Vec<(&'static str, Vec<Key>)> = Vec::new();
        for binding in self.bindings().iter() {
            match groups.iter_mut().find(|(label, _)| *label == binding.label) {
                Some((_, keys)) => keys.push(binding.key),
                None => groups.push((binding.label, vec![binding.key])),
            }
        }
        hints.extend(groups.into_iter().map(|(label, keys)| KeyHint {
            key: join_keys(&keys),
            label,
            enabled: true,
        }));
    }
}

impl<A: Action> Keymap<A> {
    /// One entry per action.
    pub fn help_entries(&self, describe: impl Fn(A) -> &'static str) -> Vec<HelpEntry> {
        let mut actions: Vec<A> = Vec::new();
        for binding in &self.bindings {
            if !actions.contains(&binding.action) {
                actions.push(binding.action);
            }
        }
        let bindings = self.bindings();
        return actions
            .into_iter()
            .map(|action| {
                let keys_of = |bindings: &[Binding<A>]| -> Vec<Key> {
                    return bindings
                        .iter()
                        .filter(|b| b.action == action)
                        .map(|b| b.key)
                        .collect();
                };
                let keys = keys_of(&bindings);
                return HelpEntry {
                    key: if keys.is_empty() {
                        UNBOUND.to_owned()
                    } else {
                        join_keys(&keys)
                    },
                    description: describe(action),
                    binding: Some(BindingInfo {
                        id: self.id(action),
                        defaults: keys_of(&self.bindings),
                    }),
                };
            })
            .collect();
    }
}

pub const UNBOUND: &str = "none";
/// Two dashes, since `-` is a key itself.
const NO_KEY: &str = "--";

/// Joins keys with "/", collapsing runs of three or more consecutive function keys into a
/// range, e.g. "Fn9-12" instead of "F9/F10/F11/F12".
pub fn join_keys(keys: &[Key]) -> String {
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
    type Action: Action;

    /// The keymap that is currently active, e.g. a different one per mode.
    fn keymap(&self) -> &Keymap<Self::Action>;

    /// Runs `action`. Errors are shown to the user in a warning dialog.
    fn perform(&mut self, action: Self::Action, state: &mut AppState) -> Result<()>;

    fn describe(action: Self::Action) -> &'static str;

    fn help_section(&self, title: &'static str) -> HelpSection {
        return HelpSection::new(title).entries(self.keymap().help_entries(Self::describe));
    }

    /// Runs the action bound to `event`, returning whether it was handled.
    fn dispatch(&mut self, state: &mut AppState, event: &Event) -> bool {
        let Event::Key(key) = event else {
            return false;
        };
        let Some(action) = self.keymap().lookup(key) else {
            return false;
        };
        if let Err(e) = self.perform(action, state) {
            state.report_error(e);
        }
        return true;
    }
}

#[cfg(test)]
impl Action for i32 {}
#[cfg(test)]
impl Action for u8 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_read_back_from_text() {
        let keys = [
            Key::ctrl('s'),
            Key::alt('o'),
            Key::char('Z'),
            Key::char('z'),
            Key::char(' '),
            Key::char('^'),
            Key::char('-'),
            Key::new(KeyCode::Char('-'), KeyModifiers::ALT),
            Key::new(
                KeyCode::Char('s'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            Key::new(KeyCode::Tab, KeyModifiers::SHIFT),
            Key::plain(KeyCode::Up),
            Key::plain(KeyCode::F(12)),
            Key::new(KeyCode::Delete, KeyModifiers::ALT),
        ];
        for key in keys {
            assert_eq!(Key::parse(&key.to_text()), Some(key), "{}", key.to_text());
        }
        assert_eq!(Key::char('Z').to_text(), "Z");
        assert_eq!(Key::ctrl('s').to_text(), "^S");
        assert_eq!(Key::parse("^"), Some(Key::char('^')));
        assert_eq!(
            Key::parse("^^"),
            Some(Key::new(KeyCode::Char('^'), KeyModifiers::CONTROL))
        );
        assert_eq!(Key::parse("Down"), Some(Key::plain(KeyCode::Down)));
        assert_eq!(Key::parse("Fx"), None);
        assert_eq!(Key::parse(""), None);
    }

    #[test]
    fn overrides_replace_every_default_key_of_their_action() {
        let keymap = Keymap::new("test")
            .bind(Key::plain(KeyCode::Up), 0, "Row")
            .bind(Key::char('k'), 0, "Row")
            .bind(Key::plain(KeyCode::Down), 1, "Row");
        let overrides = HashMap::from([
            ("test.0".to_owned(), vec![Key::char('w')]),
            ("test.1".to_owned(), vec![]),
        ]);
        let bindings = keymap.bindings_with(&overrides);
        let keys: Vec<(Key, i32)> = bindings.iter().map(|b| (b.key, b.action)).collect();
        assert_eq!(keys, vec![(Key::char('w'), 0)]);
        assert_eq!(keymap.bindings_with(&HashMap::new()).len(), 3);
    }

    #[test]
    fn action_ids_are_snake_case() {
        assert_eq!(snake_case("ToggleEdit"), "toggle_edit");
        assert_eq!(snake_case("Up"), "up");
    }

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
        assert_eq!(Key::ctrl('s').to_string(), format!("{CONTROL_SYMBOL}S"));
        assert_eq!(Key::char(' ').to_string(), "Spc");
        assert_eq!(Key::plain(KeyCode::Up).to_string(), "↑");
        assert_eq!(Key::plain(KeyCode::Enter).to_string(), "Enter");
    }

    #[test]
    fn hints_merge_keys_with_the_same_label() {
        let keymap = Keymap::new("test")
            .bind(Key::plain(KeyCode::Up), 0, "ROW")
            .bind(Key::plain(KeyCode::Down), 1, "ROW")
            .bind(Key::ctrl('s'), 2, "Save");
        let mut hints = Vec::new();
        keymap.hints(&mut hints);
        let shown: Vec<(String, &str)> = hints.into_iter().map(|h| (h.key, h.label)).collect();
        assert_eq!(
            shown,
            vec![
                ("↑/↓".to_string(), "ROW"),
                (format!("{CONTROL_SYMBOL}S"), "Save")
            ]
        );
    }

    #[test]
    fn hints_collapse_function_key_runs() {
        let keymap = (9..=12)
            .fold(Keymap::new("test"), |keymap, n| {
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
        let keymap = Keymap::new("test")
            .bind(Key::plain(KeyCode::Delete), 0, "Del")
            .bind(Key::plain(KeyCode::Backspace), 1, "Del")
            .bind(Key::new(KeyCode::Delete, KeyModifiers::SHIFT), 2, "Del")
            .bind(Key::new(KeyCode::Delete, KeyModifiers::ALT), 3, "Del");
        let mut hints = Vec::new();
        keymap.hints(&mut hints);
        let shown: Vec<(String, &str)> = hints.into_iter().map(|h| (h.key, h.label)).collect();
        assert_eq!(
            shown,
            vec![(format!("Del/Bksp/⇧Del/{ALT_SYMBOL}Del"), "Del")]
        );
    }
}
