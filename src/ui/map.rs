//! Where you went: the recorded path on the zone's Classic world map, with
//! quests, deaths and level-ups where they happened, and a scrubber to replay it.

use super::character;
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::memory::Character;
use crate::theme::{self, DANGER, EDGE, GOLD, INK, MUTED, PANEL, RAISED};
use egui::{Color32, Pos2, Rect, RichText, Stroke, Ui, Vec2};
use std::collections::BTreeMap;

struct Sample {
    t: i64,
    map: i64,
    x: f64,
    y: f64,
    facing: Option<f64>,
}

fn samples(c: &Character) -> Vec<Sample> {
    c.events
        .iter()
        .filter(|e| e.e == "pos")
        .filter_map(|e| {
            Some(Sample {
                t: e.t,
                map: e.i("map")?,
                x: e.f("x")?,
                y: e.f("y")?,
                facing: e.f("f"),
            })
        })
        .collect()
}

fn map_names(c: &Character) -> BTreeMap<i64, String> {
    let mut out = BTreeMap::new();
    for e in &c.events {
        if e.e == "zone" {
            if let (Some(m), Some(z)) = (e.i("map"), e.s("zone")) {
                out.insert(m, z.to_string());
            }
        }
    }
    out.entry(1415).or_insert_with(|| "Eastern Kingdoms".into());
    out.entry(1414).or_insert_with(|| "Kalimdor".into());
    out
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    let all = samples(c);
    if all.is_empty() {
        super::empty(
            ui,
            "No positions recorded yet. The addon samples them while you move.",
        );
        return;
    }
    let names = map_names(c);
    let mut minutes: BTreeMap<i64, f64> = BTreeMap::new();
    for w in all.windows(2) {
        if w[0].map == w[1].map && w[1].t - w[0].t < 60 {
            *minutes.entry(w[0].map).or_default() += (w[1].t - w[0].t) as f64 / 60.0;
        }
    }
    let mut maps: Vec<(i64, f64)> = minutes.into_iter().collect();
    maps.sort_by(|a, b| b.1.total_cmp(&a.1));
    let zone = st
        .map_zone
        .filter(|z| maps.iter().any(|(m, _)| m == z))
        .unwrap_or(maps.first().map(|m| m.0).unwrap_or(all[0].map));
    st.map_zone = Some(zone);

    ui.horizontal_wrapped(|ui| {
        for (id, mins) in &maps {
            let name = names
                .get(id)
                .cloned()
                .unwrap_or_else(|| format!("Map {id}"));
            let selected = *id == zone;
            let text = RichText::new(format!("{name}  {:.0} min", mins)).color(if selected {
                GOLD
            } else {
                INK
            });
            if ui
                .add(egui::Button::new(text).fill(if selected {
                    RAISED
                } else {
                    Color32::TRANSPARENT
                }))
                .clicked()
            {
                st.map_zone = Some(*id);
                st.map_time = 0.0;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Session").color(MUTED));
        let label = |i: Option<usize>| match i {
            None => "All sessions".to_string(),
            Some(i) => c
                .sessions
                .get(i)
                .map(|s| {
                    format!(
                        "{}, {}",
                        theme::day(s.start as f64),
                        theme::clock(s.start as f64)
                    )
                })
                .unwrap_or_default(),
        };
        egui::ComboBox::from_id_salt("map-session")
            .selected_text(label(st.map_session))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut st.map_session, None, label(None));
                for i in (0..c.sessions.len()).rev() {
                    ui.selectable_value(&mut st.map_session, Some(i), label(Some(i)));
                }
            });
    });

    let range = st
        .map_session
        .and_then(|i| c.sessions.get(i))
        .map(|s| (s.start, s.end));
    let pts: Vec<&Sample> = all
        .iter()
        .filter(|s| s.map == zone && range.is_none_or(|(a, b)| s.t >= a && s.t <= b))
        .collect();
    if pts.is_empty() {
        super::empty(ui, "No movement on this map in that session.");
        return;
    }
    let (t0, t1) = (pts[0].t as f64, pts[pts.len() - 1].t as f64);
    if st.map_time < t0 || st.map_time > t1 {
        st.map_time = t1;
    }

    // Scrubber: replay at one game minute per second.
    ui.horizontal(|ui| {
        if ui
            .button(if st.map_playing {
                "⏸  Pause"
            } else {
                "▶  Replay"
            })
            .clicked()
        {
            if !st.map_playing && st.map_time >= t1 {
                st.map_time = t0;
            }
            st.map_playing = !st.map_playing;
        }
        ui.spacing_mut().slider_width = (ui.available_width() - 340.0).max(200.0);
        ui.add(egui::Slider::new(&mut st.map_time, t0..=t1).show_value(false));
        ui.label(RichText::new(theme::when(st.map_time)).color(MUTED));
        ui.checkbox(&mut st.map_whole, "Whole zone");
    });
    if st.map_playing {
        let dt = ui.input(|i| i.stable_dt) as f64;
        st.map_time += dt * 60.0;
        if st.map_time >= t1 {
            st.map_time = t1;
            st.map_playing = false;
        }
        ui.ctx().request_repaint();
    }

    let avail = ui.available_size() - Vec2::new(0.0, 40.0);
    let aspect = 1002.0 / 668.0;
    let w = avail.x.min(avail.y * aspect).max(300.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, w / aspect), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 8.0, PANEL);

    // The view: the whole zone, or a square (in map units, which keeps the
    // image's aspect) around the path with some air.
    let view = if st.map_whole {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
    } else {
        let (mut x0, mut y0, mut x1, mut y1) = (1.0f64, 1.0f64, 0.0f64, 0.0f64);
        for s in &pts {
            x0 = x0.min(s.x);
            y0 = y0.min(s.y);
            x1 = x1.max(s.x);
            y1 = y1.max(s.y);
        }
        let side = ((x1 - x0).max(y1 - y0) * 1.6).clamp(0.3, 1.0); // the old map art blurs beyond ~3x
        let cx = ((x0 + x1) / 2.0 - side / 2.0).clamp(0.0, 1.0 - side);
        let cy = ((y0 + y1) / 2.0 - side / 2.0).clamp(0.0, 1.0 - side);
        Rect::from_min_size(Pos2::new(cx as f32, cy as f32), Vec2::splat(side as f32))
    };
    match art.get(ui.ctx(), &format!("map-{zone}.jpg")) {
        Some(tex) => {
            p.image(tex.id(), rect, view, Color32::from_gray(210));
        }
        None => {
            for i in 1..10 {
                let f = i as f32 / 10.0;
                p.line_segment(
                    [
                        rect.lerp_inside(Vec2::new(f, 0.0)),
                        rect.lerp_inside(Vec2::new(f, 1.0)),
                    ],
                    Stroke::new(1.0, EDGE),
                );
                p.line_segment(
                    [
                        rect.lerp_inside(Vec2::new(0.0, f)),
                        rect.lerp_inside(Vec2::new(1.0, f)),
                    ],
                    Stroke::new(1.0, EDGE),
                );
            }
        }
    }
    p.rect_stroke(rect, 8.0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
    let at = |x: f64, y: f64| {
        rect.lerp_inside(Vec2::new(
            (x as f32 - view.min.x) / view.width(),
            (y as f32 - view.min.y) / view.height(),
        ))
    };

    // The path fades with age; gaps longer than a minute break it.
    let now = st.map_time;
    let span = (now - t0).max(1.0);
    let visible: Vec<&&Sample> = pts.iter().filter(|s| s.t as f64 <= now).collect();
    for w in visible.windows(2) {
        if w[1].t - w[0].t > 60 {
            continue;
        }
        let age = ((now - w[1].t as f64) / span).clamp(0.0, 1.0);
        let alpha = (255.0 * (1.0 - 0.75 * age)) as u8;
        p.line_segment(
            [at(w[0].x, w[0].y), at(w[1].x, w[1].y)],
            Stroke::new(3.0, Color32::from_rgba_unmultiplied(30, 20, 0, alpha / 2)),
        );
        p.line_segment(
            [at(w[0].x, w[0].y), at(w[1].x, w[1].y)],
            Stroke::new(
                2.0,
                Color32::from_rgba_unmultiplied(0xff, 0xd1, 0x00, alpha),
            ),
        );
    }

    // Events placed at the nearest position sample.
    let nearest = |t: i64| {
        pts.iter()
            .min_by_key(|s| (s.t - t).abs())
            .filter(|s| (s.t - t).abs() < 90)
    };
    let titles: std::collections::HashMap<i64, &str> =
        c.quests.iter().map(|q| (q.id, q.title.as_str())).collect();
    let mut hover_text: Option<String> = None;
    for e in c
        .events
        .iter()
        .filter(|e| (e.t as f64) <= now && e.t as f64 >= t0 - 90.0)
    {
        let (glyph, color, text) = match (e.e.as_str(), e.s("act")) {
            ("quest", Some("accept")) => (
                "!",
                GOLD,
                format!(
                    "Accepted {}",
                    e.i("id")
                        .and_then(|i| titles.get(&i).copied())
                        .unwrap_or("a quest")
                ),
            ),
            ("quest", Some("turnin")) => (
                "?",
                GOLD,
                format!(
                    "Completed {}",
                    e.i("id")
                        .and_then(|i| titles.get(&i).copied())
                        .unwrap_or("a quest")
                ),
            ),
            ("death", _) => ("☠", DANGER, "Died here".to_string()),
            ("level", _) => (
                "★",
                GOLD,
                format!("Reached level {}", e.i("level").unwrap_or(0)),
            ),
            _ => continue,
        };
        let Some(s) = nearest(e.t) else { continue };
        let pos = at(s.x, s.y);
        p.circle_filled(pos, 10.0, Color32::from_rgba_unmultiplied(7, 11, 28, 220));
        p.circle_stroke(pos, 10.0, Stroke::new(1.5, color));
        p.text(
            pos,
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(14.0),
            color,
        );
        if resp.hover_pos().is_some_and(|h| h.distance(pos) < 12.0) {
            hover_text = Some(format!("{}\n{}", text, theme::when(e.t as f64)));
        }
    }
    if let Some(last) = visible.last() {
        let pos = at(last.x, last.y);
        let color = theme::class_color(&c.class_file);
        if let Some(f) = last.facing {
            // Facing 0 is north and grows counter-clockwise.
            let dir = Vec2::new(-(f as f32).sin(), -(f as f32).cos());
            p.line_segment([pos, pos + dir * 18.0], Stroke::new(3.0, color));
        }
        p.circle_filled(pos, 7.0, color);
        p.circle_stroke(pos, 7.0, Stroke::new(2.0, Color32::WHITE));
    }
    if let Some(t) = hover_text {
        resp.on_hover_text(t);
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for (g, c, l) in [
            ("!", GOLD, "quest accepted"),
            ("?", GOLD, "quest done"),
            ("☠", DANGER, "death"),
            ("★", GOLD, "level up"),
        ] {
            ui.label(RichText::new(g).color(c));
            ui.label(RichText::new(l).small().color(MUTED));
            ui.add_space(10.0);
        }
        ui.label(RichText::new("Path fades with age").small().color(MUTED));
    });
}
