//! Standing: where a character stands with the factions of the world, and how it changed.

use super::widgets;
use super::{card, character, label};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::standing::{self, Group, Row};
use crate::theme::{self, DANGER, EDGE, GOLD, INK, MUTED, RAISED};
use crate::tr;
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};

/// The bar colours, Hated red through Neutral yellow to Exalted teal.
pub fn color(standing: i64) -> Color32 {
    match standing {
        1 => Color32::from_rgb(0xb8, 0x2e, 0x26),
        2 => Color32::from_rgb(0xcc, 0x4d, 0x38),
        3 => Color32::from_rgb(0xc8, 0x6a, 0x12),
        4 => Color32::from_rgb(0xe0, 0xb0, 0x10),
        5 => Color32::from_rgb(0x3f, 0xa5, 0x35),
        6 => Color32::from_rgb(0x1f, 0x9a, 0x55),
        7 => Color32::from_rgb(0x17, 0x9c, 0x88),
        _ => Color32::from_rgb(0x2b, 0xc4, 0xb8),
    }
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    let groups = super::story::memo(ui, m, c, "standing", || standing::build(c));
    if groups.is_empty() {
        let name = c.name.split(' ').next().unwrap_or("");
        super::empty(
            ui,
            art,
            236683,
            tr!("Unknown to the world"),
            &tr!("No faction has taken notice of {name} yet.", name = name),
        );
        super::hint(
            ui,
            tr!("Standing is kept by the addon from version 0.4.0 on."),
        );
        return;
    }
    if let Some(line) = standing::summary(&groups) {
        ui.label(
            RichText::new(line)
                .family(theme::italic())
                .size(19.0)
                .color(INK),
        );
    }
    ui.add_space(10.0);
    // Every sparkline spans the same time, the whole record, so they compare.
    let span = (
        c.sessions.first().map(|s| s.start).unwrap_or(0),
        c.sessions.last().map(|s| s.end).unwrap_or(0),
    );
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            figures(ui, &groups, art);
            ui.add_space(12.0);
            for g in groups.iter() {
                let name = if !g.name.is_empty() {
                    g.name.as_str()
                } else if groups.len() == 1 {
                    tr!("From the chat log")
                } else {
                    tr!("Heard of, before the addon kept standings")
                };
                label(ui, name);
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut sub = "";
                    for (i, r) in g.rows.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(2.0);
                        }
                        // Sub-headers, as the frame indents them.
                        if r.faction.sub != sub {
                            sub = &r.faction.sub;
                            if !sub.is_empty() {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(sub).small().color(GOLD.gamma_multiply(0.75)),
                                );
                            }
                        }
                        row(ui, r, span, st, art);
                    }
                });
                ui.add_space(16.0);
            }
            ui.add_space(24.0);
        });
}

fn figures(ui: &mut Ui, groups: &[Group], art: &mut Art) {
    let rows: Vec<&Row> = groups.iter().flat_map(|g| &g.rows).collect();
    let known = rows.iter().filter(|r| r.known).count();
    let gained: i64 = rows.iter().map(|r| r.gained.max(0)).sum();
    let first = rows
        .iter()
        .filter_map(|r| r.history.first().map(|h| h.0))
        .min();
    let friends = rows
        .iter()
        .filter(|r| r.known && r.faction.standing >= 5)
        .count();
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::figure_row(ui, |ui| {
            if known > 0 {
                widgets::figure_text(ui, art, 236683, &known.to_string(), tr!("factions known"));
                widgets::figure_text(
                    ui,
                    art,
                    134472,
                    &friends.to_string(),
                    tr!("friendly or better"),
                );
            }
            if gained > 0 {
                let caption = match first {
                    Some(t) => tr!(
                        "reputation earned since {when}",
                        when = theme::day(t as f64)
                    ),
                    None => tr!("reputation earned").to_string(),
                };
                widgets::figure_text(ui, art, 134939, &theme::thousands(gained), &caption);
            }
        });
    });
}

fn row(ui: &mut Ui, r: &Row, span: (i64, i64), st: &mut State, art: &mut Art) {
    let f = &r.faction;
    let open = st.faction.as_deref() == Some(f.name.as_str());
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 54.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        st.faction = if open { None } else { Some(f.name.clone()) };
    }
    let p = ui.painter();
    if open || resp.hovered() {
        let bg = if open {
            RAISED
        } else {
            Color32::from_rgb(0x14, 0x1a, 0x36)
        };
        p.rect_filled(rect.expand2(vec2(6.0, 0.0)), 7.0, bg);
    }
    let col = if r.known { color(f.standing) } else { MUTED };

    // The faction's mark, ringed in gold for the character's own people.
    let ic = Rect::from_min_size(pos2(rect.left(), rect.center().y - 19.0), Vec2::splat(38.0));
    p.rect_filled(ic, 6.0, Color32::from_rgb(5, 7, 15));
    if let Some(t) = art.icon(ui.ctx(), Some(standing::icon(f.id))) {
        p.image(
            t.id(),
            ic.shrink(2.0),
            Rect::from_min_max(pos2(0.07, 0.07), pos2(0.93, 0.93)),
            Color32::WHITE,
        );
    }
    let ring = if r.home {
        GOLD
    } else {
        Color32::from_rgb(0x6b, 0x5a, 0x2e)
    };
    p.rect_stroke(ic, 6.0, Stroke::new(1.5, ring), egui::StrokeKind::Inside);

    // The name, and what sets the faction apart.
    let x = ic.right() + 12.0;
    let name_w = (w * 0.26).clamp(190.0, 300.0);
    let mut notes = vec![];
    if r.home {
        notes.push((tr!("home").to_string(), GOLD.gamma_multiply(0.8)));
    }
    if f.war {
        notes.push((tr!("at war").to_string(), DANGER));
    }
    if let Some(t) = r.since() {
        notes.push((
            tr!(
                "{standing} since {when}",
                standing = standing::label(f.standing),
                when = theme::day(t as f64)
            ),
            MUTED,
        ));
    }
    let name_y = if notes.is_empty() { 0.0 } else { -9.0 };
    p.text(
        pos2(x, rect.center().y + name_y),
        Align2::LEFT_CENTER,
        &f.name,
        FontId::proportional(17.0),
        if r.home { GOLD } else { INK },
    );
    let mut nx = x;
    for (text, c) in notes {
        let g = p.layout_no_wrap(text, FontId::proportional(13.5), c);
        let width = g.size().x;
        p.galley(pos2(nx, rect.center().y + 3.0), g, c);
        nx += width + 10.0;
    }

    // The change over the recorded time, at the right.
    let change_w = 118.0;
    let right = rect.right();
    if r.gained != 0 {
        let sign = if r.gained > 0 { "+" } else { "" };
        p.text(
            pos2(right, rect.center().y - 9.0),
            Align2::RIGHT_CENTER,
            format!("{sign}{}", theme::thousands(r.gained)),
            FontId::proportional(17.0),
            if r.gained > 0 {
                theme::GOOD.gamma_multiply(0.8)
            } else {
                DANGER
            },
        );
        if let Some((t, _)) = r.history.first() {
            p.text(
                pos2(right, rect.center().y + 10.0),
                Align2::RIGHT_CENTER,
                tr!("since {when}", when = theme::day(*t as f64)),
                FontId::proportional(13.5),
                MUTED,
            );
        }
    }
    // The bar, as the reputation frame draws it, then the sparkline taking
    // whatever room is left.
    let room = right - change_w - (x + name_w);
    let bar_w = (room * 0.62).clamp(60.0, 560.0);
    let bar = Rect::from_min_size(pos2(x + name_w, rect.center().y - 11.0), vec2(bar_w, 22.0));
    if room > 200.0 {
        standing_bar(ui, bar, r);
    }
    let spark = Rect::from_min_max(
        pos2(bar.right() + 24.0, rect.center().y - 16.0),
        pos2(right - change_w, rect.center().y + 16.0),
    );
    if spark.width() > 40.0 {
        sparkline(ui, spark, r, span, col);
    }

    if open {
        detail(ui, r, col);
    }
}

fn standing_bar(ui: &Ui, bar: Rect, r: &Row) {
    let f = &r.faction;
    let p = ui.painter();
    p.rect_filled(bar, 4.0, Color32::from_rgb(0x07, 0x0a, 0x18));
    if r.known {
        let span = (f.max - f.min).max(1) as f32;
        let share = ((f.value - f.min) as f32 / span).clamp(0.0, 1.0);
        let mut fill = bar.shrink(2.0);
        fill.set_width((fill.width() * share).max(if share > 0.0 { 4.0 } else { 0.0 }));
        let top = color(f.standing);
        let bottom = top.gamma_multiply(0.55).to_opaque();
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(fill.left_top(), top);
        mesh.colored_vertex(fill.right_top(), top);
        mesh.colored_vertex(fill.right_bottom(), bottom);
        mesh.colored_vertex(fill.left_bottom(), bottom);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(mesh);
        let name = standing::label(f.standing);
        let text = if f.standing < 8 && f.max > f.min {
            format!(
                "{name}   {} / {}",
                theme::thousands(f.value - f.min),
                theme::thousands(f.max - f.min)
            )
        } else {
            name.to_string()
        };
        // White with a dark outline, readable on every colour.
        let font = FontId::proportional(14.0);
        for d in [
            vec2(1.0, 1.0),
            vec2(-1.0, 1.0),
            vec2(1.0, -1.0),
            vec2(-1.0, -1.0),
            vec2(0.0, 1.5),
        ] {
            p.text(
                bar.center() + d,
                Align2::CENTER_CENTER,
                &text,
                font.clone(),
                Color32::from_black_alpha(230),
            );
        }
        p.text(
            bar.center(),
            Align2::CENTER_CENTER,
            &text,
            font,
            Color32::WHITE,
        );
    } else {
        p.text(
            bar.center(),
            Align2::CENTER_CENTER,
            tr!("standing not recorded"),
            FontId::proportional(13.5),
            MUTED,
        );
    }
    p.rect_stroke(
        bar,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(0x6b, 0x5a, 0x2e)),
        egui::StrokeKind::Outside,
    );
}

/// The value over the recorded time as steps (reputation moves in jumps),
/// with the borders between standings faintly behind.
fn sparkline(ui: &Ui, rect: Rect, r: &Row, span: (i64, i64), col: Color32) {
    let p = ui.painter();
    if r.history.len() < 2 {
        p.line_segment(
            [
                pos2(rect.left(), rect.center().y),
                pos2(rect.right(), rect.center().y),
            ],
            Stroke::new(1.0, EDGE),
        );
        return;
    }
    let t0 = span.0.min(r.history[0].0) as f32;
    let t1 = span.1.max(r.history[r.history.len() - 1].0) as f32;
    let lo = r.history.iter().map(|h| h.1).min().unwrap_or(0) as f32;
    let hi = r.history.iter().map(|h| h.1).max().unwrap_or(1) as f32;
    let span = (hi - lo).max(1.0);
    let x = |t: i64| rect.left() + (t as f32 - t0) / (t1 - t0).max(1.0) * rect.width();
    let y = |v: f32| rect.bottom() - (v - lo) / span * rect.height();
    if r.known {
        for &b in &standing::THRESHOLDS {
            let b = b as f32;
            if b > lo && b < hi {
                p.line_segment(
                    [pos2(rect.left(), y(b)), pos2(rect.right(), y(b))],
                    Stroke::new(1.0, EDGE),
                );
            }
        }
    }
    let first = r.history[0].1 as f32;
    let mut pts: Vec<egui::Pos2> = vec![pos2(rect.left(), y(first))];
    for &(t, v) in &r.history {
        let at = pos2(x(t), y(v as f32));
        let prev = pts[pts.len() - 1];
        pts.push(pos2(at.x, prev.y));
        pts.push(at);
    }
    pts.push(pos2(rect.right(), pts[pts.len() - 1].y));
    let last = pts[pts.len() - 1];
    p.add(egui::Shape::line(pts, Stroke::new(1.8, col)));
    p.circle_filled(last, 2.8, col);
}

fn detail(ui: &mut Ui, r: &Row, col: Color32) {
    let f = &r.faction;
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(Color32::from_rgb(0x0c, 0x11, 0x26))
        .stroke(Stroke::new(1.0, EDGE))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // The road so far: each standing and when it was reached.
            if !r.reached.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (i, (&s, &t)) in r.reached.iter().enumerate() {
                        if i > 0 {
                            ui.label(RichText::new("›").color(MUTED));
                        }
                        egui::Frame::new()
                            .fill(color(s).gamma_multiply(0.22))
                            .stroke(Stroke::new(1.0, color(s)))
                            .corner_radius(5)
                            .inner_margin(egui::Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                let text =
                                    format!("{}  {}", standing::label(s), theme::day(t as f64));
                                ui.label(RichText::new(text).color(INK));
                            });
                    }
                });
                ui.add_space(8.0);
            }
            if r.known && f.standing < 8 && f.max > f.value {
                ui.label(
                    RichText::new(tr!(
                        "{n} more to {standing}",
                        n = theme::thousands(f.max - f.value),
                        standing = standing::label(f.standing + 1)
                    ))
                    .color(col),
                );
            }
            if r.history.len() < 2 {
                ui.label(RichText::new(tr!("No change recorded yet.")).color(MUTED));
                return;
            }
            // Changes by day: a day of questing is many small steps.
            let mut days: Vec<(String, i64, usize)> = vec![];
            for w in r.history.windows(2) {
                let (day, d) = (theme::day(w[1].0 as f64), w[1].1 - w[0].1);
                match days.last_mut() {
                    Some(l) if l.0 == day => {
                        l.1 += d;
                        l.2 += 1;
                    }
                    _ => days.push((day, d, 1)),
                }
            }
            ui.add_space(4.0);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            egui::Grid::new(("rep-days", &f.name))
                .spacing(vec2(18.0, 4.0))
                .show(ui, |ui| {
                    for (day, d, n) in days.iter().rev().take(10) {
                        ui.label(RichText::new(day).color(MUTED));
                        let sign = if *d > 0 { "+" } else { "" };
                        ui.label(
                            RichText::new(format!("{sign}{}", theme::thousands(*d))).color(
                                if *d >= 0 {
                                    theme::GOOD.gamma_multiply(0.8)
                                } else {
                                    DANGER
                                },
                            ),
                        );
                        let changes = if *n == 1 {
                            tr!("1 change").to_string()
                        } else {
                            tr!("{n} changes", n = n)
                        };
                        ui.label(RichText::new(changes).small().color(MUTED));
                        ui.end_row();
                    }
                });
        });
    ui.add_space(6.0);
}
