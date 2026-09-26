//! Diary narration with ElevenLabs. Each character gets one voice per
//! language, designed once from their race, class and personality note and
//! kept in the archive (characters/<slug>/voice.json, voice.de.json, ...), so
//! every entry is read in the same voice.
//! Spoken entries are cached as mp3 and only regenerated when the text changes.

use crate::data::memory::Character;
use crate::i18n::Lang;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

const API: &str = "https://api.elevenlabs.io/v1";
/// The most expressive model; reads up to 5,000 characters in one request.
const TTS_MODEL: &str = "eleven_v3";
const DESIGN_MODEL: &str = "eleven_ttv_v3";

pub fn api_key() -> Option<String> {
    crate::config::get().elevenlabs_key()
}

pub fn save_key(key: &str) -> Result<(), String> {
    crate::config::update(|s| s.elevenlabs_key = key.trim().to_string())
}

fn lang() -> Lang {
    crate::i18n::current()
}

#[derive(Clone, Debug, Default)]
pub struct Voice {
    pub voice_id: String,
    pub name: String,
    pub description: String,
}

pub fn voice_path(repo: &Path, c: &Character) -> PathBuf {
    let file = match lang() {
        Lang::En => "voice.json".to_string(),
        l => format!("voice.{}.json", l.code()),
    };
    repo.join("characters").join(&c.slug).join(file)
}

pub fn load_voice(repo: &Path, c: &Character) -> Option<Voice> {
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(voice_path(repo, c)).ok()?).ok()?;
    Some(Voice {
        voice_id: v.get("voice_id")?.as_str()?.to_string(),
        name: v
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        description: v
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

/// A voice description from what we know about the character.
/// How each race and gender sounds in the game, as a voice description.
/// The designed voice is in that style, not a copy of the game's actors.
fn in_game_style(race: &str, female: bool) -> &'static str {
    match (race, female) {
        ("Undead", false) => {
            "Forsaken undead man: raspy, hollow, gravelly half-whisper with a dry, sardonic, faintly sinister edge, like a corpse's dry throat, with a slight eerie echo"
        }
        ("Undead", true) => {
            "Forsaken undead woman: raspy, breathy and hollow, cold and eerie, dry and sardonic, with a slight eerie echo"
        }
        ("Orc", false) => "Orc man: very deep, guttural, gravelly and powerful, proud and blunt",
        ("Orc", true) => "Orc woman: husky, strong and gravelly, fierce and direct",
        ("Tauren", false) => {
            "Tauren man: extremely deep, slow, resonant and gentle, calm like the plains"
        }
        ("Tauren", true) => "Tauren woman: deep, warm, earthy and calm, slow and kind",
        ("Troll", false) => {
            "Jungle troll man: laid-back Caribbean island accent, raspy and playful, rolling rhythm"
        }
        ("Troll", true) => {
            "Jungle troll woman: lilting Caribbean island accent, sly, rhythmic and warm"
        }
        ("Dwarf", false) => {
            "Dwarf man: gruff, hearty and booming, thick Scottish accent, cheerful and stubborn"
        }
        ("Dwarf", true) => "Dwarf woman: hearty, bright and bold, thick Scottish accent, cheerful",
        ("Gnome", false) => "Gnome man: high-pitched, quick and nasal, excitable and clever",
        ("Gnome", true) => "Gnome woman: high, bright and squeaky, fast and cheerful",
        ("Night Elf", false) => {
            "Night elf man: deep, calm and measured, quiet and ancient, slightly ethereal"
        }
        ("Night Elf", true) => {
            "Night elf woman: soft, graceful and calm, mysterious and slightly ethereal"
        }
        (_, false) => {
            "Human man from Stormwind: clear, confident baritone, light old-world English accent, earnest"
        }
        (_, true) => {
            "Human woman from Stormwind: clear, warm and confident, light old-world English accent"
        }
    }
}

/// The race's internal name ("Scourge", "NightElf"), the same in every
/// client language, as the English name the voice styles use.
fn race(c: &Character) -> &str {
    match c.snapshot.get("raceFile").and_then(Value::as_str) {
        Some("Scourge") => "Undead",
        Some("NightElf") => "Night Elf",
        Some(r @ ("Human" | "Dwarf" | "Gnome" | "Orc" | "Troll" | "Tauren")) => r,
        _ => &c.race,
    }
}

fn female(c: &Character) -> bool {
    c.snapshot.get("sex").and_then(Value::as_i64) == Some(3)
}

/// The voice description: the race and gender's in-game style, then the
/// personality note.
pub fn describe(c: &Character) -> String {
    let mut d = format!(
        "{}. A {} reading their own travel journal aloud, unhurried and personal.",
        in_game_style(race(c), female(c)),
        c.class_file.to_lowercase()
    );
    if lang() != Lang::En {
        d += &format!(
            " Speaks {} as a native speaker, like the {} voice cast of the game.",
            lang().english_name(),
            lang().english_name()
        );
    }
    let note = c.personality.trim();
    if !note.is_empty() {
        d += &format!(" Character: {note}");
    }
    d.chars().take(990).collect()
}

/// The same race and gender start from the same seed, so voices of one
/// kind sound related, as they do in the game.
pub fn seed(c: &Character) -> u32 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (race(c), female(c), lang().code()).hash(&mut h);
    (h.finish() % 2_000_000_000) as u32
}

fn post(path: &str, key: &str, body: Value) -> Result<ureq::http::Response<ureq::Body>, String> {
    ureq::post(format!("{API}{path}"))
        .header("xi-api-key", key)
        .header("content-type", "application/json")
        .config()
        .timeout_global(Some(Duration::from_secs(300)))
        .http_status_as_error(false)
        .build()
        .send_json(body)
        .map_err(|e| format!("Could not reach ElevenLabs: {e}"))
}

fn check(
    mut resp: ureq::http::Response<ureq::Body>,
) -> Result<ureq::http::Response<ureq::Body>, String> {
    let status = resp.status().as_u16();
    if status == 200 {
        return Ok(resp);
    }
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let detail = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| {
            v.pointer("/detail/message")
                .or_else(|| v.get("detail"))
                .map(|d| {
                    d.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| d.to_string())
                })
        })
        .unwrap_or(text);
    Err(match status {
        401 => "ElevenLabs rejected the API key.".into(),
        _ => format!("ElevenLabs error {status}: {detail}"),
    })
}

/// Designs voices for a description; returns (generated voice id, preview mp3).
pub fn design(key: &str, description: &str, seed: u32) -> Result<Vec<(String, Vec<u8>)>, String> {
    // The preview is spoken in the diary's language, so the voice is designed for it.
    let body = match sample(lang()) {
        None => {
            json!({"voice_description": description, "model_id": DESIGN_MODEL, "auto_generate_text": true, "seed": seed})
        }
        Some(text) => {
            json!({"voice_description": description, "model_id": DESIGN_MODEL, "text": text, "seed": seed})
        }
    };
    let mut resp = check(post("/text-to-voice/design", key, body)?)?;
    let v: Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
    use base64::Engine;
    let previews = v
        .get("previews")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let out: Vec<(String, Vec<u8>)> = previews
        .iter()
        .filter_map(|p| {
            let id = p.get("generated_voice_id")?.as_str()?.to_string();
            let b64 = p
                .get("audio_base_64")
                .or_else(|| p.get("audio_base64"))?
                .as_str()?;
            Some((
                id,
                base64::engine::general_purpose::STANDARD.decode(b64).ok()?,
            ))
        })
        .collect();
    if out.is_empty() {
        return Err("ElevenLabs returned no voice previews.".into());
    }
    Ok(out)
}

/// A journal passage for designing a voice in a language other than English.
fn sample(l: Lang) -> Option<&'static str> {
    Some(match l {
        Lang::En => return None,
        Lang::De => {
            "Heute führte mich der Weg durch die alten Friedhöfe. Ich habe nicht gelacht, aber ich habe seine Worte aufgeschrieben, bevor ich irgendetwas anderes tat, denn ich ahne, dass ich sie noch brauchen werde. Das Licht hat mir geantwortet."
        }
        Lang::Fr => {
            "Aujourd'hui, la route m'a mené à travers les vieux cimetières. Je n'ai pas ri, mais j'ai noté ses paroles avant toute autre chose, car je sens que j'en aurai besoin. La Lumière m'a répondu, malgré ce que je suis devenu."
        }
        Lang::Es => {
            "Hoy el camino me llevó por los viejos cementerios. No me reí, pero escribí sus palabras antes que nada, porque sospecho que las voy a necesitar. La Luz me respondió, a pesar de lo que me he convertido."
        }
        Lang::Pt => {
            "Hoje a estrada me levou pelos velhos cemitérios. Eu não ri, mas anotei as palavras dele antes de qualquer outra coisa, porque desconfio que vou precisar delas. A Luz me respondeu, apesar do que me tornei."
        }
        Lang::Zh => {
            "今天，道路带我穿过了古老的墓地。我没有笑，但我在做任何事之前先把他的话写了下来，因为我预感自己终将需要它们。尽管我已变成如今的模样，圣光依然回应了我。我会继续向前走，直到找到答案为止。"
        }
    })
}

/// Keeps a designed voice for good; returns its voice id.
pub fn create(
    key: &str,
    name: &str,
    description: &str,
    generated_id: &str,
) -> Result<String, String> {
    let body = json!({"voice_name": name, "voice_description": description, "generated_voice_id": generated_id, "labels": {"app": "forever-memory"}});
    let mut resp = check(post("/text-to-voice", key, body)?)?;
    let v: Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
    v.get("voice_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "No voice id in the answer.".into())
}

pub fn save_voice(repo: &Path, c: &Character, voice: &Voice) -> Result<(), String> {
    let p = voice_path(repo, c);
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let body = json!({"voice_id": voice.voice_id, "name": voice.name, "description": voice.description, "created": chrono::Local::now().to_rfc3339()});
    std::fs::write(&p, serde_json::to_string_pretty(&body).unwrap()).map_err(|e| e.to_string())?;
    crate::data::diary::commit(repo, &p, &format!("voice: {}", c.name))
}

/// Speaks the text; returns mp3 bytes.
pub fn speak(key: &str, voice_id: &str, text: &str) -> Result<Vec<u8>, String> {
    let body = json!({"text": text, "model_id": TTS_MODEL});
    let mut resp = check(post(
        &format!("/text-to-speech/{voice_id}?output_format=mp3_44100_128"),
        key,
        body,
    )?)?;
    resp.body_mut()
        .with_config()
        .limit(64 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

/// The diary text as it should be read: the title, then the prose.
pub fn spoken_text(prose: &str) -> String {
    prose
        .split("\n\n")
        .map(|b| {
            b.trim()
                .trim_start_matches('#')
                .trim()
                .replace("**", "")
                .replace('*', "")
                .replace('\n', " ")
        })
        .filter(|b| !b.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Where the spoken entry is cached; the name changes with text and voice.
pub fn audio_path(c: &Character, day: &str, voice_id: &str, text: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (voice_id, text).hash(&mut h);
    crate::platform::data_dir()
        .join("audio")
        .join(&c.slug)
        .join(format!("{day}-{:016x}.mp3", h.finish()))
}

// ---- playback ----

/// One narration playing at a time.
pub struct Player {
    _sink: rodio::MixerDeviceSink,
    player: rodio::Player,
    pub key: String,
    pub length: f64,
}

impl Player {
    pub fn play(path: &Path, key: String) -> Result<Player, String> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("No audio output: {e}"))?;
        let player = rodio::Player::connect_new(sink.mixer());
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let source =
            rodio::Decoder::new_mp3(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
        player.append(source);
        let mut sink = sink;
        sink.log_on_drop(false);
        // 128 kbit/s mp3: bytes to seconds.
        Ok(Player {
            _sink: sink,
            player,
            key,
            length: len as f64 * 8.0 / 128_000.0,
        })
    }
    pub fn position(&self) -> f64 {
        self.player.get_pos().as_secs_f64()
    }
    pub fn finished(&self) -> bool {
        self.player.empty()
    }
    pub fn paused(&self) -> bool {
        self.player.is_paused()
    }
    pub fn toggle(&self) {
        if self.player.is_paused() {
            self.player.play()
        } else {
            self.player.pause()
        }
    }
}
