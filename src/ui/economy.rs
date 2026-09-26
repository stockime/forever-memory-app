//! Gold and loot: where money comes from and goes, and every item that
//! passed through the bags, with when it was first seen.

use super::widgets::{self, Col, Key, icons};
use super::{card, character, label};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::memory::{Character, parse_link};
use crate::theme::{self, INK, MUTED, SERIES};
use crate::tr;
use egui::{RichText, Ui};
use std::collections::HashMap;

fn source(ctx: Option<&str>) -> &'static str {
    match ctx {
        Some("loot") => tr!("Loot"),
        Some("quest") => tr!("Quests"),
        Some("merchant") => tr!("Vendors"),
        Some("trainer") => tr!("Trainers"),
        Some("mail") => tr!("Mail"),
        Some("auction") => tr!("Auction house"),
        Some("trade") => tr!("Trade"),
        _ => tr!("Other"),
    }
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    let mut earned: HashMap<&str, i64> = HashMap::new();
    let mut spent: HashMap<&str, i64> = HashMap::new();
    for e in c.events.iter().filter(|e| e.e == "money") {
        let d = e.i("d").unwrap_or(0);
        let s = source(e.s("ctx"));
        if d > 0 {
            *earned.entry(s).or_default() += d;
        } else {
            *spent.entry(s).or_default() -= d;
        }
    }
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                super::widgets::figure_row(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 40.0;
                    let total: i64 = earned.values().sum();
                    widgets::figure_money(ui, art, icons::COIN, total, tr!("earned"));
                    widgets::figure_money(ui, art, icons::BAG, spent.values().sum(), tr!("spent"));
                    widgets::figure_money(ui, art, icons::COINS, c.money, tr!("carried now"));
                    let played = c.total_play() as f64;
                    if played > 60.0 {
                        widgets::figure_money(
                            ui,
                            art,
                            icons::WATCH,
                            (total as f64 / played * 3600.0) as i64,
                            tr!("earned per hour"),
                        );
                    }
                });
            });
            ui.add_space(14.0);
            super::pair_by(ui, |ui, i| {
                if i == 0 {
                    flows(ui, art, tr!("Where money comes from"), &earned)
                } else {
                    flows(ui, art, tr!("Where it goes"), &spent)
                }
            });
            ui.add_space(14.0);
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    label(ui, tr!("Everything that went through the bags"));
                });
                ui.add(
                    egui::TextEdit::singleline(&mut st.loot_search)
                        .hint_text(tr!("Filter items"))
                        .desired_width(260.0),
                );
                ui.add_space(6.0);
                items(ui, m, c, &st.loot_search, art);
            });
            ui.add_space(24.0);
        });
}

/// Money by source as share bars with coins, largest first.
fn flows(ui: &mut Ui, art: &mut Art, title: &str, map: &HashMap<&str, i64>) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        label(ui, title);
        let mut rows: Vec<(&str, i64)> = map.iter().map(|(k, v)| (*k, *v)).collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        if rows.is_empty() {
            ui.label(RichText::new(tr!("Nothing yet.")).color(MUTED));
            return;
        }
        let max = rows[0].1.max(1) as f32;
        let bar_w = (ui.available_width() - 290.0).max(80.0);
        for (name, v) in rows {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [140.0, 22.0],
                    egui::Label::new(RichText::new(name).color(INK)),
                );
                widgets::bar(ui, v as f32 / max, SERIES[0], bar_w);
                ui.add_space(8.0);
                widgets::coins(ui, art, v, 15.0);
            });
        }
    });
}

struct Row {
    id: i64,
    icon: Option<i64>,
    name: String,
    quality: Option<i64>,
    gained: i64,
    lost: i64,
    from: String,
    first: Option<i64>,
}

fn items(ui: &mut Ui, m: &Model, c: &Character, filter: &str, art: &mut Art) {
    let mut by: HashMap<i64, (Row, HashMap<&'static str, i64>)> = HashMap::new();
    for e in c.events.iter().filter(|e| e.e == "item") {
        let Some(l) = e.s("link").and_then(parse_link) else {
            continue;
        };
        let d = e.i("d").unwrap_or(0);
        let info = m.memory.items.get(&l.id);
        let (r, src) = by.entry(l.id).or_insert_with(|| {
            let name = if l.name.is_empty() {
                info.map(|i| i.name.clone()).unwrap_or_default()
            } else {
                l.name.clone()
            };
            (
                Row {
                    id: l.id,
                    icon: info.and_then(|i| i.icon),
                    name: if name.is_empty() {
                        tr!("Item {id}", id = l.id)
                    } else {
                        name
                    },
                    quality: l.quality.or(info.and_then(|i| i.quality)),
                    gained: 0,
                    lost: 0,
                    from: String::new(),
                    first: c.seen.get(e.s("key").unwrap_or("")).copied(),
                },
                HashMap::new(),
            )
        });
        if d > 0 {
            r.gained += d;
            *src.entry(source(e.s("ctx"))).or_default() += d;
        } else {
            r.lost -= d;
        }
    }
    let needle = filter.to_lowercase();
    let rows: Vec<Row> = by
        .into_values()
        .map(|(mut r, src)| {
            let mut s: Vec<_> = src.into_iter().collect();
            s.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            r.from = s.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ");
            r
        })
        .filter(|r| needle.is_empty() || r.name.to_lowercase().contains(&needle))
        .collect();
    if rows.is_empty() {
        ui.label(RichText::new(tr!("No items match.")).color(MUTED));
        return;
    }
    let stories = super::story::all(ui, m, c);
    let cols = [
        Col::grow(tr!("Item")),
        Col::num(tr!("Gained"), 80.0),
        Col::num(tr!("Gone"), 70.0),
        Col::fit(tr!("From"), 130.0),
        Col::fit(tr!("First seen"), 150.0),
    ];
    widgets::table(
        ui,
        "bags",
        &cols,
        &rows,
        (1, true),
        34.0,
        |r, i| match i {
            0 => Key::Text(r.name.clone()),
            1 => Key::Num(r.gained as f64),
            2 => Key::Num(r.lost as f64),
            3 => Key::Text(r.from.clone()),
            _ => Key::Num(r.first.unwrap_or(0) as f64),
        },
        |ui, r, i| match i {
            0 => {
                match r.icon {
                    Some(icon) => {
                        super::icon(ui, art, Some(icon), theme::quality(r.quality), 26.0);
                    }
                    None => ui.add_space(30.0),
                }
                ui.label(RichText::new(&r.name).color(theme::quality(r.quality)));
                let lines = stories
                    .get(&r.id)
                    .map(|s| super::story::lines(m, c, s))
                    .unwrap_or_default();
                if !lines.is_empty() {
                    let cell = ui.interact(ui.max_rect(), ui.id().with(r.id), egui::Sense::hover());
                    cell.on_hover_ui(|ui| {
                        ui.set_max_width(320.0);
                        ui.label(RichText::new(&r.name).size(17.0).color(theme::quality(r.quality)));
                        ui.separator();
                        ui.label(RichText::new(tr!("Story")).small().color(theme::GOLD));
                        ui.label(RichText::new(lines.join(" ")).color(INK));
                    });
                }
            }
            1 => {
                ui.label(RichText::new(r.gained.to_string()).color(INK));
            }
            2 => {
                if r.lost > 0 {
                    ui.label(RichText::new(r.lost.to_string()).color(MUTED));
                }
            }
            3 => {
                ui.label(RichText::new(&r.from).small().color(MUTED));
            }
            _ => {
                ui.label(
                    RichText::new(r.first.map(|t| theme::when(t as f64)).unwrap_or_default())
                        .small()
                        .color(MUTED),
                );
            }
        },
    );
}
