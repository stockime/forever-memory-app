//! Item stories: how each item came to a character, when it was worn and
//! replaced, and what happened while it was.

use super::Model;
use super::deeds::Mine;
use super::memory::{Character, entries, parse_link};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum How {
    /// From a corpse: the nearest kill before it, if there was one.
    Loot(Option<String>),
    Quest(String),
    Vendor,
    Mail,
    Trade,
    Auction,
}

#[derive(Clone, Debug, Default)]
pub struct Story {
    pub first: Option<i64>,
    pub how: Option<How>,
    /// Where it came: the subzone, else the zone.
    pub place: Option<String>,
    pub slot: Option<i64>,
    /// Times worn; an open end means it still is.
    pub worn: Vec<(f64, Option<f64>)>,
    /// The item that took its place, and when.
    pub replaced: Option<(i64, f64)>,
    pub kills: usize,
    pub taken: i64,
    /// Last left the bags: when, and to whom (the interaction).
    pub gone: Option<(f64, Option<String>)>,
}

impl Story {
    pub fn weapon(&self) -> bool {
        matches!(self.slot, Some(16..=18))
    }
    pub fn is_empty(&self) -> bool {
        self.first.is_none() && self.how.is_none() && self.worn.is_empty()
    }
}

fn id_of(key: &str) -> Option<i64> {
    key.split(':').next()?.parse().ok()
}

/// Every item's story for one character, by item ID.
pub fn stories(m: &Model, c: &Character) -> HashMap<i64, Story> {
    let mine = Mine::new(m, c);
    let mut out: HashMap<i64, Story> = HashMap::new();
    for (key, t) in &c.seen {
        if let Some(id) = id_of(key) {
            let s = out.entry(id).or_default();
            s.first = Some(s.first.map_or(*t, |f| f.min(*t)));
        }
    }
    let worn_now: HashMap<i64, i64> = c
        .snapshot
        .get("equipment")
        .map(entries)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(slot, it)| Some((it.get("id").and_then(Value::as_i64)?, slot)))
        .collect();

    let (mut zone, mut sub) = (None::<&str>, None::<&str>);
    let mut turnin: Option<(i64, i64)> = None; // (time, quest id)
    let mut on: HashMap<i64, (i64, f64)> = HashMap::new(); // slot -> (item, since)
    let mut equipped_ever: HashMap<i64, bool> = HashMap::new();
    for e in &c.events {
        match e.e.as_str() {
            "zone" | "login" => {
                zone = e.s("zone").filter(|z| !z.is_empty());
                sub = e.s("sub").filter(|s| !s.is_empty());
            }
            "subzone" => sub = e.s("sub").filter(|s| !s.is_empty()),
            "quest" if e.s("act") == Some("turnin") => turnin = e.i("id").map(|id| (e.t, id)),
            "item" => {
                let Some(l) = e.s("link").and_then(parse_link) else {
                    continue;
                };
                let d = e.i("d").unwrap_or(0);
                let s = out.entry(l.id).or_default();
                if d > 0 {
                    s.gone = None;
                    if s.how.is_none() {
                        s.first.get_or_insert(e.t);
                        s.place = sub.or(zone).map(str::to_string);
                        s.how = how(e.s("ctx"), e.t, &mine, c, turnin);
                    }
                } else if e.i("total") == Some(0) {
                    s.gone = Some((e.t as f64, e.s("ctx").map(str::to_string)));
                }
            }
            "equip" => {
                let Some(slot) = e.i("slot") else { continue };
                let new = e.s("link").and_then(parse_link).map(|l| l.id);
                let t = e.t as f64;
                if let Some(&(old, since)) = on.get(&slot) {
                    if Some(old) == new {
                        continue;
                    }
                    let s = out.entry(old).or_default();
                    s.worn.push((since, Some(t)));
                    if let Some(n) = new {
                        s.replaced = Some((n, t));
                    }
                    on.remove(&slot);
                }
                if let Some(n) = new {
                    on.insert(slot, (n, t));
                    equipped_ever.insert(n, true);
                    let s = out.entry(n).or_default();
                    s.slot = Some(slot);
                    s.replaced = None;
                }
            }
            _ => {}
        }
    }
    let last = c.events.last().map_or(0.0, |e| e.t as f64);
    for (slot, (id, since)) in on {
        let still = worn_now.get(&id) == Some(&slot);
        out.entry(id)
            .or_default()
            .worn
            .push((since, if still { None } else { Some(last) }));
    }
    // Worn now but never seen being put on: worn since it first showed up.
    for (&id, &slot) in &worn_now {
        if equipped_ever.contains_key(&id) {
            continue;
        }
        let s = out.entry(id).or_default();
        s.slot = Some(slot);
        if let Some(f) = s.first {
            s.worn.push((f as f64, None));
        }
    }

    // What happened while each was worn.
    let mut taken_sum = Vec::with_capacity(mine.taken.len() + 1);
    taken_sum.push(0i64);
    for h in &mine.taken {
        taken_sum.push(taken_sum.last().unwrap() + h.amount);
    }
    let now = mine
        .kills
        .last()
        .map(|k| k.0)
        .unwrap_or(0.0)
        .max(last)
        .max(mine.taken.last().map_or(0.0, |h| h.t));
    for s in out.values_mut() {
        for &(from, to) in &s.worn {
            let to = to.unwrap_or(now);
            let k = |t: f64| mine.kills.partition_point(|x| x.0 < t);
            s.kills += k(to) - k(from);
            let h = |t: f64| mine.taken.partition_point(|x| x.t < t);
            s.taken += taken_sum[h(to)] - taken_sum[h(from)];
        }
    }
    out
}

fn how(
    ctx: Option<&str>,
    t: i64,
    mine: &Mine,
    c: &Character,
    turnin: Option<(i64, i64)>,
) -> Option<How> {
    Some(match ctx? {
        "loot" => {
            // The nearest kill before the corpse was opened.
            let t = t as f64;
            let i = mine.kills.partition_point(|k| k.0 <= t + 1.5);
            let from = i
                .checked_sub(1)
                .map(|i| mine.kills[i])
                .filter(|k| t - k.0 <= 60.0)
                .map(|k| mine.cb.unit_name(k.1).to_string());
            How::Loot(from)
        }
        "quest" => {
            let (_, id) = turnin.filter(|(at, _)| (t - at).abs() <= 10)?;
            let title = c.quests.iter().find(|q| q.id == id)?.title.clone();
            How::Quest(title)
        }
        "merchant" => How::Vendor,
        "mail" => How::Mail,
        "trade" => How::Trade,
        "auction" => How::Auction,
        _ => return None,
    })
}
