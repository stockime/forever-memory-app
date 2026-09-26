//! The Book of the Dead: every death, remembered by the one who died. Each
//! one is a gravestone and a card: where and when, what struck them down in
//! the ten seconds before, how they came back, and an epitaph they write
//! themselves, in the background like the diary.

use super::{character, widgets};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::dead::{self, Death, Return};
use crate::data::diary;
use crate::theme::{self, DANGER, GOLD, INK, MUTED};
use crate::tr;
use egui::{Color32, CornerRadius, RichText, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use std::collections::HashMap;

/// The Classic stone and marble backgrounds of item texts (plaques, tablets).
const SLATE: i64 = 136279;
const MARBLE: i64 = 136271;
const SKULL_AND_BONES: i64 = 237758;
const BONE: Color32 = Color32::from_rgb(0xdc, 0xcf, 0xae);

pub struct Job {
    slug: String,
    t: i64,
    facts: Vec<String>,
    rx: std::sync::mpsc::Receiver<Result<String, String>>,
}

/// The deaths and epitaphs as last read, for one character and one read of
/// the archive.
struct Read {
    slug: String,
    loaded: std::time::SystemTime,
    deaths: Vec<Death>,
    epitaphs: HashMap<i64, String>,
}

#[derive(Default)]
pub struct Deaths {
    job: Option<Job>,
    error: Option<String>,
    read: Option<Read>,
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    finish(m, st);
    if !st
        .dead
        .read
        .as_ref()
        .is_some_and(|r| r.slug == c.slug && r.loaded == m.loaded_at)
    {
        st.dead.read = Some(Read {
            slug: c.slug.clone(),
            loaded: m.loaded_at,
            deaths: dead::deaths(m, c),
            epitaphs: dead::load(&st.repo, c),
        });
    }
    let read = st.dead.read.take().unwrap();
    let first = c.name.split(' ').next().unwrap_or(&c.name).to_string();

    super::heading(ui, tr!("Book of the Dead"));
    let n = read.deaths.len();
    if n == 0 {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(150.0, 200.0), Sense::hover());
            stone(ui, art, rect, &[], 0.6);
            ui.add_space(16.0);
            ui.label(
                RichText::new(tr!("{name} has never died.", name = first))
                    .font(theme::display_font(22.0))
                    .color(INK),
            );
            ui.label(
                RichText::new(tr!("The book waits, patient as the grave."))
                    .family(theme::italic())
                    .size(17.0)
                    .color(MUTED),
            );
        });
        st.dead.read = Some(read);
        return;
    }
    ui.label(
        RichText::new(if n == 1 {
            tr!(
                "{name} has died once. It is remembered here by the one who died.",
                name = first
            )
        } else {
            tr!(
                "{name} has died {n} times. Each death is remembered here by the one who died.",
                name = first,
                n = n
            )
        })
        .color(MUTED),
    );
    if let Some(e) = &st.dead.error {
        ui.label(RichText::new(e).color(DANGER));
    }
    ui.add_space(10.0);
    let mut write = None;
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for d in read.deaths.iter().rev() {
                let busy = st
                    .dead
                    .job
                    .as_ref()
                    .is_some_and(|j| j.slug == c.slug && j.t == d.t);
                let epitaph = read.epitaphs.get(&d.t).map(|s| diary::split(s));
                if grave(ui, art, &first, d, epitaph, busy, st.dead.job.is_none()) {
                    write = Some(d.clone());
                }
                ui.add_space(18.0);
            }
            ui.add_space(12.0);
        });
    if let Some(d) = write {
        start(c, st, &d, n);
    }
    st.dead.read = Some(read);
    if st.dead.job.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
    }
}

fn start(c: &crate::data::memory::Character, st: &mut State, d: &Death, all: usize) {
    let facts = dead::facts(c, d, all);
    let prompt = dead::prompt(c, &facts);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        tx.send(crate::claude::write(dead::SYSTEM, &prompt)).ok();
    });
    st.dead.error = None;
    st.dead.job = Some(Job {
        slug: c.slug.clone(),
        t: d.t,
        facts,
        rx,
    });
}

fn finish(m: &Model, st: &mut State) {
    let Some(job) = &st.dead.job else { return };
    let Ok(result) = job.rx.try_recv() else {
        return;
    };
    let job = st.dead.job.take().unwrap();
    let Some(c) = m.memory.characters.iter().find(|c| c.slug == job.slug) else {
        return;
    };
    match result.and_then(|e| dead::store(&st.repo, c, job.t, &e, &job.facts)) {
        Ok(()) => st.dead.read = None, // read the epitaphs again
        Err(e) => st.dead.error = Some(e),
    }
}

/// A death as a gravestone and its story; true when the epitaph is asked for.
fn grave(
    ui: &mut Ui,
    art: &mut Art,
    first: &str,
    d: &Death,
    epitaph: Option<(&str, &str)>,
    busy: bool,
    idle: bool,
) -> bool {
    let mut write = false;
    let width = ui.available_width().min(980.0);
    let bg = ui.painter().add(Shape::Noop);
    let frame = egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 22,
            right: 28,
            top: 22,
            bottom: 22,
        })
        .show(ui, |ui| {
            ui.set_width(width - 50.0);
            ui.horizontal_top(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(150.0, 206.0), Sense::hover());
                let date = theme::local(d.t as f64);
                stone(
                    ui,
                    art,
                    rect,
                    &[
                        (roman(d.nth), 30.0),
                        (theme::day(d.t as f64), 15.0),
                        (date.format("%Y").to_string(), 15.0),
                        (tr!("Level {level}", level = d.level), 14.0),
                    ],
                    1.0,
                );
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 5.0;
                    let place = if d.place().is_empty() {
                        tr!("Somewhere unrecorded").to_string()
                    } else {
                        d.place()
                    };
                    ui.label(
                        RichText::new(place)
                            .font(theme::display_font(24.0))
                            .color(BONE),
                    );
                    ui.label(
                        RichText::new(tr!(
                            "{when}, at level {n}",
                            when = theme::when(d.t as f64),
                            n = d.level
                        ))
                        .color(MUTED),
                    );
                    ui.add_space(6.0);
                    story(ui, art, first, d);
                    ui.add_space(10.0);
                    epitaph_part(ui, first, epitaph, busy, idle, &mut write);
                });
            });
        });
    let rect = frame.response.rect;
    let mut shapes = vec![Shape::rect_filled(
        rect.translate(vec2(0.0, 5.0)).expand(2.0),
        12.0,
        Color32::from_black_alpha(90),
    )];
    match art.icon(ui.ctx(), Some(SLATE)) {
        Some(tex) => shapes.push(
            egui::epaint::RectShape::filled(rect, 10.0, Color32::from_rgb(0xd8, 0xd0, 0xc8))
                .with_texture(
                    tex.id(),
                    egui::Rect::from_min_max(pos2(0.02, 0.05), pos2(0.98, 0.98)),
                )
                .into(),
        ),
        None => shapes.push(Shape::rect_filled(
            rect,
            10.0,
            Color32::from_rgb(0x1c, 0x1a, 0x20),
        )),
    }
    shapes.push(Shape::rect_stroke(
        rect,
        10.0,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xdc, 0xcf, 0xae, 50)),
        egui::StrokeKind::Inside,
    ));
    ui.painter().set(bg, Shape::Vec(shapes));
    write
}

/// Killers, the last ten seconds as a line of blows, and the way back.
fn story(ui: &mut Ui, art: &mut Art, first: &str, d: &Death) {
    if d.killers.is_empty() {
        ui.label(
            RichText::new(tr!(
                "Nothing in the combat log shows what struck {name} down.",
                name = first
            ))
            .family(theme::italic())
            .color(MUTED),
        );
    } else {
        let total = d.damage().max(1);
        for k in d.killers.iter().take(4) {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(190.0, 20.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(190.0);
                        ui.add(
                            egui::Label::new(RichText::new(&k.name).size(16.5).color(INK))
                                .truncate(),
                        );
                    },
                );
                widgets::bar(ui, k.damage as f32 / total as f32, DANGER, 120.0);
                ui.label(
                    RichText::new(tr!(
                        "{damage} damage in {n} blows",
                        damage = theme::thousands(k.damage),
                        n = k.blows
                    ))
                    .small()
                    .color(MUTED),
                );
                ui.label(RichText::new(k.with.join(", ")).small().color(MUTED));
            });
        }
        ui.add_space(4.0);
        timeline(ui, art, d);
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if let Some(t) = art.icon(ui.ctx(), Some(widgets::icons::SPIRIT)) {
            ui.add(
                egui::Image::new(&t)
                    .fit_to_exact_size(Vec2::splat(18.0))
                    .corner_radius(3),
            );
        }
        let text = match d.back {
            Return::Ghost(s) => tr!(
                "Walked back as a ghost and rose again {time} later.",
                time = theme::duration(s as f64)
            ),
            Return::Raised(s) => tr!(
                "Brought back where {name} fell, {time} later.",
                name = first,
                time = theme::duration(s as f64)
            ),
            Return::Unknown => tr!("How {name} came back was not recorded.", name = first),
        };
        ui.label(RichText::new(text).color(INK));
    });
    if !d.tasks.is_empty() {
        ui.label(
            RichText::new(tr!("On the road for: {tasks}", tasks = d.tasks.join(", ")))
                .small()
                .color(MUTED),
        );
    }
}

/// The ten seconds before as a strip: one dot per blow, sized by its damage,
/// the killing blow's end marked with a skull.
fn timeline(ui: &mut Ui, art: &mut Art, d: &Death) {
    let w = ui.available_width().min(520.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::hover());
    let p = ui.painter();
    let y = rect.top() + 14.0;
    let (x0, x1) = (rect.left() + 12.0, rect.right() - 22.0);
    let x = |before: f64| x1 - (before.min(10.0) as f32 / 10.0) * (x1 - x0);
    p.line_segment(
        [pos2(x0, y), pos2(x1, y)],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xdc, 0xcf, 0xae, 70)),
    );
    for s in [10, 5, 0] {
        let sx = x(s as f64);
        p.line_segment(
            [pos2(sx, y - 4.0), pos2(sx, y + 4.0)],
            Stroke::new(1.0, MUTED),
        );
        if s > 0 {
            p.text(
                pos2(sx, y + 8.0),
                egui::Align2::CENTER_TOP,
                format!("−{}", tr!("{s}s", s = s)),
                egui::FontId::proportional(12.0),
                MUTED,
            );
        }
    }
    let max = d.blows.iter().map(|b| b.amount).max().unwrap_or(1).max(1) as f32;
    let mut hovered = None;
    for (i, b) in d.blows.iter().enumerate() {
        let c = pos2(x(b.before), y);
        let r = 2.5 + 6.5 * (b.amount as f32 / max).sqrt();
        let a = if b.crit { 235 } else { 160 };
        p.circle_filled(c, r, Color32::from_rgba_unmultiplied(0xe6, 0x67, 0x67, a));
        if resp
            .hover_pos()
            .is_some_and(|h| (h.x - c.x).abs() < r + 3.0 && (h.y - c.y).abs() < 14.0)
        {
            hovered = Some(i);
        }
    }
    if let Some(tex) = art.icon(ui.ctx(), Some(SKULL_AND_BONES)) {
        p.image(
            tex.id(),
            egui::Rect::from_center_size(pos2(x1 + 14.0, y), Vec2::splat(26.0)),
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            BONE,
        );
    }
    if let Some(b) = hovered.map(|i| &d.blows[i]) {
        resp.on_hover_text_at_pointer(format!(
            "−{} · {} · {} · {}",
            tr!("{s}s", s = format!("{:.1}", b.before)),
            b.from,
            b.with,
            if b.crit {
                tr!("{damage} crit", damage = b.amount)
            } else {
                b.amount.to_string()
            }
        ));
    }
}

fn epitaph_part(
    ui: &mut Ui,
    first: &str,
    epitaph: Option<(&str, &str)>,
    busy: bool,
    idle: bool,
    write: &mut bool,
) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width().min(560.0), 9.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xdc, 0xcf, 0xae, 45)),
    );
    if busy {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(
                RichText::new(tr!("{name} is choosing the words…", name = first))
                    .family(theme::italic())
                    .color(MUTED),
            );
        });
        return;
    }
    match epitaph {
        Some((prose, facts)) => {
            let (line, rest) = dead::inscription(prose);
            ui.set_max_width(660.0);
            if !line.is_empty() {
                ui.label(
                    RichText::new(line)
                        .font(theme::display_font(21.0))
                        .color(Color32::from_rgb(0xe8, 0xc9, 0x7a)),
                );
                ui.add_space(2.0);
            }
            ui.label(
                RichText::new(rest)
                    .family(theme::italic())
                    .size(17.5)
                    .color(INK),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        idle,
                        egui::Button::new(RichText::new(tr!("Rewrite")).small()),
                    )
                    .on_hover_text(tr!("Writes the epitaph again from the facts"))
                    .clicked()
                {
                    *write = true;
                }
                egui::CollapsingHeader::new(
                    RichText::new(tr!("The facts behind this epitaph"))
                        .small()
                        .color(MUTED),
                )
                .id_salt(("dead-facts", facts.len(), prose.len()))
                .show(ui, |ui| {
                    ui.label(RichText::new(facts).small().color(MUTED));
                });
            });
        }
        None if !crate::claude::available() => {
            ui.label(
                RichText::new(tr!("Nobody is set up to write. Pick a writer in Settings."))
                    .color(DANGER),
            );
        }
        None => {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        idle,
                        egui::Button::new(RichText::new(tr!("Write the epitaph")).color(GOLD)),
                    )
                    .clicked()
                {
                    *write = true;
                }
                ui.label(
                    RichText::new(tr!(
                        "In {name}'s own words, from what was recorded. Written by {writer}.",
                        name = first,
                        writer = crate::claude::name()
                    ))
                    .small()
                    .color(MUTED),
                );
            });
        }
    }
}

/// A weathered marble headstone: a carved skull and
/// crossbones, and lines of carved text.
fn stone(ui: &Ui, art: &mut Art, rect: egui::Rect, lines: &[(String, f32)], fade: f32) {
    let p = ui.painter();
    let arch = (rect.width() / 2.0).min(255.0) as u8;
    let round = CornerRadius {
        nw: arch,
        ne: arch,
        sw: 3,
        se: 3,
    };
    p.rect_filled(
        rect.translate(vec2(5.0, 6.0)),
        round,
        Color32::from_black_alpha(110),
    );
    let tint = Color32::from_rgb(0xb4, 0xb3, 0xb0).gamma_multiply(fade);
    match art.icon(ui.ctx(), Some(MARBLE)) {
        Some(tex) => p.add(
            egui::epaint::RectShape::filled(rect, round, tint).with_texture(
                tex.id(),
                egui::Rect::from_min_max(pos2(0.04, 0.08), pos2(0.96, 0.97)),
            ),
        ),
        None => p.rect_filled(rect, round, Color32::from_rgb(0x5d, 0x5f, 0x66)),
    };
    // A carved border: a dark groove with light catching its lower edge.
    let inner = rect.shrink(9.0);
    let inner_round = CornerRadius {
        nw: arch.saturating_sub(9),
        ne: arch.saturating_sub(9),
        sw: 2,
        se: 2,
    };
    p.rect_stroke(
        inner.translate(vec2(0.0, 1.0)),
        inner_round,
        Stroke::new(1.0, Color32::from_white_alpha(40)),
        egui::StrokeKind::Middle,
    );
    p.rect_stroke(
        inner,
        inner_round,
        Stroke::new(1.5, Color32::from_black_alpha(120)),
        egui::StrokeKind::Middle,
    );
    let carve = Color32::from_rgb(0x3a, 0x3a, 0x40).gamma_multiply(fade);
    let catch = Color32::from_white_alpha(70);
    let mut y = rect.top() + 26.0;
    if let Some(tex) = art.icon(ui.ctx(), Some(SKULL_AND_BONES)) {
        let r = egui::Rect::from_center_size(pos2(rect.center().x, y + 22.0), Vec2::splat(58.0));
        let uv = egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        p.image(tex.id(), r.translate(vec2(0.0, 1.5)), uv, catch);
        p.image(
            tex.id(),
            r,
            uv,
            Color32::from_rgb(0x44, 0x44, 0x4a).gamma_multiply(fade),
        );
        y += 54.0;
    }
    for (text, size) in lines {
        let font = egui::FontId::new(*size, theme::display());
        let at = pos2(rect.center().x, y);
        p.text(
            at + vec2(0.0, 1.0),
            egui::Align2::CENTER_TOP,
            text,
            font.clone(),
            catch,
        );
        p.text(at, egui::Align2::CENTER_TOP, text, font, carve);
        y += size * 1.35;
    }
}

pub(super) fn roman(mut n: usize) -> String {
    const R: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (v, s) in R {
        while n >= v {
            out += s;
            n -= v;
        }
    }
    out
}
