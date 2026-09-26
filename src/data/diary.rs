//! The diary: a day's recordings turned into facts, and entries written from
//! them in the character's own voice. Entries live in the memory repository at
//! characters/<slug>/diary/<date>.md with the facts they were written from
//! underneath, so every sentence can be checked.

use super::Model;
use super::chat::Kind;
use super::memory::{Character, Event, QuestStatus, parse_link};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const MARKER: &str = "<!-- forever-memory:";

pub fn day_of(t: i64) -> String {
    crate::theme::local(t as f64).format("%Y-%m-%d").to_string()
}

pub fn pretty_day(day: &str) -> String {
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(crate::theme::long_day)
        .unwrap_or_else(|_| day.to_string())
}

/// Days with play, newest first, with seconds played on each.
pub fn days(c: &Character) -> Vec<(String, i64)> {
    let mut by: BTreeMap<String, i64> = BTreeMap::new();
    for s in &c.sessions {
        *by.entry(day_of(s.start)).or_default() += s.seconds();
    }
    by.into_iter().rev().collect()
}

pub fn path(repo: &Path, c: &Character, day: &str) -> PathBuf {
    repo.join("characters")
        .join(&c.slug)
        .join("diary")
        .join(format!("{day}.md"))
}

/// Splits a stored entry into the prose and the facts it was written from.
pub fn split(stored: &str) -> (&str, &str) {
    match stored.find(MARKER) {
        Some(i) => (
            stored[..i].trim(),
            stored[i..]
                .split_once("-->")
                .map(|(_, f)| f.trim())
                .unwrap_or(""),
        ),
        None => (stored.trim(), ""),
    }
}

pub fn store(
    repo: &Path,
    c: &Character,
    day: &str,
    entry: &str,
    facts: &[String],
) -> Result<(), String> {
    let p = path(repo, c, day);
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut out = format!(
        "{}\n\n{MARKER} written {} by {} from these facts -->\n\n## The facts behind this entry\n\n",
        entry.trim(),
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
        crate::claude::name()
    );
    for f in facts {
        out += &format!("- {f}\n");
    }
    std::fs::write(&p, out).map_err(|e| e.to_string())?;
    commit(repo, &p, &format!("diary: {}, {day}", c.name))
}

/// The latest entry before `day`, read from disk so a batch of entries sees
/// the one it just wrote.
pub fn previous(repo: &Path, c: &Character, day: &str) -> Option<(String, String)> {
    let dir = repo.join("characters").join(&c.slug).join("diary");
    let mut days: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter_map(|f| {
            f.file_name()
                .to_string_lossy()
                .strip_suffix(".md")
                .map(str::to_string)
        })
        .filter(|d| d.as_str() < day)
        .collect();
    days.sort();
    let d = days.pop()?;
    let text = std::fs::read_to_string(dir.join(format!("{d}.md"))).ok()?;
    Some((d, split(&text).0.to_string()))
}

pub fn save_personality(repo: &Path, c: &Character, text: &str) -> Result<(), String> {
    let p = repo.join("characters").join(&c.slug).join("personality.md");
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&p, format!("{}\n", text.trim())).map_err(|e| e.to_string())?;
    commit(repo, &p, &format!("personality: {}", c.name))
}

/// Commits one file. The recorder commits in the same repository, so a
/// held index lock is waited out briefly.
pub fn commit(repo: &Path, file: &Path, msg: &str) -> Result<(), String> {
    if !crate::sync::git_available() || !repo.join(".git").exists() {
        return Ok(()); // the file is written; there is just no history
    }
    let rel = file.strip_prefix(repo).unwrap_or(file);
    for attempt in 0..5 {
        let add = crate::platform::command("git")
            .arg("-C")
            .arg(repo)
            .arg("add")
            .arg(rel)
            .output()
            .map_err(|e| e.to_string())?;
        let out = if add.status.success() {
            crate::platform::command("git")
                .arg("-C")
                .arg(repo)
                .args(crate::sync::identity(repo))
                .args(["commit", "-q", "-m", msg, "--"])
                .arg(rel)
                .output()
                .map_err(|e| e.to_string())?
        } else {
            add
        };
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        if out.status.success()
            || String::from_utf8_lossy(&out.stdout).contains("nothing to commit")
        {
            return Ok(());
        }
        if !err.contains("index.lock") || attempt == 4 {
            return Err(format!("git: {}", err.trim()));
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    Ok(())
}

pub fn clip(s: &str, n: usize) -> String {
    let s = s.replace("$B", " ").replace('\n', " ");
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= n {
        s
    } else {
        format!("{}…", s.chars().take(n).collect::<String>().trim_end())
    }
}

fn item_name(m: &Model, link: &str) -> String {
    let l = parse_link(link).unwrap_or_default();
    if !l.name.is_empty() {
        return l.name;
    }
    m.memory
        .items
        .get(&l.id)
        .map(|i| i.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "something".into())
}

/// One stretch of the day in one place.
#[derive(Default)]
struct Leg {
    place: String,
    from: f64,
    to: f64,
    things: Vec<String>,
}

/// A day's recordings as a journey: the places in the order they were
/// reached, what happened in each, then what the day added up to. No clock
/// times, so the writing can't turn into a log. Other players' chat stays out.
pub fn facts(m: &Model, c: &Character, day: &str) -> Vec<String> {
    let events: Vec<&Event> = c.events.iter().filter(|e| day_of(e.t) == day).collect();
    let sessions: Vec<_> = c
        .sessions
        .iter()
        .filter(|s| day_of(s.start) == day)
        .collect();
    let mut out = vec![];
    if sessions.is_empty() {
        return out;
    }
    let (from, to) = (
        sessions[0].start as f64,
        sessions[sessions.len() - 1].end as f64,
    );
    let lvl_from = sessions[0].level_from;
    let lvl_to = sessions
        .iter()
        .map(|s| s.level_to)
        .max()
        .unwrap_or(lvl_from);
    let quests: HashMap<i64, &crate::data::memory::Quest> =
        c.quests.iter().map(|q| (q.id, q)).collect();

    let mut legs: Vec<Leg> = vec![];
    let mut zone = String::new();
    let mut last_turnin: Option<(i64, i64)> = None;
    let mut talked: HashSet<String> = HashSet::new();
    let mut loot: BTreeMap<String, (i64, Option<i64>)> = BTreeMap::new();
    let (mut looted_money, mut quest_money, mut sold, mut spent) = (0i64, 0i64, 0i64, 0i64);
    let mut learned = 0;
    let mut skills: BTreeMap<String, (String, String)> = BTreeMap::new();
    for e in &events {
        let t = e.t as f64;
        // A new leg whenever the place changes.
        let place = match e.e.as_str() {
            "zone" | "login" => {
                if let Some(z) = e.s("zone").filter(|z| !z.is_empty()) {
                    zone = z.to_string();
                }
                Some(zone.clone())
            }
            "subzone" => e
                .s("sub")
                .filter(|s| !s.is_empty())
                .map(|s| {
                    if zone.is_empty() {
                        s.to_string()
                    } else {
                        format!("{s} ({zone})")
                    }
                })
                .or(Some(zone.clone())),
            _ => None,
        };
        if let Some(p) = place.filter(|p| !p.is_empty())
            && legs.last().is_none_or(|l| l.place != p) {
                // Coming back to a place visited moments ago continues that stretch.
                if let Some(pos) = legs.iter().rposition(|l| l.place == p && t - l.to < 120.0) {
                    let leg = legs.remove(pos);
                    legs.push(leg);
                } else {
                    legs.push(Leg {
                        place: p,
                        from: t,
                        to: t,
                        things: vec![],
                    });
                }
            }
        if legs.is_empty() {
            legs.push(Leg {
                place: if zone.is_empty() {
                    "somewhere".into()
                } else {
                    zone.clone()
                },
                from: t,
                to: t,
                things: vec![],
            });
        }
        let leg = legs.last_mut().unwrap();
        leg.to = t;
        let mut say = |s: String| leg.things.push(s);
        match e.e.as_str() {
            "quest" => {
                let id = e.i("id").unwrap_or(0);
                let q = quests.get(&id);
                let title = q
                    .map(|q| q.title.clone())
                    .unwrap_or_else(|| e.s("title").unwrap_or("a task").to_string());
                match e.s("act") {
                    Some("accept") => {
                        let mut f = format!("was asked to take on \"{title}\"");
                        if let Some(q) = q {
                            if !q.objective.is_empty() {
                                f += &format!(" ({})", clip(&q.objective, 200));
                            }
                            if !q.text.is_empty() {
                                f +=
                                    &format!(". As it was put to them: \"{}\"", clip(&q.text, 380));
                            }
                        }
                        say(f);
                    }
                    Some("turnin") => {
                        last_turnin = Some((id, e.t));
                        let mut f = format!("saw \"{title}\" through");
                        if let Some(l) = e.s("choice") {
                            f += &format!(" and took {} as thanks", item_name(m, l));
                        }
                        if let Some(q) = q.filter(|q| !q.reward.is_empty()) {
                            f += &format!(". Told: \"{}\"", clip(&q.reward, 260));
                        }
                        quest_money += e.i("money").unwrap_or(0);
                        say(f);
                    }
                    Some("remove")
                        if !last_turnin.is_some_and(|(i, t)| i == id && e.t - t < 10) =>
                    {
                        say(format!("gave up on \"{title}\""))
                    }
                    _ => {}
                }
            }
            "level" => say(format!(
                "felt themselves grow stronger (level {})",
                e.i("level").unwrap_or(0)
            )),
            "death" => {
                let mut by: HashMap<&str, i64> = HashMap::new();
                for h in m
                    .combat
                    .taken
                    .iter()
                    .filter(|h| h.t <= t + 2.0 && h.t > t - 12.0)
                {
                    *by.entry(m.combat.unit_name(h.src)).or_default() += h.amount;
                }
                let mut by: Vec<_> = by.into_iter().collect();
                by.sort_by_key(|x| std::cmp::Reverse(x.1));
                let who = by
                    .iter()
                    .take(3)
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(" and ");
                say(if who.is_empty() {
                    "died".into()
                } else {
                    format!("died, struck down by {who}")
                });
            }
            "alive" | "unghost" => say("came back from death".into()),
            "equip" => {
                if let Some(l) = e.s("link") {
                    say(format!("put on {}", item_name(m, l)));
                }
            }
            "gossip" => {
                let name = e.s("name").unwrap_or("someone").to_string();
                if talked.insert(name.clone()) {
                    let said = m
                        .memory
                        .gossip
                        .iter()
                        .find(|(n, _, _)| *n == name)
                        .map(|(_, t, _)| format!(", who said: \"{}\"", clip(t, 240)))
                        .unwrap_or_default();
                    say(format!("spoke with {name}{said}"));
                }
            }
            "open" => match e.s("what") {
                Some("trainer") => say("sought out a trainer of their order".into()),
                Some("merchant") => say("traded with a merchant".into()),
                Some("bank") => say("stopped at the bank".into()),
                Some("mail") => say("checked the mail".into()),
                _ => {}
            },
            "group" => {
                let me = c.name.split(' ').next().unwrap_or("");
                let names: Vec<String> =
                    e.v.get("members")
                        .map(crate::data::memory::entries)
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|(_, v)| v.as_str().map(str::to_string))
                        .filter(|n| !n.starts_with(me))
                        .collect();
                say(if names.is_empty() {
                    "went on alone again".into()
                } else {
                    format!("travelled together with {}", names.join(", "))
                });
            }
            "msg" if e.s("kind") == Some("rep") => say(clip(e.s("text").unwrap_or(""), 120)),
            "msg" if e.s("kind") == Some("skill") => {
                if let Some((skill, lvl)) = e
                    .s("text")
                    .and_then(|t| t.strip_prefix("Your skill in "))
                    .and_then(|r| r.split_once(" has increased to "))
                {
                    let lvl = lvl.trim_end_matches('.').to_string();
                    skills
                        .entry(skill.to_string())
                        .and_modify(|v| v.1 = lvl.clone())
                        .or_insert((lvl.clone(), lvl));
                }
            }
            "spell" => learned += 1,
            "item" if e.i("d").unwrap_or(0) > 0 && e.s("ctx") == Some("loot") => {
                if let Some(l) = e.s("link") {
                    let r = loot
                        .entry(item_name(m, l))
                        .or_insert((0, parse_link(l).and_then(|x| x.quality)));
                    r.0 += e.i("d").unwrap_or(0);
                }
            }
            "money" => {
                let d = e.i("d").unwrap_or(0);
                match (e.s("ctx"), d > 0) {
                    (Some("loot"), true) => looted_money += d,
                    (Some("merchant"), true) => sold += d,
                    (Some("quest"), true) => {}
                    (_, false) => spent -= d,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // What they fought and heard in each place, from the logs.
    let players: HashSet<String> = m.players.iter().map(|p| p.first.to_lowercase()).collect();
    for leg in legs.iter_mut() {
        let mut slain: BTreeMap<&str, usize> = BTreeMap::new();
        for (t, u) in &m.combat.kills {
            if *t >= leg.from - 30.0 && *t <= leg.to + 30.0 {
                *slain.entry(m.combat.unit_name(*u)).or_default() += 1;
            }
        }
        if !slain.is_empty() {
            let mut v: Vec<_> = slain.into_iter().collect();
            v.sort_by_key(|x| std::cmp::Reverse(x.1));
            leg.things.push(format!(
                "fought here: {}",
                v.iter()
                    .map(|(n, k)| if *k > 1 {
                        format!("{k} {n}s")
                    } else {
                        format!("a {n}")
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let mut heard = vec![];
        for l in m.chat.iter().filter(|l| {
            l.t >= leg.from && l.t <= leg.to + 30.0 && matches!(l.kind, Kind::Say | Kind::Yell)
        }) {
            let who = l.speaker.clone().unwrap_or_default();
            let first = who.split(' ').next().unwrap_or("").to_lowercase();
            if players.contains(&first) || c.name.to_lowercase().starts_with(&first) {
                continue;
            }
            let line = format!(
                "heard {who} {}: \"{}\"",
                if l.kind == Kind::Yell { "yell" } else { "say" },
                clip(&l.text, 140)
            );
            if !heard.contains(&line) {
                heard.push(line);
            }
        }
        leg.things.extend(heard.into_iter().take(3));
    }

    out.push(format!(
        "{} {} {}.",
        c.name,
        if lvl_to > lvl_from {
            format!("began the day at level {lvl_from} and ended it at level {lvl_to},")
        } else {
            format!("stayed at level {lvl_from},")
        },
        "travelling through these places in this order"
    ));
    for leg in legs.into_iter().filter(|l| !l.things.is_empty()) {
        out.push(format!("{}: {}.", leg.place, leg.things.join("; ")));
    }
    let (junk, good): (Vec<_>, Vec<_>) = loot.iter().partition(|(_, (_, q))| *q == Some(0));
    if !good.is_empty() || !junk.is_empty() {
        let mut f = String::from("Carried away along the way");
        if !good.is_empty() {
            f += &format!(
                ": {}",
                good.iter()
                    .map(|(n, (k, _))| if *k > 1 {
                        format!("{k} {n}")
                    } else {
                        n.to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !junk.is_empty() {
            f += &format!(
                "{}worthless odds and ends like {}",
                if good.is_empty() { ": " } else { "; also " },
                junk.iter()
                    .take(4)
                    .map(|(n, _)| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        out.push(format!("{f}."));
    }
    let money = crate::theme::money;
    if looted_money + quest_money + sold + spent > 0 {
        out.push(format!(
            "Coin: found {}, paid {} for their help, got {} from selling, spent {}.",
            money(looted_money),
            money(quest_money),
            money(sold),
            money(spent)
        ));
    }
    if learned > 0 {
        out.push(format!(
            "Learned {learned} new abilit{}.",
            if learned == 1 { "y" } else { "ies" }
        ));
    }
    if !skills.is_empty() {
        out.push(format!(
            "Grew more practised with {}.",
            skills.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    let taken: i64 = m
        .combat
        .taken
        .iter()
        .filter(|h| h.t >= from && h.t <= to)
        .map(|h| h.amount)
        .sum();
    if taken > 0 {
        out.push(format!(
            "Wounds taken over the day: {taken}; healed themselves for {}.",
            m.combat
                .healed
                .iter()
                .filter(|h| h.t >= from && h.t <= to)
                .map(|h| h.amount - h.over)
                .sum::<i64>()
        ));
    }
    let open: Vec<String> = c
        .quests
        .iter()
        .filter(|q| q.status == QuestStatus::Active)
        .map(|q| format!("\"{}\"", q.title))
        .collect();
    if !open.is_empty() {
        out.push(format!("Still unfinished: {}.", open.join(", ")));
    }
    let met = m
        .players
        .iter()
        .filter(|p| p.last_seen >= from && p.first_seen <= to)
        .count();
    if met > 0 {
        out.push(format!("Crossed paths with {met} other adventurers."));
    }
    out
}

pub const SYSTEM: &str = "You write a character's journal in Azeroth, the world of World of Warcraft (WoW Forever, which plays like Classic). At the end of a day of travel the character sits down and writes, in the first person and in their own voice, shaped by the personality note the player gives you.

Write a summary of the journey, not a log. Tell where the road took them, what they set out to do and why it mattered to them, who they met, what went wrong, what changed in them. Pick the few moments that carried weight and give them room; fold the rest into a line or leave it out. Repetition becomes one sentence (\"most of the afternoon went to clearing the crypt of its restless dead\"). No times of day, no lists, no counts, no running tally of things picked up. It should read like someone looking back on the day, with reflection and a sense of where they are headed.

Stay true to what happened: every event must come from the facts. Add feeling, sensory detail and the connective moments between events, but never invent people, places, fights, gifts or outcomes. Quote people only with words from the facts.

Write from inside the world. The character has never heard of levels, experience, quests, loot, gold per hour, logging in or players. Growing stronger is felt, a quest is a task someone asked of them, a trainer teaches them, coins are coins, other adventurers are fellow travellers and strangers. Dying and coming back is something they live through and make sense of in their own way (the Forsaken know death well).

200 to 450 words of Markdown: a short title line of your own for the day, then the entry. No preamble and no notes about how you wrote it.";

pub fn prompt(
    c: &Character,
    day: &str,
    facts: &[String],
    previous: Option<(&str, &str)>,
) -> String {
    let mut p = format!("The writer: {}, a {} {}.\n\n", c.name, c.race, c.class);
    let note = super::presets::note_or_preset(c);
    p += &format!(
        "<personality>\n{}\n</personality>\n\n",
        note.as_deref()
            .unwrap_or("(No note yet: find a voice that fits their race and class.)")
    );
    if let Some((d, text)) = previous {
        p += &format!(
            "Their previous entry ({}), for continuity of voice and threads:\n<previous_entry>\n{}\n</previous_entry>\n\n",
            pretty_day(d),
            text
        );
    }
    p += &format!("Their day, {}:\n<facts>\n", pretty_day(day));
    for f in facts {
        p += &format!("- {f}\n");
    }
    p += "</facts>\n\nWrite the journal entry for this day.";
    let lang = crate::i18n::current();
    if lang != crate::i18n::Lang::En {
        p += &format!(
            " Write it in {}, as a native speaker would, using the names the facts give for people, places and things.",
            lang.english_name()
        );
    }
    p
}
