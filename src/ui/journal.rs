//! The journal: sessions on the left, and for the chosen one everything that
//! happened, in order, with the chat that was going on at the time.

use super::{character, chips, item_link};
use crate::art::Art;
use crate::data::chat::Kind;
use crate::data::memory::{Character, Event};
use crate::data::Model;
use crate::theme::{self, DANGER, GOLD, INK, MUTED, RAISED};
use crate::State;
use egui::{Color32, RichText, Ui};

const CATEGORIES: [&str; 9] = ["Quests", "Loot", "Money", "Experience", "Places", "Combat", "People", "Chat", "System"];

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    if c.sessions.is_empty() {
        super::empty(ui, "No sessions recorded yet.");
        return;
    }
    let sel = st.session.unwrap_or(c.sessions.len() - 1).min(c.sessions.len() - 1);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            super::heading(ui, "Sessions");
            egui::ScrollArea::vertical().id_salt("sessions").auto_shrink(false).show(ui, |ui| {
                for (i, s) in c.sessions.iter().enumerate().rev() {
                    let selected = i == sel;
                    let r = egui::Frame::new()
                        .fill(if selected { RAISED } else { Color32::TRANSPARENT })
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(format!("{}, {}", theme::day(s.start as f64), theme::clock(s.start as f64))).color(if selected { GOLD } else { INK }).size(16.0));
                            let levels = if s.level_to > s.level_from { format!("level {} → {}", s.level_from, s.level_to) } else { format!("level {}", s.level_from) };
                            ui.label(RichText::new(format!("{}, {}, {} XP", theme::duration(s.seconds() as f64), levels, theme::thousands(s.xp))).small().color(MUTED));
                            if !s.zones.is_empty() {
                                ui.label(RichText::new(s.zones.join(", ")).small().color(MUTED));
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
        ui.vertical(|ui| {
            let s = &c.sessions[sel];
            ui.label(RichText::new(format!("{}, {} to {}", theme::day(s.start as f64), theme::clock(s.start as f64), theme::clock(s.end as f64))).font(theme::display_font(28.0)));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 36.0;
                super::figure(ui, &theme::duration(s.seconds() as f64), "played");
                super::figure(ui, &theme::thousands(s.xp), "experience");
                super::figure(ui, &theme::money(s.money), "money");
                super::figure(ui, &s.items.to_string(), "items looted");
                super::figure(ui, &s.quests.to_string(), "quests done");
                super::figure(ui, &s.deaths.to_string(), "deaths");
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

fn feed(ui: &mut Ui, m: &Model, c: &Character, sel: usize, hide: &std::collections::HashSet<&'static str>, art: &mut Art) {
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
    let titles: std::collections::HashMap<i64, &str> = c.quests.iter().map(|q| (q.id, q.title.as_str())).collect();
    egui::ScrollArea::vertical().id_salt("feed").auto_shrink(false).show(ui, |ui| {
        let mut last_turnin: Option<(i64, i64)> = None;
        for (t, row) in rows {
            match row {
                Row::Chat(i) => {
                    let l = &m.chat[i];
                    line(ui, t, "›", |ui| {
                        ui.label(RichText::new(format!("[{}]", l.label())).small().color(MUTED));
                        ui.label(RichText::new(format!("{}:", l.speaker.as_deref().unwrap_or(""))).color(MUTED));
                        ui.label(RichText::new(&l.text).color(Color32::from_rgb(0xc8, 0xc3, 0xb4)));
                    });
                }
                Row::Event(e) => {
                    if e.e == "quest" && e.s("act") == Some("turnin") {
                        last_turnin = e.i("id").map(|id| (id, e.t));
                    }
                    if e.e == "quest" && e.s("act") == Some("remove") && last_turnin.is_some_and(|(id, at)| Some(id) == e.i("id") && e.t - at < 10) {
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
        (Some("loot"), _) => "Looted",
        (Some("merchant"), true) => "Bought",
        (Some("merchant"), false) => "Sold",
        (Some("quest"), true) => "Quest reward",
        (Some("quest"), false) => "Handed in",
        (Some("mail"), true) => "From the mail",
        (Some("trade"), _) => "Traded",
        (Some("auction"), _) => "Auction house",
        (Some("bank"), _) => "Bank",
        (_, true) => "Received",
        (_, false) => "Used up",
    }
}

fn event(ui: &mut Ui, m: &Model, e: &Event, titles: &std::collections::HashMap<i64, &str>, hide: &std::collections::HashSet<&'static str>, art: &mut Art) {
    let cat = category(e);
    if hide.contains(cat) {
        return;
    }
    let t = e.t as f64;
    let title = |id: Option<i64>| id.and_then(|i| titles.get(&i).copied()).map(str::to_string).or_else(|| e.s("title").map(str::to_string)).unwrap_or_else(|| "a quest".into());
    match e.e.as_str() {
        "login" => line(ui, t, "▶", |ui| {
            let place = e.s("zone").filter(|z| !z.is_empty()).map(|z| format!(" in {z}")).unwrap_or_default();
            ui.label(RichText::new(format!("Logged in at level {}{place}", e.i("level").unwrap_or(0))).color(INK));
        }),
        "logout" => line(ui, t, "■", |ui| {
            ui.label(RichText::new(format!("Logged out in {}", e.s("zone").unwrap_or("?"))).color(MUTED));
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
                ui.label(RichText::new("Equipped").color(MUTED));
                item_link(ui, m, art, l, 20.0);
            }
            None => {
                ui.label(RichText::new(format!("Took off slot {}", e.i("slot").unwrap_or(0))).color(MUTED));
            }
        }),
        "money" => {
            let d = e.i("d").unwrap_or(0);
            line(ui, t, "⛃", |ui| {
                ui.label(RichText::new(if d > 0 { format!("+{}", theme::money(d)) } else { theme::money(d) }).color(if d > 0 { GOLD } else { INK }));
                ui.label(RichText::new(match e.s("ctx") { Some("loot") => "looted", Some("merchant") => "at a vendor", Some("quest") => "quest reward", Some("trainer") => "at a trainer", Some("mail") => "mail", Some(x) => x, None => "" }).color(MUTED));
            });
        }
        "xp" => line(ui, t, "↑", |ui| {
            ui.label(RichText::new(format!("+{} XP", e.i("d").unwrap_or(0))).color(MUTED));
        }),
        "level" => line(ui, t, "★", |ui| {
            ui.label(RichText::new(format!("Reached level {}", e.i("level").unwrap_or(0))).font(theme::display_font(20.0)).color(GOLD));
        }),
        "quest" => line(ui, t, "❗", |ui| match e.s("act") {
            Some("accept") => {
                ui.label(RichText::new("Accepted").color(MUTED));
                ui.label(RichText::new(title(e.i("id"))).color(INK));
            }
            Some("turnin") => {
                ui.label(RichText::new("Completed").color(GOLD));
                ui.label(RichText::new(title(e.i("id"))).color(INK));
                if let Some(xp) = e.i("xp").filter(|x| *x > 0) {
                    ui.label(RichText::new(format!("+{xp} XP")).color(MUTED));
                }
                if let Some(l) = e.s("choice") {
                    item_link(ui, m, art, l, 20.0);
                }
            }
            _ => {
                ui.label(RichText::new("Abandoned").color(MUTED));
                ui.label(RichText::new(title(e.i("id"))).color(INK));
            }
        }),
        "objective" => line(ui, t, "◦", |ui| {
            ui.label(RichText::new(format!("{}/{}", e.i("have").unwrap_or(0), e.i("need").unwrap_or(0))).color(INK));
            ui.label(RichText::new(e.s("text").unwrap_or("")).color(MUTED));
        }),
        "zone" => line(ui, t, "⌖", |ui| {
            ui.label(RichText::new(format!("Entered {}", e.s("zone").unwrap_or("?"))).color(INK));
        }),
        "subzone" => {
            if let Some(sub) = e.s("sub").filter(|s| !s.is_empty()) {
                line(ui, t, "⌖", |ui| {
                    ui.label(RichText::new(sub).color(MUTED));
                });
            }
        }
        "death" => line(ui, t, "☠", |ui| {
            let place = [e.s("sub").unwrap_or(""), e.s("zone").unwrap_or("")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
            ui.label(RichText::new(format!("Died in {place}")).color(DANGER));
        }),
        "alive" | "unghost" => line(ui, t, "✚", |ui| {
            ui.label(RichText::new("Back on your feet").color(MUTED));
        }),
        "gossip" => line(ui, t, "☺", |ui| {
            let name = e.s("name").unwrap_or("someone");
            let r = ui.label(RichText::new(format!("Talked to {name}")).color(MUTED));
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
            let names: Vec<String> = e.v.get("members").map(crate::data::memory::entries).unwrap_or_default().into_iter().filter_map(|(_, v)| v.as_str().map(str::to_string)).collect();
            ui.label(RichText::new(if names.is_empty() { "Left the group".into() } else { format!("Group: {}", names.join(", ")) }).color(INK));
        }),
        "open" => {
            if let Some(w) = e.s("what").filter(|w| *w != "loot") {
                line(ui, t, "⌂", |ui| {
                    ui.label(RichText::new(format!("Visited a {w}")).color(MUTED));
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
            ui.label(RichText::new("Learned a new spell").color(INK));
        }),
        "played" => line(ui, t, "•", |ui| {
            ui.label(RichText::new(format!("/played: {}", theme::duration(e.f("total").unwrap_or(0.0)))).color(MUTED));
        }),
        _ => {}
    }
}
