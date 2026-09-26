//! Letters between the characters of one account, each in the writer's own
//! voice. They share a household: the mounts and companions the account
//! collected (with who brought each home), the Legacy earned together, and
//! news of each other's days. Letters live in the archive at
//! letters/<date>-<from>-to-<to>.md with the facts they were written from.

use super::Model;
use super::diary::{self, MARKER, clip};
use super::memory::{Character, QuestStatus, entries};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Letter {
    pub path: PathBuf,
    /// Character slugs.
    pub from: String,
    pub to: String,
    pub day: String,
    /// "2026-09-26 16:11", from the marker; orders letters within a day.
    pub written: String,
    pub text: String,
    pub facts: String,
}

pub fn dir(repo: &Path) -> PathBuf {
    repo.join("letters")
}

/// "2026-09-26-tom-crusader-to-usain-frostbolt-2" → (day, from, to), using
/// the known slugs since slugs have dashes too.
fn parse_name(stem: &str, slugs: &[&str]) -> Option<(String, String, String)> {
    let day = stem.get(..10)?;
    let rest = stem.get(11..)?;
    for from in slugs {
        let Some(r) = rest.strip_prefix(&format!("{from}-to-")) else {
            continue;
        };
        for to in slugs {
            let n = r.strip_prefix(to);
            if n.is_some_and(|n| {
                n.is_empty() || n.strip_prefix('-').is_some_and(|d| d.chars().all(|c| c.is_ascii_digit()))
            }) {
                return Some((day.into(), from.to_string(), to.to_string()));
            }
        }
    }
    let (from, to) = rest.split_once("-to-")?;
    Some((day.into(), from.into(), to.into()))
}

/// Every letter in the archive, oldest first.
pub fn load(repo: &Path, slugs: &[&str]) -> Vec<Letter> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir(repo)) else {
        return out;
    };
    for f in rd.flatten() {
        let path = f.path();
        let Some(stem) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".md"))
        else {
            continue;
        };
        let (Some((day, from, to)), Ok(stored)) =
            (parse_name(stem, slugs), std::fs::read_to_string(&path))
        else {
            continue;
        };
        let (text, facts) = diary::split(&stored);
        let written = stored
            .split_once(&format!("{MARKER} written "))
            .map(|(_, r)| r.chars().take(16).collect())
            .unwrap_or_else(|| day.clone());
        out.push(Letter {
            text: text.to_string(),
            facts: facts.to_string(),
            path,
            from,
            to,
            day,
            written,
        });
    }
    out.sort_by(|a, b| (&a.written, &a.path).cmp(&(&b.written, &b.path)));
    out
}

/// The letters between two characters, either way, oldest first.
pub fn thread<'a>(all: &'a [Letter], a: &str, b: &str) -> Vec<&'a Letter> {
    all.iter()
        .filter(|l| (l.from == a && l.to == b) || (l.from == b && l.to == a))
        .collect()
}

pub fn store(
    repo: &Path,
    from: &Character,
    to: &Character,
    text: &str,
    facts: &Facts,
) -> Result<PathBuf, String> {
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    let base = format!("{day}-{}-to-{}", from.slug, to.slug);
    let d = dir(repo);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let mut p = d.join(format!("{base}.md"));
    for n in 2.. {
        if !p.exists() {
            break;
        }
        p = d.join(format!("{base}-{n}.md"));
    }
    let mut out = format!(
        "{}\n\n{MARKER} written {} by {} from these facts -->\n\n## The facts behind this letter\n",
        text.trim(),
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
        crate::claude::name()
    );
    for (head, lines) in facts.sections(from, to) {
        out += &format!("\n### {head}\n\n");
        for f in lines {
            out += &format!("- {f}\n");
        }
    }
    std::fs::write(&p, out).map_err(|e| e.to_string())?;
    diary::commit(repo, &p, &format!("letter: {} to {}", from.name, to.name))?;
    Ok(p)
}

/// What a letter may draw on.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub writer: Vec<String>,
    pub reader: Vec<String>,
    pub household: Vec<String>,
}

impl Facts {
    fn sections(&self, from: &Character, to: &Character) -> [(String, &[String]); 3] {
        [
            (from.name.clone(), &self.writer),
            (to.name.clone(), &self.reader),
            ("The household".into(), &self.household),
        ]
    }
}

fn describe(c: &Character) -> String {
    let faction = c.snapshot.get("faction").and_then(Value::as_str).unwrap_or("");
    let mut s = format!("{}, a level {} {} {}", c.name, c.level, c.race, c.class);
    if !faction.is_empty() {
        s += &format!(" of the {faction}");
    }
    s
}

/// Where a character was at time t, from their own events.
fn place_at(c: &Character, t: i64) -> Option<String> {
    let mut zone = String::new();
    let mut sub = String::new();
    let mut before: Vec<_> = c.events.iter().filter(|e| e.t <= t).collect();
    before.sort_by_key(|e| e.t);
    for e in before {
        match e.e.as_str() {
            "zone" | "login" => {
                if let Some(z) = e.s("zone").filter(|z| !z.is_empty()) {
                    zone = z.to_string();
                    sub = e.s("sub").unwrap_or("").to_string();
                }
            }
            "subzone" => sub = e.s("sub").unwrap_or("").to_string(),
            _ => {}
        }
    }
    match (sub.is_empty() || sub == zone, zone.is_empty()) {
        (_, true) => None,
        (true, false) => Some(zone),
        (false, false) => Some(format!("{sub} ({zone})")),
    }
}

fn professions(c: &Character) -> Vec<String> {
    const HEADERS: [&str; 6] = ["Professions", "Berufe", "Métiers", "Profesiones", "Profissões", "专业技能"];
    let mut on = false;
    let mut out = vec![];
    for (_, s) in c.snapshot.get("skills").map(entries).unwrap_or_default() {
        let name = s.get("name").and_then(Value::as_str).unwrap_or("");
        if s.get("isHeader").and_then(Value::as_bool) == Some(true) {
            on = HEADERS.contains(&name);
        } else if on && !name.is_empty() {
            out.push(name.to_string());
        }
    }
    out
}

/// A character in a few lines: who they are, what they have done, and
/// their latest days (from their diary where it is written).
fn about(m: &Model, c: &Character, days: usize) -> Vec<String> {
    let mut out = vec![describe(c) + "."];
    if !c.zone.is_empty() {
        out.push(format!("Last seen in {}.", c.zone));
    }
    if let Some(s) = c.sessions.first() {
        out.push(format!("On the road since {}.", diary::pretty_day(&diary::day_of(s.start))));
    }
    let profs = professions(c);
    if !profs.is_empty() {
        out.push(format!("Skilled in {}.", profs.join(" and ")));
    }
    let mut done: Vec<_> = c
        .quests
        .iter()
        .filter(|q| q.status == QuestStatus::Completed)
        .collect();
    if !done.is_empty() {
        done.sort_by_key(|q| q.done.unwrap_or(0));
        out.push(format!(
            "Has seen {} tasks through, lately {}.",
            done.len(),
            done.iter()
                .rev()
                .take(3)
                .map(|q| format!("\"{}\"", q.title))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let open: Vec<String> = c
        .quests
        .iter()
        .filter(|q| q.status == QuestStatus::Active)
        .take(3)
        .map(|q| format!("\"{}\"", q.title))
        .collect();
    if !open.is_empty() {
        out.push(format!("Still unfinished: {}.", open.join(", ")));
    }
    let deaths = c.events.iter().filter(|e| e.e == "death").count();
    if deaths > 0 {
        out.push(match deaths {
            1 => "Has died once and come back.".to_string(),
            n => format!("Has died and come back {n} times."),
        });
    }
    for (day, _) in diary::days(c).into_iter().take(days) {
        let told = match c.diary.get(&day) {
            Some(stored) => format!("in their own journal: {}", clip(diary::split(stored).0, 900)),
            None => clip(&diary::facts(m, c, &day).join(" "), 900),
        };
        if !told.is_empty() {
            out.push(format!("{}: {told}", diary::pretty_day(&day)));
        }
    }
    out
}

/// The shared stable and menagerie, the Legacy, and the rest of the household.
fn household(m: &Model, from: &Character, to: &Character) -> Vec<String> {
    let mut out = vec![];
    let chars = &m.memory.characters;
    for a in &m.memory.account {
        let what = if a.kind == "mounts" { "A mount" } else { "A companion" };
        let mut f = format!("{what}, {}", a.name);
        if a.first > 0 {
            f += &format!(", came to the household on {}", diary::pretty_day(&diary::day_of(a.first)));
        }
        if let Some(c) = chars.iter().find(|c| !a.by.is_empty() && c.guid == a.by) {
            f += &format!(", brought home by {}", c.name);
            if let Some(p) = place_at(c, a.first) {
                f += &format!(" in {p}");
            }
        }
        out.push(f + ".");
    }
    // Legacy is account-wide; the latest snapshot has the latest count.
    let legacy = chars
        .iter()
        .filter(|c| c.snapshot.pointer("/legacy/trees").is_some())
        .max_by_key(|c| c.snapshot.get("updated").and_then(Value::as_i64).unwrap_or(0))
        .and_then(|c| c.snapshot.get("legacy"));
    if let Some(l) = legacy {
        let trees = l.get("trees").map(entries).unwrap_or_default();
        let cur = trees
            .first()
            .and_then(|(_, t)| t.get("currency"))
            .and_then(|c| entries(c).into_iter().next().map(|(_, v)| v));
        let n = |k: &str| cur.and_then(|c| c.get(k)).and_then(Value::as_i64).unwrap_or(0);
        let total = n("spent") + n("quantity");
        let learned: Vec<String> = trees
            .iter()
            .flat_map(|(_, t)| t.get("nodes").map(entries).unwrap_or_default())
            .filter(|(_, node)| node.get("ranks").and_then(Value::as_i64).unwrap_or(0) > 0)
            .filter_map(|(_, node)| {
                node.pointer("/entries/0/name")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect();
        if total > 0 {
            let mut f = format!("The household's Legacy: {total} marks earned together by all of them");
            if !learned.is_empty() {
                f += &format!(", put into {}", learned.join(", "));
            }
            out.push(f + ".");
        }
    }
    let others: Vec<String> = chars
        .iter()
        .filter(|c| c.slug != from.slug && c.slug != to.slug)
        .map(describe)
        .collect();
    if !others.is_empty() {
        out.push(format!("Also of the household: {}.", others.join("; ")));
    }
    out
}

pub fn facts(m: &Model, from: &Character, to: &Character) -> Facts {
    Facts {
        writer: about(m, from, 2),
        reader: about(m, to, 2),
        household: household(m, from, to),
    }
}

pub const SYSTEM: &str = "You write letters between characters in Azeroth, the world of World of Warcraft (WoW Forever, which plays like Classic). The characters belong to one household. They know each other and share a stable of mounts and a menagerie of companions that any of them may ride or call, and a Legacy of lessons they earned together. One of them sits down to write to another, in the first person and in their own voice, shaped by the personality notes the player gives you.

Write a letter, not a report: a greeting, then what the writer wants the other to know, ask, tease or confess, then a closing and the writer's first name. Pick one or two things from the writer's recent days that mattered to them and tell those properly; ask after the reader's road, which the writer knows from news that reached them (never quote the reader's journal back at them). Where the household shares something, a mount one of them brought home, a companion, the Legacy, let it come up the way shared things do between people who live with them: who found it first, who rides it now, what it means. Let the two personalities set the tone, warm, wry, formal, teasing or worried, whatever fits these two.

Stay true to the facts: every event, place, person and thing comes from them. Add feeling and texture, but never invent fights, gifts, meetings or outcomes. The writer has not been with the reader unless the facts say so.

Write from inside the world. Nobody has heard of levels, experience, quests, loot, accounts, characters, logging in or players. Growing stronger is felt, a quest is a task someone asked of them, coins are coins, a mount is an animal in the stable. Don't explain how the household is bound together; kin, sworn companions or old friends, it simply is. If the two serve opposing factions, the letter travels by some quiet, neutral way and both know it must not be found.

120 to 300 words of Markdown: no title, only the letter, with the signature on its own line. No preamble and no notes about how you wrote it.";

fn personality(c: &Character) -> String {
    super::presets::note_or_preset(c)
        .unwrap_or_else(|| "(No note yet: find a voice that fits their race and class.)".into())
}

/// `thread` is the correspondence so far; `answering` the letter being
/// replied to, if any.
pub fn prompt(
    m: &Model,
    from: &Character,
    to: &Character,
    facts: &Facts,
    thread: &[&Letter],
    answering: Option<&Letter>,
) -> String {
    let name = |slug: &str| {
        m.memory
            .characters
            .iter()
            .find(|c| c.slug == slug)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| slug.to_string())
    };
    let first = |c: &Character| c.name.split(' ').next().unwrap_or(&c.name).to_string();
    let list = |v: &[String]| v.iter().map(|f| format!("- {f}\n")).collect::<String>();
    let mut p = format!(
        "The writer: {}.\n<writer_personality>\n{}\n</writer_personality>\n\nThe reader: {}.\n<reader_personality>\n{}\n</reader_personality>\n\n",
        describe(from),
        personality(from),
        describe(to),
        personality(to)
    );
    p += &format!("The writer's own life and recent days:\n<writer>\n{}</writer>\n\n", list(&facts.writer));
    p += &format!(
        "News of the reader that has reached the writer:\n<reader>\n{}</reader>\n\n",
        list(&facts.reader)
    );
    if !facts.household.is_empty() {
        p += &format!("What the household shares:\n<household>\n{}</household>\n\n", list(&facts.household));
    }
    let earlier: Vec<&&Letter> = thread
        .iter()
        .filter(|l| answering.is_none_or(|a| a.path != l.path))
        .collect();
    if !earlier.is_empty() {
        p += "Their earlier letters, oldest first, for continuity (don't repeat them):\n<earlier_letters>\n";
        for l in earlier.iter().rev().take(3).rev() {
            p += &format!(
                "<letter from=\"{}\" to=\"{}\">\n{}\n</letter>\n",
                name(&l.from),
                name(&l.to),
                l.text
            );
        }
        p += "</earlier_letters>\n\n";
    }
    match answering {
        Some(l) => {
            p += &format!(
                "The letter {} is answering, from {}:\n<letter_to_answer>\n{}\n</letter_to_answer>\n\nWrite {}'s reply to {}.",
                first(from),
                first(to),
                l.text,
                first(from),
                first(to)
            );
        }
        None => p += &format!("Write {}'s letter to {}.", first(from), first(to)),
    }
    let lang = crate::i18n::current();
    if lang != crate::i18n::Lang::En {
        p += &format!(
            " Write it in {}, as a native speaker would, using the names the facts give for people, places and things.",
            lang.english_name()
        );
    }
    p
}

/// `forever-memory letter <from> <to> [reply]` writes one letter without
/// opening the window; with `reply` it answers the latest letter from <to>.
pub fn cli(args: &[String]) -> ! {
    let settings = crate::config::get();
    crate::i18n::set(settings.lang());
    let paths = super::Paths::from_settings(&settings);
    let m = super::load(&paths, None);
    let find = |who: Option<&String>| {
        let who = who.map(|s| s.to_lowercase()).unwrap_or_default();
        m.memory
            .characters
            .iter()
            .find(|c| c.slug == who || c.name.to_lowercase() == who)
    };
    let (Some(from), Some(to)) = (find(args.first()), find(args.get(1))) else {
        eprintln!("usage: forever-memory letter <from> <to> [reply]");
        std::process::exit(2);
    };
    let slugs: Vec<&str> = m.memory.characters.iter().map(|c| c.slug.as_str()).collect();
    let all = load(&paths.repo, &slugs);
    let thread = thread(&all, &from.slug, &to.slug);
    let answering = (args.get(2).map(String::as_str) == Some("reply"))
        .then(|| thread.iter().rev().find(|l| l.from == to.slug).copied())
        .flatten();
    let facts = facts(&m, from, to);
    let prompt = prompt(&m, from, to, &facts, &thread, answering);
    match crate::claude::write(SYSTEM, &prompt).and_then(|text| {
        store(&paths.repo, from, to, &text, &facts)?;
        Ok(text)
    }) {
        Ok(text) => {
            println!("{text}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let slugs = ["tom-crusader", "usain-frostbolt", "tom"];
        assert_eq!(
            parse_name("2026-09-26-tom-crusader-to-usain-frostbolt", &slugs),
            Some(("2026-09-26".into(), "tom-crusader".into(), "usain-frostbolt".into()))
        );
        assert_eq!(
            parse_name("2026-09-26-usain-frostbolt-to-tom-crusader-3", &slugs),
            Some(("2026-09-26".into(), "usain-frostbolt".into(), "tom-crusader".into()))
        );
        assert_eq!(
            parse_name("2026-09-26-tom-to-usain-frostbolt", &slugs),
            Some(("2026-09-26".into(), "tom".into(), "usain-frostbolt".into()))
        );
    }
}
