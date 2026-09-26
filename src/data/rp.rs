//! Roleplay profiles from Total RP 3, MyRolePlay and XRP, read (never
//! written) from their SavedVariables in the game folder: the profiles of
//! the player's own characters, and those of other roleplayers the client
//! has met. Total RP 3 keeps its own profiles in totalRP3.lua (TRP3_Profiles,
//! TRP3_Characters) and everyone else's in totalRP3_Data.lua (TRP3_Register);
//! MyRolePlay and XRP store the Mary Sue Protocol's two-letter fields (NA
//! name, DE description, HI history, CU currently…), per character for the
//! own profiles and in XRP's account-wide xrpCache for everyone seen.

use super::memory::Character;
use crate::savedvars;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const TRP3: &str = "Total RP 3";
pub const MRP: &str = "MyRolePlay";
pub const XRP: &str = "XRP";

/// One of TRP3's personality sliders, e.g. Chaotic ↔ Lawful.
#[derive(Clone, Debug, PartialEq)]
pub struct Trait {
    pub left: String,
    pub right: String,
    /// How far the character leans to `left`, 0..=1 (TRP3 stores 0..20).
    pub left_share: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub addon: &'static str,
    pub name: String,
    /// The short title before the name ("Sister").
    pub title: String,
    /// The full title under the name ("Keeper of the Last Candle").
    pub full_title: String,
    pub race: String,
    pub class: String,
    pub age: String,
    pub eyes: String,
    pub height: String,
    pub build: String,
    pub birthplace: String,
    pub residence: String,
    pub nickname: String,
    pub motto: String,
    pub appearance: String,
    pub about: String,
    pub history: String,
    pub currently: String,
    pub ooc: String,
    /// "At first glance": (title, text).
    pub glances: Vec<(String, String)>,
    pub traits: Vec<Trait>,
}

impl Profile {
    /// The name with its short title, as the character introduces themself.
    pub fn full_name(&self) -> String {
        [self.title.as_str(), self.name.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Something about who they are, beyond a name.
    pub fn has_character(&self) -> bool {
        !(self.about.is_empty()
            && self.appearance.is_empty()
            && self.history.is_empty()
            && self.traits.is_empty())
    }

    fn is_empty(&self) -> bool {
        self.name.is_empty()
            && self.full_title.is_empty()
            && !self.has_character()
            && self.currently.is_empty()
    }

    /// Traits that lean clearly one way, as words ("very lawful").
    pub fn trait_words(&self, name: impl Fn(&str) -> String, very: &str) -> Vec<String> {
        self.traits
            .iter()
            .filter_map(|t| {
                let (word, share) = if t.left_share >= 0.5 {
                    (&t.left, t.left_share)
                } else {
                    (&t.right, 1.0 - t.left_share)
                };
                let word = name(word).to_lowercase();
                match share {
                    s if s >= 0.85 => Some(format!("{very} {word}")),
                    s if s >= 0.62 => Some(word),
                    _ => None,
                }
            })
            .collect()
    }

    /// The profile for the writers, clipped so prompts stay lean.
    pub fn prompt(&self) -> String {
        use super::diary::clip;
        let mut lines = vec![];
        let mut add = |label: &str, text: &str, n: usize| {
            if !text.trim().is_empty() {
                lines.push(format!("- {label}: {}", clip(text, n)));
            }
        };
        let mut called = self.full_name();
        if !self.full_title.is_empty() {
            called = format!("{called}, \"{}\"", self.full_title);
        }
        add("Calls themself", &called, 160);
        add("Also known as", &self.nickname, 60);
        let body: Vec<String> = [
            ("age", &self.age),
            ("eyes", &self.eyes),
            ("height", &self.height),
            ("build", &self.build),
        ]
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{k} {}", clip(v, 60)))
        .collect();
        add("Body", &body.join(", "), 200);
        add("Lives in", &self.residence, 80);
        add("Born in", &self.birthplace, 80);
        add("Motto", &self.motto, 140);
        add("Looks", &self.appearance, 350);
        for (t, x) in self.glances.iter().take(3) {
            add(&format!("At first glance ({})", clip(t, 40)), x, 140);
        }
        add("Who they are", &self.about, 600);
        add("History", &self.history, 500);
        add(
            "Temperament",
            &self.trait_words(|w| w.to_string(), "very").join(", "),
            200,
        );
        add("Lately", &self.currently, 200);
        if lines.is_empty() {
            return String::new();
        }
        format!(
            "From their {} roleplay profile, in the player's own words:\n{}",
            self.addon,
            lines.join("\n")
        )
    }
}

/// Where profiles came from, for the settings page.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub addon: &'static str,
    pub own: usize,
    pub others: usize,
}

#[derive(Debug, Default)]
pub struct Roleplay {
    /// By `key(name, realm)`.
    own: HashMap<String, Arc<Profile>>,
    others: HashMap<String, Arc<Profile>>,
    pub sources: Vec<Source>,
}

/// "Tom", "Bandarion Keep" → "tom-bandarionkeep", the way TRP3 names a
/// character (the realm without spaces, dashes or dots).
pub fn key(name: &str, realm: &str) -> String {
    let realm: String = realm
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '.'))
        .collect();
    format!("{}-{}", name.trim(), realm).to_lowercase()
}

/// "Mira-Bandarion" → the key, or None without a realm.
fn split_key(unit: &str) -> Option<String> {
    let (name, realm) = unit.split_once('-')?;
    Some(key(name, realm))
}

impl Roleplay {
    fn count(&mut self, addon: &'static str, own: usize, others: usize) {
        match self.sources.iter_mut().find(|s| s.addon == addon) {
            Some(s) => {
                s.own += own;
                s.others += others;
            }
            None => self.sources.push(Source { addon, own, others }),
        }
    }

    fn add_own(&mut self, k: String, p: Profile) {
        if !p.is_empty() && !self.own.contains_key(&k) {
            self.count(p.addon, 1, 0);
            self.own.insert(k, Arc::new(p));
        }
    }

    fn add_other(&mut self, k: String, p: Profile) {
        if !p.is_empty() && !self.others.contains_key(&k) {
            self.count(p.addon, 0, 1);
            self.others.insert(k, Arc::new(p));
        }
    }

    /// The profile of one of the player's own characters.
    pub fn own_for(&self, c: &Character) -> Option<Arc<Profile>> {
        let s = |k: &str| c.snapshot.get(k).and_then(Value::as_str).unwrap_or("");
        let name = match s("name") {
            "" => c.name.split(' ').next().unwrap_or(&c.name),
            n => n,
        };
        if let Some(p) = self.own.get(&key(name, s("realm"))) {
            return Some(p.clone());
        }
        // Without a realm to go on, a first name that only one has.
        let prefix = format!("{}-", name.to_lowercase());
        let mut hits = self.own.iter().filter(|(k, _)| k.starts_with(&prefix));
        match (hits.next(), hits.next()) {
            (Some((_, p)), None) => Some(p.clone()),
            _ => None,
        }
    }

    /// Someone else's profile, by their name ("Mira Ashvale", "Mira" or
    /// "Mira-Bandarion"), preferring `realm` when the name is on several.
    pub fn other(&self, name: &str, realm: &str) -> Option<&Arc<Profile>> {
        if self.others.is_empty() {
            return None;
        }
        if let Some(p) = split_key(name).and_then(|k| self.others.get(&k)) {
            return Some(p);
        }
        let first = super::players::first_name(name);
        if let Some(p) = self.others.get(&key(&first, realm)) {
            return Some(p);
        }
        let prefix = format!("{first}-");
        self.others
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .min_by(|a, b| a.0.cmp(b.0))
            .map(|(_, p)| p)
    }
}

// ------------------------------------------------------------ reading files

/// Every RP addon file in the client's WTF folder: account-wide ones and
/// per-character ones (with the character's key).
fn files(install: &Path, flavor: &str) -> Vec<(PathBuf, Option<String>)> {
    let mut out = vec![];
    let accounts = install.join(flavor).join("WTF").join("Account");
    let dirs = |p: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(p)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect()
    };
    for acc in dirs(&accounts) {
        let sv = acc.join("SavedVariables");
        for f in [
            "totalRP3.lua",
            "totalRP3_Data.lua",
            "xrp.lua",
            "MyRolePlay.lua",
        ] {
            if sv.join(f).is_file() {
                out.push((sv.join(f), None));
            }
        }
        for realm in dirs(&acc) {
            let realm_name = realm
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if realm_name == "SavedVariables" {
                continue;
            }
            for ch in dirs(&realm) {
                let name = ch
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                for f in ["MyRolePlay.lua", "xrp.lua"] {
                    let p = ch.join("SavedVariables").join(f);
                    if p.is_file() {
                        out.push((p, Some(key(&name, &realm_name))));
                    }
                }
            }
        }
    }
    out
}

fn fingerprint(files: &[(PathBuf, Option<String>)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (p, _) in files {
        let m = std::fs::metadata(p).ok();
        (
            p,
            m.as_ref().map(|m| m.len()),
            m.and_then(|m| m.modified().ok()),
        )
            .hash(&mut h);
    }
    h.finish()
}

/// The profiles as last read, while none of the files changed.
static READ: std::sync::Mutex<Option<(u64, Arc<Roleplay>)>> = std::sync::Mutex::new(None);

/// Every profile in the game folder. Quiet when there are none, or when a
/// file can't be read.
pub fn load(install: &Path, flavor: &str) -> Arc<Roleplay> {
    let files = files(install, flavor);
    let print = fingerprint(&files);
    let mut kept = READ.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((f, rp)) = &*kept
        && *f == print
    {
        return rp.clone();
    }
    let mut rp = Roleplay::default();
    let mut account_level = vec![];
    for (path, owner) in &files {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(vars) = savedvars::parse(&bytes) else {
            continue;
        };
        match owner {
            Some(k) => read_character(&mut rp, k, &vars),
            None => account_level.push(vars),
        }
    }
    // TRP3's own profiles first, then the per-character MSP ones (above),
    // then what XRP's cache has on everyone.
    for vars in &account_level {
        read_trp3(&mut rp, vars);
    }
    for vars in &account_level {
        read_xrp_cache(&mut rp, vars);
    }
    rp.sources
        .sort_by_key(|s| [TRP3, MRP, XRP].iter().position(|a| *a == s.addon));
    let rp = Arc::new(rp);
    *kept = Some((print, rp.clone()));
    rp
}

/// TRP3_Characters/TRP3_Profiles (own) and TRP3_Register (everyone else).
fn read_trp3(rp: &mut Roleplay, vars: &Map<String, Value>) {
    if let (Some(Value::Object(chars)), Some(Value::Object(profiles))) =
        (vars.get("TRP3_Characters"), vars.get("TRP3_Profiles"))
    {
        let mut keys: Vec<&String> = chars.keys().collect();
        keys.sort();
        for unit in keys {
            let id = chars[unit].get("profileID").and_then(Value::as_str);
            if let (Some(k), Some(player)) = (
                split_key(unit),
                id.and_then(|id| profiles.get(id))
                    .and_then(|p| p.get("player")),
            ) {
                rp.add_own(k, trp3_profile(player));
            }
        }
    }
    if let Some(reg) = vars.get("TRP3_Register") {
        let (Some(Value::Object(chars)), Some(Value::Object(profiles))) =
            (reg.get("character"), reg.get("profiles"))
        else {
            return;
        };
        let mut keys: Vec<&String> = chars.keys().collect();
        keys.sort();
        for unit in keys {
            let id = chars[unit].get("profileID").and_then(Value::as_str);
            if let (Some(k), Some(p)) = (split_key(unit), id.and_then(|id| profiles.get(id))) {
                rp.add_other(k, trp3_profile(p));
            }
        }
    }
}

/// A character's own MyRolePlay or XRP profile, from their own folder.
fn read_character(rp: &mut Roleplay, k: &str, vars: &Map<String, Value>) {
    if let Some(saved) = vars.get("mrpSaved")
        && let Some(Value::Object(profiles)) = saved.get("Profiles")
    {
        // Every profile is laid over "Default".
        let mut fields = profiles
            .get("Default")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let selected = saved
            .get("SelectedProfile")
            .and_then(Value::as_str)
            .unwrap_or("Default");
        if let Some(Value::Object(sel)) = profiles.get(selected) {
            fields.extend(sel.clone());
        }
        rp.add_own(k.to_string(), msp_profile(&fields, MRP));
    }
    if let Some(saved) = vars.get("xrpSaved")
        && let Some(Value::Object(profiles)) = saved.get("profiles")
    {
        // The selected profile, then the ones it inherits from, unless a
        // child turned inheriting a field off.
        let mut fields = Map::new();
        let mut blocked = std::collections::HashSet::new();
        let mut name = saved
            .get("selected")
            .and_then(Value::as_str)
            .unwrap_or("Default");
        for _ in 0..16 {
            let Some(p) = profiles.get(name) else { break };
            if let Some(Value::Object(f)) = p.get("fields") {
                for (k, v) in f {
                    if !blocked.contains(k) {
                        fields.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
            }
            if let Some(Value::Object(inh)) = p.get("inherits") {
                blocked.extend(
                    inh.iter()
                        .filter(|(_, v)| **v == Value::Bool(false))
                        .map(|(k, _)| k.clone()),
                );
            }
            match p.get("parent").and_then(Value::as_str) {
                Some(parent) => name = parent,
                None => break,
            }
        }
        rp.add_own(k.to_string(), msp_profile(&fields, XRP));
    }
}

/// xrpCache: everyone XRP has heard from, the own characters marked.
fn read_xrp_cache(rp: &mut Roleplay, vars: &Map<String, Value>) {
    let Some(Value::Object(cache)) = vars.get("xrpCache") else {
        return;
    };
    let mut keys: Vec<&String> = cache.keys().collect();
    keys.sort();
    for unit in keys {
        let entry = &cache[unit];
        let (Some(k), Some(Value::Object(fields))) = (split_key(unit), entry.get("fields")) else {
            continue;
        };
        let p = msp_profile(fields, XRP);
        if entry.get("own").and_then(Value::as_bool) == Some(true) {
            rp.add_own(k, p);
        } else {
            rp.add_other(k, p);
        }
    }
}

// ------------------------------------------------------------ the formats

/// TRP3's preset personality traits by ID, as its English locale names them.
const PRESETS: [(&str, &str); 11] = [
    ("Chaotic", "Lawful"),
    ("Chaste", "Lustful"),
    ("Forgiving", "Vindictive"),
    ("Altruistic", "Selfish"),
    ("Truthful", "Deceitful"),
    ("Gentle", "Brutal"),
    ("Superstitious", "Rational"),
    ("Renegade", "Paragon"),
    ("Cautious", "Impulsive"),
    ("Ascetic", "Bon vivant"),
    ("Valorous", "Spineless"),
];

/// Lists saved as arrays or as tables keyed "1", "2"…, in order.
fn list(v: Option<&Value>) -> Vec<&Value> {
    match v {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(Value::Object(o)) => {
            let mut e: Vec<(&String, &Value)> = o.iter().collect();
            e.sort_by_key(|(k, _)| k.parse::<i64>().unwrap_or(i64::MAX));
            e.into_iter().map(|(_, v)| v).collect()
        }
        _ => vec![],
    }
}

fn text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => clean(s),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// A TRP3 profile: `player` of TRP3_Profiles, or a TRP3_Register profile.
pub fn trp3_profile(p: &Value) -> Profile {
    let ch = p.get("characteristics").unwrap_or(&Value::Null);
    let f = |k: &str| text(ch.get(k));
    let mut out = Profile {
        addon: TRP3,
        name: [f("FN"), f("LN")]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        title: f("TI"),
        full_title: f("FT"),
        race: f("RA"),
        class: f("CL"),
        age: f("AG"),
        eyes: f("EC"),
        height: f("HE"),
        build: f("WE"),
        birthplace: f("BP"),
        residence: f("RE"),
        ..Default::default()
    };
    // Misc info: typed (4 motto, 3 nickname) or named by hand.
    for mi in list(ch.get("MI")) {
        let (id, name, value) = (
            mi.get("ID").and_then(Value::as_i64),
            text(mi.get("NA")),
            text(mi.get("VA")),
        );
        match (id, name.to_lowercase().as_str()) {
            (Some(4), _) | (None, "motto") => out.motto = value,
            (Some(3), _) | (None, "nickname") => out.nickname = value,
            _ => {}
        }
    }
    for ps in list(ch.get("PS")) {
        let preset = ps
            .get("ID")
            .and_then(Value::as_u64)
            .and_then(|i| PRESETS.get((i as usize).wrapping_sub(1)));
        let (left, right) = match preset {
            Some((l, r)) => (l.to_string(), r.to_string()),
            None => (text(ps.get("LT")), text(ps.get("RT"))),
        };
        if left.is_empty() && right.is_empty() {
            continue;
        }
        // V2 is 0..20; profiles from before it have VA, 0..6.
        let share = match (
            ps.get("V2").and_then(Value::as_f64),
            ps.get("VA").and_then(Value::as_f64),
        ) {
            (Some(v), _) => v / 20.0,
            (None, Some(v)) => v / 6.0,
            _ => 0.5,
        };
        out.traits.push(Trait {
            left,
            right,
            left_share: share.clamp(0.0, 1.0) as f32,
        });
    }
    let about = p.get("about").unwrap_or(&Value::Null);
    let sections = |k: &str| text(about.pointer(&format!("/T3/{k}/TX")));
    match about.get("TE").and_then(Value::as_i64) {
        Some(3) => {
            out.appearance = sections("PH");
            out.about = sections("PS");
            out.history = sections("HI");
        }
        Some(2) => {
            out.about = list(about.get("T2"))
                .into_iter()
                .map(|b| text(b.get("TX")))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
        }
        _ => out.about = text(about.pointer("/T1/TX")),
    }
    let character = p.get("character").unwrap_or(&Value::Null);
    out.currently = text(character.get("CU"));
    out.ooc = text(character.get("CO"));
    let glances = p.pointer("/misc/PE");
    for g in list(glances) {
        let active = matches!(
            g.get("AC"),
            Some(Value::Bool(true)) | Some(Value::Number(_))
        );
        let (t, x) = (text(g.get("TI")), text(g.get("TX")));
        if active && !(t.is_empty() && x.is_empty()) {
            out.glances.push((t, x));
        }
    }
    out
}

/// An MSP profile from its fields.
pub fn msp_profile(fields: &Map<String, Value>, addon: &'static str) -> Profile {
    let f = |k: &str| text(fields.get(k));
    let mut out = Profile {
        addon,
        name: f("NA"),
        full_title: f("NT"),
        nickname: f("NI"),
        race: f("RA"),
        class: f("RC"),
        age: f("AG"),
        eyes: f("AE"),
        height: f("AH"),
        build: f("AW"),
        birthplace: f("HB"),
        residence: f("HH"),
        motto: f("MO"),
        appearance: f("DE"),
        history: f("HI"),
        currently: f("CU"),
        ooc: f("CO"),
        ..Default::default()
    };
    // PE: glances joined by "---", each "|Ticon|t\n#Title\n\nText".
    if let Some(Value::String(pe)) = fields.get("PE") {
        for g in pe.split("\n\n---\n\n") {
            let g = match g.find("|t") {
                Some(i) if g.starts_with("|T") => g[i + 2..].trim_start(),
                _ => g.trim_start(),
            };
            let (t, x) = match g.strip_prefix('#') {
                Some(rest) => rest.split_once("\n\n").unwrap_or((rest, "")),
                None => ("", g),
            };
            let (t, x) = (clean(t), clean(x));
            if !(t.is_empty() && x.is_empty()) {
                out.glances.push((t, x));
            }
        }
    }
    out
}

/// Profile text without the game's colour and icon escapes or TRP3's
/// formatting tags ({h1}, {col:ff0000}, {icon:…}, {link*url*text}).
pub fn clean(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '|' {
            let mut chars = rest[1..].chars();
            match chars.next() {
                // |cAARRGGBB, or |cnNAMED_COLOR: on newer clients.
                Some('c') if rest[2..].starts_with('n') => {
                    rest = rest.find(':').map(|i| &rest[i + 1..]).unwrap_or("")
                }
                Some('c') => rest = rest.get(10..).unwrap_or(""),
                Some('r') | Some('h') => rest = &rest[2..],
                Some('|') => {
                    out.push('|');
                    rest = &rest[2..];
                }
                Some('T') | Some('H') => {
                    // |T…|t (a texture) and |H…|h (a link's target) go.
                    let end = if rest[1..].starts_with('T') {
                        "|t"
                    } else {
                        "|h"
                    };
                    rest = rest.find(end).map(|i| &rest[i + 2..]).unwrap_or("");
                }
                _ => {
                    out.push('|');
                    rest = &rest[1..];
                }
            }
            continue;
        }
        if c == '{'
            && let Some(end) = rest.find('}')
        {
            let tag = &rest[1..end];
            if let Some(link) = tag.strip_prefix("link*") {
                out.push_str(link.rsplit('*').next().unwrap_or(""));
            }
            rest = &rest[end + 1..];
            // Headings ({h1}Appearance{/h1}) only label the text below.
            if let Some(level) = tag
                .strip_prefix('h')
                .and_then(|t| t.chars().next())
                .filter(char::is_ascii_digit)
            {
                let close = format!("{{/h{level}}}");
                rest = rest
                    .find(&close)
                    .map(|i| &rest[i + close.len()..])
                    .unwrap_or(rest);
            }
            continue;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    // Trim every line, and no more than one blank line in a row.
    let mut lines: Vec<&str> = vec![];
    for l in out.lines().map(str::trim) {
        if !(l.is_empty() && lines.last().is_none_or(|p| p.is_empty())) {
            lines.push(l);
        }
    }
    while lines.last() == Some(&"") {
        lines.pop();
    }
    lines.join("\n")
}

/// Who the character is, for the writers: the note, their roleplay profile,
/// and the race-and-class preset when neither says who they are.
pub fn who(c: &Character) -> Option<String> {
    let note = c.personality.trim();
    let profile =
        c.rp.as_deref()
            .map(Profile::prompt)
            .filter(|s| !s.is_empty());
    let base = if !note.is_empty() {
        Some(note.to_string())
    } else if c.rp.as_ref().is_some_and(|p| p.has_character()) {
        None
    } else {
        super::presets::for_character(c, crate::i18n::Lang::En)
    };
    match (base, profile) {
        (Some(b), Some(p)) if !note.is_empty() => Some(format!(
            "{b}\n\n{p}\n(Where the profile and the note above disagree, the note wins.)"
        )),
        (Some(b), Some(p)) => Some(format!("{b}\n\n{p}")),
        (b, p) => b.or(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRP3_FILE: &str = r#"
TRP3_Profiles = {
	["0612134555Iwoen"] = {
		["profileName"] = "Tom",
		["player"] = {
			["characteristics"] = {
				["v"] = 5,
				["FN"] = "Tom",
				["LN"] = "Crusader",
				["TI"] = "Brother",
				["FT"] = "|cffffd100Shield of Bandarion|r",
				["AG"] = "Dead for six years",
				["EC"] = "Pale gold",
				["MI"] = {
					{
						["ID"] = 4,
						["NA"] = "Motto",
						["VA"] = "Hold the line.",
					}, -- [1]
				},
				["PS"] = {
					{
						["ID"] = 1,
						["V2"] = 17,
					}, -- [1]
					{
						["LT"] = "Hopeful",
						["RT"] = "Bitter",
						["V2"] = 4,
					}, -- [2]
					{
						["ID"] = 6,
						["VA"] = 3,
					}, -- [3]
				},
			},
			["about"] = {
				["TE"] = 3,
				["T3"] = {
					["PH"] = { ["TX"] = "{h1}Face{/h1}\nA jaw held on with wire." },
					["PS"] = { ["TX"] = "Dry. Dutiful. {col:ff0000}Stubborn{/col}." },
					["HI"] = { ["TX"] = "Died in Brill; see {link*https://x*his grave}." },
				},
			},
			["character"] = {
				["CU"] = "Guarding the gate",
				["RP"] = 1,
			},
			["misc"] = {
				["PE"] = {
					["1"] = { ["AC"] = true, ["TI"] = "Tabard", ["TX"] = "Bandarion's colours." },
					["2"] = { ["AC"] = false, ["TI"] = "Hidden" },
				},
			},
		},
	},
}
TRP3_Characters = {
	["Tom-Bandarion"] = {
		["profileID"] = "0612134555Iwoen",
	},
}
"#;

    const REGISTER_FILE: &str = r#"
TRP3_Register = {
	["character"] = {
		["Mira-Bandarion"] = { ["profileID"] = "abc", ["class"] = "PRIEST" },
		["Nobody-Bandarion"] = {},
	},
	["profiles"] = {
		["abc"] = {
			["link"] = { ["Mira-Bandarion"] = 1 },
			["characteristics"] = { ["FN"] = "Mira", ["LN"] = "Ashvale", ["TI"] = "Sister" },
			["about"] = { ["TE"] = 2, ["T2"] = { { ["TX"] = "One." }, { ["TX"] = "Two." } } },
			["character"] = { ["CU"] = "Lighting candles" },
		},
	},
}
"#;

    fn vars(s: &str) -> Map<String, Value> {
        savedvars::parse(s.as_bytes()).unwrap()
    }

    #[test]
    fn trp3_own_and_register() {
        let mut rp = Roleplay::default();
        read_trp3(&mut rp, &vars(TRP3_FILE));
        read_trp3(&mut rp, &vars(REGISTER_FILE));
        let tom = &rp.own["tom-bandarion"];
        assert_eq!(tom.full_name(), "Brother Tom Crusader");
        assert_eq!(tom.full_title, "Shield of Bandarion");
        assert_eq!(tom.motto, "Hold the line.");
        assert_eq!(tom.appearance, "A jaw held on with wire.");
        assert_eq!(tom.about, "Dry. Dutiful. Stubborn.");
        assert_eq!(tom.history, "Died in Brill; see his grave.");
        assert_eq!(tom.currently, "Guarding the gate");
        assert_eq!(
            tom.glances,
            vec![("Tabard".into(), "Bandarion's colours.".into())]
        );
        assert_eq!(tom.traits.len(), 3);
        assert_eq!(
            (tom.traits[0].left.as_str(), tom.traits[0].right.as_str()),
            ("Chaotic", "Lawful")
        );
        assert!((tom.traits[0].left_share - 0.85).abs() < 1e-6);
        assert_eq!(tom.traits[2].left, "Gentle");
        assert!((tom.traits[2].left_share - 0.5).abs() < 1e-6);
        assert_eq!(
            tom.trait_words(|w| w.to_string(), "very"),
            vec!["very chaotic", "bitter"]
        );
        let mira = rp.other("Mira Ashvale", "Bandarion").unwrap();
        assert_eq!(mira.full_name(), "Sister Mira Ashvale");
        assert_eq!(mira.about, "One.\n\nTwo.");
        assert_eq!(
            rp.other("Mira-Bandarion", "").unwrap().currently,
            "Lighting candles"
        );
        assert!(rp.other("Nobody", "Bandarion").is_none());
        assert_eq!(
            rp.sources,
            vec![Source {
                addon: TRP3,
                own: 1,
                others: 1
            }]
        );
    }

    #[test]
    fn mrp_and_xrp() {
        let mut rp = Roleplay::default();
        read_character(
            &mut rp,
            "oprah-bandarion",
            &vars(
                r#"
mrpSaved = {
	["SelectedProfile"] = "Travel",
	["Profiles"] = {
		["Default"] = { ["NA"] = "Oprah Windfury", ["DE"] = "Short and loud.", ["MO"] = "Onward!" },
		["Travel"] = { ["CU"] = "Looking for a boat", ["PE"] = "|TInterface\\Icons\\INV_Misc_Map_01:32:32|t\n#Maps\n\nCarries too many." },
	},
}
"#,
            ),
        );
        read_xrp_cache(
            &mut rp,
            &vars(
                r#"
xrpCache = {
	["Rakka-Bandarion"] = { ["fields"] = { ["NA"] = "|cffff0000Rakka|r", ["NT"] = "Ember-Hand", ["HI"] = "Born in Durotar." }, ["versions"] = {} },
	["Oprah-Bandarion"] = { ["own"] = true, ["fields"] = { ["NA"] = "Someone else" } },
}
"#,
            ),
        );
        let oprah = &rp.own["oprah-bandarion"];
        assert_eq!(oprah.addon, MRP);
        assert_eq!(oprah.name, "Oprah Windfury");
        assert_eq!(oprah.currently, "Looking for a boat");
        assert_eq!(oprah.motto, "Onward!");
        assert_eq!(
            oprah.glances,
            vec![("Maps".into(), "Carries too many.".into())]
        );
        let rakka = rp.other("Rakka", "Bandarion").unwrap();
        assert_eq!(
            (rakka.name.as_str(), rakka.full_title.as_str()),
            ("Rakka", "Ember-Hand")
        );
        assert!(rp.other("Oprah", "Bandarion").is_none());
        assert!(rakka.prompt().contains("- History: Born in Durotar."));
    }

    #[test]
    fn xrp_profiles_inherit() {
        let mut rp = Roleplay::default();
        read_character(
            &mut rp,
            "usain-bandarion",
            &vars(
                r#"
xrpSaved = {
	["selected"] = "Robes",
	["profiles"] = {
		["Default"] = { ["fields"] = { ["NA"] = "Usain", ["DE"] = "Tall.", ["CU"] = "Old news" } },
		["Robes"] = { ["parent"] = "Default", ["fields"] = { ["DE"] = "In red robes." }, ["inherits"] = { ["CU"] = false } },
	},
}
"#,
            ),
        );
        let u = &rp.own["usain-bandarion"];
        assert_eq!(
            (u.name.as_str(), u.appearance.as_str(), u.currently.as_str()),
            ("Usain", "In red robes.", "")
        );
    }

    #[test]
    fn cleaning() {
        assert_eq!(clean("|cff00ff00Green|r and ||pipes"), "Green and |pipes");
        assert_eq!(clean("|cnGREEN_FONT_COLOR:ok|r"), "ok");
        assert_eq!(clean("a\n\n\n\n  b  \n\n"), "a\n\nb");
        assert_eq!(clean("{p:c}centred{/p} {icon:INV_Misc:25}x"), "centred x");
        assert_eq!(key("Tom", "Bandarion Keep"), "tom-bandarionkeep");
    }

    #[test]
    fn who_combines_note_profile_and_preset() {
        let mut c = Character {
            name: "Tom Crusader".into(),
            class_file: "PALADIN".into(),
            snapshot: serde_json::json!({"raceFile": "Scourge", "name": "Tom", "realm": "Bandarion"}),
            ..Default::default()
        };
        let preset = who(&c).unwrap();
        c.rp = Some(Arc::new(Profile {
            addon: TRP3,
            name: "Tom".into(),
            ..Default::default()
        }));
        // A name alone says too little: the preset stays.
        assert!(who(&c).unwrap().starts_with(&preset));
        c.rp = Some(Arc::new(Profile {
            addon: TRP3,
            about: "Grim.".into(),
            ..Default::default()
        }));
        assert!(
            who(&c)
                .unwrap()
                .starts_with("From their Total RP 3 roleplay profile")
        );
        c.personality = "Dry.".into();
        let w = who(&c).unwrap();
        assert!(
            w.starts_with("Dry.\n\n")
                && w.contains("- Who they are: Grim.")
                && w.contains("note wins")
        );
    }
}
