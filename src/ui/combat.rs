//! Combat from the native combat logs: fights, abilities, kills, and the ten
//! seconds before every death.

use super::widgets::{self, Col, Key, icons};
use super::{card, character, label, plot};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::combat::Hit;
use crate::theme::{self, DANGER, INK, MUTED, SERIES};
use crate::tr;
use egui::{RichText, Ui};
use egui_plot::{Bar, BarChart, Line, PlotPoints};
use std::collections::HashMap;

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    let cb = &m.combat;
    if cb.lines == 0 {
        super::empty(
            ui,
            art,
            widgets::icons::SWORDS,
            tr!("No battles recorded yet"),
            tr!("The game keeps a log of every fight. Draw steel, and it will be read here after the session."),
        );
        return;
    }
    let in_char = |t: f64| {
        c.sessions.is_empty()
            || c.sessions
                .iter()
                .any(|s| t >= s.start as f64 - 60.0 && t <= s.end as f64 + 60.0)
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
    if fights.is_empty() && deaths.is_empty() {
        super::empty(
            ui,
            art,
            widgets::icons::SWORDS,
            tr!("No blood drawn yet"),
            &tr!("{name} hasn't fought yet. Every fight is read from the game's combat log after the session.", name = c.name.split(' ').next().unwrap_or("")),
        );
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                super::widgets::figure_row(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(36.0, 12.0);
                    let total: i64 = dealt.iter().map(|h| h.amount).sum();
                    let fight_time: f64 =
                        fights.iter().map(|(_, f)| (f.end - f.start).max(1.5)).sum();
                    widgets::figure_text(
                        ui,
                        art,
                        icons::SWORDS,
                        &theme::thousands(total),
                        tr!("damage done"),
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::SWORDS,
                        &format!("{:.1}", total as f64 / fight_time.max(1.0)),
                        tr!("per second in a fight"),
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::HEAL,
                        &theme::thousands(healed.iter().map(|h| h.amount - h.over).sum()),
                        tr!("healing done"),
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::FEIGN,
                        &theme::thousands(taken.iter().map(|h| h.amount).sum()),
                        tr!("damage taken"),
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::SKULL,
                        &kills.len().to_string(),
                        tr!("kills"),
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::FEIGN,
                        &deaths.len().to_string(),
                        tr!("deaths"),
                    );
                });
            });
            ui.add_space(14.0);

            let fight_sel = std::cell::Cell::new(st.fight); // read by one column, set by the other
            super::pair(
                ui,
                |ui| {
                    card(ui, |ui| {
                        label(ui, tr!("Fights"));
                        ui.label(
                            RichText::new(tr!(
                                "Damage per second in each fight, in order. Click one to see it."
                            ))
                            .small()
                            .color(MUTED),
                        );
                        let bars: Vec<Bar> = fights
                            .iter()
                            .enumerate()
                            .map(|(k, (i, f))| {
                                let fill = if fight_sel.get() == Some(*i) {
                                    SERIES[1]
                                } else {
                                    SERIES[0]
                                };
                                Bar::new(k as f64 + 1.0, f.dps())
                                    .width(0.8)
                                    .fill(fill)
                            })
                            .collect();
                        let r = plot("fights")
                            .height(220.0)
                            .y_axis_formatter(|g, _| format!("{:.0}", g.value))
                            .x_axis_formatter(|_, _| String::new())
                            .show(ui, |pu| {
                                pu.bar_chart(
                                    BarChart::new("DPS", bars)
                                        .element_formatter(Box::new(|_, _| String::new())),
                                );
                                pu.pointer_coordinate()
                            });
                        super::bar_tip(&r, |p| {
                                let k = p.x.round() as usize;
                                let (_, f) = fights.get(k.checked_sub(1)?)?;
                                Some(tr!(
                                    "{target} at {time}\n{dps} DPS over {duration}",
                                    target = f.target,
                                    time = theme::clock(f.start),
                                    dps = format!("{:.1}", f.dps()),
                                    duration = theme::duration(f.end - f.start)
                                ))
                            });
                        if r.response.clicked()
                            && let Some(p) = r.inner {
                                let k = p.x.round() as usize;
                                if k >= 1 && k <= fights.len() {
                                    fight_sel.set(Some(fights[k - 1].0));
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
                                label(
                                    ui,
                                    &tr!(
                                        "{target} at {time}",
                                        target = f.target,
                                        time = theme::clock(f.start)
                                    ),
                                );
                                ui.label(
                                    RichText::new(tr!(
                                        "{damage} damage in {duration}, {dps} per second",
                                        damage = theme::thousands(f.damage),
                                        duration = theme::duration(f.end - f.start),
                                        dps = format!("{:.1}", f.dps())
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
                                    .x_axis_formatter(|g, _| {
                                        tr!("{s}s", s = format!("{:.0}", g.value))
                                    })
                                    .label_formatter(|h| {
                                        let p = super::hover(h);
                                        Some(tr!(
                                            "{damage} damage after {s}s",
                                            damage = format!("{:.0}", p.y),
                                            s = format!("{:.1}", p.x)
                                        ))
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
                            None => super::hint(ui, tr!("No fights yet.")),
                        }
                    });
                },
            );
            st.fight = fight_sel.get();
            ui.add_space(14.0);

            super::pair_by(ui, |ui, col| {
                if col == 0 {
                    card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        label(ui, tr!("Abilities"));
                        struct A {
                            name: String,
                            dmg: i64,
                            hits: u32,
                            crits: u32,
                            share: f32,
                        }
                        let mut by: HashMap<u32, (i64, u32, u32)> = HashMap::new();
                        for h in &dealt {
                            let e = by.entry(h.spell).or_default();
                            e.0 += h.amount;
                            e.1 += 1;
                            e.2 += h.crit as u32;
                        }
                        let total: i64 = by.values().map(|v| v.0).sum::<i64>().max(1);
                        let rows: Vec<A> = by
                            .into_iter()
                            .map(|(s, (dmg, hits, crits))| A {
                                name: cb.spell_name(s).to_string(),
                                dmg,
                                hits,
                                crits,
                                share: dmg as f32 / total as f32,
                            })
                            .collect();
                        let cols = [
                            Col::grow(tr!("Ability")),
                            Col::fit(tr!("Share"), 100.0),
                            Col::num(tr!("Damage"), 80.0),
                            Col::num(tr!("Hits"), 110.0),
                        ];
                        widgets::table(
                            ui,
                            "abilities",
                            &cols,
                            &rows,
                            (2, true),
                            28.0,
                            |r, i| match i {
                                0 => Key::Text(r.name.clone()),
                                1 | 2 => Key::Num(r.dmg as f64),
                                _ => Key::Num(r.hits as f64),
                            },
                            |ui, r, i| match i {
                                0 => {
                                    ui.label(RichText::new(&r.name).color(INK));
                                }
                                1 => {
                                    widgets::bar(ui, r.share, SERIES[0], 60.0);
                                    ui.label(
                                        RichText::new(format!("{:.0}%", r.share * 100.0))
                                            .small()
                                            .color(MUTED),
                                    );
                                }
                                2 => {
                                    ui.label(RichText::new(theme::thousands(r.dmg)).color(INK));
                                }
                                _ => {
                                    ui.label(
                                        RichText::new(tr!(
                                            "{hits} ({pct}% crit)",
                                            hits = r.hits,
                                            pct = format!(
                                                "{:.0}",
                                                r.crits as f64 / r.hits.max(1) as f64 * 100.0
                                            )
                                        ))
                                        .small()
                                        .color(MUTED),
                                    );
                                }
                            },
                        );
                    });
                } else {
                    card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        label(ui, tr!("Kills"));
                        let mut by: HashMap<&str, usize> = HashMap::new();
                        for (_, u) in &kills {
                            *by.entry(cb.unit_name(*u)).or_default() += 1;
                        }
                        let max = by.values().copied().max().unwrap_or(1) as f32;
                        let rows: Vec<(&str, usize)> = by.into_iter().collect();
                        let cols = [
                            Col::grow(tr!("Creature")),
                            Col::fit("", 110.0),
                            Col::num(tr!("Kills"), 70.0),
                        ];
                        widgets::table(
                            ui,
                            "kills",
                            &cols,
                            &rows,
                            (2, true),
                            28.0,
                            |r, i| {
                                if i == 0 {
                                    Key::Text(r.0.to_string())
                                } else {
                                    Key::Num(r.1 as f64)
                                }
                            },
                            |ui, r, i| match i {
                                0 => {
                                    ui.label(RichText::new(r.0).color(INK));
                                }
                                1 => widgets::bar(
                                    ui,
                                    r.1 as f32 / max,
                                    theme::DANGER.gamma_multiply(0.8),
                                    100.0,
                                ),
                                _ => {
                                    ui.label(RichText::new(r.1.to_string()).color(INK));
                                }
                            },
                        );
                    });
                }
            });
            ui.add_space(14.0);

            card(ui, |ui| {
                ui.set_width(ui.available_width());
                label(ui, tr!("Deaths"));
                if deaths.is_empty() {
                    super::quiet(ui, art, widgets::icons::FEIGN, tr!("No deaths. Keep it that way."));
                }
                for t in deaths.iter().rev() {
                    ui.label(
                        RichText::new(tr!("Died {when}", when = theme::when(*t)))
                            .color(DANGER)
                            .size(17.0),
                    );
                    ui.label(
                        RichText::new(tr!("The ten seconds before:"))
                            .small()
                            .color(MUTED),
                    );
                    let before: Vec<&Hit> = taken
                        .iter()
                        .copied()
                        .filter(|h| h.t <= *t && h.t > t - 10.0)
                        .collect();
                    let cols = [
                        Col::num(tr!("Before"), 70.0),
                        Col::grow(tr!("From")),
                        Col::fit(tr!("With"), 150.0),
                        Col::num(tr!("Damage"), 90.0),
                    ];
                    widgets::table(
                        ui,
                        &format!("recap-{}", (*t * 10.0) as i64),
                        &cols,
                        &before,
                        (0, true),
                        24.0,
                        |h, i| match i {
                            0 => Key::Num(t - h.t),
                            1 => Key::Text(cb.unit_name(h.src).to_string()),
                            2 => Key::Text(cb.spell_name(h.spell).to_string()),
                            _ => Key::Num(h.amount as f64),
                        },
                        |ui, h, i| match i {
                            0 => {
                                ui.label(
                                    RichText::new(format!(
                                        "−{}",
                                        tr!("{s}s", s = format!("{:.1}", t - h.t))
                                    ))
                                    .color(MUTED),
                                );
                            }
                            1 => {
                                ui.label(RichText::new(cb.unit_name(h.src)).color(INK));
                            }
                            2 => {
                                ui.label(RichText::new(cb.spell_name(h.spell)).color(MUTED));
                            }
                            _ => {
                                ui.label(
                                    RichText::new(if h.crit {
                                        tr!("{damage} crit", damage = h.amount)
                                    } else {
                                        h.amount.to_string()
                                    })
                                    .color(DANGER),
                                );
                            }
                        },
                    );
                    ui.add_space(10.0);
                }
            });
            ui.add_space(24.0);
        });
}
