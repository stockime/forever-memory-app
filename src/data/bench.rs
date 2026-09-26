//! Load timings on a big data set: FM_BENCH=<demo dir> cargo test --release
//! bench -- --ignored --nocapture. FM_DUMP=<file> also writes the model.

use super::*;
use std::fmt::Write;
use std::time::Instant;

fn paths(dir: &Path) -> Paths {
    Paths {
        repo: dir.join("archive"),
        raw_logs: dir.join("logs"),
        live_logs: dir.join("game/_classic_beta_/Logs"),
        art: dir.join("art"),
        game: None,
    }
}

/// Everything the pages read, in a stable order.
pub fn dump(m: &Model) -> String {
    let mut s = String::new();
    let c = &m.combat;
    writeln!(s, "characters {}", m.memory.characters.len()).ok();
    for ch in &m.memory.characters {
        writeln!(s, "{} {} events {}", ch.guid, ch.name, ch.events.len()).ok();
    }
    writeln!(s, "units {:?}", c.units).ok();
    writeln!(s, "spells {:?}", c.spells).ok();
    writeln!(s, "dealt {:?}", c.dealt).ok();
    writeln!(s, "taken {:?}", c.taken).ok();
    writeln!(s, "healed {:?}", c.healed).ok();
    writeln!(s, "kills {:?}", c.kills).ok();
    writeln!(s, "deaths {:?}", c.deaths).ok();
    let mut players: Vec<_> = c.players.iter().collect();
    players.sort_by_key(|(u, _)| **u);
    for (u, p) in players {
        let mut spells: Vec<_> = p.spells.iter().collect();
        spells.sort();
        writeln!(
            s,
            "player {u} {} {} {} {spells:?} {:?} {} {} {} {} {}",
            p.first,
            p.last,
            p.lines,
            p.positions,
            p.damage_to_me,
            p.damage_from_me,
            p.heal_to_me,
            p.heal_from_me,
            p.max_hp
        )
        .ok();
    }
    writeln!(s, "mine {:?}", c.my_positions).ok();
    writeln!(s, "lines {} files {}", c.lines, c.files).ok();
    writeln!(s, "fights {:?}", m.fights).ok();
    writeln!(s, "chat {:?}", m.chat).ok();
    for p in &m.players {
        writeln!(s, "{p:?}").ok();
    }
    s
}

/// A fresh start of the app: nothing kept in memory.
fn restart() {
    *ARCHIVE.lock().unwrap() = None;
    cache::Logs::take(Path::new(""), Path::new(""));
}

fn timed(what: &str, p: &Paths) -> Model {
    let t = Instant::now();
    let m = load(p, None);
    println!("{what}: {:?}", t.elapsed());
    m
}

#[test]
#[ignore]
fn bench() {
    let dir = PathBuf::from(std::env::var("FM_BENCH").expect("FM_BENCH=<demo dir>"));
    let p = paths(&dir);
    std::fs::remove_dir_all(crate::platform::cache_dir().join("parsed")).ok();
    restart();
    let m = timed("cold", &p);
    println!("{} lines, {} chat lines", m.combat.lines, m.chat.len());
    let cold = dump(&m);
    if let Ok(out) = std::env::var("FM_DUMP") {
        std::fs::write(out, &cold).unwrap();
    }
    drop(m);
    restart();
    let m = timed("warm (disk cache)", &p);
    assert!(dump(&m) == cold, "warm load differs");
    drop(m);
    timed("reload, nothing changed", &p);

    // A save was recorded.
    let state = p.repo.join("state.json");
    let f = std::fs::File::options().append(true).open(&state).unwrap();
    f.set_modified(std::time::SystemTime::now()).unwrap();
    timed("reload after a save", &p);

    // The game wrote more to its live logs.
    let live = combat::log_files(&[&p.live_logs], "WoWCombatLog");
    let live = live.last().expect("a live combat log");
    let before = std::fs::read(live).unwrap();
    let text = String::from_utf8_lossy(&before).to_string();
    let more: String = text
        .lines()
        .take(20000)
        .map(|l| l.replacen("9/26/2026 1", "9/26/2026 2", 1) + "\n")
        .collect();
    std::fs::write(live, [before.clone(), more.into_bytes()].concat()).unwrap();
    let grown = dump(&timed("reload after the live log grew", &p));
    restart();
    let fresh = dump(&timed("the same, from scratch (disk cache)", &p));
    std::fs::write(live, &before).unwrap();
    assert!(grown == fresh, "incremental load differs");
}

#[test]
#[ignore]
fn bench_memory() {
    let p = paths(&PathBuf::from(std::env::var("FM_BENCH").unwrap()));
    let a = load(&p, None);
    let b = load(&p, None);
    println!("loaded: {} {}", a.combat.lines, b.combat.lines);
}
