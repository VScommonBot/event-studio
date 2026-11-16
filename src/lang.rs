//! The interface's languages: Korean (as the program was first written), English, Chinese
//! (Simplified) and Japanese.
//!
//! Texts stay written in Korean where they are used; `t("…")` gives the same text in the
//! language that is switched on, from the one table in strings.rs, and shows the Korean when a
//! translation is missing. VS Event Timer 1.1.0, this program's Electron predecessor, worked
//! the same way: a `language` setting ('ko' by default, 'ko' or 'en') and one table per
//! language, with a lookup that also falls back to the Korean table; its English wording is
//! kept where the same text exists there.
//!
//! Which language a first start uses follows Windows, the way programs with a table of
//! languages commonly pick theirs: the Windows display language (GetUserDefaultUILanguage),
//! first the table written for exactly that language, then the first one for the same primary
//! language, and English when there is neither.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    Ko,
    En,
    Zh,
    Ja,
}

/// The primary language is the low 10 bits of a Windows language id.
const PRIMARY_LANGUAGE: u16 = 0x3ff;
/// The Windows language each table was written for (ko-KR, en-US, zh-CN, ja-JP), in the order
/// they are looked through.
const WINDOWS_IDS: [(u16, Lang); 4] = [(0x0412, Lang::Ko), (0x0409, Lang::En), (0x0804, Lang::Zh), (0x0411, Lang::Ja)];

/// A key can name two different things ("제거": take an image out, or uninstall). Then the
/// second one is written `"제거\u{1}uninstall"`: what follows the mark only tells them apart
/// and is never shown.
pub const CONTEXT: char = '\u{1}';

impl Lang {
    pub const ALL: [Lang; 4] = [Lang::Ko, Lang::En, Lang::Zh, Lang::Ja];

    /// The language's own name for itself, readable whatever language is switched on.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Ko => "한국어",
            Lang::En => "English",
            Lang::Zh => "中文",
            Lang::Ja => "日本語",
        }
    }

    /// As saved in settings.json and given to `--lang` (1.1.0 had 'ko' and 'en').
    pub fn code(self) -> &'static str {
        match self {
            Lang::Ko => "ko",
            Lang::En => "en",
            Lang::Zh => "zh",
            Lang::Ja => "ja",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code().eq_ignore_ascii_case(code.trim()))
    }

    /// The language for a Windows language id: the table written for exactly that id, else the
    /// first one for the same primary language (zh-TW and zh-HK get the Chinese table, en-GB
    /// the English one), else English.
    pub fn from_windows(id: u16) -> Lang {
        let exact = WINDOWS_IDS.iter().find(|(known, _)| *known == id);
        let primary = || WINDOWS_IDS.iter().find(|(known, _)| known & PRIMARY_LANGUAGE == id & PRIMARY_LANGUAGE);
        exact.or_else(primary).map_or(Lang::En, |(_, lang)| *lang)
    }

    /// The language Windows itself is shown in.
    pub fn system() -> Lang {
        crate::win::ui_language().map_or(Lang::En, Lang::from_windows)
    }

    /// Where this language's text is in a row of the table.
    fn column(self) -> usize {
        self as usize
    }
}

impl Serialize for Lang {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for Lang {
    /// An unknown code opens in Korean instead of spoiling the whole settings file (1.1.0
    /// replaced a value outside its list with the default the same way).
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Lang::from_code(&String::deserialize(d)?).unwrap_or_default())
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Switches the language every `t` answers in.
pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    Lang::ALL[CURRENT.load(Ordering::Relaxed) as usize % Lang::ALL.len()]
}

fn rows() -> &'static HashMap<&'static str, &'static [&'static str; 4]> {
    static ROWS: OnceLock<HashMap<&'static str, &'static [&'static str; 4]>> = OnceLock::new();
    ROWS.get_or_init(|| crate::strings::TABLE.iter().map(|row| (row[0], row)).collect())
}

/// `key` (the Korean text) in `lang`.
pub fn tr(lang: Lang, key: &'static str) -> &'static str {
    let korean = key.split(CONTEXT).next().unwrap_or(key);
    if lang == Lang::Ko {
        return korean;
    }
    match rows().get(key).map(|row| row[lang.column()]) {
        Some(text) if !text.is_empty() => text,
        _ => korean,
    }
}

/// `key` (the Korean text) in the language that is switched on.
pub fn t(key: &'static str) -> &'static str {
    tr(current(), key)
}

/// Puts `values` into a text's numbered places: `{0}`, `{1}`… (numbered, because the order of
/// the parts differs between languages).
pub fn fill(text: &str, values: &[&dyn Display]) -> String {
    let mut out = text.to_string();
    for (i, value) in values.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), &value.to_string());
    }
    out
}

/// `tf!("저장 실패: {0}", e)`: the text in the language that is switched on, with its places filled.
macro_rules! tf {
    ($key:literal $(, $value:expr)* $(,)?) => {
        $crate::lang::fill($crate::lang::t($key), &[$(&$value as &dyn std::fmt::Display),*])
    };
}
pub(crate) use tf;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strings::TABLE;
    use std::collections::HashSet;

    fn has_hangul(s: &str) -> bool {
        s.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
    }

    fn places(s: &str) -> Vec<usize> {
        (0..10).filter(|i| s.contains(&format!("{{{i}}}"))).collect()
    }

    #[test]
    fn windows_language_ids_map_to_the_four_languages() {
        // The four the tables were written for (ko-KR, en-US, zh-CN, ja-JP); then the same primary
        // language under another id (en-GB, zh-TW, zh-HK, zh-SG); then languages without a table
        // (de-DE, es-ES, fr-FR) and no id at all, which get English.
        let cases = [
            (0x0412, Lang::Ko), (0x0409, Lang::En), (0x0804, Lang::Zh), (0x0411, Lang::Ja),
            (0x0809, Lang::En), (0x0404, Lang::Zh), (0x0c04, Lang::Zh), (0x1004, Lang::Zh),
            (0x0407, Lang::En), (0x0c0a, Lang::En), (0x040c, Lang::En), (0x0000, Lang::En),
        ];
        for (id, lang) in cases {
            assert_eq!(Lang::from_windows(id), lang, "{id:#06x}");
        }
        for lang in Lang::ALL {
            assert_eq!(Lang::from_code(lang.code()), Some(lang));
            assert_eq!(serde_json::from_str::<Lang>(&serde_json::to_string(&lang).unwrap()).unwrap(), lang);
        }
        assert_eq!(serde_json::to_string(&Lang::Zh).unwrap(), "\"zh\"");
        assert_eq!(serde_json::from_str::<Lang>("\"fr\"").unwrap(), Lang::Ko, "an unknown code opens in Korean");
        assert_eq!(Lang::from_code(" EN "), Some(Lang::En));
    }

    #[test]
    fn every_text_has_all_four_languages_and_the_same_places() {
        let mut seen = HashSet::new();
        for row in TABLE {
            assert!(seen.insert(row[0]), "twice in the table: {}", row[0]);
            for (lang, text) in Lang::ALL.iter().zip(row.iter()) {
                assert!(!text.trim().is_empty(), "{lang:?} is empty for: {}", row[0]);
                assert_eq!(places(text), places(row[0]), "{lang:?} has other places than the Korean: {}", row[0]);
                assert_eq!(text.matches('\n').count(), row[0].matches('\n').count(), "{lang:?} has other line breaks: {}", row[0]);
                if *lang != Lang::Ko {
                    assert!(!has_hangul(text), "{lang:?} still has Korean in it: {text}");
                    assert!(!text.contains(CONTEXT));
                }
            }
        }
    }

    #[test]
    fn texts_come_out_in_the_asked_language() {
        assert_eq!(tr(Lang::Ko, "시작"), "시작");
        assert_eq!(tr(Lang::En, "시작"), "Start");
        assert_eq!(tr(Lang::Zh, "시작"), "开始");
        assert_eq!(tr(Lang::Ja, "시작"), "開始");
        // The same Korean word for two things.
        assert_eq!(tr(Lang::En, "제거"), "Remove");
        assert_eq!(tr(Lang::En, "제거\u{1}uninstall"), "Uninstall");
        assert_eq!(tr(Lang::Ko, "제거\u{1}uninstall"), "제거");
        // A text without a translation is shown in Korean.
        assert_eq!(tr(Lang::Ja, "표에 없는 글"), "표에 없는 글");
        assert_eq!(fill("{1} / {0} / {1}", &[&"a", &7]), "7 / a / 7");
        assert_eq!(fill(tr(Lang::En, "설정 {0}  ·  경과 {1}"), &[&"05:00", &"00:10"]), "Set 05:00  ·  elapsed 00:10");
    }

    /// The Korean string literals of a source file, outside comments and test code.
    fn literals(source: &str) -> Vec<(String, bool)> {
        let code = source.find("mod tests {").map_or(source, |at| &source[..at]);
        let chars: Vec<char> = code.chars().collect();
        let (mut found, mut i) = (Vec::new(), 0);
        while i < chars.len() {
            if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            } else if chars[i] == '\'' && chars.get(i + 2) == Some(&'\'') {
                i += 3;
            } else if chars[i] == '"' {
                let start = i;
                let mut text = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' {
                        i += 1;
                        match chars.get(i) {
                            Some('n') => text.push('\n'),
                            Some('r') => text.push('\r'),
                            Some('t') => text.push('\t'),
                            Some('u') => {
                                let end = (i..chars.len()).find(|&k| chars[k] == '}').unwrap();
                                let hex: String = chars[i + 2..end].iter().collect();
                                text.push(char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap());
                                i = end;
                            }
                            Some(c) => text.push(*c),
                            None => {}
                        }
                    } else {
                        text.push(chars[i]);
                    }
                    i += 1;
                }
                // Handed straight to `t(` or `tf!(`? (`get("…")` also ends in `t(`: the letter before rules it out.)
                let before: String = chars[start.saturating_sub(4)..start].iter().collect();
                let part_of_a_name = chars.get(start.wrapping_sub(3)).is_some_and(|c| c.is_alphanumeric() || *c == '_');
                found.push((text, before.ends_with("tf!(") || (before.ends_with("t(") && !part_of_a_name)));
                i += 1;
            } else {
                i += 1;
            }
        }
        found.into_iter().filter(|(text, through)| *through || has_hangul(text)).collect()
    }

    #[test]
    fn every_korean_text_in_the_program_is_in_the_table() {
        // Korean on purpose: file and shortcut names that Korean installs already have, and the guide's path.
        let kept = ["사용안내.txt", "../사용안내.txt", "사용안내.lnk", "Event Studio 제거.lnk", "한국어"];
        let keys: HashSet<&str> = TABLE.iter().map(|row| row[0]).collect();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut checked = 0;
        for name in ["studio.rs", "scene.rs", "overlay.rs", "controls.rs", "setup.rs", "setup_ui.rs", "pack.rs", "shell.rs", "win.rs", "lang.rs", "main.rs", "timer.rs"] {
            let source = std::fs::read_to_string(src.join(name)).unwrap();
            for (text, through) in literals(&source) {
                if kept.contains(&text.as_str()) && !through {
                    continue;
                }
                assert!(keys.contains(text.as_str()), "{name}: not in strings.rs: {text:?}");
                checked += 1;
            }
        }
        assert!(checked > 200, "only {checked} texts found: the scan is broken");
    }
}
