//! The diary: a day's recordings turned into facts, and entries written from
//! them in the character's own voice. Entries live in the memory repository at
//! characters/<slug>/diary/<date>.md with the facts they were written from
//! underneath, so every sentence can be checked.

use super::chat::Kind;
use super::memory::{parse_link, Character, Event, QuestStatus};
use super::Model;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

const MARKER: &str = "<!-- forever-memory:";

pub fn day_of(t: i64) -> String {
    crate::theme::local(t as f64).format("%Y-%m-%d").to_string()
}

pub fn pretty_day(day: &str) -> String {
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").map(|d| d.format("%A, %-d %B %Y").to_string()).unwrap_or_else(|_| day.to_string())
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
    repo.join("characters").join(&c.slug).join("diary").join(format!("{day}.md"))
}

/// Splits a stored entry into the prose and the facts it was written from.
pub fn split(stored: &str) -> (&str, &str) {
    match stored.find(MARKER) {
        Some(i) => (stored[..i].trim(), stored[i..].split_once("-->").map(|(_, f)| f.trim()).unwrap_or("")),
        None => (stored.trim(), ""),
    }
}

pub fn store(repo: &Path, c: &Character, day: &str, entry: &str, facts: &[String]) -> Result<(), String> {
    let p = path(repo, c, day);
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut out = format!("{}\n\n{MARKER} written {} by {} from these facts -->\n\n## The facts behind this entry\n\n", entry.trim(), chrono::Local::now().format("%Y-%m-%d %H:%M"), crate::claude::MODEL);
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
    let mut days: Vec<String> = std::fs::read_dir(&dir).ok()?.flatten().filter_map(|f| f.file_name().to_string_lossy().strip_suffix(".md").map(str::to_string)).filter(|d| d.as_str() < day).collect();
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

/// Commits one file. armory-sync commits in the same repository, so a
/// held index lock is waited out briefly.
fn commit(repo: &Path, file: &Path, msg: &str) -> Result<(), String> {
    let rel = file.strip_prefix(repo).unwrap_or(file);
    for attempt in 0..5 {
        let add = std::process::Command::new("git").arg("-C").arg(repo).arg("add").arg(rel).output().map_err(|e| e.to_string())?;
        let out = if add.status.success() {
            std::process::Command::new("git").arg("-C").arg(repo).args(["commit", "-q", "-m", msg, "--"]).arg(rel).output().map_err(|e| e.to_string())?
        } else {
            add
        };
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        if out.status.success() || String::from_utf8_lossy(&out.stdout).contains("nothing to commit") {
            return Ok(());
        }
        if !err.contains("index.lock") || attempt == 4 {
            return Err(format!("git: {}", err.trim()));
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    Ok(())
}

fn clip(s: &str, n: usize) -> String {
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
    m.memory.items.get(&l.id).map(|i| i.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| "something".into())
}

/// Everything recorded about one day, as plain statements in time order,
/// then totals. Other players' chat is left out on purpose.
pub fn facts(m: &Model, c: &Character, day: &str) -> Vec<String> {
    let events: Vec<&Event> = c.events.iter().filter(|e| day_of(e.t) == day).collect();
    let mut out = vec![];
    let sessions: Vec<_> = c.sessions.iter().filter(|s| day_of(s.start) == day).collect();
    if sessions.is_empty() {
        return out;
    }
    let (from, to) = (sessions[0].start as f64, sessions[sessions.len() - 1].end as f64);
    let clock = |t: i64| crate::theme::clock(t as f64);
    let spans: Vec<String> = sessions.iter().map(|s| format!("{}–{}", clock(s.start), clock(s.end))).collect();
    out.push(format!("Out in the world {} ({} in all).", spans.join(", "), crate::theme::duration(sessions.iter().map(|s| s.seconds()).sum::<i64>() as f64)));
    let lvl_from = sessions[0].level_from;
    let lvl_to = sessions.iter().map(|s| s.level_to).max().unwrap_or(lvl_from);
    out.push(if lvl_to > lvl_from { format!("Began the day at level {lvl_from}, ended it at level {lvl_to}.") } else { format!("Level {lvl_from} all day.") });

    let titles: HashMap<i64, &crate::data::memory::Quest> = c.quests.iter().map(|q| (q.id, q)).collect();
    let mut last_sub = String::new();
    let mut last_turnin: Option<(i64, i64)> = None;
    let mut talked: HashSet<String> = HashSet::new();
    let mut spells = 0;
    let mut loot: BTreeMap<String, (i64, Option<i64>)> = BTreeMap::new();
    let (mut looted_money, mut quest_money, mut spent, mut sold) = (0i64, 0i64, 0i64, 0i64);
    let mut skills: BTreeMap<String, (String, String)> = BTreeMap::new();
    for e in &events {
        let at = clock(e.t);
        match e.e.as_str() {
            "login" => out.push(format!("{at} set out{}.", e.s("zone").filter(|z| !z.is_empty()).map(|z| format!(" in {z}")).unwrap_or_default())),
            "logout" => out.push(format!("{at} stopped for the day{}.", e.s("zone").filter(|z| !z.is_empty()).map(|z| format!(" in {z}")).unwrap_or_default())),
            "zone" => out.push(format!("{at} travelled into {}.", e.s("zone").unwrap_or("?"))),
            "subzone" => {
                let sub = e.s("sub").unwrap_or("");
                if !sub.is_empty() && sub != last_sub {
                    out.push(format!("{at} reached {sub}."));
                    last_sub = sub.to_string();
                }
            }
            "quest" => {
                let id = e.i("id").unwrap_or(0);
                let q = titles.get(&id);
                let title = q.map(|q| q.title.clone()).unwrap_or_else(|| e.s("title").unwrap_or("a task").to_string());
                match e.s("act") {
                    Some("accept") => {
                        let mut f = format!("{at} took on \"{title}\"");
                        if let Some(q) = q {
                            if !q.objective.is_empty() {
                                f += &format!(". The task: {}", clip(&q.objective, 220));
                            }
                            if !q.text.is_empty() {
                                f += &format!(". What they were told: \"{}\"", clip(&q.text, 420));
                            }
                        }
                        out.push(f);
                    }
                    Some("turnin") => {
                        last_turnin = Some((id, e.t));
                        let mut f = format!("{at} finished \"{title}\"");
                        if let Some(l) = e.s("choice") {
                            f += &format!(" and chose {} as reward", item_name(m, l));
                        }
                        if let Some(q) = q.filter(|q| !q.reward.is_empty()) {
                            f += &format!(". Words on completion: \"{}\"", clip(&q.reward, 300));
                        }
                        quest_money += e.i("money").unwrap_or(0);
                        out.push(f);
                    }
                    Some("remove") if !last_turnin.is_some_and(|(i, t)| i == id && e.t - t < 10) => out.push(format!("{at} gave up on \"{title}\".")),
                    _ => {}
                }
            }
            "level" => out.push(format!("{at} grew stronger: reached level {}.", e.i("level").unwrap_or(0))),
            "death" => {
                let place = [e.s("sub").unwrap_or(""), e.s("zone").unwrap_or("")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                let t = e.t as f64;
                let mut by: HashMap<&str, i64> = HashMap::new();
                for h in m.combat.taken.iter().filter(|h| h.t <= t + 2.0 && h.t > t - 12.0) {
                    *by.entry(m.combat.unit_name(h.src)).or_default() += h.amount;
                }
                let mut by: Vec<_> = by.into_iter().collect();
                by.sort_by(|a, b| b.1.cmp(&a.1));
                let who = by.iter().take(3).map(|(n, _)| *n).collect::<Vec<_>>().join(", ");
                out.push(format!("{at} DIED in {place}{}.", if who.is_empty() { String::new() } else { format!(", struck down by {who}") }));
            }
            "alive" | "unghost" => out.push(format!("{at} came back to life.")),
            "equip" => {
                if let Some(l) = e.s("link") {
                    out.push(format!("{at} put on {}.", item_name(m, l)));
                }
            }
            "gossip" => {
                let name = e.s("name").unwrap_or("someone").to_string();
                if talked.insert(name.clone()) {
                    let said = m.memory.gossip.iter().find(|(n, _, _)| *n == name).map(|(_, t, _)| format!(": \"{}\"", clip(t, 260))).unwrap_or_default();
                    out.push(format!("{at} spoke with {name}{said}"));
                }
            }
            "gossip_pick" => out.push(format!("{at} answered \"{}\".", e.s("option").unwrap_or(""))),
            "open" => match e.s("what") {
                Some("merchant") => out.push(format!("{at} traded with a merchant.")),
                Some("trainer") => out.push(format!("{at} trained with a master of their class.")),
                Some("bank") => out.push(format!("{at} visited the bank.")),
                Some("mail") => out.push(format!("{at} checked the mail.")),
                Some("auction") => out.push(format!("{at} browsed the auction house.")),
                _ => {}
            },
            "spell" => spells += 1,
            "group" => {
                let names: Vec<String> = e.v.get("members").map(crate::data::memory::entries).unwrap_or_default().into_iter().filter_map(|(_, v)| v.as_str().map(str::to_string)).filter(|n| !n.starts_with(c.name.split(' ').next().unwrap_or(""))).collect();
                out.push(if names.is_empty() { format!("{at} went on alone again.") } else { format!("{at} joined up with {}.", names.join(", ")) });
            }
            "msg" if e.s("kind") == Some("rep") => out.push(format!("{at} {}", e.s("text").unwrap_or(""))),
            "msg" if e.s("kind") == Some("skill") => {
                // "Your skill in Defense has increased to 12."
                if let Some(rest) = e.s("text").and_then(|t| t.strip_prefix("Your skill in ")) {
                    if let Some((skill, lvl)) = rest.split_once(" has increased to ") {
                        let lvl = lvl.trim_end_matches('.').to_string();
                        skills.entry(skill.to_string()).and_modify(|v| v.1 = lvl.clone()).or_insert((lvl.clone(), lvl));
                    }
                }
            }
            "item" if e.i("d").unwrap_or(0) > 0 && e.s("ctx") == Some("loot") => {
                if let Some(l) = e.s("link") {
                    let q = parse_link(l).and_then(|x| x.quality);
                    let r = loot.entry(item_name(m, l)).or_insert((0, q));
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

    // NPC speech overheard in the chat log (other players' chat stays private).
    let players: HashSet<String> = m.players.iter().map(|p| p.name.to_lowercase()).collect();
    let mut heard = vec![];
    for l in m.chat.iter().filter(|l| l.t >= from && l.t <= to && matches!(l.kind, Kind::Say | Kind::Yell)) {
        let who = l.speaker.clone().unwrap_or_default();
        if players.contains(&who.to_lowercase()) || who.to_lowercase().starts_with(&c.name.to_lowercase()) {
            continue;
        }
        let line = format!("{} {} {}: \"{}\"", crate::theme::clock(l.t), who, if l.kind == Kind::Yell { "yelled" } else { "said" }, clip(&l.text, 160));
        if !heard.contains(&line) {
            heard.push(line);
        }
    }
    if !heard.is_empty() {
        out.push("Overheard from the people and creatures around them:".into());
        out.extend(heard.into_iter().take(12).map(|h| format!("  {h}")));
    }

    let kills: Vec<&str> = m.combat.kills.iter().filter(|(t, _)| *t >= from - 60.0 && *t <= to + 60.0).map(|(_, u)| m.combat.unit_name(*u)).collect();
    if !kills.is_empty() {
        let mut by: BTreeMap<&str, usize> = BTreeMap::new();
        for k in &kills {
            *by.entry(k).or_default() += 1;
        }
        let mut by: Vec<_> = by.into_iter().collect();
        by.sort_by(|a, b| b.1.cmp(&a.1));
        out.push(format!("Slew {} foes: {}.", kills.len(), by.iter().map(|(n, c)| format!("{c} {n}")).collect::<Vec<_>>().join(", ")));
    }
    let taken: i64 = m.combat.taken.iter().filter(|h| h.t >= from && h.t <= to).map(|h| h.amount).sum();
    let healed: i64 = m.combat.healed.iter().filter(|h| h.t >= from && h.t <= to).map(|h| h.amount - h.over).sum();
    if taken > 0 {
        out.push(format!("Took {taken} damage in all; mended {healed} of it with their own healing."));
    }
    if !loot.is_empty() {
        let (junk, good): (Vec<_>, Vec<_>) = loot.iter().partition(|(_, (_, q))| *q == Some(0));
        if !good.is_empty() {
            out.push(format!("Gathered: {}.", good.iter().map(|(n, (c, _))| if *c > 1 { format!("{c} {n}") } else { n.to_string() }).collect::<Vec<_>>().join(", ")));
        }
        if !junk.is_empty() {
            out.push(format!("Also picked up worthless odds and ends: {}.", junk.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")));
        }
    }
    let money = |c: i64| crate::theme::money(c);
    if looted_money + quest_money + sold + spent > 0 {
        out.push(format!("Coin: found {}, earned {} for tasks, sold goods for {}, spent {}.", money(looted_money), money(quest_money), money(sold), money(spent)));
    }
    if spells > 0 {
        out.push(format!("Learned {spells} new abilit{}.", if spells == 1 { "y" } else { "ies" }));
    }
    if !skills.is_empty() {
        out.push(format!("Practice paid off: {}.", skills.iter().map(|(s, (a, b))| format!("{s} {a}→{b}")).collect::<Vec<_>>().join(", ")));
    }
    let open: Vec<&str> = c.quests.iter().filter(|q| q.status == QuestStatus::Active).map(|q| q.title.as_str()).collect();
    if !open.is_empty() {
        out.push(format!("Still unfinished at day's end: {}.", open.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(", ")));
    }
    let met = m.players.iter().filter(|p| p.last_seen >= from && p.first_seen <= to).count();
    if met > 0 {
        out.push(format!("Crossed paths with {met} other adventurers."));
    }
    out
}

pub const SYSTEM: &str = "You write the private diary of a character living in Azeroth, the world of World of Warcraft (this is WoW Forever, which plays like Classic). The character writes the entry themselves at the end of the day, in the first person and in their own voice, shaped by the personality note the player gives you.

Stay true to what happened. Every event in the entry must come from the facts you are given; you may add feeling, sensory detail, reflection and the small connective moments between events, but never invent people, places, fights, gifts or outcomes that the facts don't contain. Things that happened in the same place can blur together; times of day can be approximate.

Write from inside the world. The character has never heard of levels, experience points, quests, loot, gold per hour, respawns, logging in or players. Growing a level is feeling stronger or surer; a quest is a task someone asked of them; a trainer teaches them; coins are coins; dying and coming back is something they experience and make sense of in their own way (the Forsaken, for instance, know death well). Other adventurers are fellow travellers, strangers, rivals. Quote NPCs only with words from the facts.

Write 250 to 550 words of Markdown: start with a short heading line of your own choosing for the day, then the entry. No preamble and no notes about how you wrote it.";

pub fn prompt(c: &Character, day: &str, facts: &[String], previous: Option<(&str, &str)>) -> String {
    let mut p = format!("The diarist: {}, a {} {}.\n\n", c.name, c.race, c.class);
    let note = c.personality.trim();
    p += &format!("<personality>\n{}\n</personality>\n\n", if note.is_empty() { "(No note yet: find a voice that fits their race and class.)" } else { note });
    if let Some((d, text)) = previous {
        p += &format!("Their previous entry ({}), for continuity of voice and threads:\n<previous_entry>\n{}\n</previous_entry>\n\n", pretty_day(d), text);
    }
    p += &format!("What happened on {}:\n<facts>\n", pretty_day(day));
    for f in facts {
        p += &format!("- {f}\n");
    }
    p += "</facts>\n\nWrite this day's diary entry.";
    p
}
