//! Parses the game's combat logs (Advanced Combat Logging, log version 22).
//!
//! Field layout after the event name: source (guid, name, flags, raid flags),
//! dest (same), then for SPELL_/RANGE_ events the spell (id, name, school),
//! then 19 advanced fields (unit guid, owner, hp, max hp, ap, sp, armor,
//! absorb, 5 power fields, x, y, uiMap, facing, level … as this client writes
//! them), then the suffix (amount first for damage and heals).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

const ADVANCED: usize = 19;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Player,
    Creature,
    Pet,
    Other,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub guid: String,
    pub name: String,
    pub kind: UnitKind,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f64,
    pub src: u32,
    pub dst: u32,
    pub spell: u32, // index into spells; 0 is melee
    pub amount: i64,
    pub over: i64, // overkill or overheal
    pub crit: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerSeen {
    pub first: f64,
    pub last: f64,
    pub lines: u64,
    pub spells: HashMap<u32, u32>,
    /// (time, uiMap, x, y) samples, at most one per 10 seconds.
    pub positions: Vec<(f64, i64, f64, f64)>,
    pub damage_to_me: i64,
    pub damage_from_me: i64,
    pub heal_to_me: i64,
    pub heal_from_me: i64,
    pub max_hp: i64,
}

#[derive(Default)]
pub struct Combat {
    pub units: Vec<Unit>,
    index: HashMap<String, u32>,
    pub spells: Vec<String>,
    spell_index: HashMap<String, u32>,
    pub dealt: Vec<Hit>,  // by me (or my pet)
    pub taken: Vec<Hit>,  // to me
    pub healed: Vec<Hit>, // by me
    pub kills: Vec<(f64, u32)>,
    pub deaths: Vec<f64>,
    pub players: HashMap<u32, PlayerSeen>,
    /// My own positions from the log: (time, uiMap, x, y).
    pub my_positions: Vec<(f64, i64, f64, f64)>,
    pub lines: u64,
    pub files: usize,
}

impl Combat {
    fn unit(&mut self, guid: &str, name: &str) -> u32 {
        if let Some(&i) = self.index.get(guid) {
            return i;
        }
        let kind = if guid.starts_with("Player-") {
            UnitKind::Player
        } else if guid.starts_with("Creature-") || guid.starts_with("Vehicle-") {
            UnitKind::Creature
        } else if guid.starts_with("Pet-") {
            UnitKind::Pet
        } else {
            UnitKind::Other
        };
        let name = name.trim_matches('"');
        // Players come as "First-Realm-"; the realm is noise here.
        let name = if kind == UnitKind::Player {
            name.split('-').next().unwrap_or(name)
        } else {
            name
        };
        let i = self.units.len() as u32;
        self.units.push(Unit {
            guid: guid.to_string(),
            name: name.to_string(),
            kind,
        });
        self.index.insert(guid.to_string(), i);
        i
    }

    fn spell(&mut self, name: &str) -> u32 {
        if self.spells.is_empty() {
            self.spells.push("Melee".into());
        }
        let name = name.trim_matches('"');
        if let Some(&i) = self.spell_index.get(name) {
            return i;
        }
        let i = self.spells.len() as u32;
        self.spells.push(name.to_string());
        self.spell_index.insert(name.to_string(), i);
        i
    }

    pub fn unit_name(&self, i: u32) -> &str {
        self.units
            .get(i as usize)
            .map(|u| u.name.as_str())
            .unwrap_or("?")
    }
    pub fn spell_name(&self, i: u32) -> &str {
        self.spells
            .get(i as usize)
            .map(String::as_str)
            .unwrap_or("?")
    }
}

/// Splits a combat log line's fields on commas outside quotes.
fn fields(s: &str) -> Vec<&str> {
    let mut out = Vec::with_capacity(48);
    let (mut start, mut quoted) = (0, false);
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b',' if !quoted => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// "9/26/2026 10:53:45.8742" in local time -> unix seconds.
pub fn parse_time(s: &str) -> Option<f64> {
    use chrono::{Local, NaiveDate, TimeZone};
    let (date, time) = s.split_once(' ')?;
    let mut d = date.split('/');
    let (m, day, y) = (
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
    );
    let mut t = time.split(':');
    let (h, min) = (t.next()?.parse().ok()?, t.next()?.parse().ok()?);
    let sec: f64 = t.next()?.parse().ok()?;
    let naive = NaiveDate::from_ymd_opt(y, m, day)?.and_hms_opt(h, min, sec as u32)?;
    let local = Local.from_local_datetime(&naive).earliest()?;
    Some(local.timestamp() as f64 + sec.fract())
}

pub fn load(files: &[std::path::PathBuf], me: &[String]) -> Combat {
    let mut c = Combat::default();
    c.spell("Melee");
    let mine: Vec<u32> = me.iter().map(|g| c.unit(g, "")).collect();
    let mut last_hit: HashMap<u32, f64> = HashMap::new();
    for f in files {
        if let Ok(text) = fs::read_to_string(f) {
            c.files += 1;
            parse(&mut c, &text, &mine, &mut last_hit);
        }
    }
    c.dealt.sort_by(|a, b| a.t.total_cmp(&b.t));
    c.taken.sort_by(|a, b| a.t.total_cmp(&b.t));
    c
}

fn parse(c: &mut Combat, text: &str, mine: &[u32], last_hit: &mut HashMap<u32, f64>) {
    let is_me = |u: u32| mine.contains(&u);
    for line in text.lines() {
        let Some((ts, rest)) = line.split_once("  ") else {
            continue;
        };
        let Some(t) = parse_time(ts) else { continue };
        let f = fields(rest);
        if f.len() < 9 {
            continue;
        }
        c.lines += 1;
        let event = f[0];
        let src = if f[1].starts_with("0000") || f[1] == "nil" {
            u32::MAX
        } else {
            c.unit(f[1], f[2])
        };
        let dst = if f[5].starts_with("0000") || f[5] == "nil" {
            u32::MAX
        } else {
            c.unit(f[5], f[6])
        };
        let spell_prefix = event.starts_with("SPELL_") || event.starts_with("RANGE_");
        let spell = if spell_prefix && f.len() > 10 {
            c.spell(f[10])
        } else {
            0
        };
        let adv = if spell_prefix { 12 } else { 9 };
        let amount_at = adv + ADVANCED;

        // Advanced info: position of the unit it describes.
        if f.len() > adv + 17 {
            let info = f[adv];
            if let (Ok(x), Ok(y), Ok(map)) = (
                f[adv + 14].parse::<f64>(),
                f[adv + 15].parse::<f64>(),
                f[adv + 16].parse::<i64>(),
            ) {
                if let Some(&u) = c.index.get(info) {
                    if is_me(u) {
                        if c.my_positions.last().map_or(true, |p| t - p.0 >= 2.0) {
                            c.my_positions.push((t, map, x, y));
                        }
                    } else if c.units[u as usize].kind == UnitKind::Player {
                        let p = c.players.entry(u).or_default();
                        if p.positions.last().map_or(true, |q| t - q.0 >= 10.0) {
                            p.positions.push((t, map, x, y));
                        }
                        if let Ok(hp) = f[adv + 3].parse::<i64>() {
                            p.max_hp = p.max_hp.max(hp);
                        }
                    }
                }
            }
        }

        for u in [src, dst] {
            if u != u32::MAX && !is_me(u) && c.units[u as usize].kind == UnitKind::Player {
                let p = c.players.entry(u).or_default();
                if p.first == 0.0 {
                    p.first = t;
                }
                p.last = t;
                p.lines += 1;
            }
        }

        let amount = |i: usize| f.get(i).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        let damage = matches!(
            event,
            "SWING_DAMAGE"
                | "SPELL_DAMAGE"
                | "SPELL_PERIODIC_DAMAGE"
                | "RANGE_DAMAGE"
                | "DAMAGE_SHIELD"
        );
        let heal = matches!(event, "SPELL_HEAL" | "SPELL_PERIODIC_HEAL");
        if damage || heal {
            let hit = Hit {
                t,
                src,
                dst,
                spell,
                amount: amount(amount_at),
                over: amount(amount_at + 2).max(0),
                // Damage: amount, base, overkill, school, resisted, blocked, absorbed, critical…
                // Heal: amount, base, overheal, absorbed, critical.
                crit: f.get(amount_at + if damage { 7 } else { 4 }) == Some(&"1"),
            };
            let src_player =
                src != u32::MAX && c.units[src as usize].kind == UnitKind::Player && !is_me(src);
            let dst_player =
                dst != u32::MAX && c.units[dst as usize].kind == UnitKind::Player && !is_me(dst);
            if damage {
                if src != u32::MAX && is_me(src) {
                    c.dealt.push(hit);
                    last_hit.insert(dst, t);
                    if dst_player {
                        c.players.entry(dst).or_default().damage_from_me += hit.amount;
                    }
                }
                if dst != u32::MAX && is_me(dst) {
                    c.taken.push(hit);
                    if src_player {
                        c.players.entry(src).or_default().damage_to_me += hit.amount;
                    }
                }
            } else {
                if src != u32::MAX && is_me(src) {
                    c.healed.push(hit);
                    if dst_player {
                        c.players.entry(dst).or_default().heal_from_me += hit.amount - hit.over;
                    }
                }
                if dst != u32::MAX && is_me(dst) && src_player {
                    c.players.entry(src).or_default().heal_to_me += hit.amount - hit.over;
                }
            }
        }
        match event {
            "SPELL_CAST_SUCCESS"
                if src != u32::MAX
                    && !is_me(src)
                    && c.units[src as usize].kind == UnitKind::Player =>
            {
                *c.players
                    .entry(src)
                    .or_default()
                    .spells
                    .entry(spell)
                    .or_default() += 1;
            }
            "UNIT_DIED" if dst != u32::MAX => {
                if is_me(dst) {
                    c.deaths.push(t);
                } else if last_hit.get(&dst).is_some_and(|&h| t - h < 20.0) {
                    c.kills.push((t, dst));
                }
            }
            _ => {}
        }
    }
}

/// A stretch of fighting: my damage with no gap longer than 6 seconds.
#[derive(Clone, Debug)]
pub struct Fight {
    pub start: f64,
    pub end: f64,
    pub from: usize,
    pub to: usize, // range in Combat::dealt
    pub damage: i64,
    pub target: String,
}

impl Fight {
    pub fn dps(&self) -> f64 {
        self.damage as f64 / (self.end - self.start).max(1.5)
    }
}

pub fn fights(c: &Combat) -> Vec<Fight> {
    let mut out: Vec<Fight> = vec![];
    for (i, h) in c.dealt.iter().enumerate() {
        match out.last_mut() {
            Some(f) if h.t - f.end <= 6.0 => {
                f.end = h.t;
                f.to = i + 1;
                f.damage += h.amount;
            }
            _ => out.push(Fight {
                start: h.t,
                end: h.t,
                from: i,
                to: i + 1,
                damage: h.amount,
                target: c.unit_name(h.dst).to_string(),
            }),
        }
    }
    out
}

/// A best guess at a player's class from the spells they cast.
pub fn guess_class(spells: &HashMap<u32, u32>, c: &Combat) -> Option<&'static str> {
    const SIGNS: &[(&str, &str)] = &[
        ("Fireball", "MAGE"),
        ("Frostbolt", "MAGE"),
        ("Arcane Missiles", "MAGE"),
        ("Arcane Intellect", "MAGE"),
        ("Frost Armor", "MAGE"),
        ("Fire Blast", "MAGE"),
        ("Shadow Bolt", "WARLOCK"),
        ("Immolate", "WARLOCK"),
        ("Corruption", "WARLOCK"),
        ("Curse of Agony", "WARLOCK"),
        ("Demon Skin", "WARLOCK"),
        ("Life Tap", "WARLOCK"),
        ("Summon Imp", "WARLOCK"),
        ("Smite", "PRIEST"),
        ("Lesser Heal", "PRIEST"),
        ("Power Word: Fortitude", "PRIEST"),
        ("Power Word: Shield", "PRIEST"),
        ("Shadow Word: Pain", "PRIEST"),
        ("Renew", "PRIEST"),
        ("Holy Light", "PALADIN"),
        ("Seal of Righteousness", "PALADIN"),
        ("Blessing of Might", "PALADIN"),
        ("Judgement", "PALADIN"),
        ("Devotion Aura", "PALADIN"),
        ("Holy Strike", "PALADIN"),
        ("Heroic Strike", "WARRIOR"),
        ("Battle Shout", "WARRIOR"),
        ("Charge", "WARRIOR"),
        ("Rend", "WARRIOR"),
        ("Thunder Clap", "WARRIOR"),
        ("Sinister Strike", "ROGUE"),
        ("Eviscerate", "ROGUE"),
        ("Stealth", "ROGUE"),
        ("Backstab", "ROGUE"),
        ("Raptor Strike", "HUNTER"),
        ("Auto Shot", "HUNTER"),
        ("Serpent Sting", "HUNTER"),
        ("Arcane Shot", "HUNTER"),
        ("Aspect of the Monkey", "HUNTER"),
        ("Hunter's Mark", "HUNTER"),
        ("Lightning Bolt", "SHAMAN"),
        ("Earth Shock", "SHAMAN"),
        ("Rockbiter Weapon", "SHAMAN"),
        ("Healing Wave", "SHAMAN"),
        ("Lightning Shield", "SHAMAN"),
        ("Wrath", "DRUID"),
        ("Moonfire", "DRUID"),
        ("Healing Touch", "DRUID"),
        ("Rejuvenation", "DRUID"),
        ("Mark of the Wild", "DRUID"),
        ("Thorns", "DRUID"),
    ];
    let mut votes: HashMap<&str, u32> = HashMap::new();
    for (&s, &n) in spells {
        let name = c.spell_name(s);
        if let Some((_, class)) = SIGNS.iter().find(|(sp, _)| *sp == name) {
            *votes.entry(class).or_default() += n;
        }
    }
    votes.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c)
}

pub fn log_files(dirs: &[&Path], prefix: &str) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for d in dirs {
        if let Ok(rd) = fs::read_dir(d) {
            for f in rd.flatten() {
                let n = f.file_name().to_string_lossy().to_string();
                if n.contains(prefix)
                    && n.ends_with(".txt")
                    && f.metadata().map(|m| m.len() > 0).unwrap_or(false)
                {
                    out.push(f.path());
                }
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_lines() {
        let log = "9/26/2026 10:53:48.3342  SPELL_DAMAGE,Pet-0-4615-0-44328-416-010024095A,\"Unknown\",0x1118,0x80000000,Creature-0-4615-0-2087-1502-00003787FF,\"Wretched Zombie\",0xa28,0x80000000,3110,\"Firebolt\",0x4,Creature-0-4615-0-2087-1502-00003787FF,0000000000000000,1,42,3,0,20,0,0,0,1,0,0,0,1898.94,1589.05,1415,3.0346,1,4,4,-1,4,0,0,0,nil,nil,nil,ST
9/26/2026 10:55:28.2642  SPELL_HEAL,Player-4613-00A46D50,\"Dead-ClassicBetaPvP2-\",0x511,0x80000000,Player-4613-00A46D50,\"Dead-ClassicBetaPvP2-\",0x511,0x80000000,635,\"Holy Light\",0x2,Player-4613-00A46D50,0000000000000000,104,104,56,0,167,0,0,0,0,103,103,0,1870.52,1491.64,1420,4.9584,0,41,41,26,0,nil
9/26/2026 10:55:30.0000  SWING_DAMAGE,Player-4613-00A46D50,\"Dead-ClassicBetaPvP2-\",0x511,0x80000000,Creature-0-1-2-3-4-5,\"Rattlecage Skeleton\",0xa48,0x0,Player-4613-00A46D50,0000000000000000,104,104,56,0,167,0,0,0,0,103,103,0,1870.52,1491.64,1420,4.9584,0,12,12,-1,1,0,0,0,nil,nil,nil
9/26/2026 10:55:31.0000  UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,Creature-0-1-2-3-4-5,\"Rattlecage Skeleton\",0xa48,0x0,0";
        let dir = std::env::temp_dir().join(format!("fm-combat-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("WoWCombatLog-test.txt");
        fs::write(&file, log).unwrap();
        let c = load(&[file], &["Player-4613-00A46D50".to_string()]);
        fs::remove_dir_all(&dir).ok();
        assert_eq!(c.lines, 4);
        assert_eq!(c.healed.len(), 1);
        assert_eq!(c.healed[0].amount, 41);
        assert_eq!(c.healed[0].over, 26);
        assert_eq!(c.dealt.len(), 1);
        assert_eq!(c.dealt[0].amount, 12);
        assert_eq!(c.kills.len(), 1);
        assert_eq!(c.unit_name(c.kills[0].1), "Rattlecage Skeleton");
        assert!(c.my_positions.iter().any(|p| p.1 == 1420));
    }
}
