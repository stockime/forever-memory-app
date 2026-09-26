//! Gold and loot: where money comes from and goes, and every item that
//! passed through the bags, with when it was first seen.

use super::{card, character, icon, label, plot};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::memory::parse_link;
use crate::theme::{self, INK, MUTED, SERIES};
use egui::{RichText, Ui};
use egui_plot::{Bar, BarChart};
use std::collections::HashMap;

fn source(ctx: Option<&str>) -> &'static str {
    match ctx {
        Some("loot") => "Loot",
        Some("quest") => "Quests",
        Some("merchant") => "Vendors",
        Some("trainer") => "Trainers",
        Some("mail") => "Mail",
        Some("auction") => "Auction house",
        Some("trade") => "Trade",
        _ => "Other",
    }
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    // Quest money arrives as a money row during the turn-in; the quest context covers it.
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
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 44.0;
                    super::figure(ui, &theme::money(earned.values().sum()), "earned");
                    super::figure(ui, &theme::money(spent.values().sum()), "spent");
                    super::figure(ui, &theme::money(c.money), "carried now");
                    let played = c.total_play() as f64;
                    if played > 60.0 {
                        super::figure(
                            ui,
                            &theme::money(
                                (earned.values().sum::<i64>() as f64 / played * 3600.0) as i64,
                            ),
                            "earned per hour",
                        );
                    }
                });
            });
            ui.add_space(14.0);
            super::pair(
                ui,
                |ui| money_card(ui, 0, "Where money comes from", &earned),
                |ui| money_card(ui, 1, "Where it goes", &spent),
            );
            ui.add_space(14.0);
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    label(ui, "Everything that went through the bags");
                    ui.add_space(12.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut st.loot_search)
                            .hint_text("Filter items")
                            .desired_width(240.0),
                    );
                });
                ui.add_space(6.0);
                items(ui, m, c, &st.loot_search, art);
            });
            ui.add_space(24.0);
        });
}

fn money_card(ui: &mut Ui, k: usize, title: &str, map: &HashMap<&str, i64>) {
    card(ui, |ui| {
        label(ui, title);
        let mut rows: Vec<(&str, i64)> = map.iter().map(|(k, v)| (*k, *v)).collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1));
        if rows.is_empty() {
            ui.label(RichText::new("Nothing yet.").color(MUTED));
            return;
        }
        let names: Vec<String> = rows.iter().map(|r| r.0.to_string()).collect();
        let bars: Vec<Bar> = rows
            .iter()
            .enumerate()
            .map(|(i, (n, v))| {
                Bar::new(-(i as f64), *v as f64)
                    .width(0.6)
                    .name(*n)
                    .fill(SERIES[0])
            })
            .collect();
        plot(&format!("money-{k}"))
            .height(60.0 + 34.0 * rows.len() as f32)
            .y_axis_formatter(move |g, _| {
                names
                    .get((-g.value).round() as usize)
                    .cloned()
                    .unwrap_or_default()
            })
            .y_axis_min_width(110.0)
            .x_axis_formatter(|g, _| theme::money(g.value as i64))
            .show(ui, |pu| {
                pu.bar_chart(BarChart::new("Money", bars).horizontal().element_formatter(
                    Box::new(|b, _| format!("{}: {}", b.name, theme::money(b.value as i64))),
                ));
            });
    });
}

struct Row {
    id: i64,
    name: String,
    quality: Option<i64>,
    gained: i64,
    lost: i64,
    sources: HashMap<&'static str, i64>,
    first: Option<i64>,
}

fn items(ui: &mut Ui, m: &Model, c: &crate::data::memory::Character, filter: &str, art: &mut Art) {
    let mut rows: HashMap<i64, Row> = HashMap::new();
    for e in c.events.iter().filter(|e| e.e == "item") {
        let Some(l) = e.s("link").and_then(parse_link) else {
            continue;
        };
        let d = e.i("d").unwrap_or(0);
        let info = m.memory.items.get(&l.id);
        let r = rows.entry(l.id).or_insert_with(|| Row {
            id: l.id,
            name: if l.name.is_empty() {
                info.map(|i| i.name.clone()).unwrap_or_default()
            } else {
                l.name.clone()
            },
            quality: l.quality.or(info.and_then(|i| i.quality)),
            gained: 0,
            lost: 0,
            sources: HashMap::new(),
            first: c.seen.get(e.s("key").unwrap_or("")).copied(),
        });
        if d > 0 {
            r.gained += d;
            *r.sources.entry(source(e.s("ctx"))).or_default() += d;
        } else {
            r.lost -= d;
        }
    }
    let needle = filter.to_lowercase();
    let mut list: Vec<Row> = rows
        .into_values()
        .filter(|r| needle.is_empty() || r.name.to_lowercase().contains(&needle))
        .collect();
    list.sort_by(|a, b| b.gained.cmp(&a.gained).then(a.name.cmp(&b.name)));
    if list.is_empty() {
        ui.label(RichText::new("No items match.").color(MUTED));
        return;
    }
    super::nowrap(ui);
    egui::Grid::new("items")
        .num_columns(5)
        .striped(false)
        .spacing([20.0, 6.0])
        .show(ui, |ui| {
            for h in ["Item", "Gained", "Gone", "From", "First seen"] {
                ui.label(RichText::new(h).small().color(MUTED));
            }
            ui.end_row();
            for r in list {
                ui.horizontal(|ui| {
                    match m.memory.items.get(&r.id).and_then(|i| i.icon) {
                        Some(i) => {
                            icon(ui, art, Some(i), theme::quality(r.quality), 26.0);
                        }
                        None => {
                            ui.add_space(34.0);
                        }
                    }
                    let name = if r.name.is_empty() {
                        format!("Item {}", r.id)
                    } else {
                        r.name.clone()
                    };
                    ui.label(RichText::new(name).color(theme::quality(r.quality)));
                });
                ui.label(RichText::new(r.gained.to_string()).color(INK));
                ui.label(
                    RichText::new(if r.lost > 0 {
                        r.lost.to_string()
                    } else {
                        String::new()
                    })
                    .color(MUTED),
                );
                let mut src: Vec<_> = r.sources.into_iter().collect();
                src.sort_by(|a, b| b.1.cmp(&a.1));
                ui.label(
                    RichText::new(src.iter().map(|(s, _)| *s).collect::<Vec<_>>().join(", "))
                        .small()
                        .color(MUTED),
                );
                ui.label(
                    RichText::new(r.first.map(|t| theme::when(t as f64)).unwrap_or_default())
                        .small()
                        .color(MUTED),
                );
                ui.end_row();
            }
        });
}
