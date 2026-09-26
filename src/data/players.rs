//! Everyone met in the world, merged from the combat log (GUID, first name,
//! spells, positions) and the chat log (full name with surname, lines).

use super::chat::{Kind, Line};
use super::combat::{self, Combat, UnitKind};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug, Default)]
pub struct Player {
    pub name: String,  // "Scyle Reaper" when chat told us the surname, else "Scyle"
    pub first: String, // "Scyle"
    pub unit: Option<u32>,
    pub class: Option<&'static str>,
    pub class_known: bool, // seen up close, not guessed from spells
    pub race: String,
    pub level: i64,
    pub guild: String,
    pub first_seen: f64,
    pub last_seen: f64,
    pub days: BTreeSet<String>,
    pub chat: Vec<usize>, // indices into the chat lines
    pub combat_lines: u64,
    pub maps: Vec<i64>,
}

pub fn build(
    c: &Combat,
    chat: &[Line],
    me: &[String],
    known: &std::collections::HashMap<String, super::memory::KnownPlayer>,
) -> Vec<Player> {
    let mine: Vec<String> = me.iter().map(|n| first_name(n)).collect();
    let mut by_first: HashMap<String, Player> = HashMap::new();
    let day = |t: f64| {
        chrono::DateTime::from_timestamp(t as i64, 0)
            .map(|d| {
                d.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d")
                    .to_string()
            })
            .unwrap_or_default()
    };

    for (&u, seen) in &c.players {
        let unit = &c.units[u as usize];
        if unit.kind != UnitKind::Player || mine.contains(&unit.name.to_lowercase()) {
            continue;
        }
        let p = by_first.entry(unit.name.to_lowercase()).or_default();
        p.first = unit.name.clone();
        p.name = unit.name.clone();
        p.unit = Some(u);
        p.class = combat::guess_class(&seen.spells, c);
        if let Some(k) = known.get(&unit.guid) {
            if !k.surname.is_empty() {
                p.name = format!("{} {}", k.name, k.surname);
            }
            if let Some(cf) = CLASSES.iter().find(|x| **x == k.class) {
                p.class = Some(cf);
                p.class_known = true;
            }
            p.race = k.race.clone();
            p.level = k.level;
            p.guild = k.guild.clone();
        }
        p.first_seen = seen.first;
        p.last_seen = seen.last;
        p.combat_lines = seen.lines;
        p.days.insert(day(seen.first));
        p.days.insert(day(seen.last));
        for &(_, map, _, _) in &seen.positions {
            if !p.maps.contains(&map) {
                p.maps.push(map);
            }
        }
    }
    let combat_first: BTreeSet<String> = by_first.keys().cloned().collect();
    for (i, l) in chat.iter().enumerate() {
        let Some(who) = &l.speaker else { continue };
        let first = first_name(who);
        if mine.contains(&first) {
            continue;
        }
        // Says and yells come from NPCs too; only count them for known players.
        let player_kind = !matches!(l.kind, Kind::Say | Kind::Yell);
        if !player_kind && !combat_first.contains(&first) {
            continue;
        }
        let p = by_first.entry(first.clone()).or_default();
        if p.first.is_empty() {
            p.first = who.split(' ').next().unwrap_or(who).to_string();
        }
        if who.contains(' ') {
            p.name = who.clone();
        } else if p.name.is_empty() {
            p.name = who.clone();
        }
        p.chat.push(i);
        if p.first_seen == 0.0 || l.t < p.first_seen {
            p.first_seen = l.t;
        }
        p.last_seen = p.last_seen.max(l.t);
        p.days.insert(day(l.t));
    }
    let mut out: Vec<Player> = by_first.into_values().collect();
    out.sort_by(|a, b| b.last_seen.total_cmp(&a.last_seen).then_with(|| a.name.cmp(&b.name)));
    out
}

const CLASSES: [&str; 9] = [
    "WARRIOR", "PALADIN", "HUNTER", "ROGUE", "PRIEST", "SHAMAN", "MAGE", "WARLOCK", "DRUID",
];

pub fn first_name(n: &str) -> String {
    n.split([' ', '-']).next().unwrap_or(n).to_lowercase()
}
