//! The Book of the Dead: every death of a character, what struck them down in
//! the ten seconds before, how they came back, and the epitaph they write
//! about it themselves. Epitaphs live in the memory repository at
//! characters/<slug>/dead/<unix time>.md with the facts underneath, like the
//! diary.

use super::Model;
use super::memory::Character;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// One blow taken in the seconds before a death.
#[derive(Clone, Debug)]
pub struct Blow {
    /// Seconds before the death.
    pub before: f64,
    pub from: String,
    pub with: String,
    pub amount: i64,
    pub crit: bool,
}

/// Who dealt the damage, most first.
#[derive(Clone, Debug)]
pub struct Killer {
    pub name: String,
    pub damage: i64,
    pub blows: usize,
    pub with: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Return {
    /// Released the spirit and walked back as a ghost; seconds dead.
    Ghost(i64),
    /// Brought back where they fell; seconds dead.
    Raised(i64),
    Unknown,
}

#[derive(Clone, Debug)]
pub struct Death {
    pub t: i64,
    pub zone: String,
    pub sub: String,
    pub level: i64,
    /// 1 for the first death.
    pub nth: usize,
    /// Oldest first; the last one is the killing blow.
    pub blows: Vec<Blow>,
    pub killers: Vec<Killer>,
    pub back: Return,
    /// Tasks they were on in that zone when it happened.
    pub tasks: Vec<String>,
}

impl Death {
    pub fn place(&self) -> String {
        [self.sub.as_str(), self.zone.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }
    pub fn damage(&self) -> i64 {
        self.blows.iter().map(|b| b.amount).sum()
    }
}

/// Every death, oldest first.
pub fn deaths(m: &Model, c: &Character) -> Vec<Death> {
    let mut level = c.sessions.first().map(|s| s.level_from).unwrap_or(c.level);
    let mut out: Vec<Death> = vec![];
    for (i, e) in c.events.iter().enumerate() {
        match e.e.as_str() {
            "level" => level = e.i("level").unwrap_or(level),
            "death" => {
                let rest = &c.events[i + 1..];
                let until = rest
                    .iter()
                    .position(|x| x.e == "death")
                    .unwrap_or(rest.len());
                let rest = &rest[..until];
                let ghost = rest.iter().find(|x| x.e == "unghost");
                let alive = rest.iter().find(|x| x.e == "alive");
                let back = match (ghost, alive) {
                    (Some(g), _) => Return::Ghost(g.t - e.t),
                    (None, Some(a)) => Return::Raised(a.t - e.t),
                    _ => Return::Unknown,
                };
                // The combat log knows the moment more exactly than the addon.
                let t = e.t as f64;
                let at = m
                    .combat
                    .deaths
                    .iter()
                    .copied()
                    .filter(|d| (d - t).abs() <= 3.0)
                    .min_by(|a, b| (a - t).abs().total_cmp(&(b - t).abs()))
                    .unwrap_or(t);
                let blows: Vec<Blow> = m
                    .combat
                    .taken
                    .iter()
                    .filter(|h| h.t <= at + 0.5 && h.t > at - 10.0)
                    .map(|h| Blow {
                        before: (at - h.t).max(0.0),
                        from: m.combat.unit_name(h.src).to_string(),
                        with: m.combat.spell_name(h.spell).to_string(),
                        amount: h.amount,
                        crit: h.crit,
                    })
                    .collect();
                let mut by: BTreeMap<&str, Killer> = BTreeMap::new();
                for b in &blows {
                    let k = by.entry(&b.from).or_insert_with(|| Killer {
                        name: b.from.clone(),
                        damage: 0,
                        blows: 0,
                        with: vec![],
                    });
                    k.damage += b.amount;
                    k.blows += 1;
                    if !k.with.contains(&b.with) {
                        k.with.push(b.with.clone());
                    }
                }
                let mut killers: Vec<Killer> = by.into_values().collect();
                killers.sort_by_key(|k| std::cmp::Reverse(k.damage));
                let zone = e.s("zone").unwrap_or("").to_string();
                let tasks = c
                    .quests
                    .iter()
                    .filter(|q| {
                        q.accepted.is_some_and(|a| a <= e.t)
                            && q.done.is_none_or(|d| d > e.t)
                            && q.removed.is_none_or(|r| r > e.t)
                            && q.zone.as_deref() == Some(zone.as_str())
                    })
                    .map(|q| q.title.clone())
                    .take(3)
                    .collect();
                out.push(Death {
                    t: e.t,
                    sub: e.s("sub").unwrap_or("").to_string(),
                    zone,
                    level,
                    nth: out.len() + 1,
                    blows,
                    killers,
                    back,
                    tasks,
                });
            }
            _ => {}
        }
    }
    out
}

fn minutes(secs: i64) -> String {
    match secs {
        s if s < 90 => format!("{s} seconds"),
        s if s < 5400 => format!("{} minutes", (s + 30) / 60),
        s => format!("{} hours", (s + 1800) / 3600),
    }
}

/// What the epitaph may draw on, in plain English.
pub fn facts(c: &Character, d: &Death, all: usize) -> Vec<String> {
    let mut out = vec![format!(
        "{} died on {}, in {}, at level {}.",
        c.name,
        super::diary::pretty_day(&super::diary::day_of(d.t)),
        if d.place().is_empty() {
            "an unknown place".into()
        } else {
            d.place()
        },
        d.level
    )];
    out.push(match (d.nth, all) {
        (1, 1) => "It is the only death they have suffered so far.".into(),
        (1, _) => format!("It was their first death; {all} deaths are recorded in all."),
        (n, _) if n == all => format!("It is their latest death, number {n}."),
        (n, _) => format!("It was death number {n} of {all}."),
    });
    if !d.tasks.is_empty() {
        out.push(format!(
            "At the time they were about these tasks: {}.",
            d.tasks
                .iter()
                .map(|t| format!("\"{t}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if d.killers.is_empty() {
        out.push("Nothing in the combat log shows what struck them down.".into());
    } else {
        for k in &d.killers {
            out.push(format!(
                "In the last ten seconds, {} dealt {} damage in {} blow{} ({}).",
                k.name,
                k.damage,
                k.blows,
                if k.blows == 1 { "" } else { "s" },
                k.with.join(", ")
            ));
        }
        if let Some(b) = d.blows.last() {
            out.push(format!(
                "The killing blow: {}'s {} for {}{}.",
                b.from,
                b.with,
                b.amount,
                if b.crit { ", a critical strike" } else { "" }
            ));
        }
        out.push(format!(
            "Damage taken in those ten seconds: {} in all.",
            d.damage()
        ));
    }
    out.push(match d.back {
        Return::Ghost(s) => format!(
            "They released their spirit, walked back as a ghost and rose again about {} later.",
            minutes(s)
        ),
        Return::Raised(s) => format!(
            "They were brought back where they fell, about {} later.",
            minutes(s)
        ),
        Return::Unknown => "How they came back was not recorded.".into(),
    });
    out
}

pub const SYSTEM: &str = "You write an epitaph in Azeroth, the world of World of Warcraft (WoW Forever, which plays like Classic). A character looks back on one of their own deaths and writes about it themselves, in the first person and in their own voice, shaped by the personality note the player gives you.

Dark and wry: death is an old acquaintance (the Forsaken know it better than anyone) and they can laugh at it without making light of it. Name what killed them and where, and how they came back.

Stay true to what happened: everything must come from the facts. Add feeling and sensation, never invented people, places, fights or outcomes. Write from inside the world: the character has never heard of levels, experience, quests, damage numbers or players; a strong blow is felt, not counted.

Plain text, 40 to 120 words in all: first a single line of at most twelve words fit to be carved on a gravestone, then a blank line, then one short paragraph. No title, no Markdown, no preamble.";

pub fn prompt(c: &Character, facts: &[String]) -> String {
    let mut p = format!("The writer: {}, a {} {}.\n\n", c.name, c.race, c.class);
    let note = super::rp::who(c);
    p += &format!(
        "<personality>\n{}\n</personality>\n\n",
        note.as_deref()
            .unwrap_or("(No note yet: find a voice that fits their race and class.)")
    );
    p += "The death:\n<facts>\n";
    for f in facts {
        p += &format!("- {f}\n");
    }
    p += "</facts>\n\nWrite the epitaph for this death.";
    let lang = crate::i18n::current();
    if lang != crate::i18n::Lang::En {
        p += &format!(
            " Write it in {}, as a native speaker would, using the names the facts give for people, places and things.",
            lang.english_name()
        );
    }
    p
}

pub fn path(repo: &Path, c: &Character, t: i64) -> PathBuf {
    repo.join("characters")
        .join(&c.slug)
        .join("dead")
        .join(format!("{t}.md"))
}

/// The epitaphs written so far, by the time of the death, as stored.
pub fn load(repo: &Path, c: &Character) -> HashMap<i64, String> {
    let dir = repo.join("characters").join(&c.slug).join("dead");
    std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .filter_map(|f| {
                    let t = f
                        .file_name()
                        .to_string_lossy()
                        .strip_suffix(".md")?
                        .parse()
                        .ok()?;
                    Some((t, std::fs::read_to_string(f.path()).ok()?))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn store(
    repo: &Path,
    c: &Character,
    t: i64,
    entry: &str,
    facts: &[String],
) -> Result<(), String> {
    let p = path(repo, c, t);
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut out = format!(
        "{}\n\n{} written {} by {} from these facts -->\n\n## The facts behind this epitaph\n\n",
        entry.trim(),
        super::diary::MARKER,
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
        crate::claude::name()
    );
    for f in facts {
        out += &format!("- {f}\n");
    }
    std::fs::write(&p, out).map_err(|e| e.to_string())?;
    super::diary::commit(
        repo,
        &p,
        &format!(
            "epitaph: {}, {}",
            c.name,
            crate::theme::local(t as f64).format("%Y-%m-%d %H:%M")
        ),
    )
}

/// The epitaph's line for the stone and the paragraph under it.
pub fn inscription(prose: &str) -> (String, String) {
    let clean = |s: &str| {
        s.replace(['*', '_'], "")
            .trim()
            .trim_start_matches('#')
            .trim()
            .to_string()
    };
    let prose = prose.trim();
    match prose.split_once("\n\n") {
        Some((line, rest)) if line.split_whitespace().count() <= 16 => {
            (clean(line), clean(&rest.replace('\n', " ")))
        }
        _ => (String::new(), clean(&prose.replace('\n', " "))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inscription_splits_the_stone_line() {
        let (line, rest) =
            inscription("Here lies Tom, twice.\n\nThe gnolls were rude.\nVery rude.");
        assert_eq!(line, "Here lies Tom, twice.");
        assert_eq!(rest, "The gnolls were rude. Very rude.");
        let (line, rest) = inscription("Only a paragraph.");
        assert!(line.is_empty());
        assert_eq!(rest, "Only a paragraph.");
    }
}
