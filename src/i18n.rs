//! The app's languages. English text is the key; locales/{de,fr,es,pt,zh}.json
//! map it to the translation, in the words the game itself uses in that
//! language. Anything missing falls back to the English.

#![allow(dead_code)] // parts of the API are for the settings page and main.rs

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Lang {
    #[default]
    En,
    De,
    Fr,
    Es,
    Pt,
    Zh,
}

impl Lang {
    pub const ALL: [Lang; 6] = [Lang::En, Lang::De, Lang::Fr, Lang::Es, Lang::Pt, Lang::Zh];

    /// ISO 639-1, which is also what ElevenLabs takes.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::Pt => "pt",
            Lang::Zh => "zh",
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::Es => "Español",
            Lang::Pt => "Português (Brasil)",
            Lang::Zh => "简体中文",
        }
    }

    /// For prompts ("Write in German.").
    pub fn english_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::De => "German",
            Lang::Fr => "French",
            Lang::Es => "Spanish",
            Lang::Pt => "Brazilian Portuguese",
            Lang::Zh => "Simplified Chinese",
        }
    }

    /// "de", "de-DE", "de_DE.UTF-8", "zh-Hans-CN", "pt-BR", "pt_PT"...
    pub fn from_code(s: &str) -> Option<Lang> {
        let head = s
            .trim()
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == head)
    }

    /// The game's textLocale: "deDE", "frFR", "esES", "esMX", "ptBR", "zhCN",
    /// "enUS", "enGB"; "zhTW" reads as Chinese too.
    pub fn from_game_locale(s: &str) -> Option<Lang> {
        let s = s.trim();
        if s.len() < 2 || !s.is_char_boundary(2) {
            return None;
        }
        Lang::from_code(&s[..2])
    }

    /// FM_LANG if set, else the first system language we have, else English.
    pub fn detect_os() -> Lang {
        if let Some(l) = env_override() {
            return l;
        }
        sys_locale::get_locales()
            .find_map(|l| Lang::from_code(&l))
            .unwrap_or_default()
    }

    fn index(self) -> u8 {
        Lang::ALL.iter().position(|l| *l == self).unwrap_or(0) as u8
    }
}

/// FM_LANG=de (or de_DE, zh-CN...) picks the language, for testing.
pub fn env_override() -> Option<Lang> {
    std::env::var("FM_LANG")
        .ok()
        .and_then(|v| Lang::from_code(&v))
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    CURRENT.store(lang.index(), Ordering::Relaxed);
}

pub fn current() -> Lang {
    Lang::ALL[(CURRENT.load(Ordering::Relaxed) as usize).min(Lang::ALL.len() - 1)]
}

type Table = HashMap<&'static str, &'static str>;

fn source(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "{}",
        Lang::De => include_str!("../locales/de.json"),
        Lang::Fr => include_str!("../locales/fr.json"),
        Lang::Es => include_str!("../locales/es.json"),
        Lang::Pt => include_str!("../locales/pt.json"),
        Lang::Zh => include_str!("../locales/zh.json"),
    }
}

fn parse(json: &str) -> Table {
    let map: HashMap<String, String> = serde_json::from_str(json).unwrap_or_default();
    map.into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| {
            let k: &'static str = Box::leak(k.into_boxed_str());
            let v: &'static str = Box::leak(v.into_boxed_str());
            (k, v)
        })
        .collect()
}

fn table(lang: Lang) -> &'static Table {
    static TABLES: OnceLock<Vec<Table>> = OnceLock::new();
    &TABLES.get_or_init(|| Lang::ALL.iter().map(|l| parse(source(*l))).collect())
        [lang.index() as usize]
}

/// The current language's text for this English text (itself if missing).
pub fn t(en: &'static str) -> &'static str {
    let lang = current();
    if lang == Lang::En {
        return en;
    }
    table(lang).get(en).copied().unwrap_or(en)
}

/// `t`, then `{name}` placeholders filled from `args`.
pub fn tf(en: &'static str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    fill(t(en), args)
}

fn fill(text: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let arg = after.find('}').and_then(|close| {
            let name = &after[..close];
            args.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (close, *v))
        });
        match arg {
            Some((close, v)) => {
                let _ = write!(out, "{v}");
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `tr!("Players")` is `t("Players")`; `tr!("{n} days seen", n = x)` fills
/// the named placeholders.
#[macro_export]
macro_rules! tr {
    ($en:literal $(,)?) => {
        $crate::i18n::t($en)
    };
    ($en:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::tf(
            $en,
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(s: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut rest = s;
        while let Some(open) = rest.find('{') {
            let after = &rest[open + 1..];
            match after.find('}') {
                Some(close)
                    if close > 0
                        && after[..close]
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
                {
                    out.insert(after[..close].to_string());
                    rest = &after[close + 1..];
                }
                _ => rest = after,
            }
        }
        out
    }

    fn raw(lang: Lang) -> HashMap<String, String> {
        serde_json::from_str(source(lang))
            .unwrap_or_else(|e| panic!("locales/{}.json: {e}", lang.code()))
    }

    #[test]
    fn locales_share_keys_and_placeholders() {
        let de = raw(Lang::De);
        let keys: BTreeSet<&String> = de.keys().collect();
        for lang in &Lang::ALL[1..] {
            let map = raw(*lang);
            let these: BTreeSet<&String> = map.keys().collect();
            let missing: Vec<_> = keys.difference(&these).collect();
            let extra: Vec<_> = these.difference(&keys).collect();
            assert!(
                missing.is_empty() && extra.is_empty(),
                "{}: missing {missing:?}, extra {extra:?}",
                lang.code()
            );
            for (en, tr) in &map {
                assert!(!tr.trim().is_empty(), "{}: empty for {en:?}", lang.code());
                assert_eq!(
                    placeholders(en),
                    placeholders(tr),
                    "{}: placeholders differ in {en:?} -> {tr:?}",
                    lang.code()
                );
            }
        }
    }

    /// Every `tr!("...")` in the source has a translation.
    #[test]
    fn every_tr_literal_is_translated() {
        let keys = raw(Lang::De);
        let mut missing = BTreeSet::new();
        let mut dirs = vec![std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src"
        ))];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("i18n.rs") {
                    let src = std::fs::read_to_string(&p).unwrap();
                    for lit in tr_literals(&src) {
                        if !keys.contains_key(&lit) {
                            missing.insert(lit);
                        }
                    }
                }
            }
        }
        assert!(missing.is_empty(), "untranslated: {missing:#?}");
    }

    fn tr_literals(src: &str) -> Vec<String> {
        let mut out = vec![];
        let mut rest = src;
        while let Some(at) = rest.find("tr!(") {
            rest = &rest[at + 4..];
            let body = rest.trim_start();
            let Some(body) = body.strip_prefix('"') else {
                continue;
            };
            let mut s = String::new();
            let mut chars = body.chars();
            while let Some(c) = chars.next() {
                match c {
                    '"' => break,
                    '\\' => match chars.next() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some(x) => s.push(x),
                        None => {}
                    },
                    c => s.push(c),
                }
            }
            out.push(s);
        }
        out
    }

    #[test]
    fn codes() {
        assert_eq!(Lang::from_code("de_DE.UTF-8"), Some(Lang::De));
        assert_eq!(Lang::from_code("zh-Hans-CN"), Some(Lang::Zh));
        assert_eq!(Lang::from_code("pt_PT"), Some(Lang::Pt));
        assert_eq!(Lang::from_code("pt-BR"), Some(Lang::Pt));
        assert_eq!(Lang::from_code("it"), None);
        assert_eq!(Lang::from_game_locale("esMX"), Some(Lang::Es));
        assert_eq!(Lang::from_game_locale("zhTW"), Some(Lang::Zh));
        assert_eq!(Lang::from_game_locale("enGB"), Some(Lang::En));
        assert_eq!(Lang::from_game_locale("koKR"), None);
        for l in Lang::ALL {
            assert_eq!(Lang::from_code(l.code()), Some(l));
        }
    }

    #[test]
    fn filling() {
        let n = 3;
        assert_eq!(
            fill("{name} has died {n} times.", &[("name", &"Ada"), ("n", &n)]),
            "Ada has died 3 times."
        );
        assert_eq!(fill("{a} {missing} {", &[("a", &1)]), "1 {missing} {");
    }
}
