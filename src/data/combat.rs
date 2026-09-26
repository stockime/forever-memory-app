//! Parses the game's combat logs (Advanced Combat Logging, log version 22).
//!
//! Field layout after the event name: source (guid, name, flags, raid flags),
//! dest (same), then for SPELL_/RANGE_ events the spell (id, name, school),
//! then 19 advanced fields (unit guid, owner, hp, max hp, ap, sp, armor,
//! absorb, 5 power fields, x, y, uiMap, facing, level … as this client writes
//! them), then the suffix (amount first for damage and heals).

use std::collections::{BTreeSet, HashMap};
use std::fs;
use super::cache::{Cached, R, W};
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

#[derive(Clone, Default)]
pub struct Combat {
    pub units: Vec<Unit>,
    index: HashMap<String, u32>,
    pub spells: Vec<String>,
    spell_index: HashMap<String, u32>,
    pub dealt: Vec<Hit>,  // by me (or my pet)
    pub taken: Vec<Hit>,  // to me
    pub healed: Vec<Hit>, // by me
    /// SPELL_CAST_SUCCESS by me (amount 0), for spells that neither hit nor heal.
    pub casts: Vec<Hit>,
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

/// One file parsed on its own, with its own unit and spell tables. What
/// depends on the files before it (kills, position sampling) is settled
/// when it joins the whole.
#[derive(Default)]
pub struct Part {
    c: Combat,
    /// My characters' GUIDs when parsed, sorted.
    me: Vec<String>,
    last_hit: HashMap<u32, f64>,
    loose: Loose,
}

impl Cached for Part {
    const KIND: &'static str = "combat";

    fn new(_path: &Path, me: &[String]) -> Self {
        let mut p = Part::default();
        p.c.spell("Melee");
        p.c.files = 1;
        p.me = sorted(me);
        for g in me {
            p.c.unit(g, "");
        }
        p
    }

    fn parse(&mut self, text: &str, me: &[String]) {
        let mine: Vec<u32> = me.iter().map(|g| self.c.unit(g, "")).collect();
        self.me = sorted(me);
        parse(
            &mut self.c,
            text,
            &mine,
            &mut self.last_hit,
            Some(&mut self.loose),
        );
    }

    /// Only characters were added since, and none of them is in this file.
    fn fits(&self, me: &[String]) -> bool {
        let named = |g: &String| self.c.index.contains_key(g) || self.loose.unnamed.contains(g);
        self.me.iter().all(|g| me.contains(g))
            && me.iter().all(|g| self.me.contains(g) || !named(g))
    }

    fn write(&self, w: &mut W) {
        let c = &self.c;
        w.len(c.units.len());
        for u in &c.units {
            w.str(&u.guid);
            w.str(&u.name);
            w.u8(u.kind as u8);
        }
        w.len(c.spells.len());
        for s in &c.spells {
            w.str(s);
        }
        for hits in [&c.dealt, &c.taken, &c.healed, &c.casts] {
            w.len(hits.len());
            for h in hits {
                w.f64(h.t);
                w.u32(h.src);
                w.u32(h.dst);
                w.u32(h.spell);
                w.i64(h.amount);
                w.i64(h.over);
                w.u8(h.crit as u8);
            }
        }
        w.len(c.deaths.len());
        for &t in &c.deaths {
            w.f64(t);
        }
        let positions = |w: &mut W, ps: &[(f64, i64, f64, f64)]| {
            w.len(ps.len());
            for &(t, map, x, y) in ps {
                w.f64(t);
                w.i64(map);
                w.f64(x);
                w.f64(y);
            }
        };
        w.len(c.players.len());
        for (&u, p) in &c.players {
            w.u32(u);
            w.f64(p.first);
            w.f64(p.last);
            w.u64(p.lines);
            w.len(p.spells.len());
            for (&s, &n) in &p.spells {
                w.u32(s);
                w.u32(n);
            }
            positions(w, &p.positions);
            for v in [p.damage_to_me, p.damage_from_me, p.heal_to_me, p.heal_from_me, p.max_hp] {
                w.i64(v);
            }
        }
        positions(w, &c.my_positions);
        w.u64(c.lines);
        w.len(c.files);
        w.len(self.me.len());
        for g in &self.me {
            w.str(g);
        }
        w.len(self.loose.died.len());
        for &(t, u, before) in &self.loose.died {
            w.f64(t);
            w.u32(u);
            w.f64(before.unwrap_or(f64::NAN));
        }
        w.len(self.loose.unnamed.len());
        for g in &self.loose.unnamed {
            w.str(g);
        }
    }

    fn read(r: &mut R) -> Option<Self> {
        let mut p = Part::default();
        let c = &mut p.c;
        for i in 0..r.len()? {
            let (guid, name) = (r.str()?, r.str()?);
            let kind = match r.u8()? {
                0 => UnitKind::Player,
                1 => UnitKind::Creature,
                2 => UnitKind::Pet,
                _ => UnitKind::Other,
            };
            c.index.insert(guid.clone(), i as u32);
            c.units.push(Unit { guid, name, kind });
        }
        for i in 0..r.len()? {
            let s = r.str()?;
            if i > 0 {
                c.spell_index.insert(s.clone(), i as u32);
            }
            c.spells.push(s);
        }
        for hits in [&mut c.dealt, &mut c.taken, &mut c.healed, &mut c.casts] {
            for _ in 0..r.len()? {
                hits.push(Hit {
                    t: r.f64()?,
                    src: r.u32()?,
                    dst: r.u32()?,
                    spell: r.u32()?,
                    amount: r.i64()?,
                    over: r.i64()?,
                    crit: r.u8()? == 1,
                });
            }
        }
        for _ in 0..r.len()? {
            c.deaths.push(r.f64()?);
        }
        let positions = |r: &mut R| -> Option<Vec<(f64, i64, f64, f64)>> {
            (0..r.len()?)
                .map(|_| Some((r.f64()?, r.i64()?, r.f64()?, r.f64()?)))
                .collect()
        };
        for _ in 0..r.len()? {
            let u = r.u32()?;
            let mut s = PlayerSeen {
                first: r.f64()?,
                last: r.f64()?,
                lines: r.u64()?,
                ..Default::default()
            };
            for _ in 0..r.len()? {
                s.spells.insert(r.u32()?, r.u32()?);
            }
            s.positions = positions(r)?;
            s.damage_to_me = r.i64()?;
            s.damage_from_me = r.i64()?;
            s.heal_to_me = r.i64()?;
            s.heal_from_me = r.i64()?;
            s.max_hp = r.i64()?;
            c.players.insert(u, s);
        }
        c.my_positions = positions(r)?;
        c.lines = r.u64()?;
        c.files = r.len()?;
        for _ in 0..r.len()? {
            p.me.push(r.str()?);
        }
        for _ in 0..r.len()? {
            let (t, u, before) = (r.f64()?, r.u32()?, r.f64()?);
            p.loose.died.push((t, u, (!before.is_nan()).then_some(before)));
        }
        for _ in 0..r.len()? {
            p.loose.unnamed.insert(r.str()?);
        }
        // Only needed to parse on, which archived files never do.
        for h in &p.c.dealt {
            p.last_hit.insert(h.dst, h.t);
        }
        let n = p.c.units.len() as u32;
        let ok = |u: u32| u == u32::MAX || u < n;
        let spells = p.c.spells.len() as u32;
        let valid = [&p.c.dealt, &p.c.taken, &p.c.healed, &p.c.casts]
            .iter()
            .flat_map(|h| h.iter())
            .all(|h| ok(h.src) && ok(h.dst) && h.spell < spells)
            && p.c.players.iter().all(|(&u, s)| u < n && s.spells.keys().all(|&k| k < spells))
            && p.loose.died.iter().all(|d| d.1 < n);
        valid.then_some(p)
    }
}

fn sorted(me: &[String]) -> Vec<String> {
    let mut v = me.to_vec();
    v.sort();
    v.dedup();
    v
}

/// The whole, put together from parts in file order.
pub struct Whole {
    c: Combat,
    mine: Vec<u32>,
    last_hit: HashMap<u32, f64>,
}

impl Whole {
    pub fn new(me: &[String]) -> Self {
        let mut c = Combat::default();
        c.spell("Melee");
        let mine = me.iter().map(|g| c.unit(g, "")).collect();
        Whole {
            c,
            mine,
            last_hit: HashMap::new(),
        }
    }

    /// Adds the next file. In the rare case its part cannot be joined as is
    /// (see `joins`), the file's `text` is parsed again against the whole.
    pub fn add(&mut self, p: &Part, text: impl FnOnce() -> String) {
        if !self.joins(p) {
            self.c.files += 1;
            parse(&mut self.c, &text(), &self.mine, &mut self.last_hit, None);
            return;
        }
        let c = &mut self.c;
        let units: Vec<u32> =
            p.c.units
                .iter()
                .map(|u| match c.index.get(&u.guid) {
                    Some(&i) => i,
                    None => {
                        let i = c.units.len() as u32;
                        c.units.push(u.clone());
                        c.index.insert(u.guid.clone(), i);
                        i
                    }
                })
                .collect();
        let spells: Vec<u32> =
            p.c.spells
                .iter()
                .enumerate()
                .map(|(i, s)| if i == 0 { 0 } else { c.spell(s) })
                .collect();
        let unit = |u: u32| if u == u32::MAX { u } else { units[u as usize] };
        let hit = |h: &Hit| Hit {
            src: unit(h.src),
            dst: unit(h.dst),
            spell: spells[h.spell as usize],
            ..*h
        };
        for &(t, u, before) in &p.loose.died {
            let u = unit(u);
            if before
                .or_else(|| self.last_hit.get(&u).copied())
                .is_some_and(|h| t - h < 20.0)
            {
                c.kills.push((t, u));
            }
        }
        for h in &p.c.dealt {
            let h = hit(h);
            self.last_hit.insert(h.dst, h.t);
            c.dealt.push(h);
        }
        c.taken.extend(p.c.taken.iter().map(hit));
        c.healed.extend(p.c.healed.iter().map(hit));
        c.casts.extend(p.c.casts.iter().map(hit));
        c.deaths.extend_from_slice(&p.c.deaths);
        c.my_positions.extend_from_slice(&p.c.my_positions);
        c.lines += p.c.lines;
        c.files += p.c.files;
        for (&u, seen) in &p.c.players {
            let all = c.players.entry(unit(u)).or_default();
            if all.first == 0.0 {
                all.first = seen.first;
            }
            if seen.lines > 0 {
                all.last = seen.last;
            }
            all.lines += seen.lines;
            for (&s, &n) in &seen.spells {
                *all.spells.entry(spells[s as usize]).or_default() += n;
            }
            all.positions.extend_from_slice(&seen.positions);
            all.damage_to_me += seen.damage_to_me;
            all.damage_from_me += seen.damage_from_me;
            all.heal_to_me += seen.heal_to_me;
            all.heal_from_me += seen.heal_from_me;
            all.max_hp = all.max_hp.max(seen.max_hp);
        }
    }

    /// Whether the part, parsed on its own, came out as parsing it after the
    /// files before would have: no unit it could not look up was known
    /// before, and position sampling starts the same (each unit's first
    /// sample is far enough from its last one so far).
    fn joins(&self, p: &Part) -> bool {
        let far =
            |before: Option<&(f64, i64, f64, f64)>, first: Option<&(f64, i64, f64, f64)>, gap| {
                match (before, first) {
                    (Some(b), Some(f)) => f.0 - b.0 >= gap,
                    _ => true,
                }
            };
        p.loose.unnamed.iter().all(|g| !self.c.index.contains_key(g))
            && far(self.c.my_positions.last(), p.c.my_positions.first(), 2.0)
            && p.c.players.iter().all(|(&u, seen)| {
                let before = self
                    .c
                    .index
                    .get(&p.c.units[u as usize].guid)
                    .and_then(|g| self.c.players.get(g))
                    .and_then(|s| s.positions.last());
                far(before, seen.positions.first(), 10.0)
            })
    }

    pub fn finish(mut self) -> Combat {
        self.c.dealt.sort_by(|a, b| a.t.total_cmp(&b.t));
        self.c.taken.sort_by(|a, b| a.t.total_cmp(&b.t));
        self.c
    }
}

/// When parsing a file on its own: what can only be settled once the files
/// before it are known.
#[derive(Default)]
struct Loose {
    /// Deaths of others: (time, unit, when I last hit it before, in this file).
    died: Vec<(f64, u32, Option<f64>)>,
    /// Units the advanced fields describe before the file names them; if an
    /// earlier file named one, the file is parsed again against the whole.
    unnamed: BTreeSet<String>,
}

fn parse(
    c: &mut Combat,
    text: &str,
    mine: &[u32],
    last_hit: &mut HashMap<u32, f64>,
    mut loose: Option<&mut Loose>,
) {
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
                match c.index.get(info) {
                    Some(&u) => {
                        if is_me(u) {
                            if c.my_positions.last().is_none_or(|p| t - p.0 >= 2.0) {
                                c.my_positions.push((t, map, x, y));
                            }
                        } else if c.units[u as usize].kind == UnitKind::Player {
                            let p = c.players.entry(u).or_default();
                            if p.positions.last().is_none_or(|q| t - q.0 >= 10.0) {
                                p.positions.push((t, map, x, y));
                            }
                            if let Ok(hp) = f[adv + 3].parse::<i64>() {
                                p.max_hp = p.max_hp.max(hp);
                            }
                        }
                    }
                    None => {
                        if let Some(l) = loose.as_deref_mut() {
                            l.unnamed.insert(info.to_string());
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
            "SPELL_CAST_SUCCESS" if src != u32::MAX && is_me(src) => c.casts.push(Hit {
                t,
                src,
                dst,
                spell,
                amount: 0,
                over: 0,
                crit: false,
            }),
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
                } else if let Some(l) = loose.as_deref_mut() {
                    l.died.push((t, dst, last_hit.get(&dst).copied()));
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

/// The logs in `dirs`, folder by folder (archived before live, so the
/// files come in the order they were written), each folder by name.
pub fn log_files(dirs: &[&Path], prefix: &str) -> Vec<std::path::PathBuf> {
    let mut all = vec![];
    for d in dirs {
        let mut out = vec![];
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
        out.sort();
        all.extend(out);
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each file on its own, then put together, as `data::load` does.
    fn load(files: &[std::path::PathBuf], me: &[String]) -> Combat {
        let mut whole = Whole::new(me);
        for f in files {
            let text = fs::read_to_string(f).unwrap();
            let mut part = Part::new(f, me);
            part.parse(&text, me);
            whole.add(&part, || text.clone());
        }
        whole.finish()
    }

    const LOG: &str = "9/26/2026 10:53:48.3342  SPELL_DAMAGE,Pet-0-4615-0-44328-416-010024095A,\"Unknown\",0x1118,0x80000000,Creature-0-4615-0-2087-1502-00003787FF,\"Wretched Zombie\",0xa28,0x80000000,3110,\"Firebolt\",0x4,Creature-0-4615-0-2087-1502-00003787FF,0000000000000000,1,42,3,0,20,0,0,0,1,0,0,0,1898.94,1589.05,1415,3.0346,1,4,4,-1,4,0,0,0,nil,nil,nil,ST
9/26/2026 10:55:28.2642  SPELL_HEAL,Player-4613-00A46D50,\"Dead-ClassicBetaPvP2-\",0x511,0x80000000,Player-4613-00A46D50,\"Dead-ClassicBetaPvP2-\",0x511,0x80000000,635,\"Holy Light\",0x2,Player-4613-00A46D50,0000000000000000,104,104,56,0,167,0,0,0,0,103,103,0,1870.52,1491.64,1420,4.9584,0,41,41,26,0,nil
9/26/2026 10:55:30.0000  SWING_DAMAGE,Player-4613-00A46D50,\"Dead-ClassicBetaPvP2-\",0x511,0x80000000,Creature-0-1-2-3-4-5,\"Rattlecage Skeleton\",0xa48,0x0,Player-4613-00A46D50,0000000000000000,104,104,56,0,167,0,0,0,0,103,103,0,1870.52,1491.64,1420,4.9584,0,12,12,-1,1,0,0,0,nil,nil,nil
9/26/2026 10:55:31.0000  UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,Creature-0-1-2-3-4-5,\"Rattlecage Skeleton\",0xa48,0x0,0";

    #[test]
    fn parses_real_lines() {
        let dir = std::env::temp_dir().join(format!("fm-combat-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("WoWCombatLog-test.txt");
        fs::write(&file, LOG).unwrap();
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

    fn summary(c: &Combat) -> String {
        let mut players: Vec<_> = c.players.iter().collect();
        players.sort_by_key(|(u, _)| **u);
        format!(
            "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {}",
            c.units, c.spells, c.dealt, c.taken, c.healed, c.kills, c.deaths, players,
            c.my_positions, c.lines
        )
    }

    /// Files parsed on their own, cached and put together come out as one
    /// pass over all of them, also when they depend on each other.
    #[test]
    fn parts_join_like_one_pass() {
        let me = ["Player-4613-00A46D50".to_string()];
        let later = LOG.replace("10:55:31.0000  UNIT_DIED", "10:55:45.0000  UNIT_DIED");
        for log in [LOG.to_string(), later] {
            let lines: Vec<&str> = log.lines().collect();
            let mut one = Whole::new(&me);
            parse(&mut one.c, &log, &one.mine, &mut one.last_hit, None);
            let want = summary(&one.finish());
            for split in 0..=lines.len() {
                let mut whole = Whole::new(&me);
                for text in [lines[..split].join("\n"), lines[split..].join("\n")] {
                    let mut part = Part::new(Path::new(""), &me);
                    part.parse(&text, &me);
                    let mut w = W::default();
                    part.write(&mut w);
                    let part = Part::read(&mut R(&w.0)).unwrap();
                    whole.add(&part, || text.clone());
                }
                assert_eq!(summary(&whole.finish()), want, "split at {split}");
            }
        }
    }

    #[test]
    fn parts_fit_new_characters() {
        let me = vec!["Player-4613-00A46D50".to_string()];
        let mut part = Part::new(Path::new(""), &me);
        part.parse(LOG, &me);
        assert!(part.fits(&[me[0].clone(), "Player-1-2".into()]));
        assert!(!part.fits(&[me[0].clone(), "Creature-0-1-2-3-4-5".into()]));
        assert!(!part.fits(&[]));
    }
}
