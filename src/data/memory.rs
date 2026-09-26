//! Reads the forever-memory repository: characters, their event logs, quest
//! texts, the item catalogue and the quest log.

use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

/// One recorded event row (see the repository README for the kinds).
#[derive(Clone, Debug)]
pub struct Event {
    pub n: i64,
    pub t: i64,
    pub e: String,
    pub v: Value,
}

impl Event {
    pub fn s(&self, k: &str) -> Option<&str> {
        self.v.get(k).and_then(Value::as_str)
    }
    pub fn i(&self, k: &str) -> Option<i64> {
        self.v.get(k).and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)))
    }
    pub fn f(&self, k: &str) -> Option<f64> {
        self.v.get(k).and_then(Value::as_f64)
    }
}

/// Lua tables arrive as JSON arrays (keys 1..n) or objects with numeric
/// string keys; this yields (lua index, value) either way.
pub fn entries(v: &Value) -> Vec<(i64, &Value)> {
    match v {
        Value::Array(a) => a.iter().enumerate().map(|(i, x)| (i as i64 + 1, x)).collect(),
        Value::Object(o) => {
            let mut out: Vec<_> = o.iter().filter_map(|(k, x)| k.parse().ok().map(|i| (i, x))).collect();
            out.sort_by_key(|(i, _)| *i);
            out
        }
        _ => vec![],
    }
}

#[derive(Clone, Debug, Default)]
pub struct ItemInfo {
    pub name: String,
    pub icon: Option<i64>,
    pub quality: Option<i64>,
}

/// An item link reduced to what the UI needs.
#[derive(Clone, Debug, Default)]
pub struct Link {
    pub id: i64,
    pub name: String,
    pub quality: Option<i64>,
}

pub fn parse_link(link: &str) -> Option<Link> {
    let id = link.split("item:").nth(1)?.split(':').next()?.parse().ok()?;
    let name = link.split('[').nth(1).and_then(|s| s.split(']').next()).unwrap_or("").to_string();
    let quality = link.split("|cnIQ").nth(1).and_then(|s| s.chars().next()).and_then(|c| c.to_digit(10)).map(i64::from);
    Some(Link { id, name, quality })
}

#[derive(Clone, Debug, Default)]
pub struct Objective {
    pub text: String,
    pub have: i64,
    pub need: i64,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum QuestStatus {
    Active,
    Completed,
    Abandoned,
    Seen,
}

#[derive(Clone, Debug)]
pub struct Quest {
    pub id: i64,
    pub title: String,
    pub level: Option<i64>,
    pub text: String,
    pub objective: String,
    pub progress: String,
    pub reward: String,
    pub choices: Vec<String>,
    pub accepted: Option<i64>,
    pub done: Option<i64>,
    pub removed: Option<i64>,
    pub xp: i64,
    pub money: i64,
    pub choice: Option<String>,
    pub status: QuestStatus,
    pub complete: bool, // ready to turn in
    pub objectives: Vec<Objective>,
    /// (time, objective index, have, need)
    pub progress_events: Vec<(i64, i64, i64, i64)>,
    pub zone: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Session {
    pub start: i64,
    pub end: i64,
    /// Index range into Character::events.
    pub from: usize,
    pub to: usize,
    pub xp: i64,
    pub money: i64,
    pub items: i64,
    pub quests: i64,
    pub deaths: i64,
    pub level_from: i64,
    pub level_to: i64,
    pub zones: Vec<String>,
}

impl Session {
    pub fn seconds(&self) -> i64 {
        (self.end - self.start).max(0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Character {
    pub name: String,
    pub guid: String,
    pub class: String,
    pub class_file: String,
    pub race: String,
    pub level: i64,
    pub zone: String,
    pub money: i64,
    pub xp: (i64, i64),
    pub snapshot: Value,
    pub seen: HashMap<String, i64>,
    pub events: Vec<Event>,
    pub sessions: Vec<Session>,
    pub quests: Vec<Quest>,
}

impl Character {
    /// Seconds played before wall-clock time t, summed over sessions.
    pub fn play_time(&self, t: i64) -> f64 {
        let mut total = 0;
        for s in &self.sessions {
            if t >= s.end {
                total += s.seconds();
            } else if t > s.start {
                total += t - s.start;
            }
        }
        total as f64
    }
    pub fn total_play(&self) -> i64 {
        self.sessions.iter().map(Session::seconds).sum()
    }
}

pub struct Memory {
    pub characters: Vec<Character>,
    pub items: HashMap<i64, ItemInfo>,
    /// NPC gossip: (npc name, text, options)
    pub gossip: Vec<(String, String, Vec<String>)>,
}

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(p).ok()?).ok()
}

pub fn load(repo: &Path) -> Memory {
    let mut items: HashMap<i64, ItemInfo> = HashMap::new();
    if let Some(Value::Object(o)) = read_json(&repo.join("items.json")) {
        for (k, v) in o {
            let Ok(id) = k.parse() else { continue };
            items.insert(id, ItemInfo {
                name: v.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                icon: v.get("icon").and_then(Value::as_i64),
                quality: v.get("q").and_then(Value::as_i64),
            });
        }
    }
    let mut texts: HashMap<i64, Value> = HashMap::new();
    if let Ok(dir) = fs::read_dir(repo.join("quests")) {
        for f in dir.flatten() {
            let name = f.file_name().to_string_lossy().to_string();
            if let (Some(id), Some(v)) = (name.strip_suffix(".json").and_then(|s| s.parse().ok()), read_json(&f.path())) {
                texts.insert(id, v);
            }
        }
    }
    let mut gossip = vec![];
    if let Some(Value::Object(o)) = read_json(&repo.join("gossip.json")) {
        for v in o.values() {
            let opts = v.get("options").map(|o| entries(o).into_iter().filter_map(|(_, x)| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
            gossip.push((
                v.get("name").and_then(Value::as_str).unwrap_or("?").to_string(),
                v.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
                opts,
            ));
        }
    }

    let mut characters = vec![];
    if let Ok(dir) = fs::read_dir(repo.join("characters")) {
        for d in dir.flatten() {
            if let Some(c) = load_character(&d.path(), &texts, &mut items) {
                characters.push(c);
            }
        }
    }
    characters.sort_by(|a, b| b.level.cmp(&a.level).then(a.name.cmp(&b.name)));
    Memory { characters, items, gossip }
}

fn load_character(dir: &Path, texts: &HashMap<i64, Value>, items: &mut HashMap<i64, ItemInfo>) -> Option<Character> {
    let snapshot = read_json(&dir.join("snapshot.json"))?;
    let s = |k: &str| snapshot.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let i = |k: &str| snapshot.get(k).and_then(Value::as_i64).unwrap_or(0);
    let mut name = s("displayName");
    if name.is_empty() {
        name = format!("{} {}", s("name"), s("surname")).trim().to_string();
    }
    let mut c = Character {
        name,
        guid: s("guid"),
        class: s("class"),
        class_file: s("classFile"),
        race: s("race"),
        level: i("level"),
        zone: s("zone"),
        money: i("money"),
        xp: (
            snapshot.pointer("/xp/cur").and_then(Value::as_i64).unwrap_or(0),
            snapshot.pointer("/xp/max").and_then(Value::as_i64).unwrap_or(0),
        ),
        ..Default::default()
    };
    // Worn items teach the catalogue their icons too.
    if let Some(eq) = snapshot.get("equipment") {
        for (_, it) in entries(eq) {
            if let (Some(id), Some(icon)) = (it.get("id").and_then(Value::as_i64), it.get("icon").and_then(Value::as_i64)) {
                let e = items.entry(id).or_default();
                e.icon.get_or_insert(icon);
                if e.quality.is_none() {
                    e.quality = it.get("quality").and_then(Value::as_i64);
                }
                if e.name.is_empty() {
                    if let Some(n) = it.pointer("/tooltip/0/l").and_then(Value::as_str) {
                        e.name = n.to_string();
                    }
                }
            }
        }
    }
    c.snapshot = snapshot;
    if let Some(Value::Object(o)) = read_json(&dir.join("seen.json")) {
        c.seen = o.into_iter().filter_map(|(k, v)| v.as_i64().map(|t| (k, t))).collect();
    }
    let mut files: Vec<_> = fs::read_dir(dir.join("log")).map(|d| d.flatten().map(|f| f.path()).collect()).unwrap_or_default();
    files.sort();
    for f in files {
        for line in fs::read_to_string(&f).unwrap_or_default().lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let (Some(n), Some(t), Some(e)) = (v.get("n").and_then(Value::as_i64), v.get("t").and_then(Value::as_i64), v.get("e").and_then(Value::as_str)) else { continue };
            let e = e.to_string();
            c.events.push(Event { n, t, e, v });
        }
    }
    c.events.sort_by_key(|e| e.n);
    // Before the addon waited for the bags to load, its first scan after login
    // logged everything already carried as new, with no context. Drop those.
    let mut login = i64::MIN;
    c.events.retain(|e| {
        if e.e == "login" {
            login = e.t;
        }
        !(e.e == "item" && e.s("ctx").is_none() && e.t - login <= 10)
    });
    for e in &c.events {
        for key in ["link", "choice"] {
            if let Some(l) = e.s(key).and_then(parse_link) {
                let it = items.entry(l.id).or_default();
                if it.name.is_empty() {
                    it.name = l.name;
                }
                it.quality = it.quality.or(l.quality);
            }
        }
    }
    c.sessions = sessions(&c.events);
    c.quests = quests(&c, texts, read_json(&dir.join("questlog.json")));
    Some(c)
}

fn sessions(events: &[Event]) -> Vec<Session> {
    let mut out: Vec<Session> = vec![];
    let mut cur: Option<Session> = None;
    let mut level = 0;
    for (i, e) in events.iter().enumerate() {
        if e.e == "login" {
            if let Some(mut s) = cur.take() {
                s.to = i;
                out.push(s);
            }
            level = e.i("level").unwrap_or(level);
            cur = Some(Session { start: e.t, end: e.t, from: i, to: i + 1, level_from: level, level_to: level, ..Default::default() });
            continue;
        }
        let Some(s) = cur.as_mut() else { continue };
        s.end = e.t;
        s.to = i + 1;
        match e.e.as_str() {
            "xp" => s.xp += e.i("d").unwrap_or(0).max(0),
            "money" => s.money += e.i("d").unwrap_or(0),
            "item" if e.i("d").unwrap_or(0) > 0 && e.s("ctx") == Some("loot") => s.items += e.i("d").unwrap_or(0),
            "quest" if e.s("act") == Some("turnin") => {
                s.quests += 1;
                s.xp += e.i("xp").unwrap_or(0);
            }
            "death" => s.deaths += 1,
            "level" => s.level_to = e.i("level").unwrap_or(s.level_to),
            "zone" | "login" => {
                if let Some(z) = e.s("zone") {
                    if !z.is_empty() && !s.zones.iter().any(|x| x == z) {
                        s.zones.push(z.to_string());
                    }
                }
            }
            "logout" => {
                out.push(cur.take().unwrap());
            }
            _ => {}
        }
    }
    if let Some(s) = cur {
        out.push(s);
    }
    out
}

fn quests(c: &Character, texts: &HashMap<i64, Value>, questlog: Option<Value>) -> Vec<Quest> {
    let mut by_id: BTreeMap<i64, Quest> = BTreeMap::new();
    let new = |id: i64| Quest {
        id,
        title: String::new(),
        level: None,
        text: String::new(),
        objective: String::new(),
        progress: String::new(),
        reward: String::new(),
        choices: vec![],
        accepted: None,
        done: None,
        removed: None,
        xp: 0,
        money: 0,
        choice: None,
        status: QuestStatus::Seen,
        complete: false,
        objectives: vec![],
        progress_events: vec![],
        zone: None,
    };
    let mut zone = String::new();
    for e in &c.events {
        match e.e.as_str() {
            "zone" | "login" => zone = e.s("zone").unwrap_or("").to_string(),
            "quest" => {
                let Some(id) = e.i("id") else { continue };
                let q = by_id.entry(id).or_insert_with(|| new(id));
                if let Some(t) = e.s("title") {
                    q.title = t.to_string();
                }
                match e.s("act") {
                    Some("accept") => {
                        q.accepted = Some(e.t);
                        q.zone.get_or_insert(zone.clone());
                    }
                    Some("turnin") => {
                        q.done = Some(e.t);
                        q.xp = e.i("xp").unwrap_or(0);
                        q.money = e.i("money").unwrap_or(0);
                        q.choice = e.s("choice").map(str::to_string);
                    }
                    Some("remove") => q.removed = Some(e.t),
                    _ => {}
                }
            }
            "objective" => {
                if let Some(id) = e.i("id") {
                    by_id.entry(id).or_insert_with(|| new(id)).progress_events.push((
                        e.t,
                        e.i("i").unwrap_or(0),
                        e.i("have").unwrap_or(0),
                        e.i("need").unwrap_or(0),
                    ));
                }
            }
            _ => {}
        }
    }
    let mut active = std::collections::HashSet::new();
    if let Some(ql) = questlog {
        for (_, q) in entries(&ql) {
            let Some(id) = q.get("id").and_then(Value::as_i64) else { continue };
            active.insert(id);
            let quest = by_id.entry(id).or_insert_with(|| new(id));
            if let Some(t) = q.get("title").and_then(Value::as_str) {
                quest.title = t.to_string();
            }
            quest.level = q.get("level").and_then(Value::as_i64);
            quest.complete = q.get("complete").and_then(Value::as_bool).unwrap_or(false);
            quest.objectives = q.get("objectives").map(|o| {
                entries(o).into_iter().map(|(_, o)| Objective {
                    text: o.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
                    have: o.get("have").and_then(Value::as_i64).unwrap_or(0),
                    need: o.get("need").and_then(Value::as_i64).unwrap_or(0),
                    done: o.get("done").and_then(Value::as_bool).unwrap_or(false),
                }).collect()
            }).unwrap_or_default();
        }
    }
    for q in by_id.values_mut() {
        if let Some(t) = texts.get(&q.id) {
            let s = |k: &str| t.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            if q.title.is_empty() {
                q.title = s("title");
            }
            q.text = s("text");
            q.objective = s("objective");
            q.progress = s("progress");
            q.reward = s("reward");
            q.level = q.level.or(t.get("level").and_then(Value::as_i64));
            q.choices = t.get("choices").map(|c| entries(c).into_iter().filter_map(|(_, x)| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
        }
        if q.title.is_empty() {
            q.title = format!("Quest {}", q.id);
        }
        q.status = if q.done.is_some() {
            QuestStatus::Completed
        } else if active.contains(&q.id) || (q.accepted.is_some() && q.removed.is_none()) {
            QuestStatus::Active
        } else if q.removed.is_some() {
            QuestStatus::Abandoned
        } else {
            QuestStatus::Seen
        };
    }
    by_id.into_values().collect()
}
