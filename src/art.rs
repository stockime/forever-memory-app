//! Game art as textures. Images are read from the cache folder; missing ones
//! are rendered in the background from the local game install, then picked
//! up on the next frame.

use egui::{ColorImage, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// The game install and client folder the art comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    pub install: PathBuf,
    pub flavor: String,
}

impl Game {
    pub fn from_settings(s: &crate::config::Settings) -> Option<Game> {
        let install = s.install()?;
        let flavor = s.flavor_in(&install)?;
        Some(Game { install, flavor })
    }

    /// The product name .build.info lists for this client folder.
    fn product(&self) -> Option<String> {
        crate::gamedata::products(&self.install)
            .into_iter()
            .find(|(_, f)| f.trim_matches('_') == self.flavor.trim_matches('_'))
            .map(|(p, _)| p)
    }
}

/// Renders the keys that aren't in `out` yet. Opening the game data takes a
/// second and a lot of memory, so it is opened once per batch and let go.
pub fn render_missing(game: &Game, out: &Path, keys: &[String]) {
    let missing: Vec<&String> = keys.iter().filter(|k| !out.join(k).exists()).collect();
    if missing.is_empty() {
        return;
    }
    let Some(product) = game.product() else {
        eprintln!(
            "art: no product for {} in {}",
            game.flavor,
            game.install.display()
        );
        return;
    };
    let storage = match crate::gamedata::Storage::open(&game.install, &product) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("art: {e}");
            return;
        }
    };
    let _ = std::fs::create_dir_all(out);
    for k in missing {
        match crate::gamedata::render(&storage, k) {
            Ok(bytes) => {
                // Write then rename, so a half-written file is never read.
                let tmp = out.join(format!(".{k}.tmp"));
                if std::fs::write(&tmp, bytes).is_ok() {
                    let _ = std::fs::rename(&tmp, out.join(k));
                }
            }
            Err(e) => eprintln!("art: {k}: {e}"),
        }
    }
}

pub struct Art {
    dir: PathBuf,
    textures: HashMap<String, Option<TextureHandle>>,
    requested: HashSet<String>,
    tx: Sender<String>,
    done: Receiver<String>,
}

impl Art {
    pub fn new(ctx: &egui::Context, dir: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).ok();
        let (tx, rx) = channel::<String>();
        let (done_tx, done) = channel::<String>();
        let out = dir.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut keys = vec![first];
                while let Ok(k) = rx.recv_timeout(Duration::from_millis(300)) {
                    keys.push(k);
                }
                if let Some(game) = Game::from_settings(&crate::config::get()) {
                    render_missing(&game, &out, &keys);
                }
                for k in keys {
                    done_tx.send(k).ok();
                }
                ctx.request_repaint();
            }
        });
        Art {
            dir,
            textures: HashMap::new(),
            requested: HashSet::new(),
            tx,
            done,
        }
    }

    /// Forgets what was missing, e.g. after the game folder changed.
    pub fn retry(&mut self) {
        self.textures.retain(|_, t| t.is_some());
        self.requested.clear();
    }

    /// The texture for a key like "icon-135274.png", if it is ready.
    pub fn get(&mut self, ctx: &egui::Context, key: &str) -> Option<TextureHandle> {
        while let Ok(k) = self.done.try_recv() {
            self.textures.remove(&k); // look at the disk again
        }
        if let Some(t) = self.textures.get(key) {
            return t.clone();
        }
        let path = self.dir.join(key);
        if path.exists() {
            let tex = load(&path).map(|img| ctx.load_texture(key, img, TextureOptions::LINEAR));
            self.textures.insert(key.to_string(), tex.clone());
            return tex;
        }
        if self.requested.insert(key.to_string()) {
            self.tx.send(key.to_string()).ok();
        } else {
            self.textures.insert(key.to_string(), None); // rendered once already and still missing
        }
        None
    }

    pub fn icon(&mut self, ctx: &egui::Context, id: Option<i64>) -> Option<TextureHandle> {
        id.filter(|i| *i > 0)
            .and_then(|i| self.get(ctx, &format!("icon-{i}.png")))
    }
}

fn load(path: &PathBuf) -> Option<ColorImage> {
    let img = image::open(path).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}
