use super::{card, character, figure, label, plot};
use crate::art::Art;
use crate::data::memory::{Character, QuestStatus};
use crate::data::Model;
use crate::theme::{self, INK, MUTED, NIGHT, SERIES};
use crate::{Page, State};
use egui::{Color32, Rect, RichText, Ui};
use egui_plot::{Bar, BarChart, Line, PlotPoints};

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art, page: &mut Page) {
    let c = character(m, st);
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        hero(ui, c, art);
        ui.add_space(14.0);
        card(ui, |ui| figures(ui, m, c));
        ui.add_space(14.0);
        super::pair(ui, |ui| card(ui, |ui| leveling(ui, c)), |ui| card(ui, |ui| gold(ui, c)));
        ui.add_space(14.0);
        super::pair(ui, |ui| card(ui, |ui| zones(ui, c)), |ui| card(ui, |ui| sessions(ui, c)));
        ui.add_space(14.0);
        card(ui, |ui| up_next(ui, c, st, page));
        ui.add_space(24.0);
    });
}

fn hero(ui: &mut Ui, c: &Character, art: &mut Art) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 230.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 10.0, theme::PANEL);
    if let Some(tex) = art.get(ui.ctx(), &format!("banner-{}.jpg", c.class_file.to_lowercase())) {
        // Cover the rect, keeping the hero on the right side of the art in view.
        let [tw, th] = tex.size().map(|v| v as f32);
        let scale = (rect.width() / tw).max(rect.height() / th);
        let (uw, uh) = (rect.width() / (tw * scale), rect.height() / (th * scale));
        let uv = Rect::from_min_size(egui::pos2(1.0 - uw, (1.0 - uh) * 0.3), egui::vec2(uw, uh));
        p.image(tex.id(), rect, uv, Color32::WHITE);
    }
    // Night fades in from the left and the bottom so the text reads.
    let mut mesh = egui::Mesh::default();
    let clear = Color32::from_rgba_unmultiplied(10, 14, 31, 0);
    let dark = Color32::from_rgba_unmultiplied(10, 14, 31, 235);
    let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
    let mid = l + rect.width() * 0.62;
    for (a, bb, ca, cb) in [((l, t), (mid, b), dark, clear)] {
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(egui::pos2(a.0, a.1), ca);
        mesh.colored_vertex(egui::pos2(bb.0, a.1), cb);
        mesh.colored_vertex(egui::pos2(bb.0, bb.1), cb);
        mesh.colored_vertex(egui::pos2(a.0, bb.1), ca);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    let i = mesh.vertices.len() as u32;
    let top = t + rect.height() * 0.45;
    mesh.colored_vertex(egui::pos2(l, top), clear);
    mesh.colored_vertex(egui::pos2(r, top), clear);
    mesh.colored_vertex(egui::pos2(r, b), NIGHT);
    mesh.colored_vertex(egui::pos2(l, b), NIGHT);
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
    p.add(mesh);

    let color = theme::class_color(&c.class_file);
    let x = rect.left() + 28.0;
    p.text(egui::pos2(x, rect.bottom() - 96.0), egui::Align2::LEFT_BOTTOM, &c.name, theme::display_font(56.0), color);
    let pct = if c.xp.1 > 0 { format!(", {:.0}% to {}", c.xp.0 as f64 / c.xp.1 as f64 * 100.0, c.level + 1) } else { String::new() };
    p.text(egui::pos2(x + 2.0, rect.bottom() - 62.0), egui::Align2::LEFT_BOTTOM, format!("Level {} {} {}{pct}", c.level, c.race, c.class), egui::FontId::proportional(20.0), INK);
    let played = c.total_play() as f64;
    let last = c.sessions.last().map(|s| s.end as f64).unwrap_or(0.0);
    let line = format!(
        "{}{} played over {} session{}, last seen {}",
        if c.zone.is_empty() { String::new() } else { format!("In {}. ", c.zone) },
        theme::duration(played),
        c.sessions.len(),
        if c.sessions.len() == 1 { "" } else { "s" },
        theme::ago(last)
    );
    p.text(egui::pos2(x + 2.0, rect.bottom() - 34.0), egui::Align2::LEFT_BOTTOM, line, egui::FontId::proportional(15.5), MUTED);
}

fn in_play(c: &Character, t: f64) -> bool {
    c.sessions.iter().any(|s| t >= s.start as f64 - 60.0 && t <= s.end as f64 + 60.0)
}

fn figures(ui: &mut Ui, m: &Model, c: &Character) {
    let played = c.total_play() as f64;
    let xp: i64 = c.sessions.iter().map(|s| s.xp).sum();
    let quests = c.quests.iter().filter(|q| q.status == QuestStatus::Completed).count();
    let kills = m.combat.kills.iter().filter(|(t, _)| in_play(c, *t)).count();
    let deaths = c.events.iter().filter(|e| e.e == "death").count();
    let met = m.players.iter().filter(|p| in_play(c, p.last_seen) || in_play(c, p.first_seen)).count();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 44.0;
        figure(ui, &theme::duration(played), "played");
        figure(ui, &if played > 60.0 { theme::thousands((xp as f64 / played * 3600.0) as i64) } else { "–".into() }, "XP per hour");
        figure(ui, &theme::money(c.money), "carried");
        figure(ui, &quests.to_string(), "quests done");
        figure(ui, &kills.to_string(), "kills");
        figure(ui, &deaths.to_string(), "deaths");
        figure(ui, &met.to_string(), "players met");
    });
}

/// Fractional level (level + xp share) against hours played.
pub fn level_series(c: &Character) -> Vec<[f64; 2]> {
    let mut out = vec![];
    let mut level = 0.0;
    for e in &c.events {
        let x = c.play_time(e.t) / 3600.0;
        match e.e.as_str() {
            "login" => {
                level = e.i("level").unwrap_or(0) as f64;
                out.push([x, level]);
            }
            "xp" => {
                let (cur, max) = (e.i("cur").unwrap_or(0) as f64, e.i("max").unwrap_or(1).max(1) as f64);
                out.push([x, level.floor() + cur / max]);
            }
            "level" => {
                level = e.i("level").unwrap_or(0) as f64;
                out.push([x, level]);
            }
            _ => {}
        }
        if let Some(last) = out.last() {
            level = last[1].max(level);
        }
    }
    out
}

fn leveling(ui: &mut Ui, c: &Character) {
    label(ui, "Leveling");
    ui.label(RichText::new("Level against hours played").small().color(MUTED));
    let pts = level_series(c);
    if pts.len() < 2 {
        ui.label(RichText::new("Needs a little more play to draw.").color(MUTED));
        return;
    }
    plot("leveling")
        .height(220.0)
        .x_axis_formatter(|g, _| format!("{:.1}h", g.value))
        .label_formatter(|h| {
            let p = super::hover(h);
            Some(format!("level {:.2}\n{} played", p.y, theme::duration(p.x * 3600.0)))
        })
        .show(ui, |pu| {
            pu.line(Line::new("Level", PlotPoints::from(pts)).color(SERIES[0]).width(2.0).fill(0.0).fill_alpha(0.12));
        });
}

fn gold(ui: &mut Ui, c: &Character) {
    label(ui, "Gold");
    ui.label(RichText::new("Money carried against hours played").small().color(MUTED));
    let mut pts = vec![];
    for e in &c.events {
        let v = match e.e.as_str() {
            "login" | "logout" => e.i("money"),
            "money" => e.i("total"),
            _ => None,
        };
        if let Some(v) = v {
            pts.push([c.play_time(e.t) / 3600.0, v as f64]);
        }
    }
    if pts.len() < 2 {
        ui.label(RichText::new("No money changes yet.").color(MUTED));
        return;
    }
    plot("gold")
        .height(220.0)
        .x_axis_formatter(|g, _| format!("{:.1}h", g.value))
        .y_axis_formatter(|g, _| theme::money(g.value as i64))
        .label_formatter(|h| {
            let p = super::hover(h);
            Some(format!("{}\n{} played", theme::money(p.y as i64), theme::duration(p.x * 3600.0)))
        })
        .show(ui, |pu| {
            pu.line(Line::new("Gold", PlotPoints::from(pts)).color(SERIES[0]).width(2.0).fill(0.0).fill_alpha(0.10));
        });
}

/// Seconds spent per zone: the time between events goes to the zone the
/// character was in.
pub fn zone_time(c: &Character) -> Vec<(String, f64)> {
    let mut by: std::collections::HashMap<String, f64> = Default::default();
    for s in &c.sessions {
        let mut zone = String::new();
        let mut last = s.start;
        for e in &c.events[s.from..s.to] {
            if !zone.is_empty() {
                *by.entry(zone.clone()).or_default() += (e.t - last) as f64;
            }
            last = e.t;
            if matches!(e.e.as_str(), "login" | "zone" | "logout" | "level" | "death") {
                if let Some(z) = e.s("zone").filter(|z| !z.is_empty()) {
                    zone = z.to_string();
                }
            }
        }
    }
    let mut v: Vec<_> = by.into_iter().filter(|(_, s)| *s > 0.0).collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
}

fn zones(ui: &mut Ui, c: &Character) {
    label(ui, "Where the time goes");
    ui.label(RichText::new("Minutes played per zone").small().color(MUTED));
    let z = zone_time(c);
    if z.is_empty() {
        ui.label(RichText::new("No zones recorded yet.").color(MUTED));
        return;
    }
    let names: Vec<String> = z.iter().map(|(n, _)| n.clone()).collect();
    let bars: Vec<Bar> = z.iter().enumerate().map(|(i, (n, s))| Bar::new(-(i as f64), s / 60.0).width(0.62).name(n).fill(SERIES[0])).collect();
    let height = (70.0 + 34.0 * z.len() as f32).min(320.0);
    let names2 = names.clone();
    plot("zones")
        .height(height)
        .show_x(false)
        .show_y(false)
        .y_axis_formatter(move |g, _| names2.get((-g.value).round() as usize).cloned().unwrap_or_default())
        .y_axis_min_width(120.0)
        .x_axis_formatter(|g, _| format!("{:.0} min", g.value))
        .show(ui, |pu| {
            pu.bar_chart(BarChart::new("Minutes", bars).horizontal().color(SERIES[0]).element_formatter(Box::new(|b, _| format!("{}: {:.0} min", b.name, b.value))));
        });
}

fn sessions(ui: &mut Ui, c: &Character) {
    label(ui, "Sessions");
    ui.label(RichText::new("Experience per hour, each session").small().color(MUTED));
    let bars: Vec<Bar> = c
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| s.seconds() > 60)
        .map(|(i, s)| {
            let rate = s.xp as f64 / s.seconds() as f64 * 3600.0;
            Bar::new(i as f64 + 1.0, rate).width(0.6).name(format!("{}, {}: {} XP in {}", theme::day(s.start as f64), theme::clock(s.start as f64), theme::thousands(s.xp), theme::duration(s.seconds() as f64))).fill(SERIES[0])
        })
        .collect();
    if bars.is_empty() {
        ui.label(RichText::new("No sessions longer than a minute yet.").color(MUTED));
        return;
    }
    plot("sessions")
        .height(220.0)
        .x_axis_formatter(|g, _| if g.value.fract() == 0.0 && g.value > 0.0 { format!("#{}", g.value) } else { String::new() })
        .y_axis_formatter(|g, _| theme::thousands(g.value as i64))
        .show(ui, |pu| {
            pu.bar_chart(BarChart::new("XP per hour", bars).color(SERIES[0]).element_formatter(Box::new(|b, _| format!("{}\n{} XP per hour", b.name, theme::thousands(b.value as i64)))));
        });
}

/// What's open right now: active quests nearest to done first.
fn up_next(ui: &mut Ui, c: &Character, st: &mut State, page: &mut Page) {
    label(ui, "Up next");
    let mut active: Vec<_> = c.quests.iter().filter(|q| q.status == QuestStatus::Active).collect();
    if active.is_empty() {
        ui.label(RichText::new("No open quests.").color(MUTED));
        return;
    }
    let share = |q: &crate::data::memory::Quest| {
        if q.complete {
            return 1.0;
        }
        let (h, n) = q.objectives.iter().fold((0, 0), |(h, n), o| (h + o.have.min(o.need), n + o.need));
        if n == 0 { 0.0 } else { h as f64 / n as f64 }
    };
    active.sort_by(|a, b| share(b).total_cmp(&share(a)));
    for q in active.into_iter().take(6) {
        ui.horizontal(|ui| {
            let s = share(q);
            ui.add(egui::ProgressBar::new(s as f32).desired_width(120.0).fill(if q.complete { theme::GOOD.gamma_multiply(0.6) } else { SERIES[0] }));
            let text = if q.complete { format!("{}  (ready to turn in)", q.title) } else { q.title.clone() };
            if ui.link(RichText::new(text).color(INK)).clicked() {
                st.quest = Some(q.id);
                st.quest_tab = 0;
                *page = Page::Quests;
            }
        });
    }
}
