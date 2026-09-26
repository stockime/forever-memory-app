//! Parsed native logs, kept between loads. Archived logs never change, so
//! each is parsed once and stored under the cache folder in a small binary
//! format; the live logs in the game's folder are parsed as they grow, from
//! where the last load stopped.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::SystemTime;

/// Bump when a parser or the format changes; older entries are then ignored.
const FORMAT: u32 = 1;

// ---- the format: little-endian numbers, length-prefixed strings ----

#[derive(Default)]
pub struct W(pub Vec<u8>);

impl W {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn len(&mut self, n: usize) {
        self.u32(n as u32);
    }
    pub fn str(&mut self, s: &str) {
        self.len(s.len());
        self.0.extend_from_slice(s.as_bytes());
    }
}

pub struct R<'a>(pub &'a [u8]);

impl R<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    pub fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }
    pub fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }
    pub fn u64(&mut self) -> Option<u64> {
        self.take().map(u64::from_le_bytes)
    }
    pub fn i64(&mut self) -> Option<i64> {
        self.take().map(i64::from_le_bytes)
    }
    pub fn f64(&mut self) -> Option<f64> {
        self.take().map(f64::from_le_bytes)
    }
    /// A count, checked against what is left so bad data cannot make us
    /// allocate gigabytes (every element takes at least a byte).
    pub fn len(&mut self) -> Option<usize> {
        let n = self.u32()? as usize;
        (n <= self.0.len()).then_some(n)
    }
    pub fn str(&mut self) -> Option<String> {
        let n = self.len()?;
        let (s, rest) = self.0.split_at(n);
        self.0 = rest;
        String::from_utf8(s.to_vec()).ok()
    }
}

/// Something a log file parses into.
pub trait Cached: Sized + Send {
    const KIND: &'static str;
    fn new(path: &Path, me: &[String]) -> Self;
    /// Parses more of the file (for live files, whole lines as they come).
    fn parse(&mut self, text: &str, me: &[String]);
    /// Still right for these characters (combat parts depend on who is "me").
    fn fits(&self, _me: &[String]) -> bool {
        true
    }
    /// Still right for this file as it is now.
    fn current(&self, _path: &Path) -> bool {
        true
    }
    fn write(&self, w: &mut W);
    fn read(r: &mut R) -> Option<Self>;
}

/// What identifies a file's contents without reading it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Stat {
    len: u64,
    mtime: u128,
}

fn stat(path: &Path) -> Option<Stat> {
    let m = fs::metadata(path).ok()?;
    let mtime = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Some(Stat {
        len: m.len(),
        mtime,
    })
}

/// A parsed file. Live files remember where parsing stopped (after the last
/// whole line) and their first bytes, to notice when the game starts over.
struct Parsed<T> {
    stat: Stat,
    offset: u64,
    head: Vec<u8>,
    /// False when the file is not valid UTF-8; such files are skipped.
    ok: bool,
    part: T,
}

const HEAD: usize = 64;

/// The parsed logs of one raw logs and one live logs folder.
pub struct Logs {
    raw: PathBuf,
    live: PathBuf,
    combat: HashMap<PathBuf, Parsed<super::combat::Part>>,
    chat: HashMap<PathBuf, Parsed<super::chat::Part>>,
}

static KEPT: Mutex<Option<Logs>> = Mutex::new(None);

impl Logs {
    /// What the last load of these folders parsed, if anything; hand it back
    /// with `keep` when done.
    pub fn take(raw: &Path, live: &Path) -> Logs {
        let kept = KEPT.lock().unwrap_or_else(|e| e.into_inner()).take();
        kept.filter(|l| l.raw == raw && l.live == live)
            .unwrap_or_else(|| Logs {
                raw: raw.to_path_buf(),
                live: live.to_path_buf(),
                combat: HashMap::new(),
                chat: HashMap::new(),
            })
    }

    /// Keeps the live logs' parts for the next load. Archived ones come
    /// back from the disk cache fast enough, so memory is not spent on them.
    pub fn keep(mut self) {
        let live = self.live.clone();
        self.combat.retain(|f, _| f.parent() == Some(&live));
        self.chat.retain(|f, _| f.parent() == Some(&live));
        *KEPT.lock().unwrap_or_else(|e| e.into_inner()) = Some(self);
    }

    /// Brings the combat logs up to date; returns them in the order given.
    pub fn combat<'a>(
        &'a mut self,
        files: &'a [PathBuf],
        me: &[String],
        progress: &(dyn Fn(u64, u64) + Sync),
    ) -> Vec<(&'a PathBuf, &'a super::combat::Part)> {
        self.combat = update(
            std::mem::take(&mut self.combat),
            files,
            &self.live,
            me,
            &store(&self.raw),
            progress,
        );
        parts(&self.combat, files)
    }

    pub fn chat<'a>(
        &'a mut self,
        files: &'a [PathBuf],
    ) -> Vec<(&'a PathBuf, &'a super::chat::Part)> {
        self.chat = update(
            std::mem::take(&mut self.chat),
            files,
            &self.live,
            &[],
            &store(&self.raw),
            &|_, _| {},
        );
        parts(&self.chat, files)
    }

    /// Drops cache entries no archived file uses any more.
    pub fn prune(&self) {
        use super::{chat, combat};
        let used: HashSet<String> = (self.combat.iter())
            .map(|(f, p)| (f, p.stat, combat::Part::KIND))
            .chain(self.chat.iter().map(|(f, p)| (f, p.stat, chat::Part::KIND)))
            .filter(|(f, _, _)| f.parent() != Some(&self.live))
            .map(|(f, s, kind)| entry_name(kind, f, s))
            .collect();
        if let Ok(rd) = fs::read_dir(store(&self.raw)) {
            for f in rd.flatten() {
                if !used.contains(&*f.file_name().to_string_lossy()) {
                    fs::remove_file(f.path()).ok();
                }
            }
        }
    }
}

fn parts<'a, T>(
    kept: &'a HashMap<PathBuf, Parsed<T>>,
    files: &'a [PathBuf],
) -> Vec<(&'a PathBuf, &'a T)> {
    files
        .iter()
        .filter_map(|f| kept.get(f).filter(|p| p.ok).map(|p| (f, &p.part)))
        .collect()
}

/// The parts of `files`, worked out on all cores, reporting
/// `progress(bytes done, bytes total)` as they come in.
fn update<T: Cached>(
    mut kept: HashMap<PathBuf, Parsed<T>>,
    files: &[PathBuf],
    live: &Path,
    me: &[String],
    store: &Path,
    progress: &(dyn Fn(u64, u64) + Sync),
) -> HashMap<PathBuf, Parsed<T>> {
    let total: u64 = files.iter().filter_map(|f| stat(f)).map(|s| s.len).sum();
    let done = AtomicU64::new(0);
    let jobs: Vec<Mutex<Option<Parsed<T>>>> = files
        .iter()
        .map(|f| Mutex::new(kept.remove(f).filter(|p| p.part.fits(me))))
        .collect();
    let out: Mutex<HashMap<PathBuf, Parsed<T>>> = Mutex::default();
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(files.len())
        .max(1);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    let kept = job.lock().ok().and_then(|mut j| j.take());
                    let path = &files[i];
                    let parsed = if path.parent() == Some(live) {
                        live_part(path, kept, me)
                    } else {
                        archived_part(path, me, store)
                    };
                    let len = parsed.as_ref().map_or(0, |p| p.stat.len);
                    progress(done.fetch_add(len, Ordering::Relaxed) + len, total);
                    if let (Some(p), Ok(mut out)) = (parsed, out.lock()) {
                        out.insert(path.clone(), p);
                    }
                }
            });
        }
    });
    out.into_inner().unwrap_or_default()
}

/// An archived file, from the disk cache or parsed (and then cached).
fn archived_part<T: Cached>(path: &Path, me: &[String], store: &Path) -> Option<Parsed<T>> {
    let stat = stat(path)?;
    let entry = store.join(entry_name(T::KIND, path, stat));
    if let Some(p) = fs::read(&entry)
        .ok()
        .and_then(|b| decode::<T>(&b, path, stat))
        && p.part.fits(me)
    {
        return Some(p);
    }
    let bytes = fs::read(path).ok()?;
    let mut part = T::new(path, me);
    let ok = match std::str::from_utf8(&bytes) {
        Ok(text) => {
            part.parse(text, me);
            true
        }
        Err(_) => false,
    };
    let mut w = W::default();
    header(&mut w, T::KIND, path, stat);
    w.u8(ok as u8);
    part.write(&mut w);
    let tmp = entry.with_extension("tmp");
    if fs::create_dir_all(store).is_ok() && fs::write(&tmp, &w.0).is_ok() {
        fs::rename(&tmp, &entry).ok();
    }
    Some(Parsed {
        stat,
        offset: stat.len,
        head: vec![],
        ok,
        part,
    })
}

/// A live file: what was kept, plus the lines written since.
fn live_part<T: Cached>(path: &Path, kept: Option<Parsed<T>>, me: &[String]) -> Option<Parsed<T>> {
    let stat = stat(path)?;
    let mut f = fs::File::open(path).ok()?;
    let mut head = vec![0; HEAD.min(stat.len as usize)];
    f.read_exact(&mut head).ok()?;
    let mut p = match kept {
        Some(p) if p.offset <= stat.len && head.starts_with(&p.head) && p.part.current(path) => p,
        _ => Parsed {
            stat,
            offset: 0,
            head: vec![],
            ok: true,
            part: T::new(path, me),
        },
    };
    p.stat = stat;
    p.head = head;
    if !p.ok || p.offset == stat.len {
        return Some(p);
    }
    f.seek(SeekFrom::Start(p.offset)).ok()?;
    let mut bytes = vec![];
    f.take(stat.len - p.offset).read_to_end(&mut bytes).ok()?;
    // The game may be halfway through a line; that one waits for next time.
    let Some(end) = bytes.iter().rposition(|&b| b == b'\n') else {
        return Some(p);
    };
    match std::str::from_utf8(&bytes[..=end]) {
        Ok(text) => p.part.parse(text, me),
        Err(_) => p.ok = false,
    }
    p.offset += end as u64 + 1;
    Some(p)
}

fn entry_name(kind: &str, path: &Path, stat: Stat) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (FORMAT, kind, path, stat).hash(&mut h);
    format!("{kind}-{:016x}.bin", h.finish())
}

fn header(w: &mut W, kind: &str, path: &Path, stat: Stat) {
    w.u32(FORMAT);
    w.str(kind);
    w.str(&path.to_string_lossy());
    w.u64(stat.len);
    w.u64(stat.mtime as u64);
    w.u64((stat.mtime >> 64) as u64);
}

fn decode<T: Cached>(bytes: &[u8], path: &Path, stat: Stat) -> Option<Parsed<T>> {
    let mut want = W::default();
    header(&mut want, T::KIND, path, stat);
    let body = bytes.strip_prefix(want.0.as_slice())?;
    let mut r = R(body);
    let ok = r.u8()? == 1;
    let part = T::read(&mut r)?;
    r.0.is_empty().then_some(Parsed {
        stat,
        offset: stat.len,
        head: vec![],
        ok,
        part,
    })
}

/// The cache folder for one raw logs folder, so archives do not prune each
/// other's entries.
fn store(raw: &Path) -> PathBuf {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    raw.hash(&mut h);
    crate::platform::cache_dir()
        .join("parsed")
        .join(format!("{:016x}", h.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut w = W::default();
        w.u8(7);
        w.u32(1 << 20);
        w.i64(-5);
        w.f64(1.5);
        w.str("Grüße");
        let mut r = R(&w.0);
        assert_eq!(r.u8(), Some(7));
        assert_eq!(r.u32(), Some(1 << 20));
        assert_eq!(r.i64(), Some(-5));
        assert_eq!(r.f64(), Some(1.5));
        assert_eq!(r.str().as_deref(), Some("Grüße"));
        assert_eq!(r.u8(), None);
    }

    #[test]
    fn live_logs_parse_as_they_grow() {
        use super::super::chat;
        let dir = std::env::temp_dir().join(format!("fm-live-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("WoWChatLog.txt");
        let a =
            "9/26 10:53:53.460  [1. General] Wan: one\n9/26 10:54:00.000  Tom says: two\n9/26 10:5";
        fs::write(&f, a).unwrap();
        let p = live_part::<chat::Part>(&f, None, &[]).unwrap();
        assert_eq!(p.part.lines.len(), 2);
        fs::write(&f, format!("{a}5:00.000  Tom yells: three\n")).unwrap();
        let p = live_part::<chat::Part>(&f, Some(p), &[]).unwrap();
        let fresh = live_part::<chat::Part>(&f, None, &[]).unwrap();
        assert_eq!(
            format!("{:?}", p.part.lines),
            format!("{:?}", fresh.part.lines)
        );
        assert_eq!(p.part.lines.len(), 3);
        // The game started over: parsed from the top again.
        fs::write(&f, "9/27 09:00:00.000  Tom says: new day\n").unwrap();
        let p = live_part::<chat::Part>(&f, Some(p), &[]).unwrap();
        assert_eq!(p.part.lines.len(), 1);
        fs::remove_dir_all(&dir).ok();
    }
}
