//! Who writes the diary: an agent CLI that is already logged in (Claude Code,
//! Codex, Gemini CLI or any command that reads a prompt on stdin), or any
//! OpenAI-compatible API, hosted or local (Ollama, LM Studio). Both are found
//! on their own; the settings can pick one.

use crate::config::Writer;
use serde_json::{Value, json};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

/// Agent CLIs the app knows how to run headless. In the argument templates,
/// {system} is the system prompt and {out} a file the answer is written to;
/// without {system} the system prompt goes on stdin before the prompt.
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub binary: &'static str,
    pub args: &'static [&'static str],
}

pub const PRESETS: [Preset; 3] = [
    Preset {
        id: "claude",
        name: "Claude Code",
        binary: "claude",
        // No tools, MCP servers or saved session.
        args: &[
            "-p",
            "--output-format",
            "text",
            "--tools",
            "",
            "--strict-mcp-config",
            "--no-session-persistence",
            "--system-prompt",
            "{system}",
        ],
    },
    Preset {
        id: "codex",
        name: "Codex",
        binary: "codex",
        args: &[
            "exec",
            "--skip-git-repo-check",
            "--ephemeral",
            "--sandbox",
            "read-only",
            "--color",
            "never",
            "-o",
            "{out}",
            "-",
        ],
    },
    Preset {
        id: "gemini",
        name: "Gemini CLI",
        binary: "gemini",
        args: &[
            "-o",
            "text",
            "--approval-mode",
            "plan",
            "-p",
            "Write it now, following the instructions above.",
        ],
    },
];

/// Finds a program on PATH (with .exe/.cmd on Windows).
pub fn which(name: &str) -> Option<PathBuf> {
    // Claude Code under mise: the installed binary itself, since the shim
    // rewrites the global mise config on every run.
    if name == "claude" {
        let installed =
            crate::platform::home().join(".local/share/mise/installs/claude/latest/claude");
        if installed.is_file() {
            return Some(installed);
        }
    }
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for ext in exts {
            let p = dir.join(format!("{name}{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// The agent CLIs installed here.
pub fn detect_clis() -> Vec<(&'static Preset, PathBuf)> {
    PRESETS
        .iter()
        .filter_map(|p| Some((p, which(p.binary)?)))
        .collect()
}

/// An OpenAI-compatible API found from the environment or running locally.
#[derive(Clone, Debug)]
pub struct Api {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// API keys in the environment, then local servers that answer.
pub fn detect_apis() -> Vec<Api> {
    let mut out = vec![];
    for (var, name, base, model) in [
        (
            "OPENAI_API_KEY",
            "OpenAI",
            "https://api.openai.com/v1",
            "gpt-5-mini",
        ),
        (
            "OPENROUTER_API_KEY",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
            "openai/gpt-5-mini",
        ),
        (
            "DEEPSEEK_API_KEY",
            "DeepSeek",
            "https://api.deepseek.com/v1",
            "deepseek-chat",
        ),
        (
            "MISTRAL_API_KEY",
            "Mistral",
            "https://api.mistral.ai/v1",
            "mistral-medium-latest",
        ),
    ] {
        if let Some(key) = std::env::var(var).ok().filter(|k| !k.trim().is_empty()) {
            out.push(Api {
                name: name.into(),
                base_url: base.into(),
                api_key: key,
                model: model.into(),
            });
        }
    }
    for (name, base) in [
        ("Ollama", "http://localhost:11434/v1"),
        ("LM Studio", "http://localhost:1234/v1"),
    ] {
        if let Ok(models) = list_models(base, "", Duration::from_millis(400)) {
            if let Some(m) = models.into_iter().next() {
                out.push(Api {
                    name: name.into(),
                    base_url: base.into(),
                    api_key: String::new(),
                    model: m,
                });
            }
        }
    }
    out
}

/// The models an API offers.
pub fn list_models(base_url: &str, key: &str, timeout: Duration) -> Result<Vec<String>, String> {
    let mut req = ureq::get(format!("{}/models", base_url.trim_end_matches('/')));
    if !key.is_empty() {
        req = req.header("authorization", &format!("Bearer {key}"));
    }
    let mut resp = req
        .config()
        .timeout_global(Some(timeout))
        .build()
        .call()
        .map_err(|e| e.to_string())?;
    let v: Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = v
        .get("data")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| m.get("id")?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    Ok(ids)
}

/// What will actually run.
#[derive(Clone, Debug)]
pub enum Resolved {
    Cli {
        name: String,
        program: PathBuf,
        args: Vec<String>,
    },
    Api(Api),
}

impl Resolved {
    pub fn name(&self) -> String {
        match self {
            Resolved::Cli { name, .. } => name.clone(),
            Resolved::Api(a) => format!("{} ({})", a.name, a.model),
        }
    }
}

fn preset(p: &Preset, program: PathBuf) -> Resolved {
    Resolved::Cli {
        name: p.name.into(),
        program,
        args: p.args.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn resolve(w: &Writer) -> Option<Resolved> {
    match w {
        Writer::Off => None,
        Writer::Auto => {
            if let Some((p, bin)) = detect_clis().into_iter().next() {
                return Some(preset(p, bin));
            }
            detect_apis().into_iter().next().map(Resolved::Api)
        }
        Writer::Cli { command } => {
            if let Some(p) = PRESETS.iter().find(|p| p.id == command.trim()) {
                return Some(preset(p, which(p.binary)?));
            }
            let mut words = shell_words::split(command).ok()?.into_iter();
            let first = words.next()?;
            let program = if Path::new(&first).is_file() {
                PathBuf::from(&first)
            } else {
                which(&first)?
            };
            Some(Resolved::Cli {
                name: first,
                program,
                args: words.collect(),
            })
        }
        Writer::Api {
            base_url,
            api_key,
            model,
        } => {
            if base_url.trim().is_empty() || model.trim().is_empty() {
                return None;
            }
            let name = url_host(base_url);
            Some(Resolved::Api(Api {
                name,
                base_url: base_url.trim().into(),
                api_key: api_key.trim().into(),
                model: model.trim().into(),
            }))
        }
    }
}

fn url_host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

/// Writes: the system prompt sets the voice, the prompt holds the facts.
pub fn write(r: &Resolved, system: &str, prompt: &str) -> Result<String, String> {
    match r {
        Resolved::Cli {
            name,
            program,
            args,
        } => run_cli(name, program, args, system, prompt),
        Resolved::Api(a) => call_api(a, system, prompt),
    }
}

fn run_cli(
    name: &str,
    program: &Path,
    args: &[String],
    system: &str,
    prompt: &str,
) -> Result<String, String> {
    // From an empty folder, so no project instructions or settings apply.
    let dir = std::env::temp_dir().join("forever-memory-writer");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out_file = dir.join(format!("answer-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out_file);
    let takes_system = args.iter().any(|a| a.contains("{system}"));
    let args: Vec<String> = args
        .iter()
        .map(|a| {
            a.replace("{system}", system)
                .replace("{out}", &out_file.to_string_lossy())
        })
        .collect();
    let input = if takes_system {
        prompt.to_string()
    } else {
        format!("{system}\n\n---\n\n{prompt}")
    };
    let mut cmd = crate::platform::command(program);
    cmd.current_dir(&dir)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start {name}: {e}"))?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = match std::fs::read_to_string(&out_file) {
        Ok(t) => {
            let _ = std::fs::remove_file(&out_file);
            t
        }
        Err(_) => String::from_utf8_lossy(&out.stdout).to_string(),
    };
    let text = text.trim().to_string();
    if !out.status.success() || text.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(format!(
            "{name} failed: {}",
            if err.is_empty() { text.as_str() } else { err }
        ));
    }
    Ok(text)
}

fn call_api(a: &Api, system: &str, prompt: &str) -> Result<String, String> {
    let body = json!({
        "model": a.model,
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": prompt}],
    });
    let mut req = ureq::post(format!(
        "{}/chat/completions",
        a.base_url.trim_end_matches('/')
    ));
    if !a.api_key.is_empty() {
        req = req.header("authorization", &format!("Bearer {}", a.api_key));
    }
    let mut resp = req
        .config()
        .timeout_global(Some(Duration::from_secs(600)))
        .http_status_as_error(false)
        .build()
        .send_json(body)
        .map_err(|e| format!("Could not reach {}: {e}", a.name))?;
    let status = resp.status().as_u16();
    let v: Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
    if status != 200 {
        let msg = v
            .pointer("/error/message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| v.to_string());
        return Err(format!("{} error {status}: {msg}", a.name));
    }
    let text = v
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    // Reasoning models served locally sometimes leave their thinking in.
    let text = match text.rfind("</think>") {
        Some(i) => text[i + 8..].trim(),
        None => text,
    };
    if text.is_empty() {
        return Err(format!("{} returned nothing.", a.name));
    }
    Ok(text.to_string())
}
