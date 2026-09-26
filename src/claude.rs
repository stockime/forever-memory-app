//! Writes with Claude Code (`claude -p`, the CLI behind `cx`), using the
//! login it already has: no API key. Runs with no tools, no MCP servers and
//! no saved session, from an empty directory so no project settings apply.

use std::io::Write;
use std::process::{Command, Stdio};

pub const WRITER: &str = "Claude Code";

fn binary() -> String {
    // The installed binary itself: ~/.local/bin/claude is a mise wrapper
    // that rewrites the global mise config on every run.
    std::env::var("FM_CLAUDE").unwrap_or_else(|_| {
        let installed = format!(
            "{}/.local/share/mise/installs/claude/latest/claude",
            std::env::var("HOME").unwrap_or_default()
        );
        if std::path::Path::new(&installed).exists() {
            installed
        } else {
            "claude".into()
        }
    })
}

/// Whether Claude Code can be run; checked once.
pub fn available() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| {
        Command::new(binary())
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

/// Sends the prompt on stdin and returns the answer.
pub fn write(system: &str, prompt: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join("forever-memory-writer");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut child = Command::new(binary())
        .current_dir(&dir)
        .args([
            "-p",
            "--output-format",
            "text",
            "--tools",
            "",
            "--strict-mcp-config",
            "--no-session-persistence",
            "--system-prompt",
            system,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start claude: {e}"))?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(prompt.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || text.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "claude failed: {}",
            if err.trim().is_empty() {
                text.as_str()
            } else {
                err.trim()
            }
        ));
    }
    Ok(text)
}
