//! Everyone met in the world: search, and a profile per player from the combat
//! and chat logs; the fellowship a character travelled with, and their nemeses.

use super::widgets::{self, Col, Key, icons};
use super::{card, label};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::players::Player;
use crate::theme::{self, EDGE, GOLD, INK, MUTED, RAISED, SERIES};
use crate::tr;
use egui::{Color32, RichText, Ui};

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    ui.horizontal(|ui| {
        for (i, name) in [tr!("Everyone"), tr!("Fellowship"), tr!("Nemeses")].into_iter().enumerate() {
            let selected = st.players_tab == i;
            let text = RichText::new(name).color(if selected { GOLD } else { INK });
            if ui
                .add(egui::Button::new(text).fill(if selected { RAISED } else { Color32::TRANSPARENT }))
                .clicked()
            {
                st.players_tab = i;
            }
        }
    });
    ui.add_space(6.0);
    let c = super::character(m, st);
    match st.players_tab {
        1 => fellowship(ui, m, c, art),
        2 => nemeses(ui, m, c, art),
        _ => everyone(ui, m, st, art, super::rp::realm(c)),
    }
}

/// Everyone the character's traveled with, longest first.
fn fellowship(ui: &mut Ui, m: &Model, c: &crate::data::memory::Character, art: &mut Art) {
    let list = crate::data::bonds::fellowship(m, c);
    if list.is_empty() {
        super::empty(ui, art, icons::GROUP, tr!("No fellowship yet"), tr!("Travel in company, and every mile shared is remembered here."));
        return;
    }
    let most = list.iter().map(|x| x.together).max().unwrap_or(1).max(1) as f32;
    egui::ScrollArea::vertical().id_salt("fellowship").auto_shrink(false).show(ui, |ui| {
        for comp in &list {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    class_icon(ui, art, comp.class, 40.0);
                    ui.vertical(|ui| {
                        let rp = m.rp.other(&comp.name, super::rp::realm(c)).filter(|r| !r.name.is_empty());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(rp.map(|r| r.full_name()).unwrap_or_else(|| comp.name.clone()))
                                    .font(theme::display_font(24.0))
                                    .color(comp.class.map(theme::class_color).unwrap_or(INK)),
                            );
                            if let Some(r) = rp {
                                if !r.full_title.is_empty() {
                                    ui.label(RichText::new(&r.full_title).family(theme::italic()).size(17.0).color(INK));
                                }
                                if r.full_name() != comp.name {
                                    ui.label(RichText::new(format!("({})", comp.name)).color(MUTED));
                                }
                            }
                        });
                        ui.label(
                            RichText::new(if comp.days == 1 {
                                tr!("Travelled together for {time}, since {day}", time = theme::duration(comp.together as f64), day = theme::day(comp.since as f64))
                            } else {
                                tr!("Travelled together for {time} over {days} days, since {day}", time = theme::duration(comp.together as f64), days = comp.days, day = theme::day(comp.since as f64))
                            })
                            .color(MUTED),
                        );
                    });
                });
                ui.add_space(6.0);
                widgets::bar(ui, comp.together as f32 / most, SERIES[2], ui.available_width().min(520.0));
                ui.add_space(4.0);
                widgets::figure_row(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(36.0, 12.0);
                    widgets::figure_text(ui, art, icons::SWORDS, &theme::thousands(comp.kills as i64), tr!("fell at your side"));
                    if comp.healed_me > 0 {
                        widgets::figure_text(ui, art, icons::HEAL, &theme::thousands(comp.healed_me), tr!("health they gave you"));
                    }
                    if comp.healed_them > 0 {
                        widgets::figure_text(ui, art, icons::HEAL, &theme::thousands(comp.healed_them), tr!("health you gave them"));
                    }
                    widgets::figure_text(ui, art, icons::WATCH, &theme::ago(comp.last as f64), tr!("last together"));
                });
            });
            ui.add_space(10.0);
        }
    });
}

/// Who killed the character, most often first, and whether they've been paid back.
fn nemeses(ui: &mut Ui, m: &Model, c: &crate::data::memory::Character, art: &mut Art) {
    let list = crate::data::bonds::nemeses(m, c);
    if list.is_empty() {
        super::empty(ui, art, icons::SKULL, tr!("No nemesis yet"), &tr!("Nothing has killed {name}. The world will try.", name = first_name(c)));
        return;
    }
    egui::ScrollArea::vertical().id_salt("nemeses").auto_shrink(false).show(ui, |ui| {
        for n in &list {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if n.player {
                        class_icon(ui, art, n.class, 40.0);
                    } else {
                        super::icon(ui, art, Some(icons::SKULL), theme::DANGER, 40.0);
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&n.name).font(theme::display_font(24.0)).color(theme::DANGER));
                        let last = *n.deaths.iter().max().unwrap_or(&0) as f64;
                        let times = if n.deaths.len() == 1 {
                            tr!("Killed you once, {when}", when = theme::when(last))
                        } else {
                            tr!("Killed you {n} times, last {when}", n = n.deaths.len(), when = theme::when(last))
                        };
                        let place = if n.places.is_empty() { String::new() } else { format!(" · {}", n.places.join(", ")) };
                        ui.label(RichText::new(format!("{times}{place}")).color(MUTED));
                    });
                });
                ui.add_space(6.0);
                match n.avenged {
                    Some(t) => ui.label(RichText::new(format!("⚔ {}", tr!("Avenged {when}", when = theme::when(t)))).color(GOLD)),
                    None => ui.label(RichText::new(format!("☠ {}", tr!("Not yet avenged"))).color(theme::DANGER)),
                };
                if n.slain > 0 {
                    ui.label(
                        RichText::new(if n.slain == 1 {
                            tr!("You have struck down one of them.").to_string()
                        } else {
                            tr!("You have struck down {n} of them.", n = n.slain)
                        })
                        .small()
                        .color(MUTED),
                    );
                }
            });
            ui.add_space(10.0);
        }
    });
}

fn first_name(c: &crate::data::memory::Character) -> &str {
    c.name.split(' ').next().unwrap_or(&c.name)
}

fn everyone(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art, realm: &str) {
    if m.players.is_empty() {
        super::empty(
            ui,
            art,
            icons::GROUP,
            tr!("Nobody met yet"),
            tr!("Everyone you fight beside, trade words with or cross blades against will be remembered here."),
        );
        return;
    }
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(330.0);
            ui.add(
                egui::TextEdit::singleline(&mut st.player_search)
                    .hint_text(tr!("Search by name or class"))
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
                        || m.rp.other(&p.name, realm).is_some_and(|r| r.full_name().to_lowercase().contains(&needle))
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
                RichText::new(tr!(
                    "{shown} of {total}",
                    shown = list.len(),
                    total = m.players.len()
                ))
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
                                let mut bits =
                                    vec![tr!("seen {when}", when = theme::ago(p.last_seen))];
                                if !p.chat.is_empty() {
                                    let n = p.chat.len();
                                    bits.push(if n == 1 {
                                        tr!("{n} line", n = n)
                                    } else {
                                        tr!("{n} lines", n = n)
                                    });
                                }
                                if let Some(r) = m.rp.other(&p.name, realm) {
                                    let called = if r.name.is_empty() || r.full_name() == p.name { r.full_title.clone() } else { r.full_name() };
                                    if !called.is_empty() {
                                        ui.label(RichText::new(called).family(theme::italic()).color(INK));
                                    }
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
                Some(p) => profile(ui, m, p, art, realm),
                None => super::hint(ui, tr!("Pick someone on the left.")),
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

fn profile(ui: &mut Ui, m: &Model, p: &Player, art: &mut Art, realm: &str) {
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
                                line = tr!("Level {level} {who}", level = p.level, who = line);
                            }
                            if !p.guild.is_empty() {
                                line = tr!("{who} of <{guild}>", who = line, guild = p.guild);
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
                                Some((sp, n)) => tr!(
                                    "Probably a {class}, from {n} casts of {spell}",
                                    class = theme::class_name(c),
                                    n = n,
                                    spell = m.combat.spell_name(*sp)
                                ),
                                None => tr!("Probably a {class}", class = theme::class_name(c)),
                            }
                        }
                        _ => tr!("Class unknown: never seen casting").to_string(),
                    };
                    ui.label(RichText::new(class).color(MUTED));
                });
            });
            ui.add_space(10.0);
            if let Some(r) = m.rp.other(&p.name, realm) {
                card(ui, |ui| super::rp::other(ui, r, p.class.map(theme::class_color).unwrap_or(INK)));
                ui.add_space(12.0);
            }
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                super::widgets::figure_row(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(36.0, 12.0);
                    widgets::figure_text(
                        ui,
                        art,
                        icons::MAP,
                        &theme::day(p.first_seen),
                        tr!("first seen"),
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::WATCH,
                        &theme::ago(p.last_seen),
                        tr!("last seen"),
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::BOOK,
                        &p.days.len().to_string(),
                        if p.days.len() == 1 {
                            tr!("day seen")
                        } else {
                            tr!("days seen")
                        },
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::SWORDS,
                        &theme::thousands(p.combat_lines as i64),
                        tr!("times in your combat log"),
                    );
                    widgets::figure_text(
                        ui,
                        art,
                        icons::SCROLL,
                        &p.chat.len().to_string(),
                        tr!("things said"),
                    );
                });
            });
            ui.add_space(12.0);
            if let Some(s) = seen {
                super::pair_by(ui, |ui, col| {
                    if col == 0 {
                        card(ui, |ui| {
                            ui.set_width(ui.available_width());
                            label(ui, tr!("Between you"));
                            let rows = [
                                (tr!("Damage to you"), s.damage_to_me, theme::DANGER),
                                (tr!("Your damage to them"), s.damage_from_me, SERIES[0]),
                                (tr!("Healing you got"), s.heal_to_me, SERIES[2]),
                                (tr!("Healing you gave"), s.heal_from_me, SERIES[2]),
                            ];
                            if rows.iter().all(|r| r.1 == 0) {
                                ui.label(
                                    RichText::new(tr!(
                                        "Just passing by: no damage or healing between you."
                                    ))
                                    .color(MUTED),
                                );
                            }
                            let max = rows.iter().map(|r| r.1).max().unwrap_or(1).max(1) as f32;
                            for (l, v, color) in rows.into_iter().filter(|r| r.1 > 0) {
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        [190.0, 22.0],
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
                                    RichText::new(tr!(
                                        "Most health seen: {hp}",
                                        hp = theme::thousands(s.max_hp)
                                    ))
                                    .small()
                                    .color(MUTED),
                                );
                            }
                        });
                    } else {
                        card(ui, |ui| {
                            ui.set_width(ui.available_width());
                            label(ui, tr!("What they cast"));
                            let rows: Vec<(String, u32)> = s
                                .spells
                                .iter()
                                .map(|(sp, n)| (m.combat.spell_name(*sp).to_string(), *n))
                                .collect();
                            if rows.is_empty() {
                                ui.label(RichText::new(tr!("Nothing seen.")).color(MUTED));
                                return;
                            }
                            let max = rows.iter().map(|r| r.1).max().unwrap_or(1) as f32;
                            let cols = [
                                Col::grow(tr!("Spell")),
                                Col::fit("", 110.0),
                                Col::num(tr!("Casts"), 80.0),
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
                label(ui, tr!("What they said"));
                if p.chat.is_empty() {
                    ui.label(RichText::new(tr!("Not a word in your chat log.")).color(MUTED));
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
