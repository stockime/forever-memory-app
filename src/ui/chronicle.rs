//! The Chronicle: a character's diary bound as a tome and read page by page.
//! The entries are set once per page size into leaves of shapes in page
//! coordinates, a paragraph that doesn't fit broken between its rows. A page
//! turn draws the turning leaf as strips bent around the spine: paper and
//! text are both warped onto the bend and shaded by its slope.
//!
//! FM_BOOK=<spread>[:<turn>] opens the book at that spread, with a turn to
//! the next one frozen at that point (0..1), for screenshots.

use super::character;
use super::widgets::{INK_BROWN, INK_RED};
use crate::State;
use crate::art::Art;
use crate::data::memory::Character;
use crate::data::{Model, diary};
use crate::i18n::Lang;
use crate::theme::{self, DANGER, GOLD, MUTED};
use crate::tr;
use egui::epaint::{Tessellator, Vertex};
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align, Color32, FontId, Galley, Mesh, Pos2, Rect, Sense, Shape, Stroke, TextureId, Ui, Vec2,
    pos2, vec2,
};
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::Arc;

/// The spellbook's two pages, its ribbon and a wooden rail, in 2048x1024.
const BOOK: i64 = 5834697;
/// Page width over height, as the texture draws them.
const ASPECT: f32 = 796.0 / 805.0;
const TURN_SECS: f64 = 0.55;
const STRIPS: usize = 28;

fn uv(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(
        pos2(x0 / 2048.0, y0 / 1024.0),
        pos2(x1 / 2048.0, y1 / 1024.0),
    )
}
fn left_uv() -> Rect {
    uv(1.0, 64.0, 797.0, 864.0)
}
fn right_uv() -> Rect {
    uv(924.0, 64.0, 1720.0, 864.0)
}
fn ribbon_uv() -> Rect {
    uv(821.0, 300.0, 899.0, 598.0)
}
fn rail_uv() -> Rect {
    uv(64.0, 2.0, 1600.0, 56.0)
}

#[derive(Clone)]
enum Link {
    Leaf(usize),
    Listen(String),
}

/// One side of a page: shapes and links in page coordinates.
#[derive(Default)]
struct Leaf {
    shapes: Vec<Shape>,
    links: Vec<(Rect, Link)>,
    /// Where the class crest goes, drawn at paint time since art arrives late.
    crest: Option<Rect>,
}

/// What the setting depends on: the character, its entries, page size,
/// language, display scale and whether entries can be listened to.
type Key = (String, u64, u32, u32, Lang, u32, bool);

struct Bound {
    key: Key,
    leaves: Vec<Leaf>,
    /// The leaf the latest entry starts on, for the ribbon.
    latest: usize,
}

struct Turn {
    to: usize,
    start: f64,
    frozen: Option<f32>,
}

/// The open book: which spread shows (leaves 2n and 2n+1), a turn in
/// progress, and the pages as last set.
#[derive(Default)]
pub struct Book {
    slug: String,
    spread: usize,
    turn: Option<Turn>,
    bound: Option<Bound>,
    hooked: bool,
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    super::diary::poll_voice(st);
    if c.diary.is_empty() {
        super::empty(
            ui,
            art,
            133741,
            tr!("Nothing to bind yet"),
            tr!("The entries written in the Diary become the pages of this book."),
        );
        return;
    }
    let mut book = std::mem::take(&mut st.chronicle);
    if book.slug != c.slug {
        book = Book {
            slug: c.slug.clone(),
            hooked: book.hooked,
            ..Default::default()
        };
    }

    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let area = Rect::from_min_max(
        full.min + vec2(44.0, 30.0),
        full.max - vec2(44.0, 30.0 + 46.0),
    );
    let ph = area
        .height()
        .min(area.width() / 2.0 / ASPECT)
        .clamp(240.0, 860.0)
        .floor();
    let pw = (ph * ASPECT).floor();
    let geo = Geo {
        spine: area.center().x.round(),
        top: (area.top() + (area.height() - ph) / 2.0).round(),
        w: pw,
        h: ph,
    };

    let can_listen =
        crate::voice::api_key().is_some() && crate::voice::load_voice(&st.repo, c).is_some();
    let entries = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (day, text) in &c.diary {
            (day, text.len()).hash(&mut h);
        }
        h.finish()
    };
    let key: Key = (
        c.slug.clone(),
        entries,
        pw as u32,
        ph as u32,
        crate::i18n::current(),
        ui.ctx().pixels_per_point().to_bits(),
        can_listen,
    );
    if book.bound.as_ref().is_none_or(|b| b.key != key) {
        let (leaves, latest) = bind(ui.ctx(), c, pw, ph, can_listen);
        book.bound = Some(Bound {
            key,
            leaves,
            latest,
        });
    }
    let bound = book.bound.take().unwrap();
    let spreads = bound.leaves.len().div_ceil(2);
    book.spread = book.spread.min(spreads - 1);
    if !book.hooked {
        book.hooked = true;
        if let Ok(v) = std::env::var("FM_BOOK") {
            let (s, t) = v.split_once(':').unwrap_or((&v, ""));
            book.spread = s.parse::<usize>().unwrap_or(0).min(spreads - 1);
            if let Ok(t) = t.parse::<f32>()
                && book.spread + 1 < spreads
            {
                book.turn = Some(Turn {
                    to: book.spread + 1,
                    start: 0.0,
                    frozen: Some(t),
                });
            }
        }
    }

    // Turning.
    let now = ui.input(|i| i.time);
    let go = |book: &mut Book, to: usize| {
        let to = to.min(spreads - 1);
        if let Some(t) = book.turn.take() {
            book.spread = t.to;
        }
        if to != book.spread {
            book.turn = Some(Turn {
                to,
                start: now,
                frozen: None,
            });
        }
    };
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    if !typing {
        let (next, prev, home, end) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::PageDown),
                i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::PageUp),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
            )
        });
        let at = book.turn.as_ref().map_or(book.spread, |t| t.to);
        if next && at + 1 < spreads {
            go(&mut book, at + 1);
        } else if prev && at > 0 {
            go(&mut book, at - 1);
        } else if home {
            go(&mut book, 0);
        } else if end {
            go(&mut book, spreads - 1);
        }
    }

    let p = ui.painter_at(full);
    desk(&p, ui.ctx(), art, full);
    let tex = art.icon(ui.ctx(), Some(BOOK)).map(|t| t.id());
    let crest = art
        .get(
            ui.ctx(),
            &format!("class-{}.png", c.class_file.to_lowercase()),
        )
        .map(|t| t.id());
    let leaf = |i: usize| bound.leaves.get(i);
    let s = book.spread;
    let (lr, rr) = (geo.left(), geo.right());
    cover(&p, &geo, 2 * s, bound.leaves.len());

    // The ribbon hangs from between the pages at the latest entry.
    let mark = bound.latest / 2;
    let ribbon_x = match mark.cmp(&s) {
        std::cmp::Ordering::Less => geo.spine - geo.w * 0.3,
        std::cmp::Ordering::Equal => geo.spine + geo.w * 0.04,
        std::cmp::Ordering::Greater => geo.spine + geo.w * 0.3,
    };
    let ribbon = Rect::from_min_size(
        pos2(ribbon_x, geo.top + geo.h - 30.0),
        vec2(geo.w * 0.05, geo.w * 0.05 * 298.0 / 78.0),
    );
    if let Some(t) = tex {
        p.image(t, ribbon, ribbon_uv(), Color32::WHITE);
    } else {
        p.rect_filled(ribbon, 2.0, Color32::from_rgb(0x1c, 0x4a, 0x55));
    }

    // Pointer on the book: page edges turn, links act.
    let book_rect = lr.union(rr);
    let resp = ui.interact(book_rect, ui.id().with("book"), Sense::click());
    let ribbon_resp = ui
        .interact(ribbon, ui.id().with("ribbon"), Sense::click())
        .on_hover_text(tr!("The latest entry"));
    if ribbon_resp.clicked() {
        go(&mut book, mark);
    }
    let edge = geo.w * 0.16;
    let hover = resp.hover_pos();
    let at_next = |p: Pos2| p.x > rr.right() - edge && s + 1 < spreads;
    let at_prev = |p: Pos2| p.x < lr.left() + edge && s > 0;
    let link_at = |pos: Pos2| -> Option<Link> {
        [(2 * s, lr), (2 * s + 1, rr)]
            .into_iter()
            .find_map(|(i, r)| {
                leaf(i)?.links.iter().find_map(|(lr, l)| {
                    lr.translate(r.min.to_vec2())
                        .contains(pos)
                        .then(|| l.clone())
                })
            })
    };
    let mut clicked_link = None;
    if book.turn.is_none()
        && let Some(h) = hover
    {
        if at_next(h) || at_prev(h) || link_at(h).is_some() || ribbon_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            if at_next(h) {
                go(&mut book, s + 1);
            } else if at_prev(h) {
                go(&mut book, s - 1);
            } else {
                clicked_link = link_at(h);
            }
        }
    }
    match clicked_link {
        Some(Link::Leaf(i)) => go(&mut book, i / 2),
        Some(Link::Listen(day)) => listen(st, c, &day),
        None => {}
    }

    // The spread, and the turning leaf over it.
    let s = book.spread;
    let flat = |p: &egui::Painter, i: usize, left: bool| {
        page(p, &geo, tex, leaf(i), left, crest);
    };
    match &book.turn {
        Some(t) => {
            let q = t
                .frozen
                .unwrap_or(((now - t.start) / TURN_SECS).clamp(0.0, 1.0) as f32);
            let forward = t.to > s;
            let (l, r, front, back) = if forward {
                (2 * s, 2 * t.to + 1, 2 * s + 1, 2 * t.to)
            } else {
                (2 * t.to, 2 * s + 1, 2 * t.to + 1, 2 * s)
            };
            flat(&p, l, true);
            flat(&p, r, false);
            let e = smooth(q);
            sheet(
                &p,
                ui.ctx(),
                &geo,
                tex,
                (leaf(front), leaf(back)),
                if forward { e } else { 1.0 - e },
                forward,
                crest,
            );
            if t.frozen.is_none() {
                if q >= 1.0 {
                    book.spread = t.to;
                    book.turn = None;
                }
                ui.ctx().request_repaint();
            }
        }
        None => {
            // A hovered edge lifts a little, to show it turns.
            let lift = hover.filter(|h| resp.hovered() && (at_next(*h) || at_prev(*h)));
            match lift {
                Some(h) if at_next(h) => {
                    flat(&p, 2 * s, true);
                    flat(&p, 2 * s + 3, false);
                    let pair = (leaf(2 * s + 1), leaf(2 * s + 2));
                    sheet(&p, ui.ctx(), &geo, tex, pair, 0.06, true, crest);
                }
                Some(_) => {
                    flat(&p, 2 * s - 2, true);
                    flat(&p, 2 * s + 1, false);
                    let pair = (leaf(2 * s - 1), leaf(2 * s));
                    sheet(&p, ui.ctx(), &geo, tex, pair, 0.94, false, crest);
                }
                None => {
                    flat(&p, 2 * s, true);
                    flat(&p, 2 * s + 1, false);
                }
            }
        }
    }

    // Beneath the book: turning buttons, where we are, the narration.
    let bar = Rect::from_min_max(
        pos2(lr.left(), geo.top + geo.h + 30.0),
        pos2(rr.right(), geo.top + geo.h + 64.0),
    );
    let at = book.turn.as_ref().map_or(s, |t| t.to);
    let label = match (2 * at, 2 * at + 1) {
        (0, _) => tr!("Title").to_string(),
        (a, b) if b >= bound.leaves.len() => tr!("page {n}", n = a + 1),
        (a, b) => tr!("pages {a}–{b}", a = a + 1, b = b + 1),
    };
    let mid = bar.center().x.max(geo.spine);
    let text = ui.painter().layout_no_wrap(
        format!(
            "{label}  ·  {}",
            tr!("{n} of {total}", n = at + 1, total = spreads)
        ),
        FontId::proportional(14.5),
        MUTED,
    );
    ui.painter().galley(
        pos2(
            mid - text.size().x / 2.0,
            bar.center().y - text.size().y / 2.0,
        ),
        text.clone(),
        MUTED,
    );
    let half = text.size().x / 2.0 + 18.0;
    let b = |ui: &mut Ui, r: Rect, t: &str, on: bool| {
        ui.put(r, egui::Button::new(t).min_size(r.size()))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
            && on
    };
    let size = vec2(34.0, 28.0);
    if b(
        ui,
        Rect::from_center_size(pos2(mid - half - 17.0, bar.center().y), size),
        "◀",
        at > 0,
    ) {
        go(&mut book, at - 1);
    }
    if b(
        ui,
        Rect::from_center_size(pos2(mid + half + 17.0, bar.center().y), size),
        "▶",
        at + 1 < spreads,
    ) {
        go(&mut book, at + 1);
    }
    let right = Rect::from_min_max(pos2(mid + half + 50.0, bar.top()), bar.max);
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(right)
            .layout(egui::Layout::right_to_left(Align::Center)),
        |ui| {
            if ui
                .button(egui::RichText::new(tr!("Latest entry")).color(GOLD))
                .clicked()
            {
                go(&mut book, mark);
            }
            narration(ui, st, c);
        },
    );
    book.bound = Some(bound);
    st.chronicle = book;
}

/// Where the open book lies: the spine's x, the pages' top and size.
struct Geo {
    spine: f32,
    top: f32,
    w: f32,
    h: f32,
}

impl Geo {
    fn left(&self) -> Rect {
        Rect::from_min_size(pos2(self.spine - self.w, self.top), vec2(self.w, self.h))
    }
    fn right(&self) -> Rect {
        Rect::from_min_size(pos2(self.spine, self.top), vec2(self.w, self.h))
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn quad(mesh: &mut Mesh, pts: [Pos2; 4], uvs: [Pos2; 4], colors: [Color32; 4]) {
    let i = mesh.vertices.len() as u32;
    for k in 0..4 {
        mesh.vertices.push(Vertex {
            pos: pts[k],
            uv: uvs[k],
            color: colors[k],
        });
    }
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
}

fn gradient(p: &egui::Painter, r: Rect, from: Color32, to: Color32, horizontal: bool) {
    let mut m = Mesh::default();
    let w = egui::epaint::WHITE_UV;
    let c = if horizontal {
        [from, to, to, from]
    } else {
        [from, from, to, to]
    };
    quad(
        &mut m,
        [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
        ],
        [w; 4],
        c,
    );
    p.add(m);
}

/// A desk of dark planks, cut from the spellbook's wooden rail.
fn desk(p: &egui::Painter, ctx: &egui::Context, art: &mut Art, r: Rect) {
    p.rect_filled(r, 0.0, Color32::from_rgb(0x1a, 0x10, 0x0a));
    if let Some(t) = art.icon(ctx, Some(BOOK)) {
        let rail = rail_uv();
        let plank = 62.0;
        let span = (r.width() / (1536.0 * plank / 54.0)).min(1.0);
        let mut mesh = Mesh::with_texture(t.id());
        let rows = (r.height() / plank).ceil() as usize;
        for i in 0..rows {
            let y = r.top() + i as f32 * plank;
            let u0 = rail.min.x + rail.width() * (1.0 - span) * ((i as f32 * 0.618).fract());
            let u1 = u0 + rail.width() * span;
            let g = [118, 100, 128, 108, 92][i % 5];
            let tint = Color32::from_rgb(g, (g as f32 * 0.86) as u8, (g as f32 * 0.74) as u8);
            let (flip0, flip1) = if i % 2 == 0 { (u0, u1) } else { (u1, u0) };
            quad(
                &mut mesh,
                [
                    pos2(r.left(), y),
                    pos2(r.right(), y),
                    pos2(r.right(), y + plank),
                    pos2(r.left(), y + plank),
                ],
                [
                    pos2(flip0, rail.min.y),
                    pos2(flip1, rail.min.y),
                    pos2(flip1, rail.max.y),
                    pos2(flip0, rail.max.y),
                ],
                [tint; 4],
            );
        }
        p.add(mesh);
        for i in 1..rows {
            let y = r.top() + i as f32 * plank;
            p.line_segment(
                [pos2(r.left(), y), pos2(r.right(), y)],
                Stroke::new(1.5, Color32::from_black_alpha(120)),
            );
        }
    }
    // Candlelight: the edges fall into shadow.
    let d = 160.0f32.min(r.width() / 4.0);
    let (a, z) = (Color32::from_black_alpha(190), Color32::TRANSPARENT);
    gradient(
        p,
        Rect::from_min_size(r.min, vec2(r.width(), d)),
        a,
        z,
        false,
    );
    gradient(
        p,
        Rect::from_min_max(pos2(r.left(), r.bottom() - d), r.max),
        z,
        a,
        false,
    );
    gradient(
        p,
        Rect::from_min_size(r.min, vec2(d, r.height())),
        a,
        z,
        true,
    );
    gradient(
        p,
        Rect::from_min_max(pos2(r.right() - d, r.top()), r.max),
        z,
        a,
        true,
    );
}

/// The leather cover under the pages and the edges of the pages read and to come.
fn cover(p: &egui::Painter, g: &Geo, read: usize, total: usize) {
    let pages = g.left().union(g.right());
    let c = pages
        .expand2(vec2(g.w * 0.035, g.h * 0.03))
        .translate(vec2(0.0, g.h * 0.008));
    for k in 0..6 {
        p.rect_filled(
            c.translate(vec2(0.0, 10.0)).expand(k as f32 * 4.0),
            14.0 + k as f32 * 4.0,
            Color32::from_black_alpha(40 - k * 6),
        );
    }
    p.rect_filled(c, 12.0, Color32::from_rgb(0x16, 0x2b, 0x2f));
    let mut m = Mesh::default();
    let w = egui::epaint::WHITE_UV;
    let (edge, mid) = (Color32::from_black_alpha(120), Color32::TRANSPARENT);
    let inner = c.shrink(26.0);
    // A rim darker than the middle, for the leather's curve.
    for (a, b) in [
        (
            [c.left_top(), c.right_top()],
            [inner.right_top(), inner.left_top()],
        ),
        (
            [c.right_top(), c.right_bottom()],
            [inner.right_bottom(), inner.right_top()],
        ),
        (
            [c.right_bottom(), c.left_bottom()],
            [inner.left_bottom(), inner.right_bottom()],
        ),
        (
            [c.left_bottom(), c.left_top()],
            [inner.left_top(), inner.left_bottom()],
        ),
    ] {
        quad(
            &mut m,
            [a[0], a[1], b[0], b[1]],
            [w; 4],
            [edge, edge, mid, mid],
        );
    }
    p.add(m);
    p.rect_stroke(
        c.shrink(6.0),
        9.0,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xc8, 0xa2, 0x4a, 80)),
        egui::StrokeKind::Middle,
    );
    // The page block, thicker on the side already read.
    let done = read as f32 / total.max(1) as f32;
    let paper = Color32::from_rgb(0xcf, 0xb4, 0x82);
    let line = Color32::from_rgb(0x8a, 0x6c, 0x44);
    for (rect, share, dir) in [(g.left(), done, -1.0), (g.right(), 1.0 - done, 1.0)] {
        let n = (1.0 + share * 5.0).round() as usize;
        for k in (1..=n).rev() {
            let o = k as f32 * 1.6;
            let r = rect.translate(vec2(dir * o, o * 0.9));
            p.rect_filled(r, 2.0, paper);
            p.rect_stroke(r, 2.0, Stroke::new(0.6, line), egui::StrokeKind::Inside);
        }
    }
}

/// A leaf's shapes in page coordinates, with the crest where it goes.
fn shapes(leaf: &Leaf, crest: Option<TextureId>) -> Vec<Shape> {
    let mut out = leaf.shapes.clone();
    if let (Some(r), Some(t)) = (leaf.crest, crest) {
        out.push(Shape::image(
            t,
            r,
            Rect::from_min_max(pos2(0.07, 0.07), pos2(0.93, 0.93)),
            Color32::WHITE,
        ));
        out.push(Shape::rect_stroke(
            r,
            4.0,
            Stroke::new(1.5, Color32::from_rgb(0x8a, 0x6d, 0x2c)),
            egui::StrokeKind::Outside,
        ));
    }
    out
}

fn paper(p: &egui::Painter, tex: Option<TextureId>, r: Rect, left: bool) {
    match tex {
        Some(t) => p.image(
            t,
            r,
            if left { left_uv() } else { right_uv() },
            Color32::WHITE,
        ),
        None => p.rect_filled(r, 3.0, Color32::from_rgb(0xd9, 0xb7, 0x7f)),
    };
}

/// A page lying flat.
fn page(
    p: &egui::Painter,
    g: &Geo,
    tex: Option<TextureId>,
    leaf: Option<&Leaf>,
    left: bool,
    crest: Option<TextureId>,
) {
    let r = if left { g.left() } else { g.right() };
    paper(p, tex, r, left);
    // The gutter: the page curves down into the spine.
    let d = g.w * 0.1;
    let (a, z) = (Color32::from_black_alpha(95), Color32::TRANSPARENT);
    if left {
        gradient(
            p,
            Rect::from_min_max(pos2(r.right() - d, r.top()), r.max),
            z,
            a,
            true,
        );
    } else {
        gradient(
            p,
            Rect::from_min_size(r.min, vec2(d, r.height())),
            a,
            z,
            true,
        );
    }
    if let Some(leaf) = leaf {
        for mut s in shapes(leaf, crest) {
            s.translate(r.min.to_vec2());
            p.add(s);
        }
    }
}

/// The turning leaf at `pos` (0 flat on the right, 1 flat on the left),
/// bent so its free edge leads the way it moves.
#[allow(clippy::too_many_arguments)]
fn sheet(
    p: &egui::Painter,
    ctx: &egui::Context,
    g: &Geo,
    tex: Option<TextureId>,
    (front, back): (Option<&Leaf>, Option<&Leaf>),
    pos: f32,
    forward: bool,
    crest: Option<TextureId>,
) {
    let n = STRIPS;
    let (lag, lead) = (PI * pos * pos, PI * (1.0 - (1.0 - pos) * (1.0 - pos)));
    let (at_spine, at_edge) = if forward { (lag, lead) } else { (lead, lag) };
    let du = g.w / n as f32;
    let depth = g.w * 4.5;
    let cy = g.top + g.h / 2.0;
    // Columns across the leaf: screen x and the perspective scale.
    let mut cols = Vec::with_capacity(n + 1);
    let mut angles = Vec::with_capacity(n);
    let (mut x, mut z) = (0.0f32, 0.0f32);
    cols.push((g.spine, 1.0f32, 0.0f32));
    for i in 0..n {
        let f = ((i as f32 + 0.5) / n as f32).powf(1.4);
        let a = at_spine + (at_edge - at_spine) * f;
        angles.push(a);
        x += a.cos() * du;
        z += a.sin() * du;
        let k = depth / (depth - z);
        cols.push((g.spine + x * k, k, z));
    }
    let light = vec2(-0.35, 0.94).normalized();
    let shade: Vec<(bool, f32)> = angles
        .iter()
        .map(|a| {
            let front = a.cos() >= 0.0;
            let normal = if front {
                vec2(-a.sin(), a.cos())
            } else {
                vec2(a.sin(), -a.cos())
            };
            (front, 0.52 + 0.48 * normal.dot(light).max(0.0))
        })
        .collect();
    let at = |u: f32, y: f32| -> (Pos2, usize) {
        let t = (u / du).clamp(0.0, n as f32 - 0.0001);
        let i = t as usize;
        let f = t - i as f32;
        let (x0, k0, _) = cols[i];
        let (x1, k1, _) = cols[i + 1];
        let k = k0 + (k1 - k0) * f;
        (pos2(x0 + (x1 - x0) * f, cy + (y - g.h / 2.0) * k), i)
    };
    let dim = |c: Color32, s: f32| {
        let [r, gr, b, a] = c.to_array();
        Color32::from_rgba_premultiplied(
            (r as f32 * s) as u8,
            (gr as f32 * s) as u8,
            (b as f32 * s) as u8,
            a,
        )
    };

    // The paper strips, then the text on each, both drawn far to near.
    let mut papers: Vec<Mesh> = Vec::with_capacity(n);
    let mut strips: Vec<Vec<Mesh>> = (0..n).map(|_| vec![]).collect();
    for i in 0..n {
        let (f, s) = shade[i];
        let (u0, u1) = (i as f32 * du, (i + 1) as f32 * du);
        let edge = |c: (f32, f32, f32), y: f32| pos2(c.0, cy + (y - g.h / 2.0) * c.1);
        let (a0, b0) = (edge(cols[i], 0.0), edge(cols[i], g.h));
        let (a1, b1) = (edge(cols[i + 1], 0.0), edge(cols[i + 1], g.h));
        let (r, flip) = if f {
            (right_uv(), false)
        } else {
            (left_uv(), true)
        };
        let tu = |u: f32| {
            let t = u / g.w;
            if flip {
                r.max.x - t * r.width()
            } else {
                r.min.x + t * r.width()
            }
        };
        let mut m = match tex {
            Some(t) => Mesh::with_texture(t),
            None => Mesh::default(),
        };
        let uvs = if tex.is_some() {
            [
                pos2(tu(u0), r.min.y),
                pos2(tu(u1), r.min.y),
                pos2(tu(u1), r.max.y),
                pos2(tu(u0), r.max.y),
            ]
        } else {
            [egui::epaint::WHITE_UV; 4]
        };
        let base = if tex.is_some() {
            Color32::WHITE
        } else {
            Color32::from_rgb(0xd9, 0xb7, 0x7f)
        };
        quad(&mut m, [a0, a1, b1, b0], uvs, [dim(base, s); 4]);
        papers.push(m);
    }
    let mut tess = Tessellator::new(
        ctx.pixels_per_point(),
        ctx.tessellation_options(|o| *o),
        ctx.fonts(|f| f.font_image_size()),
        vec![],
    );
    for (leaf, is_front) in [(front, true), (back, false)] {
        let Some(leaf) = leaf else { continue };
        for shape in shapes(leaf, crest) {
            let mut src = Mesh::default();
            tess.tessellate_shape(shape, &mut src);
            // Triangles go to the strip under their middle.
            let mut by: HashMap<usize, Mesh> = HashMap::new();
            for tri in src.indices.as_chunks::<3>().0 {
                let v: Vec<&Vertex> = tri.iter().map(|&i| &src.vertices[i as usize]).collect();
                let u = |x: f32| if is_front { x } else { g.w - x };
                let mx = (v[0].pos.x + v[1].pos.x + v[2].pos.x) / 3.0;
                let (_, i) = at(u(mx), 0.0);
                let (f, s) = shade[i];
                if f != is_front {
                    continue;
                }
                let m = by
                    .entry(i)
                    .or_insert_with(|| Mesh::with_texture(src.texture_id));
                let base = m.vertices.len() as u32;
                for vx in &v {
                    let (pos, _) = at(u(vx.pos.x), vx.pos.y);
                    m.vertices.push(Vertex {
                        pos,
                        uv: vx.uv,
                        color: dim(vx.color, s),
                    });
                }
                m.add_triangle(base, base + 1, base + 2);
            }
            for (i, m) in by {
                strips[i].push(m);
            }
        }
    }

    // Its shadow on the pages beneath, beside the lifted edge.
    let lift = (pos * PI).sin();
    if lift > 0.01 {
        let far = cols.iter().map(|c| c.0).fold(g.spine, |a, b| {
            if (b - g.spine).abs() > (a - g.spine).abs() {
                b
            } else {
                a
            }
        });
        let d = g.w * 0.16 * lift;
        let a = Color32::from_black_alpha((110.0 * lift) as u8);
        let page = if far > g.spine { g.right() } else { g.left() };
        let band = if far > g.spine {
            Rect::from_min_max(
                pos2(far, g.top),
                pos2((far + d).min(page.right()), g.top + g.h),
            )
        } else {
            Rect::from_min_max(
                pos2((far - d).max(page.left()), g.top),
                pos2(far, g.top + g.h),
            )
        };
        if band.width() > 0.5 {
            let (from, to) = if far > g.spine {
                (a, Color32::TRANSPARENT)
            } else {
                (Color32::TRANSPARENT, a)
            };
            gradient(p, band, from, to, true);
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| {
        let za = cols[*a].2 + cols[*a + 1].2;
        let zb = cols[*b].2 + cols[*b + 1].2;
        za.total_cmp(&zb)
    });
    for &i in &order {
        p.add(std::mem::take(&mut papers[i]));
    }
    for i in order {
        for m in std::mem::take(&mut strips[i]) {
            p.add(m);
        }
    }
}

/// Plays an entry through the narration player, speaking it first if needed.
fn listen(st: &mut State, c: &Character, day: &str) {
    use crate::voice;
    let (Some(key), Some(v), Some(stored)) = (
        voice::api_key(),
        voice::load_voice(&st.repo, c),
        c.diary.get(day),
    ) else {
        return;
    };
    let text = voice::spoken_text(diary::split(stored).0);
    let path = voice::audio_path(c, day, &v.voice_id, &text);
    let name = format!("entry-{}-{day}", c.slug);
    if st
        .narrator
        .as_ref()
        .is_some_and(|p| p.key == name && !p.finished())
    {
        st.narrator = None;
        return;
    }
    if path.exists() {
        match voice::Player::play(&path, name, crate::config::get().narration_speed) {
            Ok(p) => st.narrator = Some(p),
            Err(e) => st.voice_error = Some(e),
        }
        return;
    }
    let first = c.name.split(' ').next().unwrap_or(&c.name);
    let busy = tr!("{name} is clearing their throat…", name = first);
    super::diary::run(st, &busy, move || {
        match voice::speak(&key, &v.voice_id, &text) {
            Ok(mp3) => {
                let _ = std::fs::create_dir_all(path.parent().unwrap());
                match std::fs::write(&path, mp3) {
                    Ok(()) => super::diary::VoiceMsg::Spoken(path, name),
                    Err(e) => super::diary::VoiceMsg::Failed(e.to_string()),
                }
            }
            Err(e) => super::diary::VoiceMsg::Failed(e),
        }
    });
}

/// What the narrator is doing, in the bar under the book.
fn narration(ui: &mut Ui, st: &mut State, c: &Character) {
    if let Some(e) = &st.voice_error {
        ui.label(egui::RichText::new(e).small().color(DANGER));
    }
    if !st.voice_busy.is_empty() {
        ui.label(egui::RichText::new(&st.voice_busy).small().color(MUTED));
        ui.spinner();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        return;
    }
    let prefix = format!("entry-{}-", c.slug);
    let Some(p) = st
        .narrator
        .as_ref()
        .filter(|p| p.key.starts_with(&prefix) && !p.finished())
    else {
        return;
    };
    let (pos, len) = (p.position(), p.length);
    let mut stop = false;
    if ui.button(format!("■ {}", tr!("Stop"))).clicked() {
        stop = true;
    }
    ui.label(
        egui::RichText::new(format!(
            "{}:{:02} / {}:{:02}",
            pos as i64 / 60,
            pos as i64 % 60,
            len as i64 / 60,
            len as i64 % 60
        ))
        .small()
        .color(MUTED),
    );
    if stop {
        st.narrator = None;
    }
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(250));
}

// ---- binding: the entries set into leaves ----

struct Chapter<'a> {
    zone: String,
    levels: (i64, i64),
    days: Vec<(&'a str, &'a str)>,
}

/// Entries, oldest first, in chapters: a new one wherever the road led to
/// another zone. A day belongs to the zone it spent the most time in.
fn chapters(c: &Character) -> Vec<Chapter<'_>> {
    use chrono::Timelike;
    // The local day of a time, worked out once per day rather than per event.
    let mut span = (0i64, 0i64, String::new());
    let mut day_of = |t: i64| -> String {
        if !(span.0..span.1).contains(&t) {
            let local = theme::local(t as f64);
            let start = t - local.num_seconds_from_midnight() as i64;
            span = (start, start + 86400, local.format("%Y-%m-%d").to_string());
        }
        span.2.clone()
    };
    let mut time: HashMap<String, HashMap<String, i64>> = HashMap::new();
    let (mut zone, mut last) = (String::new(), None::<i64>);
    for e in &c.events {
        if let Some(l) = last
            && !zone.is_empty()
        {
            let day = day_of(e.t);
            if c.diary.contains_key(&day) {
                *time
                    .entry(day)
                    .or_default()
                    .entry(zone.clone())
                    .or_default() += (e.t - l).clamp(0, 300);
            }
        }
        if matches!(e.e.as_str(), "zone" | "login")
            && let Some(z) = e.s("zone").filter(|z| !z.is_empty())
        {
            zone = z.to_string();
        }
        last = Some(e.t);
    }
    let mut levels: HashMap<String, (i64, i64)> = HashMap::new();
    for s in &c.sessions {
        let l = levels
            .entry(day_of(s.start))
            .or_insert((s.level_from, s.level_to));
        l.0 = l.0.min(s.level_from);
        l.1 = l.1.max(s.level_to);
    }
    let mut out: Vec<Chapter> = vec![];
    for (day, stored) in &c.diary {
        let zone = time
            .get(day)
            .and_then(|z| z.iter().max_by_key(|(_, t)| **t).map(|(z, _)| z.clone()))
            .unwrap_or_default();
        let levels = levels.get(day).copied().unwrap_or((0, 0));
        let prose = diary::split(stored).0;
        match out.last_mut() {
            Some(ch) if ch.zone == zone || zone.is_empty() => {
                ch.days.push((day, prose));
                if levels.1 > 0 {
                    ch.levels.1 = ch.levels.1.max(levels.1);
                    if ch.levels.0 == 0 {
                        ch.levels.0 = levels.0;
                    }
                }
            }
            _ => out.push(Chapter {
                zone,
                levels,
                days: vec![(day, prose)],
            }),
        }
    }
    out
}

/// Sets the book: bookplate, title, contents, then the chapters. Returns
/// the leaves and the leaf the latest entry starts on.
fn bind(ctx: &egui::Context, c: &Character, w: f32, h: f32, listen: bool) -> (Vec<Leaf>, usize) {
    let chapters = chapters(c);
    let mut f = Flow::new(ctx, w, h);
    f.bookplate(c);
    f.page();
    f.title(c);
    let listing = |at: &[usize]| -> Vec<(String, String, String, usize)> {
        chapters
            .iter()
            .enumerate()
            .map(|(i, ch)| {
                let span = match (ch.days.first(), ch.days.last()) {
                    (Some(a), Some(b)) if a.0 != b.0 => format!("{} – {}", short(a.0), short(b.0)),
                    (Some(a), _) => short(a.0),
                    _ => String::new(),
                };
                let lv = match ch.levels {
                    (0, _) | (_, 0) => String::new(),
                    (a, b) if a == b => tr!("level {level}", level = a),
                    (a, b) => tr!("levels {a}–{b}", a = a, b = b),
                };
                let sub = [lv, span]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("  ·  ");
                (
                    super::dead::roman(i + 1),
                    chapter_name(ch),
                    sub,
                    at.get(i).copied().unwrap_or(999),
                )
            })
            .collect()
    };
    // The contents come before the chapters but list their pages: set them
    // once to count their pages, then again with the numbers.
    let mut probe = Flow::new(ctx, w, h);
    probe.contents(&listing(&[]));
    let contents_pages = probe.leaves.len();
    f.page();
    let contents_at = f.leaves.len() - 1;
    for _ in 1..contents_pages {
        f.page();
    }
    let mut starts = vec![];
    let mut latest = 0;
    for (i, ch) in chapters.iter().enumerate() {
        f.page();
        starts.push(f.leaves.len() - 1);
        f.chapter(i + 1, ch);
        for (day, prose) in &ch.days {
            latest = f.entry(day, prose, listen);
        }
    }
    f.finis();
    let mut contents = Flow::new(ctx, w, h);
    contents.contents(&listing(&starts));
    for (k, leaf) in contents.leaves.into_iter().enumerate() {
        if let Some(slot) = f.leaves.get_mut(contents_at + k) {
            *slot = leaf;
        }
    }
    if f.leaves.len() % 2 == 1 {
        f.leaves.push(Leaf::default());
    }
    f.numbers();
    (f.leaves, latest)
}

fn chapter_name(ch: &Chapter) -> String {
    if ch.zone.is_empty() {
        tr!("On the road").to_string()
    } else {
        ch.zone.clone()
    }
}

fn short(day: &str) -> String {
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|d| {
            let t = d.and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp();
            theme::day(t as f64)
        })
        .unwrap_or_else(|_| day.to_string())
}

/// Title and paragraphs of an entry, Markdown dropped.
fn parse(prose: &str) -> (Option<String>, Vec<String>) {
    let mut title = None;
    let mut paras = vec![];
    for block in prose.split("\n\n") {
        let b = block.trim();
        if b.is_empty() {
            continue;
        }
        let text = b
            .trim_start_matches('#')
            .trim()
            .replace("**", "")
            .replace(['*', '_'], "")
            .replace('\n', " ");
        if b.starts_with('#') && title.is_none() && paras.is_empty() {
            title = Some(text);
        } else {
            paras.push(text);
        }
    }
    (title, paras)
}

/// Text set onto leaves from the top of a page down.
struct Flow<'a> {
    ctx: &'a egui::Context,
    leaves: Vec<Leaf>,
    w: f32,
    h: f32,
    x0: f32,
    x1: f32,
    top: f32,
    bottom: f32,
    y: f32,
    fs: f32,
    lh: f32,
}

fn job(text: &str, font: FontId, color: Color32, width: f32) -> LayoutJob {
    let mut j = LayoutJob::single_section(
        text.to_string(),
        TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    j.wrap = TextWrapping::wrap_at_width(width);
    j
}

fn centered(mut j: LayoutJob) -> LayoutJob {
    j.halign = Align::Center;
    j
}

fn spaced(mut j: LayoutJob, s: f32) -> LayoutJob {
    for sec in &mut j.sections {
        sec.format.extra_letter_spacing = s;
    }
    j
}

fn display(size: f32) -> FontId {
    theme::display_font(size)
}

fn rows_text(g: &Galley, from: usize) -> String {
    g.rows[from..].iter().map(|r| r.text()).collect::<String>()
}

impl<'a> Flow<'a> {
    fn new(ctx: &'a egui::Context, w: f32, h: f32) -> Self {
        let fs = (w / 29.0).clamp(12.0, 21.0);
        let mut f = Flow {
            ctx,
            leaves: vec![],
            w,
            h,
            x0: (w * 0.12).round(),
            x1: (w * 0.88).round(),
            top: (h * 0.1).round(),
            bottom: (h * 0.875).round(),
            y: 0.0,
            fs,
            lh: (fs * 1.42).round(),
        };
        f.page();
        f
    }
    fn page(&mut self) {
        self.leaves.push(Leaf::default());
        self.y = self.top;
    }
    fn text(&mut self, at: Pos2, g: Arc<Galley>, color: Color32) {
        self.leaf().shapes.push(Shape::galley(at, g, color));
    }
    fn leaf(&mut self) -> &mut Leaf {
        self.leaves.last_mut().unwrap()
    }
    fn cw(&self) -> f32 {
        self.x1 - self.x0
    }
    fn cx(&self) -> f32 {
        (self.x0 + self.x1) / 2.0
    }
    fn lay(&self, j: LayoutJob) -> Arc<Galley> {
        self.ctx.fonts_mut(|f| f.layout_job(j))
    }
    /// Places a whole galley at x on this page (x is its centre when centred).
    fn put(&mut self, g: Arc<Galley>, x: f32) -> Rect {
        let r = g.rect.translate(vec2(x, self.y));
        self.y += g.rect.height();
        self.text(pos2(x, r.top()), g, INK_BROWN);
        r
    }
    /// Sets a paragraph's rows one by one, justified by widening the gaps
    /// between words (egui's own justifying also spaces the letters), going
    /// on to the next page where one runs out; a first row alone at the foot
    /// of a page goes over with the rest. Returns the text of the rows from
    /// `max` on, which are left unset.
    fn lines(&mut self, g: &Galley, x: f32, width: f32, max: usize) -> Option<String> {
        let n = g.rows.len();
        let format = g.job.sections.first()?.format.clone();
        let word =
            |f: &Self, w: &str| f.lay(LayoutJob::single_section(w.to_string(), format.clone()));
        for (k, row) in g.rows.iter().enumerate() {
            if k == max {
                return Some(rows_text(g, k));
            }
            let room = ((self.bottom - self.y) / self.lh + 0.01).floor();
            if (room < 1.0 || (k == 0 && n > 2 && room < 2.0)) && self.y > self.top + 1.0 {
                self.page();
            }
            let start = x + row.pos.x + row.glyphs.first().map_or(0.0, |g| g.pos.x);
            let text = row.text();
            let words: Vec<&str> = text.split_whitespace().collect();
            if k + 1 == n || words.len() < 2 {
                let g = word(self, text.trim());
                self.text(pos2(start, self.y), g, INK_BROWN);
            } else {
                let gs: Vec<Arc<Galley>> = words.iter().map(|w| word(self, w)).collect();
                let used: f32 = gs.iter().map(|g| g.rect.width()).sum();
                let space = self.fs * 0.26;
                let gap = (x + width - start - used) / (gs.len() - 1) as f32;
                // A row with too few words to fill stays ragged.
                let gap = if gap > space * 3.5 { space } else { gap };
                let mut at = start;
                for g in gs {
                    let w = g.rect.width();
                    self.text(pos2(at, self.y), g, INK_BROWN);
                    at += w + gap;
                }
            }
            self.y += self.lh;
        }
        None
    }
    fn ornament(&mut self, width: f32) {
        let (cx, y) = (self.cx(), self.y + self.fs * 0.5);
        let ink = Color32::from_rgba_unmultiplied(0x6b, 0x1e, 0x0a, 170);
        let d = self.fs * 0.28;
        let sh = &mut self.leaves.last_mut().unwrap().shapes;
        for s in [-1.0, 1.0] {
            sh.push(Shape::line_segment(
                [pos2(cx + s * d * 2.2, y), pos2(cx + s * width / 2.0, y)],
                Stroke::new(1.0, ink),
            ));
            sh.push(Shape::circle_filled(
                pos2(cx + s * d * 2.2 + s * 3.0, y),
                1.6,
                ink,
            ));
        }
        sh.push(Shape::convex_polygon(
            vec![
                pos2(cx, y - d),
                pos2(cx + d, y),
                pos2(cx, y + d),
                pos2(cx - d, y),
            ],
            ink,
            Stroke::NONE,
        ));
        self.y += self.fs;
    }

    fn bookplate(&mut self, c: &Character) {
        let (w, h, fs) = (self.w, self.h, self.fs);
        let plate = Rect::from_center_size(pos2(w / 2.0, h * 0.46), vec2(w * 0.54, h * 0.46));
        let ink = Color32::from_rgba_unmultiplied(0x6b, 0x1e, 0x0a, 190);
        let sh = &mut self.leaves.last_mut().unwrap().shapes;
        sh.push(Shape::rect_stroke(
            plate,
            3.0,
            Stroke::new(1.6, ink),
            egui::StrokeKind::Middle,
        ));
        sh.push(Shape::rect_stroke(
            plate.shrink(6.0),
            2.0,
            Stroke::new(0.8, ink),
            egui::StrokeKind::Middle,
        ));
        for corner in [
            plate.left_top(),
            plate.right_top(),
            plate.left_bottom(),
            plate.right_bottom(),
        ] {
            let d = 5.0;
            sh.push(Shape::convex_polygon(
                vec![
                    corner + vec2(0.0, -d),
                    corner + vec2(d, 0.0),
                    corner + vec2(0.0, d),
                    corner + vec2(-d, 0.0),
                ],
                ink,
                Stroke::NONE,
            ));
        }
        self.y = plate.top() + plate.height() * 0.1;
        let g = self.lay(centered(spaced(
            job(
                &tr!("Ex libris").to_uppercase(),
                display(fs * 0.8),
                INK_RED,
                plate.width(),
            ),
            2.0,
        )));
        self.put(g, w / 2.0);
        self.y += fs * 0.8;
        let s = plate.width() * 0.34;
        self.leaf().crest = Some(Rect::from_center_size(
            pos2(w / 2.0, self.y + s / 2.0),
            Vec2::splat(s),
        ));
        self.y += s + fs * 1.1;
        let g = self.lay(centered(job(
            &c.name,
            display(fs * 1.35),
            INK_BROWN,
            plate.width() * 0.9,
        )));
        self.put(g, w / 2.0);
    }

    fn title(&mut self, c: &Character) {
        let (cx, fs, cw) = (self.cx(), self.fs, self.cw());
        self.y = self.h * 0.2;
        let g = self.lay(centered(spaced(
            job(
                &tr!("The Chronicle of").to_uppercase(),
                display(fs * 0.85),
                INK_RED,
                cw,
            ),
            2.5,
        )));
        self.put(g, cx);
        self.y += fs * 0.8;
        let g = self.lay(centered(job(&c.name, display(fs * 2.5), INK_BROWN, cw)));
        self.put(g, cx);
        self.y += fs * 0.6;
        self.ornament(cw * 0.5);
        self.y += fs * 0.3;
        let g = self.lay(centered(job(
            &format!("{} {}", c.race, c.class),
            FontId::new(fs * 1.2, theme::italic()),
            INK_BROWN,
            cw,
        )));
        self.put(g, cx);
        let days: Vec<&String> = c.diary.keys().collect();
        let (Some(a), Some(b)) = (days.first(), days.last()) else {
            return;
        };
        self.y = self.h * 0.6;
        let g = self.lay(centered(spaced(
            job(
                &if days.len() == 1 {
                    tr!("One day").to_string()
                } else {
                    tr!("{n} days", n = days.len())
                }
                .to_uppercase(),
                display(fs * 0.75),
                INK_RED,
                cw,
            ),
            2.0,
        )));
        self.put(g, cx);
        self.y += fs * 0.35;
        let span = if a == b {
            diary::pretty_day(a)
        } else {
            format!("{}\n–\n{}", diary::pretty_day(a), diary::pretty_day(b))
        };
        let g = self.lay(centered(job(
            &span,
            FontId::new(fs * 0.95, theme::italic()),
            Color32::from_rgba_unmultiplied(0x2e, 0x1d, 0x0e, 210),
            cw,
        )));
        self.put(g, cx);
        self.y += fs * 1.2;
        let g = self.lay(centered(job(
            tr!("Set down in their own hand"),
            FontId::new(fs * 0.85, theme::italic()),
            Color32::from_rgba_unmultiplied(0x6b, 0x1e, 0x0a, 200),
            cw,
        )));
        self.put(g, cx);
    }

    fn contents(&mut self, list: &[(String, String, String, usize)]) {
        let (cx, fs, cw) = (self.cx(), self.fs, self.cw());
        let g = self.lay(centered(spaced(
            job(tr!("Contents"), display(fs * 1.5), INK_BROWN, cw),
            1.5,
        )));
        self.put(g, cx);
        self.y += fs * 0.3;
        self.ornament(cw * 0.4);
        self.y += fs * 0.8;
        let indent = fs * 2.2;
        for (roman, name, sub, leaf) in list {
            let num = self.lay(job(
                &(leaf + 1).to_string(),
                display(fs * 0.95),
                INK_BROWN,
                cw,
            ));
            let name_g = self.lay(job(
                name,
                display(fs * 1.05),
                INK_BROWN,
                cw - indent - num.rect.width() - fs * 2.0,
            ));
            let sub_g = self.lay(job(
                sub,
                FontId::new(fs * 0.8, theme::italic()),
                Color32::from_rgba_unmultiplied(0x2e, 0x1d, 0x0e, 190),
                cw - indent,
            ));
            let need = name_g.rect.height() + sub_g.rect.height() + fs * 0.9;
            if self.y + need > self.bottom {
                self.page();
            }
            let top = self.y;
            let r = self.lay(job(roman, display(fs * 0.95), INK_RED, indent));
            self.text(
                pos2(
                    self.x0,
                    top + (name_g.rows[0].height() - r.rect.height()) / 2.0,
                ),
                r,
                INK_RED,
            );
            let last = name_g.rows.last().map_or(0.0, |r| r.rect().right());
            let (x1, x0) = (self.x1, self.x0);
            self.text(
                pos2(
                    x1 - num.rect.width(),
                    top + name_g.rect.height() - num.rect.height(),
                ),
                num.clone(),
                INK_BROWN,
            );
            // Dotted leaders from the name to the number.
            let (mut dx, dy) = (
                x0 + indent + last + fs * 0.5,
                top + name_g.rect.height() - fs * 0.35,
            );
            while dx < x1 - num.rect.width() - fs * 0.5 {
                self.leaf().shapes.push(Shape::circle_filled(
                    pos2(dx, dy),
                    0.9,
                    Color32::from_rgba_unmultiplied(0x2e, 0x1d, 0x0e, 150),
                ));
                dx += fs * 0.45;
            }
            self.put(name_g, x0 + indent);
            if !sub.is_empty() {
                self.put(sub_g, x0 + indent);
            }
            let link = Rect::from_min_max(pos2(x0, top - 2.0), pos2(x1, self.y + 2.0));
            self.leaf().links.push((link, Link::Leaf(*leaf)));
            self.y += fs * 0.9;
        }
    }

    fn chapter(&mut self, n: usize, ch: &Chapter) {
        let (cx, fs, cw) = (self.cx(), self.fs, self.cw());
        self.y = self.top + (self.bottom - self.top) * 0.08;
        let g = self.lay(centered(spaced(
            job(
                &tr!("Chapter {n}", n = super::dead::roman(n)).to_uppercase(),
                display(fs * 0.85),
                INK_RED,
                cw,
            ),
            2.5,
        )));
        self.put(g, cx);
        self.y += fs * 0.5;
        let g = self.lay(centered(job(
            &chapter_name(ch),
            display(fs * 2.0),
            INK_BROWN,
            cw,
        )));
        self.put(g, cx);
        self.y += fs * 0.3;
        let lv = match ch.levels {
            (0, _) | (_, 0) => String::new(),
            (a, b) if a == b => tr!("level {level}", level = a),
            (a, b) => tr!("levels {a}–{b}", a = a, b = b),
        };
        if !lv.is_empty() {
            let g = self.lay(centered(job(
                &lv,
                FontId::new(fs * 0.9, theme::italic()),
                Color32::from_rgba_unmultiplied(0x2e, 0x1d, 0x0e, 190),
                cw,
            )));
            self.put(g, cx);
        }
        self.y += fs * 0.4;
        self.ornament(cw * 0.55);
        self.y += fs * 0.6;
    }

    /// One day's entry; returns the leaf it starts on.
    fn entry(&mut self, day: &str, prose: &str, listen: bool) -> usize {
        let (cx, fs, cw, lh) = (self.cx(), self.fs, self.cw(), self.lh);
        let (title, paras) = parse(prose);
        let date = self.lay(centered(spaced(
            job(
                &diary::pretty_day(day).to_uppercase(),
                display(fs * 0.72),
                INK_RED,
                cw,
            ),
            1.2,
        )));
        let title_g = title.map(|t| {
            let mut j = centered(job(&t, display(fs * 1.4), INK_BROWN, cw * 0.92));
            j.sections[0].format.line_height = Some(fs * 1.7);
            self.lay(j)
        });
        let need = date.rect.height()
            + title_g.as_ref().map_or(0.0, |g| g.rect.height())
            + fs * 1.6
            + lh * 3.0;
        if self.y > self.top + 1.0 {
            self.y += lh * 1.3;
        }
        if self.y + need > self.bottom {
            self.page();
        }
        let at = self.leaves.len() - 1;
        let date_rect = self.put(date, cx);
        if listen {
            let g = self.lay(job(
                &format!("▶ {}", tr!("listen")),
                FontId::proportional(fs * 0.72),
                Color32::from_rgba_unmultiplied(0x6b, 0x1e, 0x0a, 200),
                cw,
            ));
            let (x, y) = if date_rect.right() + fs + g.rect.width() <= self.x1 {
                (self.x1 - g.rect.width(), date_rect.top())
            } else {
                let y = self.y;
                self.y += g.rect.height();
                (cx - g.rect.width() / 2.0, y)
            };
            let r = g.rect.translate(vec2(x, y)).expand(4.0);
            let leaf = self.leaf();
            leaf.shapes.push(Shape::galley(pos2(x, y), g, INK_RED));
            leaf.links.push((r, Link::Listen(day.to_string())));
        }
        if let Some(g) = title_g {
            self.y += fs * 0.25;
            self.put(g, cx);
        }
        self.y += fs * 0.2;
        self.ornament(cw * 0.3);
        self.y += fs * 0.3;
        let body = |text: &str, indent: f32, width: f32| {
            let mut j = LayoutJob::default();
            j.append(
                text,
                indent,
                TextFormat {
                    font_id: FontId::new(fs, theme::italic()),
                    color: INK_BROWN,
                    line_height: Some(lh),
                    ..Default::default()
                },
            );
            j.wrap = TextWrapping::wrap_at_width(width);
            j
        };
        for (i, para) in paras.iter().enumerate() {
            let first = para.chars().next().unwrap_or(' ');
            if i == 0 && first.is_alphabetic() && para.chars().count() > 40 {
                // A drop cap three lines deep, the text beside it.
                let cap = self.lay(job(
                    &first.to_string(),
                    display(lh * 2.0 + fs * 1.05),
                    INK_RED,
                    f32::INFINITY,
                ));
                let capw = cap.rect.width() + fs * 0.35;
                let rest = &para[first.len_utf8()..];
                let narrow = self.lay(body(rest, 0.0, cw - capw));
                let n = narrow.rows.len().min(3);
                if self.y + lh * n as f32 > self.bottom {
                    self.page();
                }
                self.text(pos2(self.x0, self.y - fs * 0.32), cap, INK_RED);
                if let Some(more) = self.lines(&narrow, self.x0 + capw, cw - capw, 3) {
                    let g = self.lay(body(more.trim_start(), 0.0, cw));
                    self.lines(&g, self.x0, cw, usize::MAX);
                }
            } else {
                let g = self.lay(body(para, if i == 0 { 0.0 } else { fs * 1.4 }, cw));
                self.lines(&g, self.x0, cw, usize::MAX);
            }
        }
        at
    }

    fn finis(&mut self) {
        let (cx, fs, cw) = (self.cx(), self.fs, self.cw());
        if self.y + fs * 4.0 > self.bottom {
            self.page();
        }
        self.y += fs * 1.6;
        self.ornament(cw * 0.3);
        self.y += fs * 0.2;
        let g = self.lay(centered(job(
            tr!("To be continued"),
            FontId::new(fs * 0.9, theme::italic()),
            Color32::from_rgba_unmultiplied(0x6b, 0x1e, 0x0a, 200),
            cw,
        )));
        self.put(g, cx);
    }

    /// Page numbers in the outer top corner of every page after the title.
    fn numbers(&mut self) {
        let (w, h, fs) = (self.w, self.h, self.fs);
        for i in 2..self.leaves.len() {
            if self.leaves[i].shapes.is_empty() {
                continue;
            }
            let g = self.lay(job(
                &(i + 1).to_string(),
                display(fs * 0.78),
                Color32::from_rgba_unmultiplied(0x2e, 0x1d, 0x0e, 170),
                w,
            ));
            let x = if i % 2 == 0 {
                self.x0
            } else {
                self.x1 - g.rect.width()
            };
            self.leaves[i]
                .shapes
                .push(Shape::galley(pos2(x, h * 0.045), g, INK_BROWN));
        }
    }
}
