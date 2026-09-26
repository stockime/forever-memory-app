//! Current, completed and abandoned quests with their full text, objective
//! progress, rewards and how long each took.

use super::{card, character, item_link, label, plot};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::memory::{Character, Quest, QuestStatus};
use crate::theme::{self, EDGE, GOLD, INK, MUTED, RAISED, SERIES};
use crate::tr;
use egui::{Color32, RichText, Stroke, Ui};
use egui_plot::{Line, PlotPoints};

const TABS: [(&str, QuestStatus); 3] = [
    ("Active", QuestStatus::Active),
    ("Completed", QuestStatus::Completed),
    ("Abandoned", QuestStatus::Abandoned),
];

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    let list_w = 360.0;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(list_w);
            list(ui, c, st);
        });
        ui.add_space(16.0);
        let w = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width(w);
            let sel = st.quest.and_then(|id| c.quests.iter().find(|q| q.id == id));
            match sel {
                Some(q) => detail(ui, m, c, q, art),
                None => super::empty(ui, tr!("Pick a quest on the left.")),
            }
        });
    });
}

fn progress(q: &Quest) -> Option<(i64, i64)> {
    let (h, n) = q
        .objectives
        .iter()
        .fold((0, 0), |(h, n), o| (h + o.have.min(o.need), n + o.need));
    (n > 0).then_some((h, n))
}

fn list(ui: &mut Ui, c: &Character, st: &mut State) {
    ui.horizontal(|ui| {
        for (i, (name, status)) in TABS.iter().enumerate() {
            let n = c.quests.iter().filter(|q| q.status == *status).count();
            let text = RichText::new(format!("{} {n}", crate::i18n::t(name)))
                .color(if st.quest_tab == i { GOLD } else { INK });
            if ui
                .add(egui::Button::new(text).fill(if st.quest_tab == i {
                    RAISED
                } else {
                    Color32::TRANSPARENT
                }))
                .clicked()
            {
                st.quest_tab = i;
            }
        }
    });
    ui.add(
        egui::TextEdit::singleline(&mut st.quest_search)
            .hint_text(tr!("Filter by title, zone or text"))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    let status = TABS[st.quest_tab.min(2)].1;
    let needle = st.quest_search.to_lowercase();
    let mut qs: Vec<&Quest> = c
        .quests
        .iter()
        .filter(|q| q.status == status)
        .filter(|q| {
            needle.is_empty()
                || q.title.to_lowercase().contains(&needle)
                || q.zone
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle)
                || q.text.to_lowercase().contains(&needle)
        })
        .collect();
    match status {
        QuestStatus::Completed => qs.sort_by_key(|q| std::cmp::Reverse(q.done)),
        QuestStatus::Active => {
            qs.sort_by_key(|q| (!q.complete, q.level.unwrap_or(0), q.title.clone()))
        }
        _ => qs.sort_by_key(|q| std::cmp::Reverse(q.removed)),
    }
    if qs.is_empty() {
        ui.label(
            RichText::new(if needle.is_empty() {
                tr!("Nothing here yet.")
            } else {
                tr!("No quest matches.")
            })
            .color(MUTED),
        );
    }
    if !st.quest.is_some_and(|id| qs.iter().any(|q| q.id == id)) {
        st.quest = qs.first().map(|q| q.id);
    }
    egui::ScrollArea::vertical()
        .id_salt("quest-list")
        .auto_shrink(false)
        .show(ui, |ui| {
            for q in qs {
                let selected = st.quest == Some(q.id);
                let frame = egui::Frame::new()
                    .fill(if selected {
                        RAISED
                    } else {
                        Color32::TRANSPARENT
                    })
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(10, 7));
                let r = frame
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            // Coloured by difficulty, as in the game's quest log.
                            let diff = q.level.map(|l| super::widgets::quest_color(l, c.level));
                            if let Some(l) = q.level {
                                ui.label(
                                    RichText::new(format!("[{l}]"))
                                        .color(diff.unwrap_or(MUTED))
                                        .small(),
                                );
                            }
                            let color = if q.status == QuestStatus::Active {
                                diff.unwrap_or(INK)
                            } else {
                                INK
                            };
                            ui.label(
                                RichText::new(&q.title)
                                    .color(if selected { GOLD } else { color })
                                    .size(16.0),
                            );
                        });
                        let sub = match q.status {
                            QuestStatus::Active if q.complete => {
                                tr!("Ready to turn in").to_string()
                            }
                            QuestStatus::Active => progress(q)
                                .map(|(h, n)| format!("{h}/{n}"))
                                .unwrap_or_default(),
                            QuestStatus::Completed => {
                                q.done.map(|t| theme::day(t as f64)).unwrap_or_default()
                            }
                            _ => q
                                .removed
                                .map(|t| tr!("dropped {day}", day = theme::day(t as f64)))
                                .unwrap_or_default(),
                        };
                        let zone = q.zone.clone().unwrap_or_default();
                        ui.label(
                            RichText::new(
                                [zone, sub]
                                    .into_iter()
                                    .filter(|s| !s.is_empty())
                                    .collect::<Vec<_>>()
                                    .join(", "),
                            )
                            .small()
                            .color(if q.complete {
                                theme::GOOD
                            } else {
                                MUTED
                            }),
                        );
                    })
                    .response
                    .interact(egui::Sense::click());
                if r.clicked() {
                    st.quest = Some(q.id);
                }
            }
        });
}

fn detail(ui: &mut Ui, m: &Model, c: &Character, q: &Quest, art: &mut Art) {
    egui::ScrollArea::vertical().id_salt("quest-detail").auto_shrink(false).show(ui, |ui| {
        ui.label(RichText::new(&q.title).font(theme::display_font(32.0)).color(INK));
        let mut meta = vec![];
        if let Some(l) = q.level {
            meta.push(tr!("Level {level}", level = l));
        }
        if let Some(z) = q.zone.as_ref().filter(|z| !z.is_empty()) {
            meta.push(z.clone());
        }
        if let Some(t) = q.accepted {
            meta.push(tr!("accepted {when}", when = theme::when(t as f64)));
        }
        if let (Some(a), Some(d)) = (q.accepted, q.done) {
            meta.push(tr!("done after {time} of play", time = theme::duration(c.play_time(d) - c.play_time(a))));
        } else if let Some(d) = q.done {
            meta.push(tr!("turned in {when}", when = theme::when(d as f64)));
        }
        ui.label(RichText::new(meta.join(", ")).color(MUTED));
        ui.add_space(10.0);

        if !q.objectives.is_empty() {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                label(ui, if q.complete { tr!("Objectives, all done") } else { tr!("Objectives") });
                for o in &q.objectives {
                    ui.horizontal(|ui| {
                        let share = if o.need > 0 { o.have as f32 / o.need as f32 } else if o.done { 1.0 } else { 0.0 };
                        super::widgets::bar(ui, share, if o.done { theme::GOOD.gamma_multiply(0.55) } else { SERIES[0] }, 180.0);
                        ui.label(RichText::new(&o.text).color(if o.done { MUTED } else { INK }));
                    });
                }
            });
            ui.add_space(10.0);
        }

        if q.done.is_some() {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                label(ui, tr!("Rewards"));
                super::widgets::figure_row(ui, |ui| {
                    use super::widgets::{self, icons};
                    ui.spacing_mut().item_spacing.x = 28.0;
                    if q.xp > 0 {
                        widgets::figure_text(ui, art, icons::SPIRIT, &theme::thousands(q.xp), tr!("experience"));
                    }
                    if q.money > 0 {
                        widgets::figure_money(ui, art, icons::COIN, q.money, tr!("money"));
                    }
                });
                choices(ui, m, q, art);
            });
            ui.add_space(10.0);
        } else if !q.choices.is_empty() {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                label(ui, tr!("Reward choices"));
                choices(ui, m, q, art);
            });
            ui.add_space(10.0);
        }

        if q.progress_events.len() > 1 {
            card(ui, |ui| {
                label(ui, tr!("How it went"));
                ui.label(RichText::new(tr!("Objective progress against minutes played since accepting")).small().color(MUTED));
                let start = q.accepted.map(|a| c.play_time(a)).unwrap_or_else(|| c.play_time(q.progress_events[0].0));
                let mut by: std::collections::BTreeMap<i64, Vec<[f64; 2]>> = Default::default();
                for &(t, i, have, need) in &q.progress_events {
                    by.entry(i).or_default().push([(c.play_time(t) - start) / 60.0, have as f64 / need.max(1) as f64 * 100.0]);
                }
                let names: Vec<String> = q.objectives.iter().map(|o| o.text.clone()).collect();
                plot("quest-progress").height(180.0).include_y(0.0).include_y(100.0).y_axis_formatter(|g, _| format!("{:.0}%", g.value)).x_axis_formatter(|g, _| tr!("{n} min", n = format!("{:.0}", g.value))).legend(egui_plot::Legend::default()).show(ui, |pu| {
                    for (k, (i, pts)) in by.into_iter().enumerate() {
                        let mut pts = pts;
                        pts.insert(0, [0.0, 0.0]);
                        let name = names.get((i - 1).max(0) as usize).cloned().unwrap_or_else(|| tr!("Objective {n}", n = i));
                        pu.line(Line::new(name, PlotPoints::from(pts)).color(SERIES[k % 3]).width(2.0));
                    }
                });
            });
            ui.add_space(10.0);
        }

        // The quest text on the Classic quest log's parchment, in its ink.
        let sections: Vec<(&str, &String)> = [(tr!("Description"), &q.text), (tr!("Objective"), &q.objective), (tr!("Progress"), &q.progress), (tr!("Completion"), &q.reward)]
            .into_iter()
            .filter(|(_, t)| !t.trim().is_empty())
            .collect();
        if !sections.is_empty() {
            super::widgets::parchment(ui, art, 720.0, |ui| {
                for (title, text) in &sections {
                    ui.label(RichText::new(*title).font(theme::display_font(21.0)).color(super::widgets::INK_RED));
                    ui.add_space(2.0);
                    ui.label(RichText::new(text.replace("$B", "\n")).size(17.5).color(super::widgets::INK_BROWN));
                    ui.add_space(14.0);
                }
            });
            ui.add_space(12.0);
        }
        if q.text.is_empty() && q.objective.is_empty() && q.reward.is_empty() {
            ui.label(RichText::new(tr!("No quest text recorded: this one was picked up before the recorder was running.")).color(MUTED));
        }
        ui.add_space(24.0);
    });
}

fn choices(ui: &mut Ui, m: &Model, q: &Quest, art: &mut Art) {
    let chosen = q
        .choice
        .as_deref()
        .and_then(crate::data::memory::parse_link)
        .map(|l| l.id);
    ui.horizontal_wrapped(|ui| {
        for link in &q.choices {
            let id = crate::data::memory::parse_link(link).map(|l| l.id);
            let picked = chosen.is_some() && id == chosen;
            egui::Frame::new()
                .fill(if picked { RAISED } else { Color32::TRANSPARENT })
                .stroke(Stroke::new(1.0, if picked { GOLD } else { EDGE }))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        item_link(ui, m, art, link, 34.0);
                        if picked {
                            ui.label(RichText::new(tr!("chosen")).small().color(GOLD));
                        }
                    });
                });
        }
    });
}
