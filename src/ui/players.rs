//! Everyone met in the world: search, and a profile per player from the combat
//! and chat logs.

use super::widgets::{self, Col, Key, icons};
use super::{card, label};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::players::Player;
use crate::theme::{self, EDGE, GOLD, INK, MUTED, RAISED, SERIES};
use egui::{Color32, RichText, Ui};

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    if m.players.is_empty() {
        super::empty(
            ui,
            "Nobody met yet. Players show up from the combat and chat logs.",
        );
        return;
    }
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(330.0);
            super::heading(ui, "Players");
            ui.add(
                egui::TextEdit::singleline(&mut st.player_search)
                    .hint_text("Search by name or class")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(4.0);
            let needle = st.player_search.to_lowercase();
            let list: Vec<&Player> = m
                .players
                .iter()
                .filter(|p| {
                    needle.is_empty()
                        || p.name.to_lowercase().contains(&needle)
                        || p.class
                            .map(theme::class_name)
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&needle)
                })
                .collect();
            if !st
                .player
                .as_ref()
                .is_some_and(|n| list.iter().any(|p| &p.name == n))
            {
                st.player = list.first().map(|p| p.name.clone());
            }
            ui.label(
                RichText::new(format!("{} of {}", list.len(), m.players.len()))
                    .small()
                    .color(MUTED),
            );
            egui::ScrollArea::vertical()
                .id_salt("players")
                .auto_shrink(false)
                .show(ui, |ui| {
                    for p in list {
                        let selected = st.player.as_deref() == Some(p.name.as_str());
                        let r = egui::Frame::new()
                            .fill(if selected {
                                RAISED
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(6)
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    class_icon(ui, art, p.class, 22.0);
                                    ui.label(
                                        RichText::new(&p.name)
                                            .color(p.class.map(theme::class_color).unwrap_or(INK))
                                            .size(16.0),
                                    );
                                });
                                let mut bits = vec![format!("seen {}", theme::ago(p.last_seen))];
                                if !p.chat.is_empty() {
                                    bits.push(format!(
                                        "{} line{}",
                                        p.chat.len(),
                                        if p.chat.len() == 1 { "" } else { "s" }
                                    ));
                                }
                                ui.label(RichText::new(bits.join(", ")).small().color(MUTED));
                            })
                            .response
                            .interact(egui::Sense::click());
                        if r.clicked() {
                            st.player = Some(p.name.clone());
                        }
                    }
                });
        });
        ui.add_space(16.0);
        let w = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width(w);
            match st
                .player
                .as_ref()
                .and_then(|n| m.players.iter().find(|p| &p.name == n))
            {
                Some(p) => profile(ui, m, p, art),
                None => super::empty(ui, "Pick someone on the left."),
            }
        });
    });
}

fn class_icon(ui: &mut Ui, art: &mut Art, class: Option<&str>, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
    match class.and_then(|c| art.get(ui.ctx(), &format!("class-{}.png", c.to_lowercase()))) {
        Some(tex) => {
            ui.painter().image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.07, 0.07), egui::pos2(0.93, 0.93)),
                Color32::WHITE,
            );
        }
        None => {
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, EDGE),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(13.0),
                MUTED,
            );
        }
    }
}

fn profile(ui: &mut Ui, m: &Model, p: &Player, art: &mut Art) {
    let seen = p.unit.and_then(|u| m.combat.players.get(&u));
    egui::ScrollArea::vertical()
        .id_salt("profile")
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                class_icon(ui, art, p.class, 44.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(&p.name)
                            .font(theme::display_font(34.0))
                            .color(p.class.map(theme::class_color).unwrap_or(INK)),
                    );
                    let class = match (p.class, seen) {
                        // Seen up close: race, class, level and guild are facts.
                        (Some(c), _) if p.class_known => {
                            let mut line = [p.race.as_str(), theme::class_name(c)]
                                .into_iter()
                                .filter(|x| !x.is_empty())
                                .collect::<Vec<_>>()
                                .join(" ");
                            if p.level > 0 {
                                line = format!("Level {} {line}", p.level);
                            }
                            if !p.guild.is_empty() {
                                line += &format!(" of <{}>", p.guild);
                            }
                            line
                        }
                        (Some(c), Some(s)) => {
                            let top = s
                                .spells
                                .iter()
                                .filter(|(sp, _)| {
                                    crate::data::combat::guess_class(
                                        &[(**sp, 1)].into_iter().collect(),
                                        &m.combat,
                                    ) == Some(c)
                                })
                                .max_by_key(|(_, n)| **n);
                            match top {
                                Some((sp, n)) => format!(
                                    "Probably a {}, from {n} casts of {}",
                                    theme::class_name(c),
                                    m.combat.spell_name(*sp)
                                ),
                                None => format!("Probably a {}", theme::class_name(c)),
                            }
                        }
                        _ => "Class unknown: never seen casting".to_string(),
                    };
                    ui.label(RichText::new(class).color(MUTED));
                });
            });
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(36.0, 12.0);
                    widgets::figure_text(
                        ui,
                        art,
                        icons::MAP,
                        &theme::day(p.first_seen),
                        "first seen",
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::WATCH,
                        &theme::ago(p.last_seen),
                        "last seen",
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::BOOK,
                        &p.days.len().to_string(),
                        if p.days.len() == 1 {
                            "day seen"
                        } else {
                            "days seen"
                        },
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::SWORDS,
                        &theme::thousands(p.combat_lines as i64),
                        "times in your combat log",
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::SCROLL,
                        &p.chat.len().to_string(),
                        "things said",
                    );
                });
            });
            ui.add_space(12.0);
            if let Some(s) = seen {
                super::pair_by(ui, |ui, col| {
                    if col == 0 {
                        card(ui, |ui| {
                            ui.set_width(ui.available_width());
                            label(ui, "Between you");
                            let rows = [
                                ("Damage to you", s.damage_to_me, theme::DANGER),
                                ("Your damage to them", s.damage_from_me, SERIES[0]),
                                ("Healing you got", s.heal_to_me, SERIES[2]),
                                ("Healing you gave", s.heal_from_me, SERIES[2]),
                            ];
                            if rows.iter().all(|r| r.1 == 0) {
                                ui.label(
                                    RichText::new(
                                        "Just passing by: no damage or healing between you.",
                                    )
                                    .color(MUTED),
                                );
                            }
                            let max = rows.iter().map(|r| r.1).max().unwrap_or(1).max(1) as f32;
                            for (l, v, color) in rows.into_iter().filter(|r| r.1 > 0) {
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        [150.0, 22.0],
                                        egui::Label::new(RichText::new(l).color(MUTED)),
                                    );
                                    widgets::bar(
                                        ui,
                                        v as f32 / max,
                                        color,
                                        (ui.available_width() - 70.0).max(60.0),
                                    );
                                    ui.label(RichText::new(theme::thousands(v)).color(INK));
                                });
                            }
                            if s.max_hp > 0 {
                                ui.label(
                                    RichText::new(format!(
                                        "Most health seen: {}",
                                        theme::thousands(s.max_hp)
                                    ))
                                    .small()
                                    .color(MUTED),
                                );
                            }
                        });
                    } else {
                        card(ui, |ui| {
                            ui.set_width(ui.available_width());
                            label(ui, "What they cast");
                            let rows: Vec<(String, u32)> = s
                                .spells
                                .iter()
                                .map(|(sp, n)| (m.combat.spell_name(*sp).to_string(), *n))
                                .collect();
                            if rows.is_empty() {
                                ui.label(RichText::new("Nothing seen.").color(MUTED));
                                return;
                            }
                            let max = rows.iter().map(|r| r.1).max().unwrap_or(1) as f32;
                            let cols = [
                                Col::grow("Spell"),
                                Col::fit("", 110.0),
                                Col::num("Casts", 44.0),
                            ];
                            widgets::table(
                                ui,
                                "their-spells",
                                &cols,
                                &rows,
                                (2, true),
                                28.0,
                                |r, i| {
                                    if i == 0 {
                                        Key::Text(r.0.clone())
                                    } else {
                                        Key::Num(r.1 as f64)
                                    }
                                },
                                |ui, r, i| match i {
                                    0 => {
                                        ui.label(RichText::new(&r.0).color(INK));
                                    }
                                    1 => widgets::bar(ui, r.1 as f32 / max, SERIES[1], 100.0),
                                    _ => {
                                        ui.label(RichText::new(r.1.to_string()).color(INK));
                                    }
                                },
                            );
                        });
                    }
                });
                ui.add_space(12.0);
            }
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                label(ui, "What they said");
                if p.chat.is_empty() {
                    ui.label(RichText::new("Not a word in your chat log.").color(MUTED));
                }
                for &i in p.chat.iter().rev().take(200) {
                    let l = &m.chat[i];
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(theme::when(l.t)).small().color(MUTED));
                        ui.label(
                            RichText::new(format!("[{}]", l.label()))
                                .small()
                                .color(GOLD),
                        );
                        ui.label(RichText::new(&l.text).color(INK));
                    });
                }
            });
            ui.add_space(24.0);
        });
}
