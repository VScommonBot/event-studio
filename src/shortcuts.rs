//! The keyboard shortcuts that work while one of the program's windows has the keyboard: which
//! key does what, how a person changes that, and how a key press is turned into an action.
//!
//! Every action has a default key (`Action::default_binding`); the person can give it another key
//! or combination, take its key away, or go back to the defaults. Only what differs from the
//! defaults is saved (`Shortcuts::custom`), so a build with another action later still gets that
//! action's default key from an older settings file. Nothing here needs a window: the
//! matching works on egui's key events, which the tests make up.
//!
//! - One key does one thing: a key given to a second action is taken from the first (`assign`).
//! - Keys the system, the window or text editing already use cannot be given (`reserved`).
//! - While a text box has the keyboard no shortcut fires, except the function keys, which type nothing.
//! - A shortcut that fired is taken out of the key events, so the button or slider that has the
//!   keyboard does not act on the same press a second time.
//!
//! The system-wide hotkeys (Ctrl+Alt+Space and so on, win.rs) are separate and stay as they are.
use super::*;
use std::collections::BTreeMap;

/// What a shortcut can do.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub(super) enum Action {
    ToggleTimer,
    Reset,
    SwitchQna,
    Add1m,
    Sub1m,
    Add10s,
    Sub10s,
    PresetPrev,
    PresetNext,
    ModeCircle,
    ModeBar,
    ModeNumber,
    Fullscreen,
    OpenScreen,
    CloseScreens,
}

impl Action {
    /// In the order the shortcut list shows them.
    pub const ALL: [Action; 15] = [
        Action::ToggleTimer,
        Action::Reset,
        Action::SwitchQna,
        Action::Add1m,
        Action::Sub1m,
        Action::Add10s,
        Action::Sub10s,
        Action::PresetPrev,
        Action::PresetNext,
        Action::ModeCircle,
        Action::ModeBar,
        Action::ModeNumber,
        Action::Fullscreen,
        Action::OpenScreen,
        Action::CloseScreens,
    ];

    /// The name an action is saved under in settings.json.
    pub fn id(self) -> &'static str {
        match self {
            Action::ToggleTimer => "toggle",
            Action::Reset => "reset",
            Action::SwitchQna => "qna",
            Action::Add1m => "add_1m",
            Action::Sub1m => "sub_1m",
            Action::Add10s => "add_10s",
            Action::Sub10s => "sub_10s",
            Action::PresetPrev => "preset_prev",
            Action::PresetNext => "preset_next",
            Action::ModeCircle => "mode_circle",
            Action::ModeBar => "mode_bar",
            Action::ModeNumber => "mode_number",
            Action::Fullscreen => "fullscreen",
            Action::OpenScreen => "open_screen",
            Action::CloseScreens => "close_screens",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::ToggleTimer => t("시작 / 일시정지"),
            Action::Reset => t("초기화"),
            Action::SwitchQna => t("Q&A 전환"),
            Action::Add1m => t("+1분"),
            Action::Sub1m => t("−1분"),
            Action::Add10s => t("+10초"),
            Action::Sub10s => t("−10초"),
            Action::PresetPrev => t("이전 시간 프리셋"),
            Action::PresetNext => t("다음 시간 프리셋"),
            Action::ModeCircle => t("표시: 원그래프"),
            Action::ModeBar => t("표시: 막대그래프"),
            Action::ModeNumber => t("표시: 숫자"),
            Action::Fullscreen => t("전체 화면"),
            Action::OpenScreen => t("새 출력 화면 열기"),
            Action::CloseScreens => t("출력 화면 모두 닫기"),
        }
    }

    pub fn default_binding(self) -> Option<Binding> {
        let key = match self {
            Action::ToggleTimer => Key::Space,
            Action::Reset => Key::R,
            Action::SwitchQna => Key::Q,
            Action::Add1m => Key::ArrowUp,
            Action::Sub1m => Key::ArrowDown,
            Action::Add10s => Key::ArrowRight,
            Action::Sub10s => Key::ArrowLeft,
            Action::PresetPrev => Key::OpenBracket,
            Action::PresetNext => Key::CloseBracket,
            Action::ModeCircle => Key::Num1,
            Action::ModeBar => Key::Num2,
            Action::ModeNumber => Key::Num3,
            Action::Fullscreen => Key::F11,
            Action::OpenScreen => Key::N,
            Action::CloseScreens => return None,
        };
        Some(Binding::new(key))
    }

    /// Holding the key goes on doing it: only the time adjustments (the rest act once per press).
    pub fn repeats(self) -> bool {
        matches!(self, Action::Add1m | Action::Sub1m | Action::Add10s | Action::Sub10s)
    }
}

/// A key with the modifier keys that have to be held with it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub(super) struct Binding {
    pub key: Key,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// The Command key of a Mac (Windows has none).
    pub cmd: bool,
}

impl Binding {
    pub fn new(key: Key) -> Self {
        Self { key, ctrl: false, alt: false, shift: false, cmd: false }
    }

    pub fn from_event(modifiers: Modifiers, key: Key) -> Self {
        Self { key, ctrl: modifiers.ctrl, alt: modifiers.alt, shift: modifiers.shift, cmd: modifiers.mac_cmd }
    }

    /// As it is saved: `Ctrl+Shift+T`, `Space`, `Alt+ArrowUp`.
    pub fn text(&self) -> String {
        self.spelled(Key::name)
    }

    /// As it is shown: the same, with arrows and punctuation as their marks.
    pub fn label(&self) -> String {
        self.spelled(key_mark)
    }

    fn spelled(&self, key: impl Fn(Key) -> &'static str) -> String {
        let mut parts = Vec::new();
        for (held, name) in [(self.ctrl, "Ctrl"), (self.alt, "Alt"), (self.shift, "Shift"), (self.cmd, "Cmd")] {
            if held {
                parts.push(name);
            }
        }
        parts.push(key(self.key));
        parts.join("+")
    }

    /// The reverse of `text`; `None` for anything that is not a key with modifiers.
    pub fn parse(text: &str) -> Option<Binding> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let key = parts.pop().filter(|k| !k.is_empty()).and_then(Key::from_name)?;
        let mut b = Binding::new(key);
        for part in parts {
            let flag = match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => &mut b.ctrl,
                "alt" | "option" => &mut b.alt,
                "shift" => &mut b.shift,
                "cmd" | "command" | "meta" => &mut b.cmd,
                _ => return None,
            };
            if std::mem::replace(flag, true) {
                return None;
            }
        }
        Some(b)
    }

    /// Whether the key can be saved and read back as the same key.
    pub fn storable(&self) -> bool {
        Key::from_name(self.key.name()) == Some(self.key)
    }

    /// A function key types nothing, so it also works while a text box has the keyboard.
    pub fn fires_while_typing(&self) -> bool {
        let name = self.key.name();
        name.strip_prefix('F').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    }
}

/// How a key is written in the list: arrows and punctuation as marks, the rest by its name.
fn key_mark(key: Key) -> &'static str {
    match key {
        Key::ArrowUp => "↑",
        Key::ArrowDown => "↓",
        Key::ArrowLeft => "←",
        Key::ArrowRight => "→",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Plus => "+",
        Key::Minus => "-",
        Key::Equals => "=",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Slash => "/",
        Key::Backslash => "\\",
        Key::Semicolon => ";",
        Key::Colon => ":",
        Key::Quote => "'",
        Key::Backtick => "`",
        Key::Pipe => "|",
        Key::Questionmark => "?",
        Key::Exclamationmark => "!",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        other => other.name(),
    }
}

/// Why a key cannot be given to an action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Reserved {
    /// The system or the window already uses it (Alt+F4, Alt+Tab, Cmd+Q…).
    System,
    /// The program itself uses it and it cannot move (Esc, Tab, F2).
    Fixed,
    /// Text editing uses it (copy, paste, undo…).
    Editing,
}

impl Reserved {
    pub fn message(self) -> &'static str {
        match self {
            Reserved::System => t("운영체제나 창 관리에서 쓰는 키라서 지정할 수 없습니다"),
            Reserved::Fixed => t("프로그램에서 고정으로 쓰는 키입니다 (Esc 돌아가기 · Tab 이동 · F2 편집 화면)"),
            Reserved::Editing => t("복사 · 붙여넣기 · 실행 취소에 쓰는 조합이라 지정할 수 없습니다"),
        }
    }
}

/// `Some` for a key that must not be given to an action.
pub(super) fn reserved(b: &Binding) -> Option<Reserved> {
    let command = b.ctrl || b.cmd;
    match b.key {
        Key::Escape | Key::F2 => return Some(Reserved::Fixed),
        Key::Tab if !b.ctrl && !b.alt && !b.cmd => return Some(Reserved::Fixed),
        Key::Tab | Key::F4 if b.alt || command => return Some(Reserved::System),
        Key::Delete if b.ctrl && b.alt => return Some(Reserved::System),
        Key::Q | Key::W | Key::H | Key::M | Key::Space if b.cmd => return Some(Reserved::System),
        Key::A | Key::C | Key::V | Key::X | Key::Y | Key::Z if command && !b.alt => return Some(Reserved::Editing),
        _ => {}
    }
    None
}

/// What happened to a key a person gave to an action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Assign {
    Set,
    /// Another action had the key: it has none now.
    Replaced(Action),
    Reserved(Reserved),
}

/// The shortcut settings: on or off as a whole, and the keys that differ from the defaults.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(default)]
pub(super) struct Shortcuts {
    pub enabled: bool,
    /// Action id → key as `Binding::text`; an empty text is an action with no key.
    pub custom: BTreeMap<String, String>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self { enabled: true, custom: BTreeMap::new() }
    }
}

impl Shortcuts {
    /// The key of an action: the one the person chose, or the default; `None` when taken away.
    pub fn binding(&self, a: Action) -> Option<Binding> {
        match self.custom.get(a.id()) {
            Some(text) => Binding::parse(text),
            None => a.default_binding(),
        }
    }

    pub fn action_for(&self, b: &Binding) -> Option<Action> {
        Action::ALL.into_iter().find(|a| self.binding(*a).as_ref() == Some(b))
    }

    fn put(&mut self, a: Action, b: Option<Binding>) {
        if b == a.default_binding() {
            self.custom.remove(a.id());
        } else {
            self.custom.insert(a.id().to_string(), b.map(|b| b.text()).unwrap_or_default());
        }
    }

    /// Gives `b` to `a`, taking it from whichever other action had it.
    pub fn assign(&mut self, a: Action, b: Binding) -> Assign {
        if let Some(why) = reserved(&b) {
            return Assign::Reserved(why);
        }
        let taken = self.action_for(&b).filter(|other| *other != a);
        if let Some(other) = taken {
            self.put(other, None);
        }
        self.put(a, Some(b));
        taken.map_or(Assign::Set, Assign::Replaced)
    }

    /// Takes the key of `a` away.
    pub fn clear(&mut self, a: Action) {
        self.put(a, None);
    }

    pub fn reset(&mut self) {
        self.custom.clear();
    }

    pub fn is_default(&self) -> bool {
        self.custom.is_empty()
    }

    /// The key as shown in a hint (empty when the action has none or shortcuts are off).
    pub fn hint(&self, a: Action) -> String {
        if self.enabled {
            self.binding(a).map(|b| b.label()).unwrap_or_default()
        } else {
            String::new()
        }
    }

    /// Settings read from a file are brought into order: actions this build does not know, keys
    /// that cannot be read or may not be given, and entries equal to the default are dropped.
    /// A key that two actions share stays with the one the person gave it to (the first of them,
    /// if both were given it), not with the action that merely has it as its default.
    pub fn normalize(&mut self) {
        self.custom.retain(|id, text| {
            Action::from_id(id).is_some_and(|a| match Binding::parse(text) {
                Some(b) => b.storable() && reserved(&b).is_none() && Some(b) != a.default_binding(),
                None => text.is_empty() && a.default_binding().is_some(),
            })
        });
        let mut seen: Vec<Binding> = Vec::new();
        for chosen in [true, false] {
            let group: Vec<Action> = Action::ALL.into_iter().filter(|a| self.custom.contains_key(a.id()) == chosen).collect();
            for a in group {
                match self.binding(a) {
                    Some(b) if seen.contains(&b) => self.put(a, None),
                    Some(b) => seen.push(b),
                    None => {}
                }
            }
        }
    }
}

/// The actions the key events ask for, taken out of `events` (the rest are left for the widgets).
/// `typing`: a text box has the keyboard, so only keys that type nothing count.
pub(super) fn take(events: &mut Vec<Event>, map: &Shortcuts, typing: bool) -> Vec<Action> {
    let mut found = Vec::new();
    events.retain(|e| {
        let Event::Key { key, pressed: true, repeat, modifiers, .. } = e else { return true };
        let b = Binding::from_event(*modifiers, *key);
        let Some(a) = map.action_for(&b) else { return true };
        if typing && !b.fires_while_typing() {
            return true;
        }
        if !*repeat || a.repeats() {
            found.push(a);
        }
        false
    });
    found
}

/// What the key events say while a person is recording a shortcut.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Recorded {
    /// Nothing was pressed yet.
    Waiting,
    /// Esc on its own: leave the shortcut as it was.
    Cancelled,
    Key(Binding),
}

pub(super) fn record(events: &[Event]) -> Recorded {
    for e in events {
        if let Event::Key { key, pressed: true, modifiers, .. } = e {
            let b = Binding::from_event(*modifiers, *key);
            return if b == Binding::new(Key::Escape) { Recorded::Cancelled } else { Recorded::Key(b) };
        }
    }
    Recorded::Waiting
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }
    fn held(ctrl: bool, alt: bool, shift: bool) -> Modifiers {
        Modifiers { ctrl, alt, shift, command: ctrl, ..Modifiers::NONE }
    }

    #[test]
    fn every_action_has_its_own_name_and_default_key() {
        let mut ids = std::collections::HashSet::new();
        let mut keys = Vec::new();
        for a in Action::ALL {
            assert!(ids.insert(a.id()), "{a:?}");
            assert_eq!(Action::from_id(a.id()), Some(a));
            if let Some(b) = a.default_binding() {
                assert!(!keys.contains(&b), "{a:?} shares its default key");
                assert!(reserved(&b).is_none() && b.storable(), "{a:?}");
                keys.push(b);
            }
        }
        assert_eq!(Action::from_id("nothing"), None);
        // The keys the program has always had.
        let s = Shortcuts::default();
        for (a, key) in [(Action::ToggleTimer, Key::Space), (Action::Reset, Key::R), (Action::SwitchQna, Key::Q), (Action::Fullscreen, Key::F11)] {
            assert_eq!(s.binding(a), Some(Binding::new(key)));
        }
        assert_eq!(s.binding(Action::ModeNumber), Some(Binding::new(Key::Num3)));
        assert_eq!(s.binding(Action::CloseScreens), None);
    }

    #[test]
    fn a_key_is_written_and_read_back() {
        let b = Binding { key: Key::T, ctrl: true, alt: false, shift: true, cmd: false };
        assert_eq!(b.text(), "Ctrl+Shift+T");
        assert_eq!(Binding::parse("Ctrl+Shift+T"), Some(b));
        assert_eq!(Binding::parse(" shift + ctrl + t "), Some(b));
        assert_eq!(Binding::parse("Space"), Some(Binding::new(Key::Space)));
        assert_eq!(Binding::new(Key::ArrowUp).label(), "↑");
        assert_eq!(Binding { alt: true, ..Binding::new(Key::Num1) }.label(), "Alt+1");
        for bad in ["", "+", "Ctrl+", "Ctrl+Ctrl+T", "Hyper+T", "Ctrl+NoSuchKey", "T+Ctrl"] {
            assert_eq!(Binding::parse(bad), None, "{bad:?}");
        }
        // Whatever can be recorded and is storable comes back the same.
        for key in Key::ALL {
            let b = Binding { key: *key, ctrl: true, alt: true, shift: true, cmd: true };
            if b.storable() {
                assert_eq!(Binding::parse(&b.text()), Some(b), "{key:?}");
            }
        }
        for key in [Key::Space, Key::R, Key::Num1, Key::F11, Key::ArrowLeft, Key::OpenBracket, Key::Plus, Key::Minus] {
            assert!(Binding::new(key).storable(), "{key:?}");
        }
    }

    #[test]
    fn only_the_function_keys_work_while_typing() {
        for (key, yes) in [(Key::F1, true), (Key::F11, true), (Key::F12, true), (Key::Space, false), (Key::R, false), (Key::Num1, false)] {
            assert_eq!(Binding::new(key).fires_while_typing(), yes, "{key:?}");
        }
        assert!(Binding { ctrl: true, ..Binding::new(Key::F5) }.fires_while_typing());
        assert!(!Binding { ctrl: true, ..Binding::new(Key::T) }.fires_while_typing());
    }

    #[test]
    fn keys_the_system_or_text_editing_use_cannot_be_given() {
        let with = |key, ctrl, alt, cmd| Binding { key, ctrl, alt, shift: false, cmd };
        assert_eq!(reserved(&Binding::new(Key::Escape)), Some(Reserved::Fixed));
        assert_eq!(reserved(&Binding::new(Key::F2)), Some(Reserved::Fixed));
        assert_eq!(reserved(&Binding::new(Key::Tab)), Some(Reserved::Fixed));
        assert_eq!(reserved(&with(Key::F4, false, true, false)), Some(Reserved::System));
        assert_eq!(reserved(&with(Key::Tab, false, true, false)), Some(Reserved::System));
        assert_eq!(reserved(&with(Key::Tab, true, false, false)), Some(Reserved::System));
        assert_eq!(reserved(&with(Key::Delete, true, true, false)), Some(Reserved::System));
        assert_eq!(reserved(&with(Key::Q, false, false, true)), Some(Reserved::System));
        assert_eq!(reserved(&with(Key::C, true, false, false)), Some(Reserved::Editing));
        assert_eq!(reserved(&with(Key::Z, false, false, true)), Some(Reserved::Editing));
        // Everything else may be given, plain letters and Ctrl+Shift+T included.
        for b in [
            Binding::new(Key::R),
            Binding::new(Key::Space),
            Binding::new(Key::F11),
            Binding { ctrl: true, shift: true, ..Binding::new(Key::T) },
            with(Key::Space, true, true, false),
            with(Key::F4, false, false, false),
        ] {
            assert_eq!(reserved(&b), None, "{b:?}");
        }
    }

    #[test]
    fn giving_a_key_takes_it_from_its_owner() {
        let mut s = Shortcuts::default();
        assert_eq!(s.assign(Action::Reset, Binding { ctrl: true, shift: true, ..Binding::new(Key::T) }), Assign::Set);
        assert_eq!(s.binding(Action::Reset).unwrap().text(), "Ctrl+Shift+T");
        assert_eq!(s.action_for(&Binding::new(Key::R)), None, "the old key is free");
        // Space belongs to the start / pause toggle: Reset takes it, the toggle is left without.
        assert_eq!(s.assign(Action::Reset, Binding::new(Key::Space)), Assign::Replaced(Action::ToggleTimer));
        assert_eq!(s.binding(Action::ToggleTimer), None);
        assert_eq!(s.action_for(&Binding::new(Key::Space)), Some(Action::Reset));
        // Giving an action the key it already has changes nothing.
        let before = s.clone();
        assert_eq!(s.assign(Action::Reset, Binding::new(Key::Space)), Assign::Set);
        assert_eq!(s, before);
        // A key that may not be given leaves everything as it was.
        assert_eq!(s.assign(Action::Add1m, Binding::new(Key::Escape)), Assign::Reserved(Reserved::Fixed));
        assert_eq!(s, before);
        // No key is ever on two actions.
        let mut seen = Vec::new();
        for a in Action::ALL {
            if let Some(b) = s.binding(a) {
                assert!(!seen.contains(&b));
                seen.push(b);
            }
        }
    }

    #[test]
    fn a_key_can_be_taken_away_and_the_defaults_come_back() {
        let mut s = Shortcuts::default();
        assert!(s.is_default());
        s.clear(Action::Reset);
        assert_eq!(s.binding(Action::Reset), None);
        assert_eq!(s.custom.get("reset").map(String::as_str), Some(""));
        assert_eq!(s.hint(Action::Reset), "");
        assert_eq!(s.hint(Action::ToggleTimer), "Space");
        s.assign(Action::Reset, Binding::new(Key::R));
        assert!(s.is_default(), "the default key again is no difference");
        s.assign(Action::Fullscreen, Binding::new(Key::F));
        s.clear(Action::Add1m);
        s.enabled = false;
        assert_eq!(s.hint(Action::Fullscreen), "", "no hints while the shortcuts are off");
        s.reset();
        assert!(s.is_default() && !s.enabled, "only the keys go back, not the on / off choice");
        assert_eq!(s.binding(Action::Fullscreen), Some(Binding::new(Key::F11)));
        // An action without a default key: taking its key away is no difference either.
        s.clear(Action::CloseScreens);
        assert!(s.is_default());
    }

    #[test]
    fn saved_shortcuts_are_read_and_put_in_order() {
        let mut s: Shortcuts = serde_json::from_str(
            r#"{"enabled":false,"custom":{"reset":"Ctrl+Shift+T","nothing":"X","qna":"Ctrl+C","add_1m":"???","sub_1m":"","toggle":"Space","mode_bar":"Escape","open_screen":"Ctrl+Shift+T","preset_next":""}}"#,
        )
        .unwrap();
        s.normalize();
        assert!(!s.enabled);
        // Kept: a real choice, an action with its key taken away; the later owner of a shared key loses it.
        assert_eq!(s.binding(Action::Reset).unwrap().text(), "Ctrl+Shift+T");
        assert_eq!(s.binding(Action::OpenScreen), None, "Reset has the key first");
        assert_eq!(s.binding(Action::Sub1m), None);
        assert_eq!(s.binding(Action::PresetNext), None);
        // Dropped: unknown actions, keys that may not be given or cannot be read, the default written out.
        assert_eq!(s.binding(Action::SwitchQna), Some(Binding::new(Key::Q)));
        assert_eq!(s.binding(Action::Add1m), Some(Binding::new(Key::ArrowUp)));
        assert_eq!(s.binding(Action::ModeBar), Some(Binding::new(Key::Num2)));
        assert!(!s.custom.contains_key("nothing") && !s.custom.contains_key("toggle"));
        // A settings file without any of it starts with the defaults, switched on.
        let none: Shortcuts = serde_json::from_str("{}").unwrap();
        assert!(none.enabled && none.is_default());
        // A saved key equal to another action's default: the default owner gives it up.
        let mut s: Shortcuts = serde_json::from_str(r#"{"custom":{"add_1m":"R"}}"#).unwrap();
        s.normalize();
        assert_eq!(s.binding(Action::Add1m), Some(Binding::new(Key::R)));
        assert_eq!(s.binding(Action::Reset), None);
    }

    #[test]
    fn a_pressed_key_becomes_its_action_and_is_taken_out_of_the_events() {
        let s = Shortcuts::default();
        let mut events = vec![
            press(Key::Space, Modifiers::NONE),
            Event::Text(" ".into()),
            press(Key::Tab, Modifiers::NONE),
            press(Key::R, held(true, false, false)),
        ];
        assert_eq!(take(&mut events, &s, false), [Action::ToggleTimer]);
        // The Tab and the Ctrl+R are not shortcuts and stay for the widgets; so does the text.
        assert_eq!(events.len(), 3);
        // Several in one frame come out in the order they were pressed.
        let mut events = vec![press(Key::Num2, Modifiers::NONE), press(Key::R, Modifiers::NONE), press(Key::Num1, Modifiers::NONE)];
        assert_eq!(take(&mut events, &s, false), [Action::ModeBar, Action::Reset, Action::ModeCircle]);
        assert!(events.is_empty());
        // Releasing a key does nothing.
        let mut events = vec![Event::Key { key: Key::Space, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }];
        assert!(take(&mut events, &s, false).is_empty());
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn modifiers_have_to_match_exactly() {
        let mut s = Shortcuts::default();
        s.assign(Action::OpenScreen, Binding { ctrl: true, shift: true, ..Binding::new(Key::T) });
        for (mods, expected) in [
            (held(true, false, true), vec![Action::OpenScreen]),
            (held(true, false, false), vec![]),
            (held(false, false, true), vec![]),
            (held(true, true, true), vec![]),
            (Modifiers::NONE, vec![]),
        ] {
            assert_eq!(take(&mut vec![press(Key::T, mods)], &s, false), expected, "{mods:?}");
        }
        // A plain letter is not triggered by the same letter with Ctrl, Alt or Shift held.
        for mods in [held(true, false, false), held(false, true, false), held(false, false, true)] {
            assert!(take(&mut vec![press(Key::R, mods)], &s, false).is_empty(), "{mods:?}");
        }
        let cmd = Modifiers { mac_cmd: true, ..Modifiers::NONE };
        assert!(take(&mut vec![press(Key::R, cmd)], &s, false).is_empty());
        s.assign(Action::Fullscreen, Binding { cmd: true, ..Binding::new(Key::F) });
        assert_eq!(take(&mut vec![press(Key::F, cmd)], &s, false), [Action::Fullscreen]);
    }

    #[test]
    fn typing_in_a_text_box_does_not_fire_shortcuts() {
        let s = Shortcuts::default();
        for key in [Key::Space, Key::R, Key::Q, Key::Num1, Key::ArrowUp, Key::N] {
            let mut events = vec![press(key, Modifiers::NONE)];
            assert!(take(&mut events, &s, true).is_empty(), "{key:?}");
            assert_eq!(events.len(), 1, "{key:?} stays for the text box");
        }
        // F11 types nothing: full screen still works.
        assert_eq!(take(&mut vec![press(Key::F11, Modifiers::NONE)], &s, true), [Action::Fullscreen]);
    }

    #[test]
    fn holding_a_key_repeats_only_the_time_adjustments() {
        let s = Shortcuts::default();
        let again = |key| Event::Key { key, physical_key: None, pressed: true, repeat: true, modifiers: Modifiers::NONE };
        let mut events = vec![again(Key::ArrowUp), again(Key::Space), again(Key::R), again(Key::ArrowLeft)];
        assert_eq!(take(&mut events, &s, false), [Action::Add1m, Action::Sub10s]);
        assert!(events.is_empty(), "a repeated key press is still taken, so that no widget acts on it");
    }

    #[test]
    fn recording_takes_the_first_key_and_esc_cancels() {
        assert_eq!(record(&[]), Recorded::Waiting);
        assert_eq!(record(&[Event::Text("a".into())]), Recorded::Waiting);
        assert_eq!(
            record(&[press(Key::T, held(true, false, true)), press(Key::R, Modifiers::NONE)]),
            Recorded::Key(Binding { ctrl: true, shift: true, ..Binding::new(Key::T) })
        );
        assert_eq!(record(&[press(Key::Escape, Modifiers::NONE)]), Recorded::Cancelled);
        // Esc with a modifier is a key like any other (it is then refused as a fixed one).
        assert!(matches!(record(&[press(Key::Escape, held(true, false, false))]), Recorded::Key(_)));
        assert_eq!(
            record(&[Event::Key { key: Key::R, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }]),
            Recorded::Waiting
        );
    }

    #[test]
    fn a_taken_away_key_does_nothing_and_shortcuts_can_be_switched_off_as_a_whole() {
        let mut s = Shortcuts::default();
        s.clear(Action::Reset);
        assert!(take(&mut vec![press(Key::R, Modifiers::NONE)], &s, false).is_empty());
        // (The window skips `take` while `enabled` is off; the list itself stays as it is.)
        s.enabled = false;
        assert_eq!(s.binding(Action::ToggleTimer), Some(Binding::new(Key::Space)));
    }
}
