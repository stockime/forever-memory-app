//! The armory addon ships inside the app, so it can be installed (and kept
//! up to date) into the game's AddOns folder with one click.

use std::path::{Path, PathBuf};

const FILES: [(&str, &[u8]); 3] = [
    ("armory.toc", include_bytes!("../addon/armory.toc")),
    ("armory.lua", include_bytes!("../addon/armory.lua")),
    ("memory.lua", include_bytes!("../addon/memory.lua")),
];

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Missing,
    /// A link to a working copy, left alone.
    Linked(PathBuf),
    Installed {
        version: String,
    },
}

/// Where the game looks for it; the folder must be called "armory".
pub fn dir(install: &Path, flavor: &str) -> PathBuf {
    install
        .join(flavor)
        .join("Interface")
        .join("AddOns")
        .join("armory")
}

fn version(toc: &str) -> String {
    toc.lines()
        .find_map(|l| l.strip_prefix("## Version:"))
        .map(|v| v.trim().to_string())
        .unwrap_or_default()
}

pub fn bundled_version() -> String {
    version(std::str::from_utf8(FILES[0].1).unwrap_or(""))
}

pub fn state(dir: &Path) -> State {
    if let Ok(target) = std::fs::read_link(dir) {
        return State::Linked(target);
    }
    match std::fs::read_to_string(dir.join("armory.toc")) {
        Ok(toc) => State::Installed {
            version: version(&toc),
        },
        Err(_) => State::Missing,
    }
}

/// Whether the bundled addon is newer than the installed one.
pub fn outdated(state: &State) -> bool {
    let parse = |v: &str| {
        v.split('.')
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    matches!(state, State::Installed { version } if parse(version) < parse(&bundled_version()))
}

pub fn install(dir: &Path) -> Result<(), String> {
    if let State::Linked(t) = state(dir) {
        return Err(format!(
            "{} links to {}; not overwriting it.",
            dir.display(),
            t.display()
        ));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, bytes) in FILES {
        std::fs::write(dir.join(name), bytes).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn installs() {
        let dir = std::env::temp_dir().join(format!("fm-addon-{}", std::process::id())).join("armory");
        assert_eq!(super::state(&dir), super::State::Missing);
        super::install(&dir).unwrap();
        assert_eq!(super::state(&dir), super::State::Installed { version: super::bundled_version() });
        assert!(!super::outdated(&super::state(&dir)));
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}
