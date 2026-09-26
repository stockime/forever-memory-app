//! The journal: sessions on the left, and for the chosen one everything that
//! happened as a timeline, in chapters by zone, with the chat that was going
//! on at the time. Runs of the same thing fold into one line.

use super::widgets::{self, icons};
use super::{card, character, chips};
use crate::art::Art;
use crate::data::Model;
use crate::data::chat::Kind;
use crate::data::memory::{Character, Event, QuestStatus, Session};
use crate::theme::{self, DANGER, GOLD, INK, MUTED, RAISED};
use crate::tr;
use crate::{Page, State};
use egui::{Color32, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

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

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art, page: &mut Page) {
    let c = character(m, st);
    if c.sessions.is_empty() {
        super::empty(ui, art, icons::BOOK, tr!("The road is still unwritten"), tr!("Every time you set out, the journey is written down here, one session at a time."));
        return;
    }
    let sel = st
        .session
        .unwrap_or(c.sessions.len() - 1)
        .min(c.sessions.len() - 1);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            sessions(ui, c, st, sel);
        });
        ui.add_space(16.0);
        let w = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width(w);
            header(ui, c, st, sel, art);
            ui.add_space(12.0);
            chips(ui, &CATEGORIES, &mut st.journal_hide);
            ui.add_space(10.0);
            let hide = st.journal_hide.clone();
            feed(ui, m, c, sel, &hide, st, page, art);
        });
    });
}

/// The sessions, sortable; the chosen one is marked by its background.
fn sessions(ui: &mut Ui, c: &Character, st: &mut State, sel: usize) {
    egui::ComboBox::from_id_salt("session-sort")
        .width(ui.available_width())
        .selected_text(crate::i18n::t(SORTS[st.session_sort.min(SORTS.len() - 1)]))
        .show_ui(ui, |ui| {
            for (i, s) in SORTS.iter().enumerate() {
                ui.selectable_value(&mut st.session_sort, i, crate::i18n::t(s));
            }
        });
    ui.add_space(6.0);
    let mut order: Vec<usize> = (0..c.sessions.len()).collect();
    let rate = |s: &Session| s.xp as f64 / s.seconds().max(60) as f64;
    match st.session_sort {
        0 => order.reverse(),
        1 => {}
        2 => order.sort_by_key(|&i| std::cmp::Reverse(c.sessions[i].seconds())),
        3 => order.sort_by_key(|&i| std::cmp::Reverse(c.sessions[i].xp)),
        4 => order.sort_by(|&a, &b| rate(&c.sessions[b]).total_cmp(&rate(&c.sessions[a]))),
        5 => order.sort_by_key(|&i| std::cmp::Reverse(c.sessions[i].items)),
        _ => order.sort_by_key(|&i| std::cmp::Reverse((c.sessions[i].deaths, c.sessions[i].start))),
    }
    egui::ScrollArea::vertical()
        .id_salt("sessions")
        .auto_shrink(false)
        .show(ui, |ui| {
            for i in order {
                let s = &c.sessions[i];
                let selected = i == sel;
                let r = egui::Frame::new()
                    .fill(if selected { RAISED } else { Color32::TRANSPARENT })
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{}, {}",
                                    theme::day(s.start as f64),
                                    theme::clock(s.start as f64)
                                ))
                                .color(if selected { GOLD } else { INK })
                                .size(16.0),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(theme::duration(s.seconds() as f64)).small().color(MUTED));
                            });
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            if s.level_to > s.level_from {
                                let t = tr!("level {from} → {to}", from = s.level_from, to = s.level_to);
                                ui.label(RichText::new(t).small().color(GOLD));
                            } else {
                                ui.label(RichText::new(tr!("level {level}", level = s.level_from)).small().color(MUTED));
                            }
                            if s.xp > 0 {
                                ui.label(RichText::new(tr!("{xp} XP", xp = theme::thousands(s.xp))).small().color(MUTED));
                            }
                            if s.deaths > 0 {
                                ui.label(RichText::new(format!("☠ {}", s.deaths)).small().color(DANGER));
                            }
                        });
                        if !s.zones.is_empty() {
                            ui.label(RichText::new(s.zones.join(", ")).small().color(MUTED));
                        }
                    })
                    .response
                    .interact(Sense::click());
                if r.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if r.clicked() {
                    st.session = Some(i);
                }
            }
        });
}

/// The session's card: when, where, the figures, and the way to its neighbours.
fn header(ui: &mut Ui, c: &Character, st: &mut State, sel: usize, art: &mut Art) {
    let s = &c.sessions[sel];
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(tr!(
                        "{day}, {start} to {end}",
                        day = theme::day(s.start as f64),
                        start = theme::clock(s.start as f64),
                        end = theme::clock(s.end as f64)
                    ))
                    .font(theme::display_font(26.0))
                    .color(INK),
                );
                let levels = if s.level_to > s.level_from {
                    tr!("level {from} → {to}", from = s.level_from, to = s.level_to)
                } else {
                    tr!("level {level}", level = s.level_from)
                };
                let mut sub = s.zones.join(" → ");
                if !sub.is_empty() {
                    sub.push_str(",  ");
                }
                sub.push_str(&levels);
                ui.label(RichText::new(sub).color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let later = ui.add_enabled(sel + 1 < c.sessions.len(), egui::Button::new(RichText::new("›").size(18.0)));
                if later.on_hover_text(tr!("The session after")).clicked() {
                    st.session = Some(sel + 1);
                }
                let earlier = ui.add_enabled(sel > 0, egui::Button::new(RichText::new("‹").size(18.0)));
                if earlier.on_hover_text(tr!("The session before")).clicked() {
                    st.session = Some(sel - 1);
                }
            });
        });
        ui.add_space(10.0);
        widgets::figure_row(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(22.0, 10.0);
            widgets::figure_text(ui, art, icons::WATCH, &theme::duration(s.seconds() as f64), tr!("played"));
            widgets::figure_text(ui, art, icons::SPIRIT, &theme::thousands(s.xp), tr!("experience"));
            widgets::figure_money(ui, art, icons::COIN, s.money, tr!("money"));
            widgets::figure_text(ui, art, icons::BAG, &s.items.to_string(), tr!("items looted"));
            widgets::figure_text(ui, art, icons::NOTE, &s.quests.to_string(), tr!("quests done"));
            if s.deaths > 0 {
                widgets::figure_text(ui, art, icons::FEIGN, &s.deaths.to_string(), tr!("deaths"));
            }
        });
    });
}

enum Item<'a> {
    /// A zone entered: the start of a chapter.
    Chapter(f64, String),
    Chat(usize),
    /// One event, or a run of the same kind folded into one line.
    Events(Vec<&'a Event>),
}

/// Runs of these fold into one line: gear put on, loot of one kind, coin
/// for one reason, spells learned, experience, one objective's progress.
fn run_key(e: &Event) -> Option<String> {
    let sign = |d: i64| if d > 0 { "+" } else { "-" };
    let loot = e.s("ctx") == Some("loot") && e.i("d").unwrap_or(0) > 0;
    match e.e.as_str() {
        "xp" => Some("hunt".into()),
        "money" | "item" if loot => Some("hunt".into()),
        "equip" if e.s("link").is_some() => Some("equip".into()),
        "msg" if e.s("kind") == Some("skill") => Some("skill".into()),
        "item" => Some(format!("item{}{}", e.s("ctx").unwrap_or(""), sign(e.i("d").unwrap_or(0)))),
        "spell" => Some("trainer".into()),
        "money" if e.s("ctx") == Some("trainer") => Some("trainer".into()),
        "money" => Some(format!("money{}{}", e.s("ctx").unwrap_or(""), sign(e.i("d").unwrap_or(0)))),
        "open" if e.s("what") == Some("trainer") => Some("trainer".into()),
        "objective" => Some(format!("objective{}", e.s("text").unwrap_or(""))),
        _ => None,
    }
}

/// Whether an event makes a line of its own at all.
fn told(e: &Event) -> bool {
    match e.e.as_str() {
        "open" => e.s("what").is_some_and(|w| w != "loot"),
        "msg" => matches!(e.s("kind"), Some("skill") | Some("rep")),
        "subzone" => e.s("sub").is_some_and(|s| !s.is_empty()),
        "login" | "logout" | "item" | "equip" | "money" | "xp" | "level" | "quest" | "objective"
        | "death" | "alive" | "unghost" | "gossip" | "gossip_pick" | "group" | "spell" | "played" => true,
        _ => false,
    }
}

const TIME_W: f32 = 46.0;
/// A spellbook, for the trainer's lessons.
const TRAINER: i64 = 133741;
const RULE_W: f32 = 40.0;

#[allow(clippy::too_many_arguments)]
fn feed(
    ui: &mut Ui,
    m: &Model,
    c: &Character,
    sel: usize,
    hide: &std::collections::HashSet<&'static str>,
    st: &mut State,
    page: &mut Page,
    art: &mut Art,
) {
    let s = &c.sessions[sel];
    let events = &c.events[s.from..s.to];
    let mut rows: Vec<(f64, Option<&Event>, usize)> = events.iter().map(|e| (e.t as f64, Some(e), 0)).collect();
    if !hide.contains("Chat") {
        for (i, l) in m.chat.iter().enumerate() {
            if l.t >= s.start as f64 - 5.0 && l.t <= s.end as f64 + 5.0 && l.kind != Kind::System {
                rows.push((l.t, None, i));
            }
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut items: Vec<Item> = vec![];
    let mut zone: Option<String> = None;
    let mut last_turnin: Option<(i64, i64)> = None;
    for (t, e, chat) in rows {
        let Some(e) = e else {
            items.push(Item::Chat(chat));
            continue;
        };
        if e.e == "quest" && e.s("act") == Some("turnin") {
            last_turnin = e.i("id").map(|id| (id, e.t));
        }
        if e.e == "quest"
            && e.s("act") == Some("remove")
            && last_turnin.is_some_and(|(id, at)| Some(id) == e.i("id") && e.t - at < 10)
        {
            continue; // removal is part of every turn-in
        }
        // Zones make the chapters, whatever is filtered.
        if e.e == "zone" || e.e == "login" {
            if let Some(z) = e.s("zone").filter(|z| !z.is_empty())
                && zone.as_deref() != Some(z)
            {
                zone = Some(z.to_string());
                // The chapter opens before the login that led into it.
                let at = match items.last() {
                    Some(Item::Events(run)) if run[0].e == "login" && e.t - run[0].t <= 60 => items.len() - 1,
                    _ => items.len(),
                };
                items.insert(at, Item::Chapter(t, z.to_string()));
            }
            if e.e == "zone" {
                continue;
            }
        }
        if !told(e) || hide.contains(category(e)) {
            continue;
        }
        // A trainer's fees and lessons join the visit, whoever spoke in between.
        if run_key(e).as_deref() == Some("trainer")
            && let Some(run) = items.iter_mut().rev().take(4).find_map(|i| match i {
                Item::Events(run) if run_key(run[0]).as_deref() == Some("trainer") && e.t - run[run.len() - 1].t <= 120 => Some(run),
                _ => None,
            })
        {
            run.push(e);
            continue;
        }
        // A hunt, or a run of skill-ups, goes on while others talk.
        if matches!(run_key(e).as_deref(), Some("hunt") | Some("skill"))
            && let Some(run) = items.iter_mut().rev().find(|i| !matches!(i, Item::Chat(_))).and_then(|i| match i {
                Item::Events(run) if run_key(run[0]) == run_key(e) && e.t - run[run.len() - 1].t <= 150 => Some(run),
                _ => None,
            })
        {
            run.push(e);
            continue;
        }
        if let (Some(key), Some(Item::Events(run))) = (run_key(e), items.last_mut())
            && run_key(run[0]).as_ref() == Some(&key)
            && e.t - run[run.len() - 1].t <= 90
        {
            run.push(e);
            continue;
        }
        items.push(Item::Events(vec![e]));
    }

    egui::ScrollArea::vertical()
        .id_salt("feed")
        .auto_shrink(false)
        .show(ui, |ui| {
            if !items.iter().any(|i| !matches!(i, Item::Chapter(..))) {
                super::quiet(ui, art, icons::BOOK, tr!("Nothing of that kind in this session."));
                return;
            }
            let titles: std::collections::HashMap<i64, &str> =
                c.quests.iter().map(|q| (q.id, q.title.as_str())).collect();
            let rule_x = ui.cursor().left() + TIME_W + ui.spacing().item_spacing.x + RULE_W / 2.0;
            let rule = ui.painter().add(egui::Shape::Noop);
            let top = ui.cursor().top();
            let mut bottom = top;
            for item in &items {
                match item {
                    Item::Chapter(t, z) => {
                        ui.add_space(8.0);
                        row(ui, *t, Mark::Chapter, art, |ui, _| {
                            ui.label(RichText::new(z).font(theme::display_font(21.0)).color(GOLD));
                        });
                        ui.add_space(4.0);
                    }
                    Item::Chat(i) => {
                        let l = &m.chat[*i];
                        row(ui, l.t, Mark::Dot(false), art, |ui, _| {
                            ui.label(RichText::new(format!("[{}]", l.label())).small().color(MUTED));
                            let who = l.speaker.as_deref().unwrap_or("");
                            let color = m
                                .players
                                .iter()
                                .find(|p| p.name == who || p.first == who)
                                .and_then(|p| p.class)
                                .map(theme::class_color)
                                .unwrap_or(MUTED);
                            ui.label(RichText::new(format!("{who}:")).color(color));
                            ui.label(RichText::new(&l.text).family(theme::italic()).color(Color32::from_rgb(0xc8, 0xc3, 0xb4)));
                        });
                    }
                    Item::Events(run) => event(ui, m, c, run, &titles, st, page, art),
                }
                bottom = ui.cursor().top();
            }
            ui.painter().set(
                rule,
                egui::Shape::line_segment(
                    [pos2(rule_x, top + 10.0), pos2(rule_x, bottom - 12.0)],
                    Stroke::new(2.0, Color32::from_rgba_unmultiplied(0xb0, 0x8a, 0x2e, 70)),
                ),
            );
            ui.add_space(24.0);
        });
}

enum Mark<'a> {
    Chapter,
    Icon(i64),
    Portrait(&'a str),
    /// A small point on the rule; gold for things worth a glance.
    Dot(bool),
}

/// One line of the timeline: the time, its mark on the rule, and what happened.
fn row(ui: &mut Ui, t: f64, mark: Mark, art: &mut Art, add: impl FnOnce(&mut Ui, &mut Art)) {
    ui.horizontal_top(|ui| {
        let h = if matches!(mark, Mark::Chapter) { 30.0 } else { 24.0 };
        ui.allocate_ui_with_layout(vec2(TIME_W, h), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.set_width(TIME_W);
            ui.label(RichText::new(theme::clock(t)).small().color(MUTED));
        });
        let (rect, _) = ui.allocate_exact_size(vec2(RULE_W, h), Sense::hover());
        let c = rect.center();
        let p = ui.painter();
        match mark {
            Mark::Chapter => icon(p, art, ui.ctx(), c, 28.0, icons::MAP, true),
            Mark::Icon(id) => icon(p, art, ui.ctx(), c, 22.0, id, false),
            Mark::Portrait(name) => {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(c, vec2(24.0, 24.0))));
                super::conversations::portrait(&mut child, name, 24.0);
            }
            Mark::Dot(gold) => {
                p.circle_filled(c, 5.0, theme::NIGHT);
                let col = if gold { GOLD } else { Color32::from_rgb(0x6b, 0x5a, 0x33) };
                p.circle_filled(c, 3.0, col);
            }
        }
        let w = ui.available_width();
        ui.allocate_ui_with_layout(vec2(w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_max_width(w);
            ui.add_space((h - 20.0) / 2.0);
            ui.horizontal_wrapped(|ui| add(ui, art));
        });
    });
}

fn icon(p: &egui::Painter, art: &mut Art, ctx: &egui::Context, c: egui::Pos2, size: f32, id: i64, gold: bool) {
    let r = Rect::from_center_size(c, vec2(size, size));
    p.rect_filled(r.expand(3.0), 5.0, theme::NIGHT);
    p.rect_filled(r, 4.0, Color32::from_rgb(5, 7, 15));
    if let Some(t) = art.icon(ctx, Some(id)) {
        p.image(t.id(), r.shrink(1.0), Rect::from_min_max(pos2(0.07, 0.07), pos2(0.93, 0.93)), Color32::WHITE);
    }
    let edge = if gold { GOLD } else { Color32::from_rgb(0x8a, 0x6d, 0x2c) };
    p.rect_stroke(r, 4.0, Stroke::new(1.0, edge), egui::StrokeKind::Outside);
}

/// Text that takes you somewhere else.
fn link(ui: &mut Ui, text: RichText) -> bool {
    let r = ui.add(egui::Label::new(text).sense(Sense::click()));
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r.clicked()
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

#[allow(clippy::too_many_arguments)]
fn event(
    ui: &mut Ui,
    m: &Model,
    c: &Character,
    run: &[&Event],
    titles: &std::collections::HashMap<i64, &str>,
    st: &mut State,
    page: &mut Page,
    art: &mut Art,
) {
    let e = run[0];
    let t = e.t as f64;
    let title = |id: Option<i64>| {
        id.and_then(|i| titles.get(&i).copied())
            .map(str::to_string)
            .or_else(|| e.s("title").map(str::to_string))
            .unwrap_or_else(|| tr!("a quest").into())
    };
    let open_quest = |st: &mut State, page: &mut Page, id: Option<i64>| {
        let Some(id) = id else { return };
        st.quest = Some(id);
        st.quest_tab = match c.quests.iter().find(|q| q.id == id).map(|q| &q.status) {
            Some(QuestStatus::Active) => 0,
            Some(QuestStatus::Completed) => 1,
            _ => 2,
        };
        *page = Page::Quests;
    };
    if run_key(e).as_deref() == Some("hunt") {
        let (from, to) = (e.t as f64 - 30.0, run[run.len() - 1].t as f64 + 2.0);
        let kills = m.combat.kills.iter().filter(|(t, _)| *t >= from && *t <= to).count();
        let xp: i64 = run.iter().filter(|e| e.e == "xp").map(|e| e.i("d").unwrap_or(0)).sum();
        let coin: i64 = run.iter().filter(|e| e.e == "money").map(|e| e.i("d").unwrap_or(0)).sum();
        // Each item once, with how many.
        let mut found: Vec<(&str, i64)> = vec![];
        for e in run.iter().filter(|e| e.e == "item") {
            if let Some(l) = e.s("link") {
                let n = e.i("d").unwrap_or(1);
                match found.iter_mut().find(|(x, _)| *x == l) {
                    Some(f) => f.1 += n,
                    None => found.push((l, n)),
                }
            }
        }
        let mark = if kills > 0 || xp > 0 { Mark::Icon(icons::SWORDS) } else { Mark::Icon(icons::BAG) };
        row(ui, t, mark, art, |ui, art| {
            if kills > 0 {
                let text = if kills == 1 { tr!("1 kill").to_string() } else { tr!("{n} kills", n = kills) };
                ui.label(RichText::new(text).color(INK));
            }
            if xp > 0 {
                ui.label(RichText::new(tr!("+{xp} XP", xp = theme::thousands(xp))).color(MUTED));
            }
            if coin > 0 {
                ui.label(RichText::new("+").color(MUTED));
                widgets::coins(ui, art, coin, 15.0);
            }
            if !found.is_empty() {
                ui.label(RichText::new(tr!("Looted")).color(MUTED));
                for (l, n) in &found {
                    if *n > 1 {
                        ui.label(RichText::new(format!("{n}×")).color(INK));
                    }
                    super::item_inline(ui, m, art, l, 20.0);
                }
            }
        });
        return;
    }
    if run_key(e).as_deref() == Some("skill") {
        // Each skill once, at the last rank reached: the game's line without
        // its number names the skill in any language.
        let mut last: Vec<(String, &str)> = vec![];
        for e in run {
            let text = e.s("text").unwrap_or("");
            let skill: String = text.chars().filter(|c| !c.is_ascii_digit()).collect();
            match last.iter_mut().find(|(k, _)| *k == skill) {
                Some(l) => l.1 = text,
                None => last.push((skill, text)),
            }
        }
        row(ui, t, Mark::Dot(false), art, |ui, _| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (_, text) in &last {
                    ui.label(RichText::new(*text).color(MUTED));
                }
            });
        });
        return;
    }
    if run_key(e).as_deref() == Some("trainer") {
        let spells = run.iter().filter(|e| e.e == "spell").count();
        let fee: i64 = run.iter().filter(|e| e.e == "money").map(|e| -e.i("d").unwrap_or(0)).sum();
        row(ui, t, Mark::Icon(TRAINER), art, |ui, art| {
            let text = match spells {
                0 => tr!("Visited a trainer").to_string(),
                1 => tr!("Learned a new spell").to_string(),
                n => tr!("Learned {n} new spells", n = n),
            };
            ui.label(RichText::new(text).color(if spells > 0 { INK } else { MUTED }));
            if fee > 0 {
                ui.label(RichText::new(tr!("for")).color(MUTED));
                widgets::coins(ui, art, fee, 15.0);
            }
        });
        return;
    }
    match e.e.as_str() {
        "login" => row(ui, t, Mark::Dot(false), art, |ui, _| {
            let level = e.i("level").unwrap_or(0);
            ui.label(RichText::new(tr!("Logged in at level {level}", level = level)).color(MUTED));
        }),
        "logout" => row(ui, t, Mark::Dot(false), art, |ui, _| {
            ui.label(
                RichText::new(tr!("Logged out in {zone}", zone = e.s("zone").unwrap_or("?")))
                    .color(MUTED),
            );
        }),
        "item" => {
            let d = e.i("d").unwrap_or(0);
            row(ui, t, Mark::Icon(icons::BAG), art, |ui, art| {
                ui.label(RichText::new(context(e.s("ctx"), d > 0)).color(MUTED));
                for e in run {
                    let n = e.i("d").unwrap_or(0).abs();
                    if n > 1 {
                        ui.label(RichText::new(format!("{n}×")).color(INK));
                    }
                    if let Some(l) = e.s("link") {
                        super::item_inline(ui, m, art, l, 20.0);
                    }
                }
            });
        }
        "equip" => row(ui, t, Mark::Icon(icons::CHEST), art, |ui, art| match e.s("link") {
            Some(_) => {
                ui.label(RichText::new(tr!("Equipped")).color(MUTED));
                for e in run {
                    if let Some(l) = e.s("link") {
                        super::item_inline(ui, m, art, l, 20.0);
                    }
                }
            }
            None => {
                ui.label(
                    RichText::new(tr!("Took off slot {slot}", slot = e.i("slot").unwrap_or(0)))
                        .color(MUTED),
                );
            }
        }),
        "money" => {
            let d: i64 = run.iter().map(|e| e.i("d").unwrap_or(0)).sum();
            row(ui, t, Mark::Icon(icons::COIN), art, |ui, art| {
                ui.label(RichText::new(if d > 0 { "+" } else { "−" }).color(MUTED));
                widgets::coins(ui, art, d.abs(), 15.0);
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
        "xp" => {
            let xp: i64 = run.iter().map(|e| e.i("d").unwrap_or(0)).sum();
            row(ui, t, Mark::Dot(false), art, |ui, _| {
                ui.label(RichText::new(tr!("+{xp} XP", xp = theme::thousands(xp))).color(MUTED));
            });
        }
        "level" => {
            let level = e.i("level").unwrap_or(0);
            let icon = match level {
                10 => 134414,
                20 => 134153,
                _ => icons::SPIRIT,
            };
            ui.add_space(4.0);
            row(ui, t, Mark::Icon(icon), art, |ui, _| {
                ui.label(
                    RichText::new(tr!("Reached level {level}", level = level))
                        .font(theme::display_font(20.0))
                        .color(GOLD),
                );
            });
            ui.add_space(4.0);
        }
        "quest" => row(ui, t, Mark::Icon(icons::NOTE), art, |ui, art| {
            let (verb, color) = match e.s("act") {
                Some("accept") => (tr!("Accepted"), MUTED),
                Some("turnin") => (tr!("Completed"), GOLD),
                _ => (tr!("Abandoned"), MUTED),
            };
            ui.label(RichText::new(verb).color(color));
            if link(ui, RichText::new(title(e.i("id"))).color(INK)) {
                open_quest(st, page, e.i("id"));
            }
            if e.s("act") == Some("turnin") {
                if let Some(xp) = e.i("xp").filter(|x| *x > 0) {
                    ui.label(RichText::new(tr!("+{xp} XP", xp = theme::thousands(xp))).color(MUTED));
                }
                if let Some(l) = e.s("choice") {
                    super::item_inline(ui, m, art, l, 20.0);
                }
            }
        }),
        "objective" => {
            let e = run[run.len() - 1];
            row(ui, e.t as f64, Mark::Dot(false), art, |ui, _| {
                ui.label(
                    RichText::new(format!("{}/{}", e.i("have").unwrap_or(0), e.i("need").unwrap_or(0)))
                        .color(INK),
                );
                ui.label(RichText::new(e.s("text").unwrap_or("")).color(MUTED));
            });
        }
        "subzone" => {
            let sub = e.s("sub").unwrap_or("");
            row(ui, t, Mark::Dot(false), art, |ui, _| {
                ui.label(RichText::new(sub).family(theme::italic()).color(INK));
            });
        }
        "death" => row(ui, t, Mark::Icon(icons::SKULL), art, |ui, _| {
            let place = [e.s("sub").unwrap_or(""), e.s("zone").unwrap_or("")]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            if link(ui, RichText::new(tr!("Died in {place}", place = place)).color(DANGER)) {
                *page = Page::Dead;
            }
        }),
        "alive" | "unghost" => row(ui, t, Mark::Icon(icons::HEAL), art, |ui, _| {
            ui.label(RichText::new(tr!("Back on your feet")).color(MUTED));
        }),
        "gossip" => {
            let name = e.s("name").unwrap_or(tr!("someone"));
            row(ui, t, Mark::Portrait(name), art, |ui, _| {
                let text = RichText::new(tr!("Talked to {name}", name = name)).color(INK);
                let r = ui.add(egui::Label::new(text).sense(Sense::click()));
                if r.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let clicked = r.clicked();
                // What they said, from the gossip cache.
                if let Some(g) = m.memory.gossip.iter().find(|g| g.name == name) {
                    r.on_hover_ui(|ui| {
                        ui.set_max_width(380.0);
                        ui.label(RichText::new(&g.text).family(theme::italic()).color(INK));
                        for o in &g.options {
                            ui.label(RichText::new(format!("› {o}")).color(GOLD));
                        }
                    });
                }
                if clicked {
                    st.quest_tab = super::quests::CONVERSATIONS;
                    st.talk.npc = Some(name.to_string());
                    *page = Page::Quests;
                }
            });
        }
        "gossip_pick" => row(ui, t, Mark::Dot(false), art, |ui, _| {
            ui.label(RichText::new(format!("“{}”", e.s("option").unwrap_or(""))).family(theme::italic()).color(INK));
        }),
        "group" => {
            let names: Vec<String> =
                e.v.get("members")
                    .map(crate::data::memory::entries)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|(_, v)| v.as_str().map(str::to_string))
                    .collect();
            row(ui, t, Mark::Icon(icons::GROUP), art, |ui, _| {
                ui.label(
                    RichText::new(if names.is_empty() {
                        tr!("Left the group").into()
                    } else {
                        tr!("Group: {names}", names = names.join(", "))
                    })
                    .color(INK),
                );
            });
        }
        "open" => {
            let w = e.s("what").unwrap_or("");
            row(ui, t, Mark::Dot(false), art, |ui, _| {
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
        "msg" => {
            let mark = if e.s("kind") == Some("rep") { Mark::Icon(236683) } else { Mark::Dot(false) };
            row(ui, t, mark, art, |ui, _| {
                ui.label(RichText::new(e.s("text").unwrap_or("")).color(MUTED));
            });
        }
        "spell" => row(ui, t, Mark::Dot(true), art, |ui, _| {
            let text = if run.len() == 1 {
                tr!("Learned a new spell").to_string()
            } else {
                tr!("Learned {n} new spells", n = run.len())
            };
            ui.label(RichText::new(text).color(INK));
        }),
        "played" => row(ui, t, Mark::Dot(false), art, |ui, _| {
            ui.label(
                RichText::new(tr!("/played: {time}", time = theme::duration(e.f("total").unwrap_or(0.0))))
                    .color(MUTED),
            );
        }),
        _ => {}
    }
}
