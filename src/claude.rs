//! One call to the Claude API (raw HTTP: Rust has no official Anthropic SDK).
//! Refused requests are re-run server-side on Anthropic's recommended model
//! (`fallbacks: "default"`).

use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

pub const MODEL: &str = "claude-opus-5";

/// The key comes from ANTHROPIC_API_KEY or ~/.config/forever-memory/anthropic-api-key.
pub fn key_file() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/forever-memory/anthropic-api-key")
}

pub fn api_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .or_else(|| std::fs::read_to_string(key_file()).ok())
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

pub fn save_key(key: &str) -> std::io::Result<()> {
    let path = key_file();
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, key.trim())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Sends one system + user message and returns the text of the answer.
pub fn write(key: &str, system: &str, user: &str) -> Result<String, String> {
    let body = json!({
        "model": MODEL,
        "max_tokens": 16000,
        "thinking": {"type": "adaptive"},
        "fallbacks": "default",
        "system": system,
        "messages": [{"role": "user", "content": user}],
    });
    let mut resp = ureq::post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "server-side-fallback-2026-07-01")
        .header("content-type", "application/json")
        .config()
        .timeout_global(Some(Duration::from_secs(600)))
        .http_status_as_error(false)
        .build()
        .send_json(body)
        .map_err(|e| format!("Could not reach the Claude API: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("Unexpected answer ({status}): {text}"))?;
    if status != 200 {
        let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or(&text);
        return Err(match status {
            401 => "The API key was rejected. Paste a valid one.".to_string(),
            429 => "Rate limited by the Claude API; try again in a minute.".to_string(),
            _ => format!("Claude API error {status}: {msg}"),
        });
    }
    match v.get("stop_reason").and_then(Value::as_str) {
        Some("refusal") => return Err("Claude declined to write this entry.".into()),
        Some("max_tokens") => return Err("The entry ran past the length limit; try again.".into()),
        _ => {}
    }
    let out: String = v
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| blocks.iter().filter(|b| b.get("type").and_then(Value::as_str) == Some("text")).filter_map(|b| b.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join(""))
        .unwrap_or_default();
    if out.trim().is_empty() {
        return Err("The answer came back empty.".into());
    }
    Ok(out.trim().to_string())
}
