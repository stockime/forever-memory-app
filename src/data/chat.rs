//! Parses the game's chat log: "9/26 10:53:53.460  [1. General] Wan Heida: balance".
//! The lines carry no year; it comes from the file name (archived files start
//! with the date) or, for the live file, the file's modification time.

use chrono::{Datelike, Local, NaiveDate, TimeZone};
use std::fs;
use std::path::PathBuf;

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

pub fn load(files: &[PathBuf]) -> Vec<Line> {
    let mut out = vec![];
    for f in files {
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let year = name
            .get(0..4)
            .and_then(|y| y.parse::<i32>().ok())
            .unwrap_or_else(|| {
                fs::metadata(f)
                    .and_then(|m| m.modified())
                    .map(|t| chrono::DateTime::<Local>::from(t).year())
                    .unwrap_or(2026)
            });
        let Ok(text) = fs::read_to_string(f) else {
            continue;
        };
        for raw in text.lines() {
            if let Some(l) = parse(raw, year) {
                out.push(l);
            }
        }
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    out
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
    if let Some(r) = rest.strip_prefix('[') {
        if let Some((tag, after)) = r.split_once("] ") {
            if let Some((who, text)) = after.split_once(": ") {
                let kind = if tag.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    Kind::Channel(tag.to_string())
                } else {
                    Kind::Group(tag.to_string())
                };
                return Some(line(kind, who, text));
            }
        }
    }
    for (sep, kind) in [
        (" says: ", Kind::Say),
        (" yells: ", Kind::Yell),
        (" whispers: ", Kind::Whisper),
    ] {
        if let Some((who, text)) = rest.split_once(sep) {
            if !who.contains(' ') || who.split(' ').count() <= 4 {
                return Some(line(kind, who, text));
            }
        }
    }
    if let Some(r) = rest.strip_prefix("To ") {
        if let Some((who, text)) = r.split_once(": ") {
            return Some(line(Kind::WhisperTo, who, text));
        }
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
