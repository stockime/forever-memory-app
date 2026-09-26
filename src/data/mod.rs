mod cache;
pub mod chat;
pub mod combat;
pub mod diary;
pub mod memory;
pub mod players;

#[cfg(test)]
mod bench;

use crate::tr;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Where everything lives, from the settings.
#[derive(Clone, Debug)]
pub struct Paths {
    pub repo: PathBuf,
    pub raw_logs: PathBuf,
    /// The game's Logs folder, for the session in progress.
    pub live_logs: PathBuf,
    pub art: PathBuf,
    pub game: Option<crate::art::Game>,
}

impl Paths {
    pub fn from_settings(s: &crate::config::Settings) -> Self {
        let game = crate::art::Game::from_settings(s);
        Paths {
            repo: s.archive_dir(),
            raw_logs: s.raw_logs_dir(),
            live_logs: game
                .as_ref()
                .map(|g| g.install.join(&g.flavor).join("Logs"))
                .unwrap_or_default(),
            art: crate::platform::cache_dir().join("art"),
            game,
        }
    }
}

pub struct Model {
    pub memory: memory::Memory,
    pub combat: combat::Combat,
    pub fights: Vec<combat::Fight>,
    pub chat: Vec<chat::Line>,
    pub players: Vec<players::Player>,
    pub loaded_at: SystemTime,
    pub stamp: u64,
}

/// What the loading screen shows while `load` runs.
#[derive(Default, Clone)]
pub struct Progress {
    pub frac: f32,
    pub stage: String,
    /// Loading-screen tips made from the archive once it is read.
    pub tips: Vec<String>,
    /// "easternkingdom" or "kalimdor", for the loading screen art.
    pub continent: Option<&'static str>,
}

pub type Shared = std::sync::Arc<std::sync::Mutex<Progress>>;

fn report(p: Option<&Shared>, frac: f32, stage: &str) {
    if let Some(p) = p
        && let Ok(mut g) = p.lock() {
            g.frac = frac;
            g.stage = stage.to_string();
        }
}

/// Reads everything. With `progress`, also renders all game art the pages
/// will need, so the first frame shows the finished picture. Later loads
/// reuse what did not change: the archive when no file in it did, and the
/// native logs as parsed before (see `cache`).
pub fn load(p: &Paths, progress: Option<&Shared>) -> Model {
    report(progress, 0.02, crate::tr!("Opening the archive"));
    let memory = archive(&p.repo);
    if let (Some(pr), Some(c)) = (progress, memory.characters.first())
        && let Ok(mut g) = pr.lock() {
            let race = c.snapshot.get("raceFile").and_then(serde_json::Value::as_str);
            g.continent = Some(match race {
                Some("Orc" | "Troll" | "Tauren" | "NightElf") => "kalimdor",
                _ => "easternkingdom",
            });
            g.tips = tips(&memory);
        }
    let guids: Vec<String> = memory.characters.iter().map(|c| c.guid.clone()).collect();
    let names: Vec<String> = memory.characters.iter().map(|c| c.name.clone()).collect();
    let combat_dir = p.raw_logs.join("combat");
    let chat_dir = p.raw_logs.join("chat");
    let combat_files = combat::log_files(&[&combat_dir, &p.live_logs], "WoWCombatLog");
    let chat_files = combat::log_files(&[&chat_dir, &p.live_logs], "WoWChatLog");
    let mut logs = cache::Logs::take(&p.raw_logs, &p.live_logs);
    let mut whole = combat::Whole::new(&guids);
    let read = |done: u64, total: u64| {
        report(
            progress,
            0.08 + 0.52 * done as f32 / total.max(1) as f32,
            crate::tr!("Reading the combat logs"),
        );
    };
    for (f, part) in logs.combat(&combat_files, &guids, &read) {
        whole.add(part, || std::fs::read_to_string(f).unwrap_or_default());
    }
    let combat = whole.finish();
    let fights = combat::fights(&combat);
    report(progress, 0.62, crate::tr!("Reading what was said"));
    let mut chat: Vec<chat::Line> = logs
        .chat(&chat_files)
        .into_iter()
        .flat_map(|(_, part)| part.lines.iter().cloned())
        .collect();
    chat.sort_by(|a, b| a.t.total_cmp(&b.t));
    logs.prune();
    logs.keep();
    report(progress, 0.68, crate::tr!("Remembering faces"));
    let players = players::build(&combat, &chat, &names, &memory.players);
    let model = Model {
        memory,
        combat,
        fights,
        chat,
        players,
        loaded_at: SystemTime::now(),
        stamp: stamp(p),
    };
    if progress.is_some() {
        prefetch_art(p, &model, progress);
    }
    report(progress, 1.0, crate::tr!("Ready"));
    model
}

/// The archive as last read, while none of its files changed.
static ARCHIVE: std::sync::Mutex<Option<(PathBuf, u64, memory::Memory)>> =
    std::sync::Mutex::new(None);

fn archive(repo: &Path) -> memory::Memory {
    let print = fingerprint(repo);
    let mut kept = ARCHIVE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((r, f, m)) = &*kept
        && r == repo
        && *f == print
    {
        return m.clone();
    }
    let m = memory::load(repo);
    *kept = Some((repo.to_path_buf(), print, m.clone()));
    m
}

/// Every file in the archive (but git's own) by name, size and time.
fn fingerprint(repo: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut dirs = vec![(repo.to_path_buf(), 0)];
    while let Some((d, depth)) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let Ok(m) = std::fs::metadata(e.path()) else { continue };
            if m.is_dir() {
                if e.file_name() != ".git" && depth < 8 {
                    dirs.push((e.path(), depth + 1));
                }
                continue;
            }
            (e.path(), m.len(), m.modified().ok()).hash(&mut h);
        }
    }
    h.finish()
}

/// A few things worth knowing, for the loading screen.
fn tips(m: &memory::Memory) -> Vec<String> {
    let mut out = vec![];
    for c in &m.characters {
        let done = c
            .quests
            .iter()
            .filter(|q| q.status == memory::QuestStatus::Completed)
            .count();
        let deaths = c.events.iter().filter(|e| e.e == "death").count();
        let played: i64 = c.sessions.iter().map(|s| s.seconds()).sum();
        let name = &c.name;
        out.push(tr!(
            "{name} has spent {time} in the world.",
            name = name,
            time = crate::theme::duration(played as f64)
        ));
        if done == 1 {
            out.push(tr!(
                "{name} has seen {n} task through.",
                name = name,
                n = done
            ));
        } else if done > 1 {
            out.push(tr!(
                "{name} has seen {n} tasks through.",
                name = name,
                n = done
            ));
        }
        out.push(match deaths {
            0 => tr!("{name} has not died. Yet.", name = name),
            1 => tr!("{name} has died {n} time.", name = name, n = deaths),
            _ => tr!("{name} has died {n} times.", name = name, n = deaths),
        });
        if let Some(q) = c
            .quests
            .iter()
            .find(|q| q.status == memory::QuestStatus::Active)
        {
            out.push(tr!(
                "Still waiting on {name}: \"{quest}\".",
                name = c.name.split(' ').next().unwrap_or(""),
                quest = q.title
            ));
        }
        if !c.diary.is_empty() {
            let n = c.diary.len();
            out.push(if n == 1 {
                tr!("{name} has written {n} diary entry.", name = name, n = n)
            } else {
                tr!("{name} has written {n} diary entries.", name = name, n = n)
            });
        }
    }
    out
}

/// Every piece of game art the pages use, as art keys.
pub fn art_keys(m: &Model) -> Vec<String> {
    use serde_json::Value;
    let mut keys: std::collections::BTreeSet<String> = Default::default();
    let icon = |id: Option<i64>, keys: &mut std::collections::BTreeSet<String>| {
        if let Some(i) = id.filter(|i| *i > 0) {
            keys.insert(format!("icon-{i}.png"));
        }
    };
    for id in crate::ui::widgets::icons::ALL {
        icon(Some(*id), &mut keys);
    }
    for id in [3450737, 7963776, 7963779] {
        icon(Some(id), &mut keys);
    }
    for it in m.memory.items.values() {
        icon(it.icon, &mut keys);
    }
    let slots = [
        "ammo",
        "head",
        "neck",
        "shoulder",
        "shirt",
        "chest",
        "waist",
        "legs",
        "feet",
        "wrists",
        "hands",
        "finger",
        "rfinger",
        "trinket",
        "rear",
        "mainhand",
        "secondaryhand",
        "ranged",
        "tabard",
    ];
    for s in slots {
        keys.insert(format!("slot-{s}.png"));
    }
    for c in &m.memory.characters {
        let class = c.class_file.to_lowercase();
        for k in [
            format!("class-{class}.png"),
            format!("banner-{class}.jpg"),
            format!("scene-{class}.jpg"),
            format!("tree-{class}-0.png"),
            format!("tree-{class}-1.png"),
            format!("tree-{class}-2.png"),
        ] {
            keys.insert(k);
        }
        let mut walk = vec![&c.snapshot];
        while let Some(v) = walk.pop() {
            match v {
                Value::Object(o) => {
                    if let Some(i) = o.get("icon").and_then(Value::as_i64) {
                        icon(Some(i), &mut keys);
                    }
                    walk.extend(o.values());
                }
                Value::Array(a) => walk.extend(a.iter()),
                _ => {}
            }
        }
        for ex in c.explored.values() {
            for o in &ex.overlays {
                for id in &o.files {
                    icon(Some(*id), &mut keys);
                }
            }
        }
        for e in c.events.iter().filter(|e| e.e == "pos") {
            if let Some(map) = e.i("map").filter(|m| (1411..=1458).contains(m)) {
                keys.insert(format!("map-{map}.jpg"));
            }
        }
    }
    for p in &m.players {
        if let Some(cl) = p.class {
            keys.insert(format!("class-{}.png", cl.to_lowercase()));
        }
    }
    keys.into_iter().collect()
}

fn prefetch_art(p: &Paths, m: &Model, progress: Option<&Shared>) {
    let Some(game) = &p.game else { return };
    let missing: Vec<String> = art_keys(m)
        .into_iter()
        .filter(|k| !p.art.join(k).exists())
        .collect();
    for (n, chunk) in missing.chunks(40).enumerate() {
        report(
            progress,
            0.7 + 0.29 * (n * 40) as f32 / missing.len().max(1) as f32,
            &crate::tr!(
                "Painting icons ({done} of {total})",
                done = (n * 40).min(missing.len()),
                total = missing.len()
            ),
        );
        crate::art::render_missing(game, &p.art, chunk);
    }
}

/// Changes whenever the archive commits or the live logs grow, so the app
/// can reload on its own.
pub fn stamp(p: &Paths) -> u64 {
    let mut s = 0u64;
    let mut add = |path: &Path| {
        if let Ok(m) = std::fs::metadata(path) {
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            s = s.wrapping_mul(31).wrapping_add(t ^ m.len());
        }
    };
    add(&p.repo.join(".git/refs/heads/master"));
    add(&p.repo.join(".git/refs/heads/main"));
    add(&p.repo.join(".git/HEAD"));
    add(&p.repo.join("state.json"));
    if let Ok(rd) = std::fs::read_dir(&p.live_logs) {
        for f in rd.flatten() {
            add(&f.path());
        }
    }
    s
}
