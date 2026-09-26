//! The armory, locally: gear on the class scene, the character sheet's stats,
//! both talent specs on their Classic backgrounds, and the Legacy trees.

use super::{card, character, icon, label, tooltip_lines};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::memory::{Character, entries};
use crate::theme::{self, EDGE, GOLD, INK, MUTED, RAISED};
use crate::tr;
use egui::{Color32, Pos2, Rect, RichText, Stroke, Ui, Vec2};
use serde_json::Value;

const LEFT: [i64; 8] = [1, 2, 3, 15, 5, 4, 19, 9];
const RIGHT: [i64; 8] = [10, 6, 7, 8, 11, 12, 13, 14];
const WEAPONS: [i64; 3] = [16, 17, 18];

fn slot_name(slot: i64) -> &'static str {
    match slot {
        1 => tr!("Head"),
        2 => tr!("Neck"),
        3 => tr!("Shoulder"),
        4 => tr!("Shirt"),
        5 => tr!("Chest"),
        6 => tr!("Waist"),
        7 => tr!("Legs"),
        8 => tr!("Feet"),
        9 => tr!("Wrist"),
        10 => tr!("Hands"),
        11 | 12 => tr!("Finger"),
        13 | 14 => tr!("Trinket"),
        15 => tr!("Back"),
        16 => tr!("Main Hand"),
        17 => tr!("Off Hand"),
        18 => tr!("Ranged"),
        19 => tr!("Tabard"),
        _ => "",
    }
}
fn slot_art(slot: i64) -> &'static str {
    match slot {
        1 => "head",
        2 => "neck",
        3 => "shoulder",
        4 => "shirt",
        5 => "chest",
        6 => "waist",
        7 => "legs",
        8 => "feet",
        9 => "wrists",
        10 => "hands",
        11 => "finger",
        12 => "rfinger",
        13 | 14 => "trinket",
        15 => "rear",
        16 => "mainhand",
        17 => "secondaryhand",
        18 => "ranged",
        19 => "tabard",
        _ => "generic",
    }
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let stats_w = 320.0;
                let doll_w = ui.available_width() - stats_w - 14.0;
                let doll = ui.vertical(|ui| {
                    ui.set_width(doll_w);
                    paper_doll(ui, m, c, art)
                });
                let h = doll.response.rect.height();
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.set_width(stats_w);
                    card(ui, |ui| {
                        ui.set_height(h - 34.0);
                        egui::ScrollArea::vertical()
                            .id_salt("stats")
                            .show(ui, |ui| stats(ui, c, art));
                    });
                });
            });
            ui.add_space(18.0);
            talents(ui, c, st, art);
            ui.add_space(18.0);
            legacy(ui, c, art);
            ui.add_space(24.0);
        });
}

fn paper_doll(ui: &mut Ui, m: &Model, c: &Character, art: &mut Art) {
    // The class's second scene sits behind the doll, where the game shows the model.
    let bg_shape = ui.painter().add(egui::Shape::Noop);
    let frame = egui::Frame::new()
        .stroke(Stroke::new(1.0, EDGE))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(20));
    let r = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        let eq = c.snapshot.get("equipment").cloned().unwrap_or(Value::Null);
        let items: std::collections::HashMap<i64, &Value> = entries(&eq).into_iter().collect();
        ui.columns(2, |cols| {
            for (col, slots, right) in [(0, &LEFT, false), (1, &RIGHT, true)] {
                let ui = &mut cols[col];
                ui.with_layout(
                    egui::Layout::top_down(if right {
                        egui::Align::Max
                    } else {
                        egui::Align::Min
                    }),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 10.0;
                        for &s in slots.iter() {
                            slot(ui, m, c, art, s, items.get(&s).copied(), right);
                        }
                    },
                );
            }
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            let w = 3.0 * 170.0;
            ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
            for s in WEAPONS {
                ui.vertical(|ui| {
                    ui.set_width(170.0);
                    slot(ui, m, c, art, s, items.get(&s).copied(), false);
                });
            }
        });
    });
    let rect = r.response.rect;
    let mut shapes = vec![egui::Shape::rect_filled(rect, 8.0, theme::PANEL)];
    if let Some(tex) = art.get(
        ui.ctx(),
        &format!("scene-{}.jpg", c.class_file.to_lowercase()),
    ) {
        let [tw, th] = tex.size().map(|v| v as f32);
        let scale = (rect.width() / tw).max(rect.height() / th);
        let (uw, uh) = (rect.width() / (tw * scale), rect.height() / (th * scale));
        let uv = Rect::from_min_size(
            egui::pos2((1.0 - uw) * 0.42, (1.0 - uh) / 2.0),
            egui::vec2(uw, uh),
        );
        let mut mesh = egui::Mesh::with_texture(tex.id());
        mesh.add_rect_with_uv(rect.shrink(1.0), uv, Color32::from_gray(120));
        shapes.push(egui::Shape::mesh(mesh));
    }
    ui.painter().set(bg_shape, egui::Shape::Vec(shapes));
}

#[allow(clippy::too_many_arguments)]
fn slot(
    ui: &mut Ui,
    m: &Model,
    c: &Character,
    art: &mut Art,
    s: i64,
    item: Option<&Value>,
    right: bool,
) {
    let q = item.and_then(|i| i.get("quality")).and_then(Value::as_i64);
    let name = item
        .and_then(|i| i.pointer("/tooltip/0/l"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let sub = item
        .and_then(|i| i.get("tooltip"))
        .and_then(|t| {
            entries(t).into_iter().skip(1).find_map(|(_, l)| {
                let (a, b) = (
                    l.get("l").and_then(Value::as_str)?,
                    l.get("r").and_then(Value::as_str)?,
                );
                (!a.is_empty() && !b.is_empty()).then(|| format!("{a} {b}"))
            })
        })
        .unwrap_or_else(|| slot_name(s).to_string());
    let layout = if right {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    let r = ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 50.0), layout, |ui| {
        let border = if item.is_some() {
            theme::quality(q)
        } else {
            Color32::from_rgb(0x1c, 0x22, 0x40)
        };
        if let Some(it) = item {
            icon(
                ui,
                art,
                it.get("icon").and_then(Value::as_i64),
                border,
                48.0,
            );
        } else {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 5.0, Color32::from_rgb(5, 7, 15));
            if let Some(t) = art.get(ui.ctx(), &format!("slot-{}.png", slot_art(s))) {
                ui.painter().image(
                    t.id(),
                    rect.shrink(2.0),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::from_gray(110),
                );
            }
            ui.painter().rect_stroke(
                rect,
                5.0,
                Stroke::new(1.5, border),
                egui::StrokeKind::Inside,
            );
        }
        let text_h = if name.is_empty() { 18.0 } else { 38.0 };
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), text_h),
            egui::Layout::top_down(if right {
                egui::Align::Max
            } else {
                egui::Align::Min
            }),
            |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                if !name.is_empty() {
                    ui.label(RichText::new(name).color(theme::quality(q)));
                }
                ui.label(RichText::new(sub).small().color(MUTED));
            },
        );
    });
    if let Some(it) = item {
        r.response.on_hover_ui(|ui| {
            ui.set_max_width(320.0);
            if let Some(t) = it.get("tooltip") {
                tooltip_lines(ui, t);
            }
            let id = it.get("id").and_then(Value::as_i64);
            if id.is_some_and(|id| super::story::section(ui, m, c, id)) {
                return;
            }
            let first = it.get("firstSeen").and_then(Value::as_i64).or_else(|| {
                c.seen.get(&id?.to_string()).copied()
            });
            if let Some(t) = first {
                ui.separator();
                ui.label(
                    RichText::new(tr!("First acquired {when}", when = theme::when(t as f64)))
                        .small()
                        .color(MUTED),
                );
            }
        });
    }
}

/// Drops the client's colour and texture escapes from hover text.
fn strip_codes(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '|' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('c') => {
                if chars.peek() == Some(&'n') {
                    for x in chars.by_ref() {
                        if x == ':' {
                            break;
                        }
                    }
                } else {
                    for _ in 0..8 {
                        chars.next();
                    }
                }
            }
            Some('r') => {}
            Some('n') => out.push('\n'),
            Some('T') | Some('A') => {
                while let Some(x) = chars.next() {
                    if x == '|' {
                        chars.next();
                        break;
                    }
                }
            }
            Some(x) => {
                out.push('|');
                out.push(x);
            }
            None => out.push('|'),
        }
    }
    out
}

fn stats(ui: &mut Ui, c: &Character, art: &mut Art) {
    let mut seen = std::collections::HashSet::new();
    for (_, cat) in c.snapshot.get("stats").map(entries).unwrap_or_default() {
        let name = cat.get("name").and_then(Value::as_str).unwrap_or("");
        let rows: Vec<_> = cat
            .get("stats")
            .map(entries)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, s)| {
                s.get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|v| !v.trim().is_empty())
            })
            .collect();
        if rows.is_empty() || !seen.insert(name.to_string()) {
            continue;
        }
        label(ui, name);
        for (_, s) in rows {
            let r = ui.horizontal(|ui| {
                ui.label(
                    RichText::new(s.get("label").and_then(Value::as_str).unwrap_or(""))
                        .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(s.get("value").and_then(Value::as_str).unwrap_or(""))
                            .color(INK),
                    );
                });
            });
            // The character sheet's own hover text, recorded by the addon.
            let tips: Vec<String> = s
                .get("tooltip")
                .map(entries)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(_, t)| t.as_str().map(strip_codes))
                .filter(|t| !t.trim().is_empty())
                .collect();
            if !tips.is_empty() {
                r.response.interact(egui::Sense::hover()).on_hover_ui(|ui| {
                    ui.set_max_width(320.0);
                    for (i, t) in tips.iter().enumerate() {
                        ui.label(
                            RichText::new(t)
                                .color(if i == 0 { INK } else { GOLD })
                                .size(if i == 0 { 16.5 } else { 15.0 }),
                        );
                    }
                });
            }
        }
        ui.add_space(10.0);
    }
    label(ui, tr!("Purse"));
    super::widgets::coins(ui, art, c.money, 17.0);
}

// ---- talent trees ----

struct Node<'a> {
    col: i64,
    row: i64,
    v: &'a Value,
}

type Grid<'a> = (
    Vec<Node<'a>>,
    i64,
    i64,
    std::collections::HashMap<i64, (i64, i64)>,
);

/// Grid positions from client units: the step is the most common gap between
/// neighbouring columns (600 for class talents, 750 for Legacy trees).
fn grid<'a>(nodes: &[&'a Value]) -> Grid<'a> {
    let step = |vals: &mut Vec<i64>| {
        vals.sort();
        vals.dedup();
        let mut counts: std::collections::HashMap<i64, usize> = Default::default();
        for w in vals.windows(2) {
            if w[1] - w[0] > 100 {
                *counts.entry(w[1] - w[0]).or_default() += 1;
            }
        }
        let step = counts
            .into_iter()
            .max_by_key(|(d, n)| (*n, -d))
            .map(|(d, _)| d)
            .unwrap_or(600);
        (step, vals.first().copied().unwrap_or(0))
    };
    let mut xs: Vec<i64> = nodes
        .iter()
        .filter_map(|n| n.get("x").and_then(Value::as_i64))
        .collect();
    let mut ys: Vec<i64> = nodes
        .iter()
        .filter_map(|n| n.get("y").and_then(Value::as_i64))
        .collect();
    let (sx, minx) = step(&mut xs);
    let (sy, miny) = step(&mut ys);
    let mut out = vec![];
    let mut at = std::collections::HashMap::new();
    let (mut cols, mut rows) = (4, 1);
    for n in nodes {
        let x = n.get("x").and_then(Value::as_i64).unwrap_or(0);
        let y = n.get("y").and_then(Value::as_i64).unwrap_or(0);
        let col = ((x - minx) as f64 / sx as f64).round() as i64;
        let row = ((y - miny) as f64 / sy as f64).round() as i64;
        cols = cols.max(col + 1);
        rows = rows.max(row + 1);
        if let Some(id) = n.get("id").and_then(Value::as_i64) {
            at.insert(id, (col, row));
        }
        out.push(Node { col, row, v: n });
    }
    (out, cols, rows, at)
}

struct TreeHead<'a> {
    title: &'a str,
    spent: Option<i64>,
    icon: Option<i64>,
    bg: Option<String>,
}

fn tree(ui: &mut Ui, art: &mut Art, head: TreeHead, nodes: &[&Value], width: f32) {
    // Placeholder nodes ("Unknown", no name) aren't drawn, so they mustn't
    // take part in the layout or get lines drawn to them either.
    let named = |n: &&Value| {
        let name = n
            .pointer("/entries/0/name")
            .or_else(|| n.pointer("/entries/1/name"))
            .and_then(Value::as_str)
            .unwrap_or("");
        !name.is_empty() && name != "Unknown"
    };
    let nodes: Vec<&Value> = nodes.iter().copied().filter(named).collect();
    let (nodes, cols, rows, at) = grid(&nodes);
    let cell = (width - 24.0) / cols as f32;
    let row_h = 62.0f32.min(cell);
    let height = 50.0 + rows as f32 * row_h + 20.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 8.0, theme::PANEL);
    if let Some(key) = &head.bg
        && let Some(tex) = art.get(ui.ctx(), key) {
            p.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::from_gray(95),
            );
        }
    p.rect_stroke(rect, 8.0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
    let bar = Rect::from_min_size(rect.min, egui::vec2(width, 44.0));
    p.rect_filled(
        bar,
        egui::CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        },
        Color32::from_rgba_unmultiplied(5, 7, 15, 170),
    );
    let mut tx = bar.left() + 14.0;
    if let Some(tex) = art.icon(ui.ctx(), head.icon) {
        p.image(
            tex.id(),
            Rect::from_center_size(egui::pos2(tx + 13.0, bar.center().y), Vec2::splat(26.0)),
            Rect::from_min_max(egui::pos2(0.07, 0.07), egui::pos2(0.93, 0.93)),
            Color32::WHITE,
        );
        tx += 36.0;
    }
    p.text(
        egui::pos2(tx, bar.center().y),
        egui::Align2::LEFT_CENTER,
        head.title,
        theme::display_font(19.0),
        INK,
    );
    if let Some(s) = head.spent {
        p.text(
            egui::pos2(bar.right() - 14.0, bar.center().y),
            egui::Align2::RIGHT_CENTER,
            s.to_string(),
            egui::FontId::proportional(17.0),
            GOLD,
        );
    }
    let origin = egui::pos2(rect.left() + 12.0, bar.bottom() + 10.0);
    let center = |col: i64, row: i64| {
        origin + egui::vec2((col as f32 + 0.5) * cell, (row as f32 + 0.5) * row_h)
    };
    for n in &nodes {
        for (_, e) in n.v.get("edges").map(entries).unwrap_or_default() {
            if let Some(&(c2, r2)) = e.get("to").and_then(Value::as_i64).and_then(|t| at.get(&t)) {
                p.line_segment(
                    [center(n.col, n.row), center(c2, r2)],
                    Stroke::new(3.0, Color32::from_rgba_unmultiplied(255, 209, 0, 80)),
                );
            }
        }
    }
    for n in &nodes {
        let ranks = n.v.get("ranks").and_then(Value::as_i64).unwrap_or(0);
        let max = n.v.get("maxRanks").and_then(Value::as_i64).unwrap_or(0);
        let Some(entry) =
            n.v.get("entries")
                .map(entries)
                .unwrap_or_default()
                .into_iter()
                .next()
                .map(|(_, e)| e)
        else {
            continue;
        };
        let name = entry.get("name").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() || name == "Unknown" {
            continue;
        }
        let c = center(n.col, n.row);
        let r = Rect::from_center_size(c, Vec2::splat(38.0));
        let border = if ranks >= max && max > 0 {
            GOLD
        } else if ranks > 0 {
            theme::GOOD
        } else {
            Color32::from_rgb(0x4a, 0x4f, 0x63)
        };
        p.rect_filled(r, 5.0, Color32::from_rgb(5, 7, 15));
        if let Some(tex) = art.icon(ui.ctx(), entry.get("icon").and_then(Value::as_i64)) {
            let tint = if ranks == 0 {
                Color32::from_gray(90)
            } else {
                Color32::WHITE
            };
            p.image(
                tex.id(),
                r.shrink(2.0),
                Rect::from_min_max(egui::pos2(0.07, 0.07), egui::pos2(0.93, 0.93)),
                tint,
            );
        }
        p.rect_stroke(r, 5.0, Stroke::new(1.5, border), egui::StrokeKind::Inside);
        let badge = Rect::from_min_size(
            r.right_bottom() - egui::vec2(18.0, 10.0),
            egui::vec2(26.0, 16.0),
        );
        p.rect_filled(badge, 4.0, Color32::from_rgb(5, 7, 15));
        p.text(
            badge.center(),
            egui::Align2::CENTER_CENTER,
            format!("{ranks}/{max}"),
            egui::FontId::proportional(12.0),
            if ranks > 0 { border } else { MUTED },
        );
        let resp = ui.interact(
            r,
            ui.id().with(("node", head.title, n.col, n.row)),
            egui::Sense::hover(),
        );
        resp.on_hover_ui(|ui| {
            ui.set_max_width(300.0);
            ui.label(RichText::new(name).size(17.0));
            ui.label(
                RichText::new(tr!("Rank {ranks}/{max}", ranks = ranks, max = max)).color(MUTED),
            );
            let rank_text = |k: i64| {
                let v = entry
                    .get("ranks")
                    .map(entries)
                    .unwrap_or_default()
                    .into_iter()
                    .find(|(i, _)| *i == k)
                    .map(|(_, v)| v.clone())?;
                let lines: Vec<String> = entries(&v)
                    .into_iter()
                    .filter_map(|(_, l)| l.get("l").and_then(Value::as_str).map(str::to_string))
                    .filter(|t| !t.trim().is_empty() && t != "Passive")
                    .collect();
                Some(lines.join("\n"))
            };
            if let Some(t) = rank_text(ranks.max(1)) {
                ui.label(RichText::new(t).color(GOLD));
            }
            if ranks > 0 && ranks < max
                && let Some(t) = rank_text(ranks + 1) {
                    ui.add_space(6.0);
                    ui.label(tr!("Next rank:"));
                    ui.label(RichText::new(t).color(GOLD));
                }
        });
    }
}

fn talents(ui: &mut Ui, c: &Character, st: &mut State, art: &mut Art) {
    let specs: Vec<(i64, &Value)> = c
        .snapshot
        .pointer("/talents/specs")
        .map(entries)
        .unwrap_or_default();
    if specs.is_empty() {
        return;
    }
    let active = c
        .snapshot
        .pointer("/talents/active")
        .and_then(Value::as_i64)
        .unwrap_or(1);
    let (_, spec) = specs[st.spec.min(specs.len() - 1)];
    let mut groups: Vec<&Value> = spec
        .get("groups")
        .map(entries)
        .unwrap_or_default()
        .into_iter()
        .map(|(_, g)| g)
        .collect();
    groups.sort_by_key(|g| g.get("order").and_then(Value::as_i64).unwrap_or(0));
    let spent: Vec<String> = groups
        .iter()
        .map(|g| {
            g.get("spent")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .to_string()
        })
        .collect();
    ui.horizontal(|ui| {
        super::heading(ui, tr!("Talents"));
        ui.label(RichText::new(spent.join("/")).color(MUTED));
        if specs.len() > 1 {
            ui.add_space(16.0);
            for (i, (n, _)) in specs.iter().enumerate() {
                let name = if *n == 1 {
                    tr!("Primary")
                } else {
                    tr!("Secondary")
                };
                let label = if *n == active {
                    tr!("{spec} (active)", spec = name)
                } else {
                    name.to_string()
                };
                if ui
                    .add(egui::Button::new(label).fill(if st.spec == i {
                        RAISED
                    } else {
                        Color32::TRANSPARENT
                    }))
                    .clicked()
                {
                    st.spec = i;
                }
            }
        }
    });
    ui.add_space(6.0);
    let all: Vec<&Value> = spec
        .get("nodes")
        .map(entries)
        .unwrap_or_default()
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    let w = ((ui.available_width() - 28.0) / 3.0).max(260.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(14.0, 14.0);
        for (i, g) in groups.iter().enumerate() {
            let gid = g.get("id").and_then(Value::as_i64).unwrap_or(-1);
            let nodes: Vec<&Value> = all
                .iter()
                .copied()
                .filter(|n| {
                    n.get("groups")
                        .map(entries)
                        .unwrap_or_default()
                        .iter()
                        .any(|(_, x)| x.as_i64() == Some(gid))
                })
                .collect();
            let head = TreeHead {
                title: g.get("name").and_then(Value::as_str).unwrap_or(""),
                spent: g.get("spent").and_then(Value::as_i64),
                icon: g.get("icon").and_then(Value::as_i64),
                bg: Some(format!("tree-{}-{}.png", c.class_file.to_lowercase(), i)),
            };
            tree(ui, art, head, &nodes, w);
        }
    });
}

fn legacy(ui: &mut Ui, c: &Character, art: &mut Art) {
    if c.snapshot.pointer("/legacy/trees").map(entries).unwrap_or_default().is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        super::heading(ui, tr!("Legacy"));
        if let Some((spent, total)) = crate::data::house::legacy_points(c) {
            ui.label(
                RichText::new(if total > 0 {
                    tr!("{spent} of {total} points spent", spent = spent, total = total)
                } else {
                    tr!("No points earned yet").into()
                })
                .color(MUTED),
            );
        }
    });
    ui.add_space(6.0);
    legacy_trees(ui, c, art);
}

/// The account's Legacy trees as a character's snapshot has them.
pub fn legacy_trees(ui: &mut Ui, c: &Character, art: &mut Art) {
    let trees: Vec<(i64, &Value)> = c
        .snapshot
        .pointer("/legacy/trees")
        .map(entries)
        .unwrap_or_default();
    let names: Vec<String> = c
        .snapshot
        .pointer("/legacy/names")
        .map(entries)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, n)| n.as_str().map(str::to_string))
        .collect();
    let w = ((ui.available_width() - 28.0) / 3.0).max(240.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(14.0, 14.0);
        for (i, (_, t)) in trees.iter().enumerate() {
            let nodes: Vec<&Value> = t
                .get("nodes")
                .map(entries)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, n)| n)
                .collect();
            let spent: i64 = nodes
                .iter()
                .map(|n| n.get("ranks").and_then(Value::as_i64).unwrap_or(0))
                .sum();
            let name = names.get(i).cloned().unwrap_or_else(|| {
                [tr!("Professions"), tr!("Adventure"), tr!("Progression")][i.min(2)].into()
            });
            tree(
                ui,
                art,
                TreeHead {
                    title: &name,
                    spent: Some(spent),
                    icon: None,
                    bg: None,
                },
                &nodes,
                w,
            );
        }
    });
}
