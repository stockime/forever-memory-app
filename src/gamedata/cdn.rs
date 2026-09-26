//! Files the install doesn't hold yet, fetched from Blizzard's CDN the way
//! the game streams them: the install's own archive indices (Data/indices)
//! say which CDN archive holds a blob and where, and one ranged request gets
//! just that blob. Fetched blobs are cached, so each is downloaded once.

use super::casc::Key;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

pub struct Cdn {
    hosts: Vec<String>,
    path: String,
    indices_dir: PathBuf,
    cache: PathBuf,
    indices: OnceLock<Vec<Index>>,
}

/// One archive index: the archive's name and the last key of every block,
/// for a binary search that touches a single 4 KB block per lookup.
struct Index {
    name: String,
    file: PathBuf,
    /// Loose files (named by their own key) rather than blobs in an archive.
    loose: bool,
    block_size: usize,
    entry_size: usize,
    offset_bytes: usize,
    last_keys: Vec<Key>,
}

struct Found {
    archive: String,
    loose: bool,
    offset: u64,
    size: u64,
}

impl Cdn {
    pub fn new(data_dir: &Path, hosts: &str, path: &str, cache: PathBuf) -> Option<Cdn> {
        let hosts: Vec<String> = hosts.split_whitespace().map(str::to_string).collect();
        if hosts.is_empty() || path.is_empty() {
            return None;
        }
        Some(Cdn {
            hosts,
            path: path.trim_matches('/').to_string(),
            indices_dir: data_dir.join("indices"),
            cache,
            indices: OnceLock::new(),
        })
    }

    fn indices(&self) -> &[Index] {
        self.indices.get_or_init(|| {
            let mut out = vec![];
            for e in std::fs::read_dir(&self.indices_dir).into_iter().flatten().flatten() {
                let p = e.path();
                let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if p.extension().is_some_and(|x| x == "index") {
                    if let Some(i) = read_toc(&p, name) {
                        out.push(i);
                    }
                }
            }
            out
        })
    }

    fn find(&self, ekey: &Key) -> Option<Found> {
        for idx in self.indices() {
            let block = idx.last_keys.partition_point(|k| k < ekey);
            if block >= idx.last_keys.len() {
                continue;
            }
            let mut f = File::open(&idx.file).ok()?;
            let mut buf = vec![0; idx.block_size];
            if f.seek(SeekFrom::Start((block * idx.block_size) as u64)).is_err() || f.read_exact(&mut buf).is_err() {
                continue;
            }
            for e in buf.chunks_exact(idx.entry_size) {
                if &e[..16] == ekey {
                    let size = u32::from_be_bytes(e[16..20].try_into().unwrap()) as u64;
                    let mut offset = 0u64;
                    for b in &e[20..20 + idx.offset_bytes] {
                        offset = offset << 8 | *b as u64;
                    }
                    return Some(Found { archive: idx.name.clone(), loose: idx.loose, offset, size });
                }
            }
        }
        None
    }

    /// The raw BLTE blob for an encoding key.
    pub fn fetch(&self, ekey: &Key) -> Result<Vec<u8>, String> {
        let name = super::casc::hex(ekey);
        let cached = self.cache.join(&name[..2]).join(&name);
        if let Ok(b) = std::fs::read(&cached) {
            return Ok(b);
        }
        let found = self.find(ekey).ok_or_else(|| format!("ekey {name} is neither local nor in the CDN indices"))?;
        let (file, range) = if found.loose {
            (name.clone(), None)
        } else {
            (found.archive.clone(), Some((found.offset, found.offset + found.size - 1)))
        };
        let mut last = String::new();
        for host in &self.hosts {
            let url = format!("http://{host}/{}/data/{}/{}/{file}", self.path, &file[..2], &file[2..4]);
            let mut req = ureq::get(&url);
            if let Some((a, b)) = range {
                req = req.header("range", &format!("bytes={a}-{b}"));
            }
            match req.config().timeout_global(Some(Duration::from_secs(60))).build().call() {
                Ok(mut resp) => {
                    let body = resp
                        .body_mut()
                        .with_config()
                        .limit(512 * 1024 * 1024)
                        .read_to_vec()
                        .map_err(|e| e.to_string())?;
                    let _ = std::fs::create_dir_all(cached.parent().unwrap());
                    let _ = std::fs::write(&cached, &body);
                    return Ok(body);
                }
                Err(e) => last = format!("{host}: {e}"),
            }
        }
        Err(format!("CDN: {last}"))
    }
}

fn read_toc(path: &Path, name: String) -> Option<Index> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len() as usize;
    if len < 28 {
        return None;
    }
    let mut footer = [0u8; 28];
    f.seek(SeekFrom::Start((len - 28) as u64)).ok()?;
    f.read_exact(&mut footer).ok()?;
    // Footer: toc hash, version, 2 unknown, block size in KB, offset bytes,
    // size bytes, key size, checksum size, element count, footer hash.
    let (block_kb, offset_bytes, size_bytes, key_size, checksum) =
        (footer[11] as usize, footer[12] as usize, footer[13] as usize, footer[14] as usize, footer[15] as usize);
    if key_size != 16 || size_bytes != 4 || checksum != 8 || !(offset_bytes == 0 || offset_bytes == 4) {
        return None; // archive groups and other layouts aren't needed
    }
    let block_size = block_kb * 1024;
    let blocks = (len - 28) / (block_size + 16 + 8);
    let mut toc = vec![0u8; blocks * 16];
    f.seek(SeekFrom::Start((blocks * block_size) as u64)).ok()?;
    f.read_exact(&mut toc).ok()?;
    let last_keys = toc.chunks_exact(16).map(|c| c.try_into().unwrap()).collect();
    Some(Index {
        name,
        file: path.to_path_buf(),
        loose: offset_bytes == 0,
        block_size,
        entry_size: 16 + size_bytes + offset_bytes,
        offset_bytes,
        last_keys,
    })
}

#[cfg(test)]
mod tests {
    /// cargo test --release cdn_fetch -- --ignored --nocapture
    #[test]
    #[ignore]
    fn cdn_fetch() {
        let game = crate::platform::home().join("Games/battlenet/drive_c/Program Files (x86)/World of Warcraft");
        let s = super::super::Storage::open(&game, "wow_classic_beta").unwrap();
        let t = std::time::Instant::now();
        // character/scourge/male/scourgemale_hd.m2: not in this install until played.
        let b = s.read(959310).unwrap();
        println!("{} bytes, starts {:?}, in {:?}", b.len(), &b[..4], t.elapsed());
        assert_eq!(&b[..4], b"MD21");
    }
}
