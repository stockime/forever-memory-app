//! Everyone met in the world: search, and a profile per player from the combat
//! and chat logs.

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
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 40.0;
                    super::figure(ui, &theme::when(p.first_seen), "first seen");
                    super::figure(ui, &theme::ago(p.last_seen), "last seen");
                    super::figure(
                        ui,
                        &p.days.len().to_string(),
                        if p.days.len() == 1 {
                            "day seen"
                        } else {
                            "days seen"
                        },
                    );
                    super::figure(
                        ui,
                        &theme::thousands(p.combat_lines as i64),
                        "combat log lines",
                    );
                    super::figure(ui, &p.chat.len().to_string(), "chat lines");
                });
            });
            ui.add_space(12.0);
            if let Some(s) = seen {
                super::pair(
                    ui,
                    |ui| {
                        card(ui, |ui| {
                            label(ui, "Between you");
                            let rows = [
                                ("Damage to you", s.damage_to_me),
                                ("Your damage to them", s.damage_from_me),
                                ("Healing you got", s.heal_to_me),
                                ("Healing you gave", s.heal_from_me),
                            ];
                            if rows.iter().all(|r| r.1 == 0) {
                                ui.label(
                                    RichText::new(
                                        "Just passing by: no damage or healing between you.",
                                    )
                                    .color(MUTED),
                                );
                            }
                            for (l, v) in rows.into_iter().filter(|r| r.1 > 0) {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(l).color(MUTED));
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
                    },
                    |ui| {
                        card(ui, |ui| {
                            label(ui, "What they cast");
                            let mut spells: Vec<_> = s.spells.iter().collect();
                            spells.sort_by(|a, b| b.1.cmp(a.1));
                            let max = spells.first().map(|x| *x.1).unwrap_or(1) as f32;
                            if spells.is_empty() {
                                ui.label(RichText::new("Nothing seen.").color(MUTED));
                            }
                            super::nowrap(ui);
                            egui::Grid::new("their-spells")
                                .num_columns(2)
                                .spacing([14.0, 4.0])
                                .show(ui, |ui| {
                                    for (sp, n) in spells.into_iter().take(10) {
                                        ui.label(
                                            RichText::new(m.combat.spell_name(*sp)).color(INK),
                                        );
                                        ui.add(
                                            egui::ProgressBar::new(*n as f32 / max)
                                                .desired_width(150.0)
                                                .fill(SERIES[1])
                                                .text(n.to_string()),
                                        );
                                        ui.end_row();
                                    }
                                });
                        });
                    },
                );
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
