pub mod chat;
pub mod combat;
pub mod memory;
pub mod players;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Where everything lives; each can be overridden from the environment.
#[derive(Clone, Debug)]
pub struct Paths {
    pub repo: PathBuf,
    pub raw_logs: PathBuf,
    pub live_logs: PathBuf,
    pub art: PathBuf,
    pub wowdata: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let env = |k: &str, d: PathBuf| std::env::var(k).map(PathBuf::from).unwrap_or(d);
        Paths {
            repo: env("FM_REPO", home.join("Work/personal/forever-memory")),
            raw_logs: env("FM_RAW_LOGS", home.join(".local/share/forever-memory/logs")),
            live_logs: env("FM_LIVE_LOGS", home.join("Games/battlenet/drive_c/Program Files (x86)/World of Warcraft/_classic_beta_/Logs")),
            art: env("FM_ART", home.join(".cache/forever-memory/art")),
            wowdata: env("FM_WOWDATA", home.join(".local/bin/wowdata")),
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

pub fn load(p: &Paths) -> Model {
    let memory = memory::load(&p.repo);
    let guids: Vec<String> = memory.characters.iter().map(|c| c.guid.clone()).collect();
    let names: Vec<String> = memory.characters.iter().map(|c| c.name.clone()).collect();
    let combat_dir = p.raw_logs.join("combat");
    let chat_dir = p.raw_logs.join("chat");
    let combat_files = combat::log_files(&[&combat_dir, &p.live_logs], "WoWCombatLog");
    let chat_files = combat::log_files(&[&chat_dir, &p.live_logs], "WoWChatLog");
    let combat = combat::load(&combat_files, &guids);
    let fights = combat::fights(&combat);
    let chat = chat::load(&chat_files);
    let players = players::build(&combat, &chat, &names);
    Model { memory, combat, fights, chat, players, loaded_at: SystemTime::now(), stamp: stamp(p) }
}

/// Changes whenever the archive commits or the live logs grow, so the app
/// can reload on its own.
pub fn stamp(p: &Paths) -> u64 {
    let mut s = 0u64;
    let mut add = |path: &Path| {
        if let Ok(m) = std::fs::metadata(path) {
            let t = m.modified().ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            s = s.wrapping_mul(31).wrapping_add(t ^ m.len());
        }
    };
    add(&p.repo.join(".git/refs/heads/master"));
    add(&p.repo.join(".git/HEAD"));
    if let Ok(rd) = std::fs::read_dir(&p.live_logs) {
        for f in rd.flatten() {
            add(&f.path());
        }
    }
    s
}
