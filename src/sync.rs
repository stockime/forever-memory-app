//! Keeps the archive: turns each save of the armory addon into files, moves
//! the game's finished chat and combat logs out of its folder between
//! sessions, and, when object storage is set up, backs both up.
//!
//! The archive (git-versioned when git is installed):
//!
//!   raw/armory.lua                        the SavedVariables file as the game wrote it
//!   characters/<slug>/snapshot.json       the armory snapshot
//!   characters/<slug>/seen.json           item key -> first seen (unix seconds)
//!   characters/<slug>/log/<date>.jsonl    event rows, one JSON object per line
//!   characters/<slug>/questlog.json       the quest log with objective progress
//!   items.json, players.json, gossip.json, quests/<id>.json
//!   logs/manifest.jsonl                   every archived native log
//!   state.json                            the last archived row number
//!
//! The addon keeps 30 days of rows; the archive keeps everything.

use crate::config::{S3, Settings};
use crate::{platform, s3, savedvars};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// What the settings page shows about the recorder.
#[derive(Default, Clone)]
pub struct Status {
    pub last: Option<(SystemTime, String)>,
    pub error: Option<String>,
    pub watching: Vec<PathBuf>,
}

pub type SharedStatus = Arc<Mutex<Status>>;

/// Runs the recorder until the app quits. It reads the settings on every
/// round, so changes apply without a restart.
pub fn spawn(
    settings: Arc<Mutex<Settings>>,
    status: SharedStatus,
    changed: impl Fn() + Send + 'static,
) {
    std::thread::spawn(move || {
        let mut seen: HashMap<PathBuf, SystemTime> = HashMap::new();
        let mut last_logs = Instant::now() - Duration::from_secs(3600);
        loop {
            let s = settings.lock().map(|s| s.clone()).unwrap_or_default();
            let mut did = false;
            if let Some(install) = s.install() {
                if let Some(flavor) = s.flavor_in(&install) {
                    let files = platform::saved_variables(&install, &flavor);
                    if let Ok(mut st) = status.lock() {
                        st.watching = files.clone();
                    }
                    if s.record {
                        for f in files {
                            let Ok(modified) = std::fs::metadata(&f).and_then(|m| m.modified())
                            else {
                                continue;
                            };
                            if seen.get(&f) == Some(&modified) {
                                continue;
                            }
                            match record(&s, &f) {
                                Ok(msg) => {
                                    seen.insert(f, modified);
                                    if let Some(msg) = msg {
                                        note(&status, Ok(msg));
                                        did = true;
                                    }
                                }
                                Err(e) => note(&status, Err(e)), // tried again on the next change
                            }
                        }
                    }
                    if s.archive_logs && last_logs.elapsed() > Duration::from_secs(60) {
                        last_logs = Instant::now();
                        match archive_logs(&s, &install.join(&flavor).join("Logs")) {
                            Ok(Some(msg)) => {
                                note(&status, Ok(msg));
                                did = true;
                            }
                            Ok(None) => {}
                            Err(e) => note(&status, Err(format!("logs: {e}"))),
                        }
                    }
                }
            }
            if did {
                if s.s3.ready() {
                    match backup(&s.archive_dir(), &s.s3) {
                        Ok(Some(key)) => note(&status, Ok(format!("backed up to {key}"))),
                        Ok(None) => {}
                        Err(e) => note(&status, Err(format!("backup: {e}"))),
                    }
                }
                changed();
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

fn note(status: &SharedStatus, r: Result<String, String>) {
    if let Ok(mut st) = status.lock() {
        match r {
            Ok(msg) => {
                st.last = Some((SystemTime::now(), msg));
                st.error = None;
            }
            Err(e) => st.error = Some(e),
        }
    }
}

/// Archives one SavedVariables file; returns the commit message, or None
/// when nothing changed.
pub fn record(s: &Settings, file: &Path) -> Result<Option<String>, String> {
    let raw = std::fs::read(file).map_err(|e| e.to_string())?;
    let vars = savedvars::parse(&raw)?;
    export(&s.archive_dir(), &raw, &vars)
}

// ---- git, when installed ----

pub fn git_available() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| {
        git_cmd(Path::new("."))
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

fn git_cmd(dir: &Path) -> Command {
    let mut cmd = platform::command("git");
    cmd.arg("-C").arg(dir);
    cmd
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = git_cmd(dir)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn ensure_repo(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    if git_available() && !dir.join(".git").exists() {
        git(dir, &["init", "-q"])?;
    }
    Ok(())
}

/// Commits everything in the archive; None when nothing changed or git is
/// not installed (the files are written either way).
pub fn commit(dir: &Path, msg: &str) -> Result<Option<String>, String> {
    if !git_available() || !dir.join(".git").exists() {
        return Ok(Some(msg.to_string()));
    }
    git(dir, &["add", "-A"])?;
    if git_cmd(dir)
        .args(["diff", "--cached", "--quiet"])
        .status()
        .is_ok_and(|s| s.success())
    {
        return Ok(None);
    }
    let mut args = identity(dir);
    args.extend(["commit", "-q", "-m", msg]);
    git(dir, &args)?;
    Ok(Some(msg.to_string()))
}

/// A name for commits when git has none configured.
pub fn identity(dir: &Path) -> Vec<&'static str> {
    if git(dir, &["config", "user.email"]).is_ok() {
        return vec![];
    }
    vec![
        "-c",
        "user.name=Forever Memory",
        "-c",
        "user.email=forever-memory@localhost",
    ]
}

// ---- the export ----

fn export(dir: &Path, raw: &[u8], vars: &Map<String, Value>) -> Result<Option<String>, String> {
    ensure_repo(dir)?;
    let empty = Map::new();
    let obj = |v: Option<&Value>| v.and_then(Value::as_object).unwrap_or(&empty).clone();
    let mem = obj(vars.get("armory_memory"));
    let db = obj(vars.get("armory_db"));
    let snaps = obj(db.get("characters"));
    let chars = obj(mem.get("characters"));

    let state_path = dir.join("state.json");
    let mut last_seq = read_json(&state_path)
        .and_then(|v| v.get("lastSeq").and_then(Value::as_i64))
        .unwrap_or(0);
    if int(mem.get("seq")) < last_seq {
        last_seq = 0; // the SavedVariables were reset; start over rather than skip
    }
    write(&dir.join("raw").join("armory.lua"), raw)?;

    let mut summaries = vec![];
    let mut max_seq = last_seq;
    let guids: std::collections::BTreeSet<&String> = snaps.keys().chain(chars.keys()).collect();
    for guid in guids {
        let c = obj(chars.get(guid));
        let snap = snaps.get(guid).and_then(Value::as_object);
        let (name, slug) = names(guid, snap);
        let base = dir.join("characters").join(&slug);
        if let Some(snap) = snap {
            write_json(&base.join("snapshot.json"), &Value::Object(snap.clone()))?;
        }
        if let Some(seen) = c.get("seen").filter(|v| v.is_object()) {
            write_json(&base.join("seen.json"), seen)?;
        }
        if let Some(ql) = c.get("questlog") {
            write_json(&base.join("questlog.json"), ql)?;
        }
        let mut by_day: BTreeMap<String, Vec<&Map<String, Value>>> = BTreeMap::new();
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut n = 0;
        for row in c
            .get("log")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
        {
            let seq = int(row.get("n"));
            if seq <= last_seq {
                continue;
            }
            let day = chrono::DateTime::from_timestamp(int(row.get("t")), 0)
                .map(|t| {
                    t.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d")
                        .to_string()
                })
                .unwrap_or_default();
            by_day.entry(day).or_default().push(row);
            let kind = match row.get("e") {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => "<nil>".into(),
            };
            *counts.entry(kind).or_default() += 1;
            max_seq = max_seq.max(seq);
            n += 1;
        }
        for (day, mut rows) in by_day {
            rows.sort_by_key(|r| int(r.get("n")));
            append_lines(&base.join("log").join(format!("{day}.jsonl")), &rows)?;
        }
        if n > 0 {
            summaries.push(format!("{name}: {n} events ({})", describe(&counts)));
        }
    }
    if let Some(quests) = mem.get("quests").and_then(Value::as_object) {
        for (id, q) in quests {
            write_json(&dir.join("quests").join(format!("{id}.json")), q)?;
        }
    }
    for (key, file) in [
        ("players", "players.json"),
        ("items", "items.json"),
        ("gossip", "gossip.json"),
    ] {
        if let Some(v) = mem.get(key).filter(|v| v.is_object()) {
            write_json(&dir.join(file), v)?;
        }
    }
    write_json(&state_path, &serde_json::json!({ "lastSeq": max_seq }))?;
    let msg = if summaries.is_empty() {
        "snapshot".to_string()
    } else {
        summaries.join("; ")
    };
    commit(dir, &msg)
}

/// The display name and folder for a character: "Dead Poole", "dead-poole".
fn names(guid: &str, snap: Option<&Map<String, Value>>) -> (String, String) {
    let s = |k: &str| {
        snap.and_then(|m| m.get(k))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let mut name = s("displayName");
    if name.is_empty() {
        name = format!("{} {}", s("name"), s("surname")).trim().to_string();
    }
    if name.is_empty() {
        return (guid.into(), guid.into());
    }
    let slug = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase();
    (name, slug)
}

/// Event counts, most frequent first: "12 pos, 3 item, 1 quest".
fn describe(counts: &HashMap<String, usize>) -> String {
    let mut kinds: Vec<(&String, &usize)> = counts.iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    kinds
        .iter()
        .map(|(k, n)| format!("{n} {k}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn int(v: Option<&Value>) -> i64 {
    v.and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(0)
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    let mut s = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    s.push('\n');
    write(path, s.as_bytes())
}

fn append_lines(path: &Path, rows: &[&Map<String, Value>]) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    for r in rows {
        let line = serde_json::to_string(r).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

// ---- native logs ----

/// One line of logs/manifest.jsonl.
#[derive(serde::Serialize)]
struct Entry {
    key: String,
    kind: String,
    source: String,
    raw: String,
    sha256: String,
    bytes: u64,
    #[serde(rename = "zstBytes")]
    zst_bytes: u64,
    lines: u64,
    first: String,
    last: String,
    modified: String,
}

/// Moves every finished log out of the game's Logs folder (uploading it
/// first when object storage is set up). Does nothing while the game runs,
/// since it appends to them; moving them is also how they get rotated.
fn archive_logs(s: &Settings, logs: &Path) -> Result<Option<String>, String> {
    let Ok(dir) = std::fs::read_dir(logs) else {
        return Ok(None);
    };
    let files: Vec<(PathBuf, String)> = dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let kind = if name.starts_with("WoWCombatLog") {
                "combat"
            } else if name.starts_with("WoWChatLog") {
                "chat"
            } else {
                return None;
            };
            (name.ends_with(".txt") && e.metadata().is_ok_and(|m| m.len() > 0))
                .then(|| (e.path(), kind.to_string()))
        })
        .collect();
    if files.is_empty() || platform::game_running() {
        return Ok(None);
    }
    let mut parts = vec![];
    for (path, kind) in files {
        let e = archive_log(s, &path, &kind).map_err(|e| format!("{}: {e}", path.display()))?;
        parts.push(format!("{} {:.1} MB", e.kind, e.bytes as f64 / 1e6));
        let manifest = s.archive_dir().join("logs").join("manifest.jsonl");
        std::fs::create_dir_all(manifest.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&manifest)
            .map_err(|e| e.to_string())?;
        writeln!(f, "{}", serde_json::to_string(&e).unwrap()).map_err(|e| e.to_string())?;
    }
    commit(&s.archive_dir(), &format!("logs: {}", parts.join(", ")))
}

fn archive_log(s: &Settings, path: &Path, kind: &str) -> Result<Entry, String> {
    use sha2::{Digest, Sha256};
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let modified: chrono::DateTime<chrono::Local> =
        meta.modified().map_err(|e| e.to_string())?.into();
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&data);
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let clip = |s: &str| s.chars().take(200).collect::<String>();
    let first = lines.next().map(clip).unwrap_or_default();
    let (count, last) = lines.fold(
        (u64::from(!first.is_empty()), first.clone()),
        |(n, _), l| (n + 1, clip(l)),
    );
    let sha = hex::encode(Sha256::digest(&data));
    let source = path.file_name().unwrap().to_string_lossy().to_string();
    let stamp = modified.format("%Y-%m-%dT%H%M%S").to_string();
    let mut e = Entry {
        key: String::new(),
        kind: kind.into(),
        source: source.clone(),
        raw: String::new(),
        sha256: sha.clone(),
        bytes: data.len() as u64,
        zst_bytes: 0,
        lines: count,
        first,
        last,
        modified: modified
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
    };
    if s.s3.ready() {
        let zst = zstd::encode_all(&data[..], 9).map_err(|e| e.to_string())?;
        e.zst_bytes = zst.len() as u64;
        e.key = format!(
            "{kind}/{}/{stamp}-{}.txt.zst",
            modified.format("%Y"),
            &sha[..12]
        );
        s3::put(
            &s.s3,
            &e.key,
            &zst,
            "application/zstd",
            &[("sha256", &sha), ("source", &source)],
        )?;
    }
    let raw_dir = s.raw_logs_dir().join(kind);
    std::fs::create_dir_all(&raw_dir).map_err(|e| e.to_string())?;
    let raw = raw_dir.join(format!("{stamp}-{source}"));
    move_file(path, &raw)?;
    e.raw = raw.to_string_lossy().into_owned();
    Ok(e)
}

fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to).map_err(|e| e.to_string())?;
    std::fs::remove_file(from).map_err(|e| e.to_string())
}

// ---- backup ----

/// Marks the last commit uploaded to object storage.
const BACKUP_REF: &str = "refs/backup/s3";

/// Uploads the commits made since the last backup as a git bundle to
/// git/<unix time>-<commit>.bundle. The first bundle holds the whole
/// history, every later one the commits since the one before: restoring is
/// cloning the first and pulling the others in name order. Needs git.
pub fn backup(dir: &Path, cfg: &S3) -> Result<Option<String>, String> {
    if !git_available() || !dir.join(".git").exists() {
        return Ok(None);
    }
    let Ok(head) = git(dir, &["rev-parse", "HEAD"]) else {
        return Ok(None);
    };
    let last = git(dir, &["rev-parse", "--verify", "-q", BACKUP_REF]).unwrap_or_default();
    if last == head {
        return Ok(None);
    }
    let branch = git(dir, &["symbolic-ref", "--short", "HEAD"])?;
    let tmp = std::env::temp_dir().join(format!("forever-memory-{}.bundle", std::process::id()));
    let range = if last.is_empty() {
        branch
    } else {
        format!("{last}..{branch}")
    };
    git(
        dir,
        &["bundle", "create", "-q", &tmp.to_string_lossy(), &range],
    )?;
    let body = std::fs::read(&tmp).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&tmp);
    let key = format!(
        "git/{}-{}.bundle",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        &head[..12]
    );
    s3::put(cfg, &key, &body?, "application/x-git-bundle", &[])?;
    git(dir, &["update-ref", BACKUP_REF, &head])?;
    Ok(Some(key))
}
