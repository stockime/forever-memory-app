//! The journal: sessions on the left, and for the chosen one everything that
//! happened, in order, with the chat that was going on at the time.

use super::{character, chips, item_link};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::chat::Kind;
use crate::data::memory::{Character, Event};
use crate::theme::{self, DANGER, GOLD, INK, MUTED, RAISED};
use crate::tr;
use egui::{Color32, RichText, Ui};

const SORTS: [&str; 7] = [
    "Newest first",
    "Oldest first",
    "Longest",
    "Most experience",
    "Fastest leveling",
    "Most loot",
    "Most deaths",
];
const CATEGORIES: [&str; 9] = [
    "Quests",
    "Loot",
    "Money",
    "Experience",
    "Places",
    "Combat",
    "People",
    "Chat",
    "System",
];

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    if c.sessions.is_empty() {
        super::empty(ui, art, super::widgets::icons::BOOK, tr!("The road is still unwritten"), tr!("Every time you set out, the journey is written down here, one session at a time."));
        return;
    }
    let sel = st
        .session
        .unwrap_or(c.sessions.len() - 1)
        .min(c.sessions.len() - 1);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            ui.horizontal(|ui| {
                super::heading(ui, tr!("Sessions"));
                egui::ComboBox::from_id_salt("session-sort")
                    .selected_text(crate::i18n::t(SORTS[st.session_sort.min(SORTS.len() - 1)]))
                    .show_ui(ui, |ui| {
                        for (i, s) in SORTS.iter().enumerate() {
                            ui.selectable_value(&mut st.session_sort, i, crate::i18n::t(s));
                        }
                    });
            });
            let mut order: Vec<usize> = (0..c.sessions.len()).collect();
            let rate = |s: &crate::data::memory::Session| s.xp as f64 / s.seconds().max(60) as f64;
            match st.session_sort {
                0 => order.reverse(),
                1 => {}
                2 => order.sort_by_key(|&i| std::cmp::Reverse(c.sessions[i].seconds())),
                3 => order.sort_by_key(|&i| std::cmp::Reverse(c.sessions[i].xp)),
                4 => order.sort_by(|&a, &b| rate(&c.sessions[b]).total_cmp(&rate(&c.sessions[a]))),
                5 => order.sort_by_key(|&i| std::cmp::Reverse(c.sessions[i].items)),
                _ => order.sort_by_key(|&i| {
                    std::cmp::Reverse((c.sessions[i].deaths, c.sessions[i].start))
                }),
            }
            egui::ScrollArea::vertical()
                .id_salt("sessions")
                .auto_shrink(false)
                .show(ui, |ui| {
                    for i in order {
                        let s = &c.sessions[i];
                        let selected = i == sel;
                        let r = egui::Frame::new()
                            .fill(if selected {
                                RAISED
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(6)
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(
                                    RichText::new(format!(
                                        "{}, {}",
                                        theme::day(s.start as f64),
                                        theme::clock(s.start as f64)
                                    ))
                                    .color(if selected { GOLD } else { INK })
                                    .size(16.0),
                                );
                                let levels = if s.level_to > s.level_from {
                                    tr!("level {from} → {to}", from = s.level_from, to = s.level_to)
                                } else {
                                    tr!("level {level}", level = s.level_from)
                                };
                                ui.label(
                                    RichText::new(format!(
                                        "{}, {}, {}",
                                        theme::duration(s.seconds() as f64),
                                        levels,
                                        tr!("{xp} XP", xp = theme::thousands(s.xp))
                                    ))
                                    .small()
                                    .color(MUTED),
                                );
                                if !s.zones.is_empty() {
                                    ui.label(
                                        RichText::new(s.zones.join(", ")).small().color(MUTED),
                                    );
                                }
                            })
                            .response
                            .interact(egui::Sense::click());
                        if r.clicked() {
                            st.session = Some(i);
                        }
                    }
                });
        });
        ui.add_space(16.0);
        let w = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width(w);
            let s = &c.sessions[sel];
            ui.label(
                RichText::new(tr!(
                    "{day}, {start} to {end}",
                    day = theme::day(s.start as f64),
                    start = theme::clock(s.start as f64),
                    end = theme::clock(s.end as f64)
                ))
                .font(theme::display_font(28.0)),
            );
            super::widgets::figure_row(ui, |ui| {
                use super::widgets::{self, icons};
                ui.spacing_mut().item_spacing = egui::vec2(30.0, 10.0);
                widgets::figure_text(
                    ui,
                    art,
                    icons::WATCH,
                    &theme::duration(s.seconds() as f64),
                    tr!("played"),
                );
                widgets::figure_text(
                    ui,
                    art,
                    icons::SPIRIT,
                    &theme::thousands(s.xp),
                    tr!("experience"),
                );
                widgets::figure_money(ui, art, icons::COIN, s.money, tr!("money"));
                widgets::figure_text(
                    ui,
                    art,
                    icons::BAG,
                    &s.items.to_string(),
                    tr!("items looted"),
                );
                widgets::figure_text(
                    ui,
                    art,
                    icons::NOTE,
                    &s.quests.to_string(),
                    tr!("quests done"),
                );
                widgets::figure_text(ui, art, icons::FEIGN, &s.deaths.to_string(), tr!("deaths"));
            });
            ui.add_space(8.0);
            chips(ui, &CATEGORIES, &mut st.journal_hide);
            ui.add_space(6.0);
            feed(ui, m, c, sel, &st.journal_hide, art);
        });
    });
}

enum Row<'a> {
    Event(&'a Event),
    Chat(usize),
}

fn feed(
    ui: &mut Ui,
    m: &Model,
    c: &Character,
    sel: usize,
    hide: &std::collections::HashSet<&'static str>,
    art: &mut Art,
) {
    let s = &c.sessions[sel];
    let events = &c.events[s.from..s.to];
    let mut rows: Vec<(f64, Row)> = events.iter().map(|e| (e.t as f64, Row::Event(e))).collect();
    if !hide.contains("Chat") {
        for (i, l) in m.chat.iter().enumerate() {
            if l.t >= s.start as f64 - 5.0 && l.t <= s.end as f64 + 5.0 && l.kind != Kind::System {
                rows.push((l.t, Row::Chat(i)));
            }
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let titles: std::collections::HashMap<i64, &str> =
        c.quests.iter().map(|q| (q.id, q.title.as_str())).collect();
    egui::ScrollArea::vertical()
        .id_salt("feed")
        .auto_shrink(false)
        .show(ui, |ui| {
            let mut last_turnin: Option<(i64, i64)> = None;
            for (t, row) in rows {
                match row {
                    Row::Chat(i) => {
                        let l = &m.chat[i];
                        line(ui, t, "›", |ui| {
                            ui.label(
                                RichText::new(format!("[{}]", l.label()))
                                    .small()
                                    .color(MUTED),
                            );
                            ui.label(
                                RichText::new(format!("{}:", l.speaker.as_deref().unwrap_or("")))
                                    .color(MUTED),
                            );
                            ui.label(
                                RichText::new(&l.text).color(Color32::from_rgb(0xc8, 0xc3, 0xb4)),
                            );
                        });
                    }
                    Row::Event(e) => {
                        if e.e == "quest" && e.s("act") == Some("turnin") {
                            last_turnin = e.i("id").map(|id| (id, e.t));
                        }
                        if e.e == "quest"
                            && e.s("act") == Some("remove")
                            && last_turnin
                                .is_some_and(|(id, at)| Some(id) == e.i("id") && e.t - at < 10)
                        {
                            continue; // removal is part of every turn-in
                        }
                        event(ui, m, e, &titles, hide, art);
                    }
                }
            }
        });
}

fn line(ui: &mut Ui, t: f64, glyph: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(theme::clock(t)).monospace().color(MUTED));
        ui.label(RichText::new(glyph).color(MUTED));
        add(ui);
    });
}

fn category(e: &Event) -> &'static str {
    match e.e.as_str() {
        "quest" | "objective" => "Quests",
        "item" | "equip" => "Loot",
        "money" => "Money",
        "xp" | "level" => "Experience",
        "zone" | "subzone" | "login" | "logout" => "Places",
        "death" | "alive" | "unghost" => "Combat",
        "group" | "gossip" | "gossip_pick" => "People",
        _ => "System",
    }
}

fn context(ctx: Option<&str>, gained: bool) -> &'static str {
    match (ctx, gained) {
        (Some("loot"), _) => tr!("Looted"),
        (Some("merchant"), true) => tr!("Bought"),
        (Some("merchant"), false) => tr!("Sold"),
        (Some("quest"), true) => tr!("Quest reward"),
        (Some("quest"), false) => tr!("Handed in"),
        (Some("mail"), true) => tr!("From the mail"),
        (Some("trade"), _) => tr!("Traded"),
        (Some("auction"), _) => tr!("Auction house"),
        (Some("bank"), _) => tr!("Bank"),
        (_, true) => tr!("Received"),
        (_, false) => tr!("Used up"),
    }
}

fn event(
    ui: &mut Ui,
    m: &Model,
    e: &Event,
    titles: &std::collections::HashMap<i64, &str>,
    hide: &std::collections::HashSet<&'static str>,
    art: &mut Art,
) {
    let cat = category(e);
    if hide.contains(cat) {
        return;
    }
    let t = e.t as f64;
    let title = |id: Option<i64>| {
        id.and_then(|i| titles.get(&i).copied())
            .map(str::to_string)
            .or_else(|| e.s("title").map(str::to_string))
            .unwrap_or_else(|| tr!("a quest").into())
    };
    match e.e.as_str() {
        "login" => line(ui, t, "▶", |ui| {
            let level = e.i("level").unwrap_or(0);
            let text = match e.s("zone").filter(|z| !z.is_empty()) {
                Some(zone) => tr!(
                    "Logged in at level {level} in {zone}",
                    level = level,
                    zone = zone
                ),
                None => tr!("Logged in at level {level}", level = level),
            };
            ui.label(RichText::new(text).color(INK));
        }),
        "logout" => line(ui, t, "■", |ui| {
            ui.label(
                RichText::new(tr!(
                    "Logged out in {zone}",
                    zone = e.s("zone").unwrap_or("?")
                ))
                .color(MUTED),
            );
        }),
        "item" => {
            let d = e.i("d").unwrap_or(0);
            line(ui, t, if d > 0 { "+" } else { "−" }, |ui| {
                ui.label(RichText::new(context(e.s("ctx"), d > 0)).color(MUTED));
                if d.abs() > 1 {
                    ui.label(format!("{}×", d.abs()));
                }
                if let Some(l) = e.s("link") {
                    item_link(ui, m, art, l, 20.0);
                }
            });
        }
        "equip" => line(ui, t, "⛨", |ui| match e.s("link") {
            Some(l) => {
                ui.label(RichText::new(tr!("Equipped")).color(MUTED));
                item_link(ui, m, art, l, 20.0);
            }
            None => {
                ui.label(
                    RichText::new(tr!("Took off slot {slot}", slot = e.i("slot").unwrap_or(0)))
                        .color(MUTED),
                );
            }
        }),
        "money" => {
            let d = e.i("d").unwrap_or(0);
            line(ui, t, if d > 0 { "+" } else { "−" }, |ui| {
                super::widgets::coins(ui, art, d.abs(), 15.0);
                ui.label(
                    RichText::new(match e.s("ctx") {
                        Some("loot") => tr!("looted"),
                        Some("merchant") => tr!("at a vendor"),
                        Some("quest") => tr!("quest reward"),
                        Some("trainer") => tr!("at a trainer"),
                        Some("mail") => tr!("mail"),
                        Some(x) => x,
                        None => "",
                    })
                    .color(MUTED),
                );
            });
        }
        "xp" => line(ui, t, "↑", |ui| {
            ui.label(RichText::new(tr!("+{xp} XP", xp = e.i("d").unwrap_or(0))).color(MUTED));
        }),
        "level" => line(ui, t, "★", |ui| {
            ui.label(
                RichText::new(tr!(
                    "Reached level {level}",
                    level = e.i("level").unwrap_or(0)
                ))
                .font(theme::display_font(20.0))
                .color(GOLD),
            );
        }),
        "quest" => line(ui, t, "❗", |ui| match e.s("act") {
            Some("accept") => {
                ui.label(RichText::new(tr!("Accepted")).color(MUTED));
                ui.label(RichText::new(title(e.i("id"))).color(INK));
            }
            Some("turnin") => {
                ui.label(RichText::new(tr!("Completed")).color(GOLD));
                ui.label(RichText::new(title(e.i("id"))).color(INK));
                if let Some(xp) = e.i("xp").filter(|x| *x > 0) {
                    ui.label(RichText::new(tr!("+{xp} XP", xp = xp)).color(MUTED));
                }
                if let Some(l) = e.s("choice") {
                    item_link(ui, m, art, l, 20.0);
                }
            }
            _ => {
                ui.label(RichText::new(tr!("Abandoned")).color(MUTED));
                ui.label(RichText::new(title(e.i("id"))).color(INK));
            }
        }),
        "objective" => line(ui, t, "◦", |ui| {
            ui.label(
                RichText::new(format!(
                    "{}/{}",
                    e.i("have").unwrap_or(0),
                    e.i("need").unwrap_or(0)
                ))
                .color(INK),
            );
            ui.label(RichText::new(e.s("text").unwrap_or("")).color(MUTED));
        }),
        "zone" => line(ui, t, "⌖", |ui| {
            ui.label(
                RichText::new(tr!("Entered {zone}", zone = e.s("zone").unwrap_or("?"))).color(INK),
            );
        }),
        "subzone" => {
            if let Some(sub) = e.s("sub").filter(|s| !s.is_empty()) {
                line(ui, t, "⌖", |ui| {
                    ui.label(RichText::new(sub).color(MUTED));
                });
            }
        }
        "death" => line(ui, t, "☠", |ui| {
            let place = [e.s("sub").unwrap_or(""), e.s("zone").unwrap_or("")]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(RichText::new(tr!("Died in {place}", place = place)).color(DANGER));
        }),
        "alive" | "unghost" => line(ui, t, "✚", |ui| {
            ui.label(RichText::new(tr!("Back on your feet")).color(MUTED));
        }),
        "gossip" => line(ui, t, "☺", |ui| {
            let name = e.s("name").unwrap_or(tr!("someone"));
            let r = ui.label(RichText::new(tr!("Talked to {name}", name = name)).color(MUTED));
            // What they said, from the gossip cache.
            if let Some((_, text, options)) = m.memory.gossip.iter().find(|(n, _, _)| n == name) {
                r.on_hover_ui(|ui| {
                    ui.set_max_width(380.0);
                    ui.label(RichText::new(text).family(theme::italic()).color(INK));
                    for o in options {
                        ui.label(RichText::new(format!("› {o}")).color(GOLD));
                    }
                });
            }
        }),
        "gossip_pick" => line(ui, t, "☺", |ui| {
            ui.label(RichText::new(format!("“{}”", e.s("option").unwrap_or(""))).color(INK));
        }),
        "group" => line(ui, t, "☺", |ui| {
            let names: Vec<String> =
                e.v.get("members")
                    .map(crate::data::memory::entries)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|(_, v)| v.as_str().map(str::to_string))
                    .collect();
            ui.label(
                RichText::new(if names.is_empty() {
                    tr!("Left the group").into()
                } else {
                    tr!("Group: {names}", names = names.join(", "))
                })
                .color(INK),
            );
        }),
        "open" => {
            if let Some(w) = e.s("what").filter(|w| *w != "loot") {
                line(ui, t, "⌂", |ui| {
                    let text = match w {
                        "merchant" => tr!("Visited a vendor").into(),
                        "trainer" => tr!("Visited a trainer").into(),
                        "bank" => tr!("Visited the bank").into(),
                        "mail" | "mailbox" => tr!("Checked the mail").into(),
                        "auction" => tr!("Visited the auction house").into(),
                        "taxi" => tr!("Visited a flight master").into(),
                        _ => tr!("Visited a {what}", what = w),
                    };
                    ui.label(RichText::new(text).color(MUTED));
                });
            }
        }
        "msg" => {
            if matches!(e.s("kind"), Some("skill") | Some("rep")) {
                line(ui, t, "•", |ui| {
                    ui.label(RichText::new(e.s("text").unwrap_or("")).color(MUTED));
                });
            }
        }
        "spell" => line(ui, t, "•", |ui| {
            ui.label(RichText::new(tr!("Learned a new spell")).color(INK));
        }),
        "played" => line(ui, t, "•", |ui| {
            ui.label(
                RichText::new(tr!(
                    "/played: {time}",
                    time = theme::duration(e.f("total").unwrap_or(0.0))
                ))
                .color(MUTED),
            );
        }),
        _ => {}
    }
}
