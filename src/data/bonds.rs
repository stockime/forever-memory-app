//! Bonds with others: the fellowship (who a character travelled with, for
//! how long, and what passed between them) and the nemeses (who killed them,
//! how often, and whether they took their revenge).

use super::Model;
use super::memory::Character;
use super::players::first_name;
use std::collections::HashMap;

pub struct Companion {
    pub name: String,
    pub class: Option<&'static str>,
    /// Seconds spent in the same group.
    pub together: i64,
    pub days: usize,
    pub since: i64,
    pub last: i64,
    /// Creatures that fell while they were at the character's side.
    pub kills: usize,
    pub healed_me: i64,
    pub healed_them: i64,
}

pub struct Nemesis {
    pub name: String,
    pub player: bool,
    pub class: Option<&'static str>,
    /// Death times (unix seconds) they caused.
    pub deaths: Vec<i64>,
    pub places: Vec<String>,
    /// When the character first struck one of them down after the last death.
    pub avenged: Option<f64>,
    /// Every one of them the character has killed, before or after.
    pub slain: usize,
}

/// Group spells from the group events: (from, to, members) in unix seconds.
fn group_spans(c: &Character) -> Vec<(i64, i64, Vec<String>)> {
    let mut spans = vec![];
    let mut open: Option<(i64, Vec<String>)> = None;
    let me = first_name(&c.name);
    for e in &c.events {
        let close = |open: &mut Option<(i64, Vec<String>)>, t: i64, spans: &mut Vec<_>| {
            if let Some((from, members)) = open.take()
                && t > from {
                    spans.push((from, t, members));
                }
        };
        match e.e.as_str() {
            "group" => {
                close(&mut open, e.t, &mut spans);
                let members: Vec<String> = e
                    .v
                    .get("members")
                    .and_then(|m| m.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str())
                            .map(first_name)
                            .filter(|n| *n != me)
                            .collect()
                    })
                    .unwrap_or_default();
                if !members.is_empty() {
                    open = Some((e.t, members));
                }
            }
            "logout" => close(&mut open, e.t, &mut spans),
            _ => {}
        }
    }
    if let (Some((from, members)), Some(last)) = (open, c.events.last()) {
        spans.push((from, last.t, members));
    }
    spans
}

pub fn fellowship(m: &Model, c: &Character) -> Vec<Companion> {
    let spans = group_spans(c);
    let mut by: HashMap<String, Companion> = HashMap::new();
    for (from, to, members) in &spans {
        let kills = m
            .combat
            .kills
            .iter()
            .filter(|(t, _)| *t >= *from as f64 && *t <= *to as f64)
            .count();
        for first in members {
            let known = m.players.iter().find(|p| p.first.to_lowercase() == *first);
            let e = by.entry(first.clone()).or_insert_with(|| Companion {
                name: known.map(|p| p.name.clone()).unwrap_or_else(|| title(first)),
                class: known.and_then(|p| p.class),
                together: 0,
                days: 0,
                since: *from,
                last: *to,
                kills: 0,
                healed_me: 0,
                healed_them: 0,
            });
            e.together += to - from;
            e.since = e.since.min(*from);
            e.last = e.last.max(*to);
            e.kills += kills;
            if let Some(seen) = known.and_then(|p| p.unit).and_then(|u| m.combat.players.get(&u)) {
                e.healed_me = seen.heal_to_me;
                e.healed_them = seen.heal_from_me;
            }
        }
    }
    let mut out: Vec<Companion> = by.into_values().collect();
    for comp in &mut out {
        let first = first_name(&comp.name);
        let mut days: Vec<String> = spans
            .iter()
            .filter(|(_, _, m)| m.contains(&first))
            .map(|(f, _, _)| super::diary::day_of(*f))
            .collect();
        days.sort();
        days.dedup();
        comp.days = days.len();
    }
    out.sort_by(|a, b| b.together.cmp(&a.together).then_with(|| a.name.cmp(&b.name)));
    out
}

pub fn nemeses(m: &Model, c: &Character) -> Vec<Nemesis> {
    let mut by: HashMap<String, Nemesis> = HashMap::new();
    for e in c.events.iter().filter(|e| e.e == "death") {
        let t = e.t as f64;
        // The one who dealt the most in the moments before is the killer.
        let mut dealt: HashMap<u32, i64> = HashMap::new();
        for h in m.combat.taken.iter().filter(|h| h.t <= t + 2.0 && h.t > t - 12.0) {
            if h.src != u32::MAX {
                *dealt.entry(h.src).or_default() += h.amount;
            }
        }
        let Some((&unit, _)) = dealt.iter().max_by_key(|(u, n)| (**n, std::cmp::Reverse(**u))) else {
            continue;
        };
        let name = m.combat.unit_name(unit).to_string();
        let player = m.combat.units[unit as usize].kind == super::combat::UnitKind::Player;
        let n = by.entry(name.clone()).or_insert_with(|| Nemesis {
            class: m.players.iter().find(|p| p.unit == Some(unit)).and_then(|p| p.class),
            name,
            player,
            deaths: vec![],
            places: vec![],
            avenged: None,
            slain: 0,
        });
        n.deaths.push(e.t);
        let place = e.s("sub").filter(|s| !s.is_empty()).or(e.s("zone")).unwrap_or("").to_string();
        if !place.is_empty() && !n.places.contains(&place) {
            n.places.push(place);
        }
    }
    let (from, to) = (
        c.events.first().map(|e| e.t as f64).unwrap_or(0.0),
        c.events.last().map(|e| e.t as f64).unwrap_or(0.0),
    );
    for n in by.values_mut() {
        let last_death = *n.deaths.iter().max().unwrap_or(&0) as f64;
        for (t, u) in m.combat.kills.iter().filter(|(t, _)| *t >= from && *t <= to) {
            if m.combat.unit_name(*u) == n.name {
                n.slain += 1;
                if *t > last_death && n.avenged.is_none() {
                    n.avenged = Some(*t);
                }
            }
        }
    }
    let mut out: Vec<Nemesis> = by.into_values().collect();
    out.sort_by(|a, b| b.deaths.len().cmp(&a.deaths.len()).then_with(|| b.deaths.iter().max().cmp(&a.deaths.iter().max())));
    out
}

fn title(first: &str) -> String {
    let mut c = first.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
