//! An item's story in a few plain lines, under its tooltip.

use crate::data::Model;
use crate::data::items::{How, Story, stories};
use crate::data::memory::Character;
use crate::theme::{self, GOLD, INK};
use crate::tr;
use egui::{RichText, Ui};
use std::collections::HashMap;
use std::sync::Arc;

/// Worked out once per character and archive read, then kept in egui's memory.
pub fn memo<T: Send + Sync + 'static>(
    ui: &Ui,
    m: &Model,
    c: &Character,
    what: &str,
    make: impl FnOnce() -> T,
) -> Arc<T> {
    let id = egui::Id::new(("memo", what, m.loaded_at, &c.slug));
    if let Some(v) = ui.data(|d| d.get_temp::<Arc<T>>(id)) {
        return v;
    }
    let v = Arc::new(make());
    ui.data_mut(|d| d.insert_temp(id, v.clone()));
    v
}

pub fn all(ui: &Ui, m: &Model, c: &Character) -> Arc<HashMap<i64, Story>> {
    memo(ui, m, c, "item-stories", || stories(m, c))
}

pub fn lines(m: &Model, c: &Character, s: &Story) -> Vec<String> {
    let mut out = vec![];
    let when = s.first.map(|t| theme::day(t as f64)).unwrap_or_default();
    let place = s.place.as_deref();
    if s.first.is_some() {
        out.push(match (&s.how, place) {
            (Some(How::Loot(Some(who))), Some(place)) => {
                tr!(
                    "Taken from {who} in {place}, {when}.",
                    who = who,
                    place = place,
                    when = when
                )
            }
            (Some(How::Loot(Some(who))), None) => {
                tr!("Taken from {who}, {when}.", who = who, when = when)
            }
            (Some(How::Loot(None)), Some(place)) => {
                tr!("Looted in {place}, {when}.", place = place, when = when)
            }
            (Some(How::Quest(q)), _) => {
                tr!("A reward for \"{quest}\", {when}.", quest = q, when = when)
            }
            (Some(How::Vendor), Some(place)) => {
                tr!("Bought in {place}, {when}.", place = place, when = when)
            }
            (Some(How::Vendor), None) => tr!("Bought from a merchant, {when}.", when = when),
            (Some(How::Mail), _) => tr!("Arrived in the mail, {when}.", when = when),
            (Some(How::Trade), _) => tr!("Handed over in a trade, {when}.", when = when),
            (Some(How::Auction), _) => tr!("Won at the auction house, {when}.", when = when),
            _ => tr!("First seen {when}.", when = when),
        });
    }
    if let Some((from, _)) = s.worn.iter().find(|w| w.1.is_none()) {
        let since = theme::day(*from);
        out.push(if since == when {
            tr!("Worn ever since.").to_string()
        } else {
            tr!("Worn since {when}.", when = since)
        });
    } else if !s.worn.is_empty() {
        let days: f64 = s.worn.iter().map(|(a, b)| b.unwrap_or(*a) - a).sum::<f64>() / 86400.0;
        let play: f64 = s
            .worn
            .iter()
            .map(|(a, b)| c.play_time(b.unwrap_or(*a) as i64) - c.play_time(*a as i64))
            .sum();
        if days >= 2.0 {
            out.push(tr!("Worn for {n} days.", n = days.round() as i64));
        } else if play >= 60.0 {
            out.push(tr!(
                "Worn for {time} of play.",
                time = theme::duration(play)
            ));
        }
    }
    if let Some((id, t)) = s.replaced {
        let name = m
            .memory
            .items
            .get(&id)
            .map(|i| i.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| tr!("Item {id}", id = id));
        out.push(tr!(
            "Replaced by {item}, {when}.",
            item = name,
            when = theme::day(t)
        ));
    }
    if !s.worn.is_empty() {
        if s.weapon() {
            match s.kills {
                0 => {}
                1 => out.push(tr!("One kill while wielded.").to_string()),
                n => out.push(tr!(
                    "{n} kills while wielded.",
                    n = theme::thousands(n as i64)
                )),
            }
        } else if s.taken > 0 {
            out.push(tr!(
                "Took {n} damage while worn.",
                n = theme::thousands(s.taken)
            ));
        }
    }
    if let Some((t, ctx)) = &s.gone
        && s.worn.iter().all(|w| w.1.is_some())
    {
        let when = theme::day(*t);
        out.push(match ctx.as_deref() {
            Some("merchant") => tr!("Sold to a merchant, {when}.", when = when),
            Some("quest") => tr!("Handed in for a quest, {when}.", when = when),
            _ => tr!("Gone from the bags since {when}.", when = when),
        });
    }
    out
}

/// The "Story" section for an item's tooltip; false if there is none.
pub fn section(ui: &mut Ui, m: &Model, c: &Character, id: i64) -> bool {
    let all = all(ui, m, c);
    let Some(s) = all.get(&id).filter(|s| !s.is_empty()) else {
        return false;
    };
    let lines = lines(m, c, s);
    if lines.is_empty() {
        return false;
    }
    ui.separator();
    ui.label(RichText::new(tr!("Story")).small().color(GOLD));
    ui.label(RichText::new(lines.join(" ")).color(INK));
    true
}
