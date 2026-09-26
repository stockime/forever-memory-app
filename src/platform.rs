//! Where things live on Linux, macOS and Windows: the app's own folders, the
//! game install (found in Blizzard's usual places and the common Wine setups),
//! and whether the game is running.

use std::path::{Path, PathBuf};

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("forever-memory")
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("forever-memory")
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("forever-memory")
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

/// A folder is a WoW install when it has the game data next to .build.info.
pub fn is_install(dir: &Path) -> bool {
    dir.join(".build.info").is_file() && dir.join("Data").is_dir()
}

/// Every WoW install found in the usual places, most likely first.
pub fn find_installs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = vec![];
    let mut add = |p: PathBuf| {
        // The same install is often reachable through a linked prefix.
        let p = std::fs::canonicalize(&p).unwrap_or(p);
        if is_install(&p) && !out.contains(&p) {
            out.push(p);
        }
    };
    let wow = |base: PathBuf| {
        [
            base.join("Program Files (x86)/World of Warcraft"),
            base.join("Program Files/World of Warcraft"),
            base.join("World of Warcraft"),
            base.join("Games/World of Warcraft"),
        ]
    };
    if cfg!(windows) {
        for drive in b'C'..=b'H' {
            for p in wow(PathBuf::from(format!("{}:\\", drive as char))) {
                add(p);
            }
        }
    }
    if cfg!(target_os = "macos") {
        add(PathBuf::from("/Applications/World of Warcraft"));
        add(home().join("Applications/World of Warcraft"));
    }
    if cfg!(target_os = "linux") {
        // Wine prefixes: Lutris, Bottles, Steam (Proton), plain Wine.
        let h = home();
        let mut prefixes = vec![h.join(".wine")];
        for parent in [
            h.join("Games"),
            h.join(".local/share/bottles/bottles"),
            h.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
            h.join(".local/share/Steam/steamapps/compatdata"),
            h.join(".steam/steam/steamapps/compatdata"),
            h.join(".local/share/lutris/prefixes"),
        ] {
            for entry in std::fs::read_dir(&parent).into_iter().flatten().flatten() {
                let p = entry.path();
                prefixes.push(p.join("pfx"));
                prefixes.push(p);
            }
        }
        for pre in prefixes {
            for p in wow(pre.join("drive_c")) {
                add(p);
            }
        }
    }
    out
}

/// Game client folders in an install that the armory addon has saved into
/// (or could), e.g. "_classic_beta_"; the ones with saves first.
pub fn flavors(install: &Path) -> Vec<String> {
    let mut out: Vec<(bool, String)> = std::fs::read_dir(install)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            (name.len() > 2 && name.starts_with('_') && name.ends_with('_') && e.path().is_dir())
                .then(|| (!saved_variables(install, &name).is_empty(), name))
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|(_, n)| n).collect()
}

/// The addon's SavedVariables files, one per account.
pub fn saved_variables(install: &Path, flavor: &str) -> Vec<PathBuf> {
    let accounts = install.join(flavor).join("WTF").join("Account");
    std::fs::read_dir(accounts)
        .into_iter()
        .flatten()
        .flatten()
        .map(|a| a.path().join("SavedVariables").join("armory.lua"))
        .filter(|p| p.is_file())
        .collect()
}

/// The client's text language from WTF/Config.wtf (`SET textLocale "deDE"`).
pub fn game_locale(install: &Path, flavor: &str) -> Option<String> {
    let cfg = std::fs::read_to_string(install.join(flavor).join("WTF").join("Config.wtf")).ok()?;
    cfg.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("SET textLocale ")?;
        Some(rest.trim().trim_matches('"').to_string())
    })
}

/// Whether the game client is running (the launcher doesn't count). The
/// client appends to its logs, so they are only archived when it is not.
pub fn game_running() -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(sysinfo::UpdateKind::Always),
    );
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy().to_lowercase();
        let exe = p
            .cmd()
            .first()
            .map(|c| c.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        is_client(&name) || is_client(exe.rsplit(['/', '\\']).next().unwrap_or(""))
    })
}

fn is_client(name: &str) -> bool {
    // Wow.exe, WowB.exe, WowClassic.exe, WowClassicB.exe; "World of Warcraft Classic" on macOS.
    let exe = name.strip_suffix(".exe");
    exe.is_some_and(|e| e.starts_with("wow") && e[3..].chars().all(|c| c.is_ascii_alphabetic()))
        || name.starts_with("world of warcraft")
}

/// A command that opens no console window on Windows.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// Opens a folder in the system's file manager.
pub fn reveal(path: &Path) {
    let cmd = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(path).spawn();
}

#[cfg(test)]
mod tests {
    #[test]
    fn clients() {
        for n in [
            "wow.exe",
            "wowb.exe",
            "wowclassic.exe",
            "wowclassicb.exe",
            "world of warcraft classic",
        ] {
            assert!(super::is_client(n), "{n}");
        }
        for n in [
            "battle.net.exe",
            "wowlauncher-helper.exe",
            "firewall",
            "wowdata",
        ] {
            assert!(!super::is_client(n), "{n}");
        }
    }
}
