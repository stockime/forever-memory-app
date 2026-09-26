//! The House: an account's characters as one household, what they share (the
//! stable, the menagerie, the Legacy), and its history told across them all.

use super::Model;
use super::deeds::Mine;
use super::diary;
use super::letters::Letter;
use super::memory::{Character, entries};
use serde_json::Value;
use std::collections::HashMap;

/// The client's header over the professions in the skill list, in the
/// languages it ships.
const PROFESSIONS: [&str; 6] = [
    "Professions",
    "Berufe",
    "Métiers",
    "Profesiones",
    "Profissões",
    "专业技能",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Skill {
    pub name: String,
    pub rank: i64,
    pub max: i64,
}

/// The professions in a character's last snapshot, as the skill list has them.
pub fn professions(c: &Character) -> Vec<Skill> {
    let mut on = false;
    let mut out = vec![];
    for (_, s) in c.snapshot.get("skills").map(entries).unwrap_or_default() {
        let name = s.get("name").and_then(Value::as_str).unwrap_or("");
        if s.get("isHeader").and_then(Value::as_bool) == Some(true) {
            on = PROFESSIONS.contains(&name);
        } else if on && !name.is_empty() {
            let n = |k: &str| s.get(k).and_then(Value::as_i64).unwrap_or(0);
            out.push(Skill {
                name: name.to_string(),
                rank: n("rank"),
                max: n("maxRank"),
            });
        }
    }
    out
}

/// Who heads the House: the most time played, then the highest level.
pub fn head(m: &Model) -> Option<usize> {
    let chars = &m.memory.characters;
    (0..chars.len()).max_by_key(|&i| {
        let c = &chars[i];
        (c.total_play(), c.level, std::cmp::Reverse(i))
    })
}

pub fn surname(c: &Character) -> String {
    let s = c.snapshot.get("surname").and_then(Value::as_str).unwrap_or("").trim();
    if !s.is_empty() {
        return s.to_string();
    }
    match c.name.split_once(' ') {
        Some((_, rest)) if !rest.trim().is_empty() => rest.trim().to_string(),
        _ => String::new(),
    }
}

/// When a character was last in the world.
pub fn last_seen(c: &Character) -> i64 {
    let snap = ["loggedOut", "updated"]
        .iter()
        .filter_map(|k| c.snapshot.get(*k).and_then(Value::as_i64))
        .max()
        .unwrap_or(0);
    c.sessions.last().map(|s| s.end).unwrap_or(0).max(snap)
}

/// The Legacy is the account's; the latest snapshot has the latest of it.
pub fn legacy_source(m: &Model) -> Option<&Character> {
    m.memory
        .characters
        .iter()
        .filter(|c| c.snapshot.pointer("/legacy/trees").is_some())
        .max_by_key(|c| c.snapshot.get("updated").and_then(Value::as_i64).unwrap_or(0))
}

/// Legacy points (spent, earned) from a snapshot: every tree reports the
/// same account-wide currency.
pub fn legacy_points(c: &Character) -> Option<(i64, i64)> {
    let trees = c.snapshot.pointer("/legacy/trees").map(entries)?;
    let (_, t) = trees.first()?;
    let cur = entries(t.get("currency")?).into_iter().next()?.1;
    let n = |k: &str| cur.get(k).and_then(Value::as_i64).unwrap_or(0);
    Some((n("spent"), n("spent") + n("quantity")))
}

/// The Legacy's learned nodes by name, as the latest snapshot has them.
pub fn legacy_learned(c: &Character) -> Vec<String> {
    c.snapshot
        .pointer("/legacy/trees")
        .map(entries)
        .unwrap_or_default()
        .into_iter()
        .flat_map(|(_, t)| t.get("nodes").map(entries).unwrap_or_default())
        .filter(|(_, n)| n.get("ranks").and_then(Value::as_i64).unwrap_or(0) > 0)
        .filter_map(|(_, n)| n.pointer("/entries/0/name").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

/// The first lines of a character's latest diary entry, and its day.
pub fn latest_words(c: &Character, n: usize) -> Option<(String, String)> {
    let (day, stored) = c.diary.iter().next_back()?;
    let prose = diary::split(stored).0;
    let body: Vec<&str> = prose
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let text = body.join(" ").replace(['*', '_'], "");
    // Whole sentences while they fit.
    let mut out = String::new();
    for s in text.split_inclusive(['.', '!', '?']) {
        if !out.is_empty() && out.chars().count() + s.chars().count() > n {
            break;
        }
        out.push_str(s);
    }
    let out = diary::clip(out.trim(), n);
    (!out.is_empty()).then(|| (day.clone(), out))
}

/// One moment in the House's history. `who` is a character index.
#[derive(Clone, Debug, PartialEq)]
pub struct Moment {
    pub t: i64,
    pub who: Option<usize>,
    pub what: What,
}

#[derive(Clone, Debug, PartialEq)]
pub enum What {
    /// First seen in the world; `first` for the House's very first.
    Joined { zone: String, first: bool },
    Level(i64),
    FirstDeath { zone: String },
    Mount { name: String, icon: Option<i64> },
    Pet { name: String, icon: Option<i64> },
    Letter { to: usize, from_slug: String, to_slug: String, opening: String },
    Deed { name: String, icon: i64 },
    Slew { name: String },
}

/// An earned deed: (character index, name, icon, when).
pub type Earned = (usize, String, i64, f64);

/// Everything worth telling across the characters, newest first.
pub fn timeline(m: &Model, letters: &[Letter], deeds: &[Earned]) -> Vec<Moment> {
    let chars = &m.memory.characters;
    let mut out = vec![];
    let founder = (0..chars.len())
        .filter(|&i| !chars[i].sessions.is_empty())
        .min_by_key(|&i| chars[i].sessions[0].start);
    for (i, c) in chars.iter().enumerate() {
        let who = Some(i);
        if let Some(s) = c.sessions.first() {
            let zone = c.events[s.from..s.to]
                .iter()
                .find_map(|e| e.s("zone").filter(|z| !z.is_empty()))
                .unwrap_or("")
                .to_string();
            out.push(Moment {
                t: s.start,
                who,
                what: What::Joined { zone, first: founder == Some(i) },
            });
        }
        let (mut died, mut zone) = (false, "");
        for e in &c.events {
            if matches!(e.e.as_str(), "login" | "zone" | "subzone")
                && let Some(z) = e.s("zone").filter(|z| !z.is_empty())
            {
                zone = z;
            }
            match e.e.as_str() {
                "level" => {
                    if let Some(l) = e.i("level").filter(|l| l % 10 == 0) {
                        out.push(Moment { t: e.t, who, what: What::Level(l) });
                    }
                }
                "death" if !died => {
                    died = true;
                    let zone = e.s("zone").filter(|z| !z.is_empty()).unwrap_or(zone).to_string();
                    out.push(Moment { t: e.t, who, what: What::FirstDeath { zone } });
                }
                _ => {}
            }
        }
        for (t, name) in notable_kills(m, c) {
            out.push(Moment { t: t as i64, who, what: What::Slew { name } });
        }
    }
    for a in &m.memory.account {
        let who = chars.iter().position(|c| !a.by.is_empty() && c.guid == a.by);
        let (name, icon) = (a.name.clone(), a.icon);
        let what = if a.kind == "mounts" {
            What::Mount { name, icon }
        } else {
            What::Pet { name, icon }
        };
        if a.first > 0 {
            out.push(Moment { t: a.first, who, what });
        }
    }
    let index = |slug: &str| chars.iter().position(|c| c.slug == slug);
    for l in letters {
        let (Some(from), Some(to)) = (index(&l.from), index(&l.to)) else {
            continue;
        };
        out.push(Moment {
            t: written(l),
            who: Some(from),
            what: What::Letter {
                to,
                from_slug: l.from.clone(),
                to_slug: l.to.clone(),
                opening: opening(&l.text),
            },
        });
    }
    for (i, name, icon, t) in deeds {
        out.push(Moment {
            t: *t as i64,
            who: Some(*i),
            what: What::Deed { name: name.clone(), icon: *icon },
        });
    }
    out.sort_by_key(|m| std::cmp::Reverse(m.t));
    out
}

/// When a letter was written ("2026-09-26 16:11"), or its day at noon.
fn written(l: &Letter) -> i64 {
    use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone};
    let at = NaiveDateTime::parse_from_str(&l.written, "%Y-%m-%d %H:%M").ok().or_else(|| {
        NaiveDate::parse_from_str(&l.day, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(12, 0, 0))
    });
    at.and_then(|t| Local.from_local_datetime(&t).earliest())
        .map(|t| t.timestamp())
        .unwrap_or(0)
}

/// A letter's first line after the greeting, for a glimpse.
fn opening(text: &str) -> String {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next().unwrap_or("");
    let body = if first.ends_with(',') || first.chars().count() < 30 {
        lines.next().unwrap_or(first)
    } else {
        first
    };
    diary::clip(body, 160)
}

/// The kills that stood out: named foes (felled a few times at most) that took far
/// more than a common beast to bring down. The logs carry no creature rank,
/// so a boss is known by what it cost.
pub fn notable_kills(m: &Model, c: &Character) -> Vec<(f64, String)> {
    let mine = Mine::new(m, c);
    if mine.kills.len() < 5 {
        return vec![];
    }
    let cb = &m.combat;
    // Per unit: the damage dealt to it and when the fight with it began.
    let mut fought: HashMap<u32, (i64, f64)> = HashMap::new();
    for h in &mine.dealt {
        let e = fought.entry(h.dst).or_insert((0, h.t));
        e.0 += h.amount;
        e.1 = e.1.min(h.t);
    }
    for h in &mine.taken {
        if let Some(e) = fought.get_mut(&h.src) {
            e.1 = e.1.min(h.t);
        }
    }
    let mut times: HashMap<&str, usize> = HashMap::new();
    for (_, u) in &mine.kills {
        *times.entry(cb.unit_name(*u)).or_default() += 1;
    }
    let cost = |t: f64, u: u32| {
        fought
            .get(&u)
            .map(|(d, from)| (*d as f64, (t - from).max(0.0)))
            .unwrap_or((0.0, 0.0))
    };
    let median = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let dmg = median(mine.kills.iter().map(|(t, u)| cost(*t, *u).0).collect());
    let secs = median(mine.kills.iter().map(|(t, u)| cost(*t, *u).1).collect());
    let named = |u: u32| {
        let kind = cb.units.get(u as usize).map(|x| x.kind);
        kind == Some(super::combat::UnitKind::Creature)
            && times.get(cb.unit_name(u)).copied() <= Some(4)
    };
    let mut picked: Vec<(f64, u32)> = mine
        .kills
        .iter()
        .copied()
        .filter(|&(_, u)| named(u))
        .filter(|&(t, u)| {
            let (d, s) = cost(t, u);
            (dmg > 0.0 && d >= dmg * 2.5) || (secs > 0.0 && s >= secs * 3.0 && d >= dmg)
        })
        .collect();
    // In a group the damage is shared, so in a dungeon the hardest named
    // kill of the run counts too.
    for run in mine.runs() {
        let best = mine
            .kills
            .iter()
            .copied()
            .filter(|&(t, u)| t >= run.start && t <= run.end && named(u))
            .map(|(t, u)| (cost(t, u).0, t, u))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, t, u)) = best.filter(|b| b.0 >= dmg * 1.5) {
            picked.push((t, u));
        }
    }
    picked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, String)> = vec![];
    for (t, u) in picked {
        let name = cb.unit_name(u);
        // Felled again later: the first time is the story.
        if !out.iter().any(|(_, n)| n == name) {
            out.push((t, name.to_string()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openings_skip_the_greeting() {
        assert_eq!(opening("Usain,\n\nThe trainer looked me over."), "The trainer looked me over.");
        assert_eq!(
            opening("I have been meaning to write to you for a long while now.\n\nMore."),
            "I have been meaning to write to you for a long while now."
        );
    }

    #[test]
    fn surnames() {
        let c = Character { name: "Tom Crusader".into(), ..Default::default() };
        assert_eq!(surname(&c), "Crusader");
        let c = Character { name: "Tom".into(), ..Default::default() };
        assert_eq!(surname(&c), "");
    }

    #[test]
    fn diary_words_are_whole_sentences() {
        let mut c = Character::default();
        c.diary.insert(
            "2026-09-24".into(),
            "# A Title\n\nI sold what I had. The others found me. We went down again.".into(),
        );
        let (day, words) = latest_words(&c, 40).unwrap();
        assert_eq!(day, "2026-09-24");
        assert_eq!(words, "I sold what I had. The others found me.");
    }
}

