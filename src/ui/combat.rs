//! Combat from the native combat logs: fights, abilities, kills, and the ten
//! seconds before every death.

use super::{card, character, label, plot};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::combat::Hit;
use crate::theme::{self, DANGER, INK, MUTED, SERIES};
use egui::{RichText, Ui};
use egui_plot::{Bar, BarChart, Line, PlotPoints};
use std::collections::HashMap;

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, _art: &mut Art) {
    let c = character(m, st);
    let cb = &m.combat;
    if cb.lines == 0 {
        super::empty(
            ui,
            "No combat logs yet. They are archived after the game closes, or read live from the game's Logs folder.",
        );
        return;
    }
    let in_char = |t: f64| {
        c.sessions
            .iter()
            .any(|s| t >= s.start as f64 - 60.0 && t <= s.end as f64 + 60.0)
            || c.sessions.is_empty()
    };
    let dealt: Vec<&Hit> = cb.dealt.iter().filter(|h| in_char(h.t)).collect();
    let taken: Vec<&Hit> = cb.taken.iter().filter(|h| in_char(h.t)).collect();
    let healed: Vec<&Hit> = cb.healed.iter().filter(|h| in_char(h.t)).collect();
    let fights: Vec<(usize, &crate::data::combat::Fight)> = m
        .fights
        .iter()
        .enumerate()
        .filter(|(_, f)| in_char(f.start))
        .collect();
    let kills: Vec<&(f64, u32)> = cb.kills.iter().filter(|(t, _)| in_char(*t)).collect();
    let deaths: Vec<f64> = cb.deaths.iter().copied().filter(|t| in_char(*t)).collect();

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 44.0;
                    let total: i64 = dealt.iter().map(|h| h.amount).sum();
                    let fight_time: f64 =
                        fights.iter().map(|(_, f)| (f.end - f.start).max(1.5)).sum();
                    super::figure(ui, &theme::thousands(total), "damage done");
                    super::figure(
                        ui,
                        &format!("{:.1}", total as f64 / fight_time.max(1.0)),
                        "damage per second in fights",
                    );
                    super::figure(
                        ui,
                        &theme::thousands(healed.iter().map(|h| h.amount - h.over).sum()),
                        "healing done",
                    );
                    super::figure(
                        ui,
                        &theme::thousands(taken.iter().map(|h| h.amount).sum()),
                        "damage taken",
                    );
                    super::figure(ui, &kills.len().to_string(), "kills");
                    super::figure(ui, &deaths.len().to_string(), "deaths");
                    super::figure(ui, &fights.len().to_string(), "fights");
                });
            });
            ui.add_space(14.0);

            let fight_sel = std::cell::Cell::new(st.fight); // read by one column, set by the other
            super::pair(
                ui,
                |ui| {
                    card(ui, |ui| {
                        label(ui, "Fights");
                        ui.label(
                            RichText::new(
                                "Damage per second in each fight, in order. Click one to see it.",
                            )
                            .small()
                            .color(MUTED),
                        );
                        let bars: Vec<Bar> = fights
                            .iter()
                            .enumerate()
                            .map(|(k, (_, f))| {
                                let fill = if fight_sel.get() == Some(fights[k].0) {
                                    SERIES[1]
                                } else {
                                    SERIES[0]
                                };
                                Bar::new(k as f64 + 1.0, f.dps())
                                    .width(0.8)
                                    .fill(fill)
                                    .name(format!(
                                        "{} at {}\n{:.1} DPS over {}",
                                        f.target,
                                        theme::clock(f.start),
                                        f.dps(),
                                        theme::duration(f.end - f.start)
                                    ))
                            })
                            .collect();
                        let r = plot("fights")
                            .height(220.0)
                            .y_axis_formatter(|g, _| format!("{:.0}", g.value))
                            .x_axis_formatter(|_, _| String::new())
                            .show(ui, |pu| {
                                pu.bar_chart(
                                    BarChart::new("DPS", bars)
                                        .element_formatter(Box::new(|b, _| b.name.clone())),
                                );
                                pu.pointer_coordinate()
                            });
                        if r.response.clicked() {
                            if let Some(p) = r.inner {
                                let k = p.x.round() as usize;
                                if k >= 1 && k <= fights.len() {
                                    fight_sel.set(Some(fights[k - 1].0));
                                }
                            }
                        }
                    });
                },
                |ui| {
                    card(ui, |ui| {
                        let idx = fight_sel
                            .get()
                            .filter(|i| fights.iter().any(|(j, _)| j == i))
                            .or(fights.last().map(|(i, _)| *i));
                        match idx.map(|i| &m.fights[i]) {
                            Some(f) => {
                                label(ui, &format!("{} at {}", f.target, theme::clock(f.start)));
                                ui.label(
                                    RichText::new(format!(
                                        "{} damage in {}, {:.1} per second",
                                        theme::thousands(f.damage),
                                        theme::duration(f.end - f.start),
                                        f.dps()
                                    ))
                                    .small()
                                    .color(MUTED),
                                );
                                let mut sum = 0.0;
                                let pts: Vec<[f64; 2]> = std::iter::once([0.0, 0.0])
                                    .chain(cb.dealt[f.from..f.to].iter().map(|h| {
                                        sum += h.amount as f64;
                                        [h.t - f.start, sum]
                                    }))
                                    .collect();
                                plot("fight")
                                    .height(220.0)
                                    .x_axis_formatter(|g, _| format!("{:.0}s", g.value))
                                    .label_formatter(|h| {
                                        let p = super::hover(h);
                                        Some(format!("{:.0} damage after {:.1}s", p.y, p.x))
                                    })
                                    .show(ui, |pu| {
                                        pu.line(
                                            Line::new("Damage", PlotPoints::from(pts))
                                                .color(SERIES[0])
                                                .width(2.0)
                                                .fill(0.0)
                                                .fill_alpha(0.12),
                                        );
                                    });
                            }
                            None => super::empty(ui, "No fights yet."),
                        }
                    });
                },
            );
            st.fight = fight_sel.get();
            ui.add_space(14.0);

            super::pair(
                ui,
                |ui| {
                    card(ui, |ui| {
                        label(ui, "Abilities");
                        ui.label(RichText::new("Your damage by ability").small().color(MUTED));
                        let mut by: HashMap<u32, (i64, u32, u32)> = HashMap::new();
                        for h in &dealt {
                            let e = by.entry(h.spell).or_default();
                            e.0 += h.amount;
                            e.1 += 1;
                            e.2 += h.crit as u32;
                        }
                        let total: i64 = by.values().map(|v| v.0).sum::<i64>().max(1);
                        let mut rows: Vec<_> = by.into_iter().collect();
                        rows.sort_by(|a, b| b.1.0.cmp(&a.1.0));
                        super::nowrap(ui);
                        egui::Grid::new("abilities")
                            .num_columns(4)
                            .spacing([18.0, 6.0])
                            .show(ui, |ui| {
                                for (spell, (dmg, n, crits)) in rows.iter().take(12) {
                                    ui.label(RichText::new(cb.spell_name(*spell)).color(INK));
                                    ui.add(
                                        egui::ProgressBar::new(*dmg as f32 / total as f32)
                                            .desired_width(160.0)
                                            .fill(SERIES[0])
                                            .text(format!(
                                                "{:.0}%",
                                                *dmg as f64 / total as f64 * 100.0
                                            )),
                                    );
                                    ui.label(RichText::new(theme::thousands(*dmg)).color(INK));
                                    ui.label(
                                        RichText::new(format!(
                                            "{n} hits, {:.0}% crit",
                                            *crits as f64 / (*n).max(1) as f64 * 100.0
                                        ))
                                        .small()
                                        .color(MUTED),
                                    );
                                    ui.end_row();
                                }
                            });
                    });
                },
                |ui| {
                    card(ui, |ui| {
                        label(ui, "Kills");
                        let mut by: HashMap<&str, usize> = HashMap::new();
                        for (_, u) in &kills {
                            *by.entry(cb.unit_name(*u)).or_default() += 1;
                        }
                        let mut rows: Vec<_> = by.into_iter().collect();
                        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
                        super::nowrap(ui);
                        egui::Grid::new("kills")
                            .num_columns(2)
                            .spacing([18.0, 4.0])
                            .show(ui, |ui| {
                                for (name, n) in rows.iter().take(16) {
                                    ui.label(RichText::new(*name).color(INK));
                                    ui.label(RichText::new(format!("{n}×")).color(MUTED));
                                    ui.end_row();
                                }
                            });
                    });
                },
            );
            ui.add_space(14.0);

            card(ui, |ui| {
                ui.set_width(ui.available_width());
                label(ui, "Deaths");
                if deaths.is_empty() {
                    ui.label(RichText::new("No deaths. Keep it that way.").color(MUTED));
                }
                for t in deaths.iter().rev() {
                    ui.label(
                        RichText::new(format!("Died {}", theme::when(*t)))
                            .color(DANGER)
                            .size(17.0),
                    );
                    ui.label(
                        RichText::new("The ten seconds before:")
                            .small()
                            .color(MUTED),
                    );
                    let before: Vec<&&Hit> = taken
                        .iter()
                        .filter(|h| h.t <= *t && h.t > t - 10.0)
                        .collect();
                    super::nowrap(ui);
                    egui::Grid::new(("recap", (*t * 10.0) as i64))
                        .num_columns(4)
                        .spacing([16.0, 2.0])
                        .show(ui, |ui| {
                            for h in before {
                                ui.label(
                                    RichText::new(format!("−{:.1}s", t - h.t))
                                        .monospace()
                                        .color(MUTED),
                                );
                                ui.label(RichText::new(cb.unit_name(h.src)).color(INK));
                                ui.label(RichText::new(cb.spell_name(h.spell)).color(MUTED));
                                ui.label(
                                    RichText::new(format!(
                                        "{}{}",
                                        h.amount,
                                        if h.crit { " crit" } else { "" }
                                    ))
                                    .color(DANGER),
                                );
                                ui.end_row();
                            }
                        });
                    ui.add_space(8.0);
                }
            });
            ui.add_space(24.0);
        });
}
