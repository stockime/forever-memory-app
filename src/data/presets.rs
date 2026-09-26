//! A starting personality for every race and class, for characters whose
//! "Who X is" note is still empty. The texts live in
//! locales/presets/<lang>.json, keyed "<raceFile> <classFile>", with
//! "* <classFile>" and "<raceFile> *" as fallbacks; English is the source.

use super::memory::Character;
use crate::i18n::Lang;
use std::collections::HashMap;
use std::sync::OnceLock;

// A macro, so the i18n test's tr!( scan doesn't read these paths as texts.
macro_rules! presets {
    ($lang:literal) => {
        include_str!(concat!("../../locales/presets/", $lang, ".json"))
    };
}

fn source(lang: Lang) -> &'static str {
    match lang {
        Lang::En => presets!("en"),
        Lang::De => presets!("de"),
        Lang::Fr => presets!("fr"),
        Lang::Es => presets!("es"),
        Lang::Pt => presets!("pt"),
        Lang::Zh => presets!("zh"),
    }
}

fn table(lang: Lang) -> &'static HashMap<String, String> {
    static TABLES: OnceLock<Vec<HashMap<String, String>>> = OnceLock::new();
    let i = Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0);
    &TABLES.get_or_init(|| {
        Lang::ALL
            .iter()
            .map(|l| serde_json::from_str(source(*l)).unwrap_or_default())
            .collect()
    })[i]
}

/// The preset for a race and class, most specific first; `{name}` is left in.
pub fn lookup(race_file: &str, class_file: &str, lang: Lang) -> Option<&'static str> {
    let keys = [
        format!("{race_file} {class_file}"),
        format!("* {class_file}"),
        format!("{race_file} *"),
    ];
    [lang, Lang::En].into_iter().find_map(|l| {
        keys.iter()
            .find_map(|k| table(l).get(k))
            .map(String::as_str)
    })
}

/// The character's preset in `lang`, with their first name filled in.
pub fn for_character(c: &Character, lang: Lang) -> Option<String> {
    let race = c
        .snapshot
        .get("raceFile")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let first = c.name.split(' ').next().unwrap_or(&c.name);
    lookup(race, &c.class_file.to_uppercase(), lang).map(|p| p.replace("{name}", first))
}

/// What the writers are given: the note, or else the English preset.
pub fn note_or_preset(c: &Character) -> Option<String> {
    let note = c.personality.trim();
    if note.is_empty() {
        for_character(c, Lang::En)
    } else {
        Some(note.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMBOS: [(&str, &[&str]); 8] = [
        ("Human", &["WARRIOR", "PALADIN", "HUNTER", "ROGUE", "PRIEST", "MAGE", "WARLOCK"]),
        ("Dwarf", &["WARRIOR", "PALADIN", "HUNTER", "ROGUE", "PRIEST", "SHAMAN"]),
        ("NightElf", &["WARRIOR", "HUNTER", "ROGUE", "PRIEST", "DRUID"]),
        ("Gnome", &["WARRIOR", "ROGUE", "PRIEST", "MAGE", "WARLOCK"]),
        ("Orc", &["WARRIOR", "HUNTER", "ROGUE", "SHAMAN", "MAGE", "WARLOCK"]),
        ("Scourge", &["WARRIOR", "PALADIN", "ROGUE", "PRIEST", "MAGE", "WARLOCK"]),
        ("Tauren", &["WARRIOR", "HUNTER", "SHAMAN", "DRUID"]),
        ("Troll", &["WARRIOR", "HUNTER", "ROGUE", "PRIEST", "SHAMAN", "MAGE", "WARLOCK"]),
    ];
    const CLASSES: [&str; 9] = [
        "WARRIOR", "PALADIN", "HUNTER", "ROGUE", "PRIEST", "SHAMAN", "MAGE", "WARLOCK", "DRUID",
    ];

    #[test]
    fn every_language_has_every_preset() {
        let en = table(Lang::En);
        for (race, classes) in COMBOS {
            assert!(en.contains_key(&format!("{race} *")), "{race}");
            for class in classes {
                assert!(en.contains_key(&format!("{race} {class}")), "{race} {class}");
            }
        }
        for class in CLASSES {
            assert!(en.contains_key(&format!("* {class}")), "{class}");
        }
        for lang in Lang::ALL {
            let t = table(lang);
            assert_eq!(t.len(), en.len(), "{}", lang.code());
            for (k, v) in t {
                assert!(en.contains_key(k), "{}: {k}", lang.code());
                assert!(v.contains("{name}"), "{}: {k}", lang.code());
            }
        }
    }

    #[test]
    fn unknown_races_fall_back_to_the_class() {
        assert_eq!(lookup("Skyborne", "DRUID", Lang::De), lookup("", "DRUID", Lang::De));
        assert!(lookup("Tauren", "PALADIN", Lang::En).unwrap().contains("Light"));
        assert!(lookup("Tauren", "", Lang::En).unwrap().contains("Mulgore"));
        assert!(lookup("Skyborne", "", Lang::En).is_none());
    }
}
