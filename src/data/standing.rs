//! Where a character stands with the factions of the world: the reputation
//! frame as last saved (addon 0.4.0), each faction's history from the logged
//! changes, and a line that says it in words. Before 0.4.0 only the chat
//! lines were kept ("Your reputation with Undercity has increased by 50."),
//! so those fill in the history from before.

use super::memory::{Character, Faction};
use crate::tr;
use std::collections::{BTreeMap, HashMap};

/// Where each standing begins, Hated (1) to Exalted (8), and the top.
pub const THRESHOLDS: [i64; 9] = [-42000, -6000, -3000, 0, 3000, 9000, 21000, 42000, 43000];

pub fn standing_of(value: i64) -> i64 {
    (1..=8)
        .rev()
        .find(|&s| value >= THRESHOLDS[s as usize - 1])
        .unwrap_or(1)
}

/// The standing's name, as the game calls it.
pub fn label(standing: i64) -> &'static str {
    match standing {
        1 => tr!("Hated"),
        2 => tr!("Hostile"),
        3 => tr!("Unfriendly"),
        4 => tr!("Neutral"),
        5 => tr!("Friendly"),
        6 => tr!("Honored"),
        7 => tr!("Revered"),
        _ => tr!("Exalted"),
    }
}

/// The faction a race calls home (by faction ID).
pub fn home(race_file: &str) -> Option<i64> {
    Some(match race_file {
        "Scourge" | "Undead" => 68,
        "Orc" => 76,
        "Tauren" => 81,
        "Troll" => 530,
        "Human" => 72,
        "Dwarf" => 47,
        "Gnome" => 54,
        "NightElf" => 69,
        _ => return None,
    })
}

/// A game icon (file ID) for a faction: the capitals' teleport runes, the
/// orders' tokens; the reputation seal for the rest.
pub fn icon(id: i64) -> i64 {
    match id {
        68 => 135766,                   // Undercity
        76 => 135759,                   // Orgrimmar
        81 => 135765,                   // Thunder Bluff
        530 => 134178,                  // Darkspear Trolls
        72 => 135763,                   // Stormwind
        47 => 135757,                   // Ironforge
        69 => 135755,                   // Darnassus
        54 => 134164,                   // Gnomeregan
        56 => 134503,                   // Scarlet Crusade
        529 => 134499,                  // Argent Dawn
        576 => 236696,                  // Timbermaw Hold
        609 => 135758,                  // Cenarion Circle
        729 => 133283,                  // Frostwolf Clan
        730 => 133429,                  // Stormpike Guard
        909 => 134481,                  // Darkmoon Faire
        59 => 133235,                   // Thorium Brotherhood
        21 | 369 | 470 | 577 => 133784, // the Steamwheedle towns
        _ => 236683,
    }
}

pub struct Row {
    pub faction: Faction,
    /// False when only chat lines tell of it: the value is then the sum of
    /// the recorded changes, not the real standing.
    pub known: bool,
    pub home: bool,
    /// (time, value after), oldest first, starting just before the first change.
    pub history: Vec<(i64, i64)>,
    /// All recorded changes added up.
    pub gained: i64,
    /// When each standing was reached, as far as the record goes.
    pub reached: BTreeMap<i64, i64>,
}

impl Row {
    /// When the current standing was reached, if the record saw it happen.
    pub fn since(&self) -> Option<i64> {
        self.reached.get(&self.faction.standing).copied()
    }
}

pub struct Group {
    pub name: String,
    pub rows: Vec<Row>,
}

/// "Your reputation with Undercity has increased by 50." and "You are now
/// Honored with Undercity.", as English clients write them.
fn parse_msg(text: &str) -> Option<Msg> {
    let text = text.trim().trim_end_matches('.');
    if let Some(rest) = text.strip_prefix("Your reputation with ") {
        for (word, sign) in [(" has increased by ", 1), (" has decreased by ", -1)] {
            if let Some((name, n)) = rest.split_once(word) {
                let n: i64 = n.trim().replace(',', "").parse().ok()?;
                return Some(Msg::Change(name.to_string(), sign * n));
            }
        }
    }
    let rest = text.strip_prefix("You are now ")?;
    let (word, name) = rest.split_once(" with ")?;
    let standing = [
        "Hated",
        "Hostile",
        "Unfriendly",
        "Neutral",
        "Friendly",
        "Honored",
        "Revered",
        "Exalted",
    ]
    .iter()
    .position(|s| *s == word)? as i64
        + 1;
    Some(Msg::Now(name.to_string(), standing))
}

enum Msg {
    Change(String, i64),
    Now(String, i64),
}

/// A change: when, by how much, and the value after when the addon said.
struct Change {
    t: i64,
    d: i64,
    value: Option<i64>,
}

pub fn build(c: &Character) -> Vec<Group> {
    let home_id = c
        .snapshot
        .get("raceFile")
        .and_then(serde_json::Value::as_str)
        .and_then(home);
    // Once the addon logs changes itself, its chat lines only repeat them.
    let cutoff = c
        .events
        .iter()
        .find(|e| e.e == "rep")
        .map(|e| e.t - 5)
        .unwrap_or(i64::MAX);
    let names: HashMap<i64, &str> = c
        .reputation
        .iter()
        .map(|f| (f.id, f.name.as_str()))
        .collect();
    let mut changes: HashMap<String, Vec<Change>> = HashMap::new();
    let mut announced: HashMap<String, BTreeMap<i64, i64>> = HashMap::new();
    for e in &c.events {
        match e.e.as_str() {
            "rep" => {
                let name = e
                    .i("id")
                    .and_then(|id| names.get(&id).map(|n| n.to_string()))
                    .or_else(|| e.s("faction").map(str::to_string));
                let Some(name) = name else { continue };
                let value = e.i("value");
                if let Some(d) = e.i("d") {
                    changes
                        .entry(name)
                        .or_default()
                        .push(Change { t: e.t, d, value });
                }
            }
            "msg" if e.s("kind") == Some("rep") && e.t < cutoff => {
                match e.s("text").and_then(parse_msg) {
                    Some(Msg::Change(name, d)) => changes.entry(name).or_default().push(Change {
                        t: e.t,
                        d,
                        value: None,
                    }),
                    Some(Msg::Now(name, s)) => {
                        announced.entry(name).or_default().entry(s).or_insert(e.t);
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }

    let row = |f: Faction, known: bool, changes: &mut HashMap<String, Vec<Change>>| {
        let list = changes.remove(&f.name).unwrap_or_default();
        let gained = list.iter().map(|c| c.d).sum();
        // Walk back from the value now to find the value after each change.
        let mut after = vec![0; list.len()];
        let mut v = if known { f.value } else { gained };
        for (i, ch) in list.iter().enumerate().rev() {
            if known && let Some(x) = ch.value {
                v = x;
            }
            after[i] = v;
            v -= ch.d;
        }
        let mut history = vec![];
        if let Some(first) = list.first() {
            history.push((first.t - 1, v));
        }
        history.extend(list.iter().zip(&after).map(|(ch, &a)| (ch.t, a)));
        let mut reached: BTreeMap<i64, i64> = BTreeMap::new();
        if known {
            for w in history.windows(2) {
                let (a, b) = (standing_of(w[0].1), standing_of(w[1].1));
                if a != b {
                    reached.entry(b).or_insert(w[1].0);
                }
            }
            // The addon notes the standing it first saw too; that is only
            // news when it came after the first look.
            for (&s, &t) in &f.reached {
                if t > f.since + 60 {
                    let e = reached.entry(s).or_insert(t);
                    *e = (*e).min(t);
                }
            }
        }
        if let Some(a) = announced.get(&f.name) {
            for (&s, &t) in a {
                let e = reached.entry(s).or_insert(t);
                *e = (*e).min(t);
            }
        }
        Row {
            home: Some(f.id) == home_id,
            known,
            history,
            gained,
            reached,
            faction: f,
        }
    };

    let mut groups: Vec<Group> = vec![];
    for f in &c.reputation {
        let r = row(f.clone(), true, &mut changes);
        match groups.iter_mut().find(|g| g.name == f.group) {
            Some(g) => g.rows.push(r),
            None => groups.push(Group {
                name: f.group.clone(),
                rows: vec![r],
            }),
        }
    }
    // Factions only the chat lines speak of (before the addon kept standings).
    let mut rest: Vec<String> = changes.keys().cloned().collect();
    rest.sort();
    let heard: Vec<Row> = rest
        .into_iter()
        .map(|name| {
            let f = Faction {
                name,
                ..Default::default()
            };
            row(f, false, &mut changes)
        })
        .collect();
    if !heard.is_empty() {
        groups.push(Group {
            name: String::new(),
            rows: heard,
        });
    }
    groups
}

/// A line in words: "Known to Undercity as Honored; tolerated by Orgrimmar;
/// Scarlet Crusade wants you dead."
pub fn summary(groups: &[Group]) -> Option<String> {
    let rows: Vec<&Row> = groups
        .iter()
        .flat_map(|g| &g.rows)
        .filter(|r| r.known)
        .collect();
    let mut clauses = vec![];
    let mut used = vec![];
    for r in rows.iter().filter(|r| r.home) {
        clauses.push(tr!(
            "Known to {faction} as {standing}",
            faction = r.faction.name,
            standing = label(r.faction.standing)
        ));
        used.push(r.faction.id);
    }
    let others: Vec<&&Row> = rows.iter().filter(|r| !r.home).collect();
    if let Some(best) = others
        .iter()
        .max_by_key(|r| (r.faction.standing, r.faction.value))
    {
        clauses.push(phrase(&best.faction));
        used.push(best.faction.id);
    }
    if let Some(worst) = others
        .iter()
        .filter(|r| !used.contains(&r.faction.id))
        .min_by_key(|r| (r.faction.standing, r.faction.value))
        .filter(|r| r.faction.standing <= 3 || r.faction.war)
    {
        clauses.push(phrase(&worst.faction));
    }
    let first = clauses.first_mut()?;
    let mut chars = first.chars();
    if let Some(c) = chars.next() {
        *first = c.to_uppercase().chain(chars).collect();
    }
    Some(format!("{}.", clauses.join("; ")))
}

fn phrase(f: &Faction) -> String {
    let faction = &f.name;
    match f.standing {
        8 => tr!("a hero to {faction}", faction = faction),
        7 => tr!("revered by {faction}", faction = faction),
        6 => tr!("honoured by {faction}", faction = faction),
        5 => tr!("a friend to {faction}", faction = faction),
        4 => tr!("tolerated by {faction}", faction = faction),
        3 => tr!("distrusted by {faction}", faction = faction),
        2 => tr!("{faction} would rather see you gone", faction = faction),
        _ => tr!("{faction} wants you dead", faction = faction),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::memory::Event;
    use serde_json::json;

    fn ev(n: i64, t: i64, e: &str, v: serde_json::Value) -> Event {
        Event {
            n,
            t,
            e: e.into(),
            v,
        }
    }

    #[test]
    fn history_from_chat_lines_and_events() {
        let mut c = Character {
            snapshot: json!({ "raceFile": "Scourge" }),
            reputation: vec![Faction {
                id: 68,
                name: "Undercity".into(),
                group: "Horde".into(),
                standing: 6,
                value: 9100,
                min: 9000,
                max: 21000,
                since: 200,
                ..Default::default()
            }],
            ..Default::default()
        };
        let msg = |t, text: &str| json!({ "kind": "rep", "text": text, "t": t });
        c.events = vec![
            ev(
                1,
                100,
                "msg",
                msg(100, "Your reputation with Undercity has increased by 250."),
            ),
            ev(
                2,
                110,
                "msg",
                msg(110, "Your reputation with Orgrimmar has increased by 25."),
            ),
            ev(
                3,
                300,
                "msg",
                msg(300, "Your reputation with Undercity has increased by 150."),
            ),
            ev(4, 301, "rep", json!({ "id": 68, "d": 150, "value": 9100 })),
        ];
        let groups = build(&c);
        assert_eq!(groups.len(), 2);
        let uc = &groups[0].rows[0];
        assert!(uc.home && uc.known);
        assert_eq!(uc.gained, 400);
        assert_eq!(uc.history, vec![(99, 8700), (100, 8950), (301, 9100)]);
        assert_eq!(uc.since(), Some(301));
        let og = &groups[1].rows[0];
        assert!(!og.known);
        assert_eq!(og.faction.name, "Orgrimmar");
        assert_eq!(og.gained, 25);
        assert_eq!(
            summary(&groups).as_deref(),
            Some("Known to Undercity as Honored.")
        );
    }

    #[test]
    fn standings() {
        assert_eq!(standing_of(-42000), 1);
        assert_eq!(standing_of(-3001), 2);
        assert_eq!(standing_of(0), 4);
        assert_eq!(standing_of(8999), 5);
        assert_eq!(standing_of(42999), 8);
    }
}
