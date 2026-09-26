//! Settings, kept as JSON in the config folder. Everything has a working
//! default: the game install is found on its own, the archive lives in the
//! app's data folder, and object storage is off until it is set up.

use crate::i18n::Lang;
use crate::platform;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// The World of Warcraft folder (the one with Data/ and .build.info).
    pub game_dir: Option<PathBuf>,
    /// The client folder inside it, e.g. "_classic_beta_"; empty picks the
    /// one the addon saves into.
    pub flavor: String,
    /// The archive the recordings are written into.
    pub archive: Option<PathBuf>,
    /// Where finished chat and combat logs are moved between sessions.
    pub raw_logs: Option<PathBuf>,
    /// A language code; empty follows the game, then the system.
    pub language: String,
    /// Turns the addon's saves into the archive while the app runs.
    pub record: bool,
    /// Moves finished native logs out of the game folder between sessions.
    pub archive_logs: bool,
    pub writer: Writer,
    pub elevenlabs_key: String,
    /// How fast diary entries are read aloud.
    pub narration_speed: f32,
    pub s3: S3,
    /// The welcome has been seen.
    pub onboarded: bool,
    /// The Help page has been shown once (it opens on the first start).
    pub help_seen: bool,
    /// What the player calls their House; empty takes the head's surname.
    pub house_name: String,
    /// The emblem on the House's banner (a game icon); 0 for the default.
    pub house_emblem: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            game_dir: None,
            flavor: String::new(),
            archive: None,
            raw_logs: None,
            language: String::new(),
            record: true,
            archive_logs: true,
            writer: Writer::Auto,
            elevenlabs_key: String::new(),
            narration_speed: 1.0,
            s3: S3::default(),
            onboarded: false,
            help_seen: false,
            house_name: String::new(),
            house_emblem: 0,
        }
    }
}

/// Who writes the diary.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Writer {
    /// The first agent CLI or API found.
    #[default]
    Auto,
    /// An agent CLI: a preset name or a full command line.
    Cli {
        command: String,
    },
    /// Any OpenAI-compatible chat completions API.
    Api {
        base_url: String,
        api_key: String,
        model: String,
    },
    Off,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct S3 {
    pub enabled: bool,
    /// Host name, e.g. "fsn1.your-objectstorage.com" or "s3.eu-central-1.amazonaws.com".
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

impl S3 {
    pub fn ready(&self) -> bool {
        self.enabled
            && !self.endpoint.trim().is_empty()
            && !self.bucket.trim().is_empty()
            && !self.access_key.trim().is_empty()
            && !self.secret_key.trim().is_empty()
    }
}

pub fn file() -> PathBuf {
    platform::config_dir().join("settings.json")
}

impl Settings {
    pub fn load() -> Settings {
        match std::fs::read_to_string(file()) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => {
                let mut s = Settings::default();
                // The narration key from before there were settings.
                if let Ok(k) =
                    std::fs::read_to_string(platform::config_dir().join("elevenlabs-api-key"))
                {
                    s.elevenlabs_key = k.trim().to_string();
                }
                s
            }
        }
    }

    /// Saved readable only by the user, since it holds API keys.
    pub fn save(&self) -> Result<(), String> {
        let path = file();
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap())
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    /// The game install: the chosen one if it still exists, else the first found.
    pub fn install(&self) -> Option<PathBuf> {
        self.game_dir
            .clone()
            .filter(|d| platform::is_install(d))
            .or_else(|| platform::find_installs().into_iter().next())
    }

    pub fn flavor_in(&self, install: &Path) -> Option<String> {
        if !self.flavor.is_empty() && install.join(&self.flavor).is_dir() {
            return Some(self.flavor.clone());
        }
        platform::flavors(install).into_iter().next()
    }

    pub fn archive_dir(&self) -> PathBuf {
        if let Some(dir) = std::env::var_os("FM_ARCHIVE") {
            return dir.into(); // for tests and one-off runs
        }
        self.archive
            .clone()
            .unwrap_or_else(|| platform::data_dir().join("archive"))
    }

    pub fn raw_logs_dir(&self) -> PathBuf {
        self.raw_logs
            .clone()
            .unwrap_or_else(|| platform::data_dir().join("logs"))
    }

    /// The chosen language, else the game client's, else the system's.
    pub fn lang(&self) -> Lang {
        // FM_LANG wins, for screenshots and testing.
        if let Some(l) = std::env::var("FM_LANG")
            .ok()
            .and_then(|c| Lang::from_code(&c))
        {
            return l;
        }
        if let Some(l) = Lang::from_code(&self.language) {
            return l;
        }
        self.install()
            .and_then(|i| {
                let f = self.flavor_in(&i)?;
                platform::game_locale(&i, &f)
            })
            .and_then(|l| Lang::from_game_locale(&l))
            .unwrap_or_else(Lang::detect_os)
    }

    pub fn elevenlabs_key(&self) -> Option<String> {
        std::env::var("ELEVENLABS_API_KEY")
            .ok()
            .or_else(|| Some(self.elevenlabs_key.clone()))
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
    }
}

static SHARED: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<Settings>>> =
    std::sync::OnceLock::new();

/// The settings the whole app shares; loaded on first use.
pub fn shared() -> std::sync::Arc<std::sync::Mutex<Settings>> {
    SHARED
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(Settings::load())))
        .clone()
}

/// A copy of the current settings.
pub fn get() -> Settings {
    shared().lock().map(|s| s.clone()).unwrap_or_default()
}

/// Changes and saves the settings.
pub fn update(f: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let arc = shared();
    let mut s = arc.lock().map_err(|e| e.to_string())?;
    f(&mut s);
    s.save()
}
