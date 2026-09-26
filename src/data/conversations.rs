//! Conversations: what NPCs said to a character, gathered into scenes from
//! the gossip the addon kept, the quests taken and handed in right after
//! speaking to someone, and what NPCs said and yelled nearby (chat log).

use super::Model;
use super::chat::Kind;
use super::memory::{Character, Gossip, Quest};
use std::collections::{HashMap, HashSet};

/// One line of a scene. The NPC speaks the first six, the character the rest.
#[derive(Clone, Debug, PartialEq)]
pub enum Said {
    Greeting(String),
    Offer {
        title: String,
        text: String,
        objective: String,
    },
    Progress(String),
    Reward(String),
    Say(String),
    Yell(String),
    Chose(String),
    Accepted(String),
    TurnedIn {
        title: String,
        /// The reward picked, as an item link.
        choice: Option<String>,
    },
}

impl Said {
    pub fn by_npc(&self) -> bool {
        !matches!(
            self,
            Said::Chose(_) | Said::Accepted(_) | Said::TurnedIn { .. }
        )
    }
    /// The text a typewriter reveals.
    pub fn text(&self) -> &str {
        match self {
            Said::Greeting(t)
            | Said::Progress(t)
            | Said::Reward(t)
            | Said::Say(t)
            | Said::Yell(t) => t,
            Said::Chose(t) | Said::Accepted(t) => t,
            Said::Offer { text, .. } => text,
            Said::TurnedIn { title, .. } => title,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub start: i64,
    pub end: i64,
    pub place: String,
    pub lines: Vec<(f64, Said)>,
}

#[derive(Clone, Debug, Default)]
pub struct Npc {
    pub name: String,
    /// Where they were met most often.
    pub place: String,
    pub first: i64,
    pub last: i64,
    pub scenes: Vec<Scene>,
}

/// A quest taken or handed in this long after speaking to someone came from them.
const WITHIN: i64 = 120;
/// Speech this close to a scene belongs to it; farther apart it is a new one.
const GAP: f64 = 180.0;

/// The greeting an NPC gave around time t: the text last shown closest after
/// it (the addon keeps each text with the last time it was shown).
fn greeting<'a>(gossip: &'a [Gossip], npc: Option<i64>, name: &str, t: i64) -> Option<&'a Gossip> {
    let theirs: Vec<&Gossip> = gossip
        .iter()
        .filter(|g| match (npc, g.npc) {
            (Some(a), Some(b)) => a == b,
            _ => g.name == name,
        })
        .collect();
    theirs
        .iter()
        .filter(|g| g.seen >= t - 10)
        .min_by_key(|g| g.seen)
        .or_else(|| theirs.iter().max_by_key(|g| g.seen))
        .copied()
}

pub fn build(m: &Model, c: &Character) -> Vec<Npc> {
    let quests: HashMap<i64, &Quest> = c.quests.iter().map(|q| (q.id, q)).collect();
    let mut places: Vec<(i64, String)> = vec![];
    let mut zone = String::new();
    let mut scenes: Vec<(String, Scene)> = vec![];
    // The scene open right now: its index and the last time anything was said.
    let mut open: Option<(usize, i64)> = None;
    for e in &c.events {
        match e.e.as_str() {
            "login" | "zone" | "subzone" => {
                if let Some(z) = e.s("zone") {
                    zone = z.to_string();
                }
                let sub = e.s("sub").unwrap_or("");
                let place = if sub.is_empty() || sub == zone {
                    zone.clone()
                } else {
                    format!("{sub}, {zone}")
                };
                places.push((e.t, place));
            }
            "gossip" | "gossip_pick" => {
                let name = e.s("name").unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }
                let current = open.filter(|(i, last)| scenes[*i].0 == name && e.t - last < 300);
                let i = match current {
                    Some((i, _)) => i,
                    None => {
                        scenes.push((
                            name.clone(),
                            Scene {
                                start: e.t,
                                place: places.last().map(|p| p.1.clone()).unwrap_or_default(),
                                ..Default::default()
                            },
                        ));
                        scenes.len() - 1
                    }
                };
                let s = &mut scenes[i].1;
                if e.e == "gossip" {
                    if let Some(g) = greeting(&m.memory.gossip, e.i("npc"), &name, e.t)
                        && !s
                            .lines
                            .iter()
                            .any(|(_, l)| *l == Said::Greeting(g.text.clone()))
                    {
                        s.lines.push((e.t as f64, Said::Greeting(g.text.clone())));
                    }
                } else if let Some(o) = e.s("option") {
                    s.lines.push((e.t as f64, Said::Chose(o.to_string())));
                }
                s.end = e.t;
                open = Some((i, e.t));
            }
            "quest" => {
                let Some((i, last)) = open.filter(|(_, last)| e.t - last <= WITHIN) else {
                    continue;
                };
                let q = e.i("id").and_then(|id| quests.get(&id));
                let title = q
                    .map(|q| q.title.clone())
                    .or_else(|| e.s("title").map(str::to_string))
                    .unwrap_or_default();
                let s = &mut scenes[i].1;
                let t = e.t as f64;
                match e.s("act") {
                    Some("accept") => {
                        if let Some(q) = q.filter(|q| !q.text.is_empty()) {
                            s.lines.push((
                                t,
                                Said::Offer {
                                    title: title.clone(),
                                    text: q.text.clone(),
                                    objective: q.objective.clone(),
                                },
                            ));
                        }
                        s.lines.push((t, Said::Accepted(title)));
                    }
                    Some("turnin") => {
                        if let Some(q) = q {
                            if !q.progress.trim().is_empty() {
                                s.lines.push((t, Said::Progress(q.progress.clone())));
                            }
                            if !q.reward.trim().is_empty() {
                                s.lines.push((t, Said::Reward(q.reward.clone())));
                            }
                        }
                        s.lines.push((
                            t,
                            Said::TurnedIn {
                                title,
                                choice: e.s("choice").map(str::to_string),
                            },
                        ));
                    }
                    _ => continue,
                }
                s.end = e.t;
                open = Some((i, last.max(e.t)));
            }
            _ => {}
        }
    }

    // NPC speech from the chat log, while this character was playing.
    let players: HashSet<String> = m
        .players
        .iter()
        .map(|p| p.first.to_lowercase())
        .chain(
            m.memory
                .characters
                .iter()
                .map(|c| super::players::first_name(&c.name)),
        )
        .collect();
    let playing = |t: f64| {
        c.sessions
            .iter()
            .any(|s| t >= s.start as f64 - 5.0 && t <= s.end as f64 + 5.0)
    };
    let place_at = |t: i64| {
        let i = places.partition_point(|p| p.0 <= t);
        places[..i].last().map(|p| p.1.clone()).unwrap_or_default()
    };
    for l in &m.chat {
        let said = match l.kind {
            Kind::Say => Said::Say(l.text.clone()),
            Kind::Yell => Said::Yell(l.text.clone()),
            _ => continue,
        };
        let Some(who) = &l.speaker else { continue };
        if players.contains(&super::players::first_name(who)) || !playing(l.t) {
            continue;
        }
        let t = l.t as i64;
        let near = scenes.iter().rposition(|(n, s)| {
            n == who && l.t >= s.start as f64 - GAP && l.t <= s.end as f64 + GAP
        });
        let i = match near {
            Some(i) => i,
            None => {
                scenes.push((
                    who.clone(),
                    Scene {
                        start: t,
                        place: place_at(t),
                        ..Default::default()
                    },
                ));
                scenes.len() - 1
            }
        };
        let s = &mut scenes[i].1;
        if !s.lines.iter().any(|(_, x)| *x == said) {
            s.lines.push((l.t, said));
        }
        s.start = s.start.min(t);
        s.end = s.end.max(t);
    }

    let mut by: HashMap<String, Npc> = HashMap::new();
    for (name, mut s) in scenes {
        if s.lines.is_empty() {
            continue;
        }
        s.lines.sort_by(|a, b| a.0.total_cmp(&b.0));
        let n = by.entry(name.clone()).or_insert_with(|| Npc {
            name,
            first: s.start,
            ..Default::default()
        });
        n.first = n.first.min(s.start);
        n.last = n.last.max(s.end);
        n.scenes.push(s);
    }
    let mut out: Vec<Npc> = by
        .into_values()
        .map(|mut n| {
            n.scenes.sort_by_key(|s| s.start);
            let mut count: HashMap<&str, usize> = HashMap::new();
            for s in &n.scenes {
                *count.entry(s.place.as_str()).or_default() += 1;
            }
            n.place = count
                .into_iter()
                .max_by_key(|(p, k)| (*k, std::cmp::Reverse(*p)))
                .map(|(p, _)| p.to_string())
                .unwrap_or_default();
            n
        })
        .collect();
    out.sort_by(|a, b| b.last.cmp(&a.last).then_with(|| a.name.cmp(&b.name)));
    out
}
