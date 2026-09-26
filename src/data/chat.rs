//! Parses the game's chat log: "9/26 10:53:53.460  [1. General] Wan Heida: balance".
//! The lines carry no year; it comes from the file name (archived files start
//! with the date) or, for the live file, the file's modification time.

use super::cache::{Cached, R, W};
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Channel(String),
    Say,
    Yell,
    Whisper,
    WhisperTo,
    Group(String), // Party, Raid, Guild, Officer, …
    System,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub t: f64,
    pub kind: Kind,
    pub speaker: Option<String>,
    pub text: String,
}

impl Line {
    pub fn label(&self) -> String {
        match &self.kind {
            Kind::Channel(c) => c.split_once(". ").map(|(_, n)| n).unwrap_or(c).to_string(),
            Kind::Say => crate::tr!("Say").into(),
            Kind::Yell => crate::tr!("Yell").into(),
            Kind::Whisper => crate::tr!("Whisper").into(),
            Kind::WhisperTo => crate::tr!("Whisper to").into(),
            Kind::Group(g) => g.clone(),
            Kind::System => crate::tr!("System").into(),
        }
    }
}

/// The lines of one file, in file order.
pub struct Part {
    pub lines: Vec<Line>,
    year: i32,
}

fn year(f: &Path) -> i32 {
    let name = f
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    name.get(0..4)
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or_else(|| {
            fs::metadata(f)
                .and_then(|m| m.modified())
                .map(|t| chrono::DateTime::<Local>::from(t).year())
                .unwrap_or(2026)
        })
}

impl Cached for Part {
    const KIND: &'static str = "chat";

    fn new(path: &Path, _me: &[String]) -> Self {
        Part {
            lines: vec![],
            year: year(path),
        }
    }

    fn parse(&mut self, text: &str, _me: &[String]) {
        self.lines
            .extend(text.lines().filter_map(|raw| parse(raw, self.year)));
    }

    /// The live file takes its year from when it was last written.
    fn current(&self, path: &Path) -> bool {
        year(path) == self.year
    }

    fn write(&self, w: &mut W) {
        w.u32(self.year as u32);
        w.len(self.lines.len());
        for l in &self.lines {
            w.f64(l.t);
            match &l.kind {
                Kind::Channel(c) => {
                    w.u8(0);
                    w.str(c);
                }
                Kind::Group(g) => {
                    w.u8(1);
                    w.str(g);
                }
                Kind::Say => w.u8(2),
                Kind::Yell => w.u8(3),
                Kind::Whisper => w.u8(4),
                Kind::WhisperTo => w.u8(5),
                Kind::System => w.u8(6),
            }
            match &l.speaker {
                Some(s) => {
                    w.u8(1);
                    w.str(s);
                }
                None => w.u8(0),
            }
            w.str(&l.text);
        }
    }

    fn read(r: &mut R) -> Option<Self> {
        let year = r.u32()? as i32;
        let n = r.len()?;
        let mut lines = Vec::with_capacity(n);
        for _ in 0..n {
            let t = r.f64()?;
            let kind = match r.u8()? {
                0 => Kind::Channel(r.str()?),
                1 => Kind::Group(r.str()?),
                2 => Kind::Say,
                3 => Kind::Yell,
                4 => Kind::Whisper,
                5 => Kind::WhisperTo,
                6 => Kind::System,
                _ => return None,
            };
            let speaker = match r.u8()? {
                1 => Some(r.str()?),
                _ => None,
            };
            let text = r.str()?;
            lines.push(Line {
                t,
                kind,
                speaker,
                text,
            });
        }
        Some(Part { lines, year })
    }
}

fn parse(raw: &str, year: i32) -> Option<Line> {
    let (ts, rest) = raw.split_once("  ")?;
    let (date, time) = ts.split_once(' ')?;
    let (m, d) = date.split_once('/')?;
    let mut hms = time.split(':');
    let (h, min) = (hms.next()?.parse().ok()?, hms.next()?.parse().ok()?);
    let sec: f64 = hms.next()?.parse().ok()?;
    let naive = NaiveDate::from_ymd_opt(year, m.parse().ok()?, d.parse().ok()?)?
        .and_hms_opt(h, min, sec as u32)?;
    let t = Local.from_local_datetime(&naive).earliest()?.timestamp() as f64 + sec.fract();
    let rest = rest.trim();

    let line = |kind: Kind, speaker: &str, text: &str| Line {
        t,
        kind,
        speaker: Some(clean(speaker)),
        text: text.to_string(),
    };
    if let Some(r) = rest.strip_prefix('[')
        && let Some((tag, after)) = r.split_once("] ")
        && let Some((who, text)) = after.split_once(": ")
    {
        let kind = if tag.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            Kind::Channel(tag.to_string())
        } else {
            Kind::Group(tag.to_string())
        };
        return Some(line(kind, who, text));
    }
    for (sep, kind) in [
        (" says: ", Kind::Say),
        (" yells: ", Kind::Yell),
        (" whispers: ", Kind::Whisper),
    ] {
        if let Some((who, text)) = rest.split_once(sep)
            && (!who.contains(' ') || who.split(' ').count() <= 4)
        {
            return Some(line(kind, who, text));
        }
    }
    if let Some(r) = rest.strip_prefix("To ")
        && let Some((who, text)) = r.split_once(": ")
    {
        return Some(line(Kind::WhisperTo, who, text));
    }
    Some(Line {
        t,
        kind: Kind::System,
        speaker: None,
        text: rest.to_string(),
    })
}

/// Speakers can come wrapped as "[Name]" or with a realm suffix.
fn clean(s: &str) -> String {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    s.split('-').next().unwrap_or(s).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        let l = parse("9/26 10:53:53.460  [1. General] Wan Heida: balance", 2026).unwrap();
        assert_eq!(l.kind, Kind::Channel("1. General".into()));
        assert_eq!(l.speaker.as_deref(), Some("Wan Heida"));
        assert_eq!(l.label(), "General");
        let l = parse(
            "9/26 11:10:31.498  Deathguard Saltain says: Delicious pain.",
            2026,
        )
        .unwrap();
        assert_eq!(l.kind, Kind::Say);
        let l = parse("9/26 11:00:00.000  You receive loot: [Duskbat Wing].", 2026).unwrap();
        assert_eq!(l.kind, Kind::System);
    }
}
