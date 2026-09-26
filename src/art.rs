//! Game art as textures. Images are read from the cache directory; missing
//! ones are rendered in the background by `wowdata art` from the local game
//! install, then picked up on the next frame.

use egui::{ColorImage, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

pub struct Art {
    dir: PathBuf,
    textures: HashMap<String, Option<TextureHandle>>,
    requested: HashSet<String>,
    tx: Sender<String>,
    done: Receiver<String>,
}

impl Art {
    pub fn new(ctx: &egui::Context, dir: PathBuf, wowdata: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).ok();
        let (tx, rx) = channel::<String>();
        let (done_tx, done) = channel::<String>();
        let out = dir.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // Batch requests: opening the game data takes a second and ~900 MB.
            while let Ok(first) = rx.recv() {
                let mut keys = vec![first];
                while let Ok(k) = rx.recv_timeout(Duration::from_millis(300)) {
                    keys.push(k);
                }
                let status = std::process::Command::new(&wowdata)
                    .arg("art")
                    .args(&keys)
                    .env("OUT", &out)
                    .stderr(std::process::Stdio::null())
                    .status();
                if status.is_err() {
                    eprintln!("art: {} not runnable; icons stay blank", wowdata.display());
                }
                for k in keys {
                    done_tx.send(k).ok();
                }
                ctx.request_repaint();
            }
        });
        Art { dir, textures: HashMap::new(), requested: HashSet::new(), tx, done }
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
        id.filter(|i| *i > 0).and_then(|i| self.get(ctx, &format!("icon-{i}.png")))
    }
}

fn load(path: &PathBuf) -> Option<ColorImage> {
    let img = image::open(path).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}
