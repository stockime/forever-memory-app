//! CASC local storage: build info, local indices, the encoding table and the
//! TSFM root manifest that maps file data IDs to content keys. The layout
//! under the game folder (Data/config, Data/data) is the same on every OS.

use super::{be16, be32, blte, bytes, hash::name_hash, le32};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub(super) type Key = [u8; 16];

pub struct Storage {
    dir: PathBuf, // the install's Data directory
    pub version: String,
    pub build: HashMap<String, Vec<String>>,
    index: HashMap<[u8; 9], Location>,
    encoding: HashMap<Key, Key>, // content key -> first encoding key
    root: HashMap<u32, Key>,     // file data ID -> content key
    names: HashMap<u64, u32>,    // path name hash -> file data ID
    /// Where files the install hasn't downloaded yet come from.
    cdn: Option<super::cdn::Cdn>,
}

struct Location {
    archive: u16,
    offset: u32,
    size: u32,
}

const LOCALE_EN_US: u32 = 0x2;
const CONTENT_NO_NAME: u32 = 0x1000_0000;

impl Storage {
    /// Reads the install at `game_dir` (the folder holding .build.info) for
    /// the given product, e.g. "wow_classic_beta".
    pub fn open(game_dir: &Path, product: &str) -> Result<Storage, String> {
        let rows = build_info(game_dir)?;
        let row = rows
            .iter()
            .find(|r| r.get("Product").is_some_and(|p| p == product))
            .ok_or_else(|| format!("product {product:?} not in .build.info"))?;
        let mut s = Storage {
            dir: game_dir.join("Data"),
            version: row.get("Version").cloned().unwrap_or_default(),
            build: HashMap::new(),
            index: HashMap::new(),
            encoding: HashMap::new(),
            root: HashMap::new(),
            names: HashMap::new(),
            cdn: super::cdn::Cdn::new(
                &game_dir.join("Data"),
                row.get("CDN Hosts").map(String::as_str).unwrap_or(""),
                row.get("CDN Path").map(String::as_str).unwrap_or(""),
                crate::platform::cache_dir().join("cdn"),
            ),
        };
        let build_key = row.get("Build Key").map(String::as_str).unwrap_or("");
        s.build = s
            .read_config(build_key)
            .map_err(|e| format!("build config: {e}"))?;
        s.read_indices()?;
        s.read_encoding().map_err(|e| format!("encoding: {e}"))?;
        s.read_root().map_err(|e| format!("root: {e}"))?;
        Ok(s)
    }

    /// Returns a file by data ID.
    pub fn read(&self, fdid: u32) -> Result<Vec<u8>, String> {
        let ck = self
            .root
            .get(&fdid)
            .ok_or_else(|| format!("file {fdid} not in root"))?;
        let ek = self
            .encoding
            .get(ck)
            .ok_or_else(|| format!("file {fdid}: ckey {} not in encoding", hex(ck)))?;
        self.read_ekey(ek)
    }

    /// Resolves a game path through the manifest's name hashes.
    pub fn lookup(&self, path: &str) -> Option<u32> {
        self.names.get(&name_hash(path)).copied()
    }

    /// Returns a file by game path.
    pub fn read_path(&self, path: &str) -> Result<Vec<u8>, String> {
        let id = self
            .lookup(path)
            .ok_or_else(|| format!("{path}: no such file"))?;
        self.read(id)
    }

    pub fn has(&self, fdid: u32) -> bool {
        self.root.contains_key(&fdid)
    }

    /// The number of files in the manifest.
    pub fn count(&self) -> usize {
        self.root.len()
    }

    fn read_config(&self, key: &str) -> Result<HashMap<String, Vec<String>>, String> {
        if key.len() < 4 || !key.is_ascii() {
            return Err(format!("bad key {key:?}"));
        }
        let path = self
            .dir
            .join("config")
            .join(&key[0..2])
            .join(&key[2..4])
            .join(key);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(text
            .lines()
            .filter_map(|line| line.split_once(" = "))
            .filter(|(k, _)| !k.starts_with('#'))
            .map(|(k, v)| {
                (
                    k.to_string(),
                    v.split_whitespace().map(String::from).collect(),
                )
            })
            .collect())
    }

    /// Loads the newest .idx file of each of the 16 buckets.
    fn read_indices(&mut self) -> Result<(), String> {
        let data = self.dir.join("data");
        let mut names: Vec<String> = std::fs::read_dir(&data)
            .map_err(|e| format!("{}: {e}", data.display()))?
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.ends_with(".idx") && n.len() > 2 && n.is_ascii())
            .collect();
        names.sort(); // bucket + version, so the last of each bucket is the newest
        let mut latest: HashMap<String, String> = HashMap::new();
        for n in names {
            latest.insert(n[..2].to_string(), n);
        }
        for name in latest.values() {
            let b = std::fs::read(data.join(name)).map_err(|e| format!("{name}: {e}"))?;
            self.read_index(&b).map_err(|e| format!("{name}: {e}"))?;
        }
        if self.index.is_empty() {
            return Err("no local index entries".into());
        }
        Ok(())
    }

    /// An .idx v7 file: a header, then 18-byte entries of a 9-byte key
    /// prefix, a 5-byte big-endian archive/offset and a little-endian size.
    fn read_index(&mut self, b: &[u8]) -> Result<(), String> {
        let header_size = le32(b, 0)? as usize;
        let mut pos = (8 + header_size + 15) & !15;
        let end = pos + 8 + le32(b, pos)? as usize;
        pos += 8;
        while pos + 18 <= end {
            let e = bytes(b, pos, 18)?;
            let off = (e[9] as u64) << 32 | be32(e, 10)? as u64;
            self.index.insert(
                e[..9].try_into().unwrap(),
                Location {
                    archive: (off >> 30) as u16,
                    offset: (off & ((1 << 30) - 1)) as u32,
                    size: le32(e, 14)?,
                },
            );
            pos += 18;
        }
        Ok(())
    }

    /// The decoded contents of the blob with the given encoding key.
    fn read_ekey(&self, ekey: &Key) -> Result<Vec<u8>, String> {
        let Some(loc) = self.index.get(&ekey[..9]) else {
            // Not downloaded by the game yet: stream it like the game would.
            let cdn = self.cdn.as_ref().ok_or_else(|| format!("ekey {} not stored locally", hex(ekey)))?;
            return blte::decode(&cdn.fetch(ekey)?);
        };
        let path = self
            .dir
            .join("data")
            .join(format!("data.{:03}", loc.archive));
        let mut f = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut buf = vec![0; loc.size as usize];
        f.seek(SeekFrom::Start(loc.offset as u64))
            .and_then(|_| f.read_exact(&mut buf))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // 30-byte local header: reversed ekey, size, flags, checksums.
        blte::decode(buf.get(30..).ok_or("blob shorter than its header")?)
    }

    fn read_encoding(&mut self) -> Result<(), String> {
        let ekey = parse_key(self.build.get("encoding").and_then(|v| v.get(1)))?;
        let b = self.read_ekey(&ekey)?;
        if !b.starts_with(b"EN") {
            return Err("bad magic".into());
        }
        let page_size = be16(&b, 5)? as usize * 1024;
        let page_count = be32(&b, 9)? as usize;
        let espec_size = be32(&b, 18)? as usize;
        let pos = 22 + espec_size + page_count * 32;
        self.encoding = HashMap::with_capacity(page_count * 100);
        for p in 0..page_count {
            let page = bytes(&b, pos + p * page_size, page_size)?;
            let mut i = 0;
            while i + 22 + 16 <= page.len() {
                let n = page[i] as usize;
                if n == 0 {
                    break;
                }
                let ck = page[i + 6..i + 22].try_into().unwrap();
                let ek = page[i + 22..i + 38].try_into().unwrap();
                self.encoding.insert(ck, ek);
                i += 22 + n * 16;
            }
        }
        Ok(())
    }

    fn read_root(&mut self) -> Result<(), String> {
        let ckey = parse_key(self.build.get("root").and_then(|v| v.first()))?;
        let ekey = *self
            .encoding
            .get(&ckey)
            .ok_or("root ckey missing from encoding")?;
        let b = self.read_ekey(&ekey)?;
        if !b.starts_with(b"TSFM") {
            return Err("unsupported root format (want TSFM)".into());
        }
        let (mut pos, mut version) = (12, 1);
        let h = le32(&b, 4)?;
        if h == 20 || h == 24 {
            // header size + version since 10.1.7
            version = le32(&b, 8)?;
            pos = h as usize;
        }
        while pos < b.len() {
            let n = le32(&b, pos)? as usize;
            let (content, locale);
            if version >= 2 {
                locale = le32(&b, pos + 4)?;
                content = le32(&b, pos + 8)?
                    | le32(&b, pos + 12)?
                    | (bytes(&b, pos + 16, 1)?[0] as u32) << 17;
                pos += 17;
            } else {
                content = le32(&b, pos + 4)?;
                locale = le32(&b, pos + 8)?;
                pos += 12;
            }
            let deltas = bytes(&b, pos, n * 4)?;
            pos += n * 4;
            let keys = bytes(&b, pos, n * 16)?;
            pos += n * 16;
            let hashes = if content & CONTENT_NO_NAME == 0 {
                pos += n * 8;
                Some(bytes(&b, pos - n * 8, n * 8)?)
            } else {
                None
            };
            if locale & LOCALE_EN_US == 0 {
                continue;
            }
            let mut fdid: i64 = -1;
            for i in 0..n {
                fdid += 1 + le32(deltas, i * 4)? as i32 as i64;
                let id = fdid as u32;
                if self.root.contains_key(&id) {
                    continue;
                }
                self.root
                    .insert(id, keys[i * 16..i * 16 + 16].try_into().unwrap());
                if let Some(h) = hashes {
                    let hash = u64::from_le_bytes(h[i * 8..i * 8 + 8].try_into().unwrap());
                    self.names.insert(hash, id);
                }
            }
        }
        Ok(())
    }
}

/// The products installed under `game_dir` with their flavor folders, e.g.
/// ("wow_classic_beta", "_classic_beta_"), for choosing which one to read.
pub fn products(game_dir: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for row in build_info(game_dir).unwrap_or_default() {
        let Some(product) = row.get("Product").filter(|p| !p.is_empty()) else {
            continue;
        };
        if out.iter().any(|(p, _)| p == product) {
            continue;
        }
        if let Some(dir) = flavor_dir(game_dir, product) {
            out.push((product.clone(), dir));
        }
    }
    out
}

/// Each flavor folder names its product in .flavor.info; installs without
/// one fall back to the launcher's usual folder names.
fn flavor_dir(game_dir: &Path, product: &str) -> Option<String> {
    let entries = std::fs::read_dir(game_dir).ok()?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !(name.len() > 2 && name.starts_with('_') && name.ends_with('_')) {
            continue;
        }
        let info = std::fs::read_to_string(e.path().join(".flavor.info")).unwrap_or_default();
        if info.lines().skip(1).any(|l| l.trim() == product) {
            return Some(name);
        }
    }
    let dir = match product {
        "wow" => "_retail_".to_string(),
        "wowt" => "_ptr_".to_string(),
        "wowxptr" => "_xptr_".to_string(),
        p => format!("_{}_", p.strip_prefix("wow_")?),
    };
    game_dir.join(&dir).is_dir().then_some(dir)
}

/// The rows of .build.info, a pipe-separated table whose header names each
/// column as "Name!TYPE:size".
fn build_info(game_dir: &Path) -> Result<Vec<HashMap<String, String>>, String> {
    let path = game_dir.join(".build.info");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text.lines();
    let cols: Vec<&str> = lines
        .next()
        .unwrap_or("")
        .split('|')
        .map(|c| c.split('!').next().unwrap_or(c))
        .collect();
    Ok(lines
        .map(|line| {
            cols.iter()
                .zip(line.split('|'))
                .map(|(c, v)| (c.to_string(), v.to_string()))
                .collect()
        })
        .collect())
}

fn parse_key(h: Option<&String>) -> Result<Key, String> {
    let h = h.map(String::as_str).unwrap_or("");
    let bad = || format!("bad key {h:?}");
    if h.len() != 32 || !h.is_ascii() {
        return Err(bad());
    }
    let mut k = [0; 16];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).map_err(|_| bad())?;
    }
    Ok(k)
}

pub(super) fn hex(k: &[u8]) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}
