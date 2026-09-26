//! The House: the whole account at a glance, as one household. Its banner
//! and name, its members, the stable and menagerie they share, the Legacy
//! they earn together, and the history of them all, newest first.

use super::widgets::{self, icons};
use super::{card, label};
use crate::art::Art;
use crate::data::Model;
use crate::data::deeds::{self, Status};
use crate::data::house::{self, Earned, Moment, What};
use crate::data::memory::{Character, Collected};
use crate::data::{diary, letters};
use crate::theme::{self, EDGE, GOLD, INK, MUTED, NIGHT, PANEL};
use crate::tr;
use crate::{Page, State};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};
use std::sync::Arc;

/// A white swallowtail banner from the PvP frame and its border.
const BANNER: i64 = 136743;
const BANNER_EDGE: i64 = 136738;
/// The emblems a House may bear, from the same frame; a click on the banner
/// moves to the next.
const EMBLEMS: [i64; 12] = [
    136632, 136630, 136633, 136641, 136648, 136666, 136702, 136701, 136710, 136628, 136646,
    136720,
];
const LETTER: i64 = 133468;
const RIBBON: i64 = 134411;
const FIRST_DEATH: i64 = 136147;
const MOUNT: i64 = 132264;
const PET: i64 = 132599;
const LEVEL: [(i64, i64); 2] = [(10, 134414), (20, 134153)];
/// Deeds the history already tells in its own words.
const TOLD: [&str; 3] = ["first-death", "level-10", "level-20"];

const BRASS: Color32 = Color32::from_rgb(0xb0, 0x8a, 0x2e);
const TREASURE: Color32 = Color32::from_rgb(0xe6, 0xc8, 0x7a);
const FOE: Color32 = Color32::from_rgb(0xe8, 0x8a, 0x5a);
const MEMBER_W: f32 = 380.0;
const MEMBER_H: f32 = 282.0;
const GAP: f32 = 14.0;

/// Every icon the page draws, for fetching the art ahead.
pub fn art() -> Vec<i64> {
    let mut v = vec![BANNER, BANNER_EDGE, LETTER, RIBBON, FIRST_DEATH, MOUNT, PET];
    v.extend(EMBLEMS);
    v.extend(LEVEL.map(|(_, i)| i));
    v
}

/// What the page works out once per archive read.
struct Account {
    /// Deeds earned, per character.
    deeds: Vec<usize>,
    letters: usize,
    timeline: Vec<Moment>,
}

fn account(ui: &Ui, m: &Model, st: &State) -> Arc<Account> {
    let chars = &m.memory.characters;
    let mut earned: Vec<Earned> = vec![];
    let mut counts = vec![];
    for (i, c) in chars.iter().enumerate() {
        let status = super::deeds::statuses(ui, m, c);
        let mut n = 0;
        for d in deeds::for_class(&c.class_file) {
            if let Some(Status::Earned(t)) = status.get(d.id) {
                n += 1;
                if let Some(t) = t.filter(|_| !TOLD.contains(&d.id)) {
                    earned.push((i, d.name.to_string(), d.icon, t));
                }
            }
        }
        counts.push(n);
    }
    // Keyed on the archive read like the per-character memos; the first
    // character only names the slot.
    super::story::memo(ui, m, &chars[0], "house", || {
        let slugs: Vec<&str> = chars.iter().map(|c| c.slug.as_str()).collect();
        let all = letters::load(&st.repo, &slugs);
        Account {
            deeds: counts,
            letters: all.len(),
            timeline: house::timeline(m, &all, &earned),
        }
    })
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art, page: &mut Page) {
    let chars = &m.memory.characters;
    let Some(head) = house::head(m) else {
        return;
    };
    let acc = account(ui, m, st);
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            header(ui, m, art, &chars[head]);
            ui.add_space(14.0);
            card(ui, |ui| totals(ui, m, art, &acc));
            ui.add_space(22.0);
            label(ui, tr!("The members"));
            ui.add_space(6.0);
            members(ui, m, st, art, page, &acc, head);
            if chars.len() == 1 {
                alone(ui, art, &chars[0]);
            }
            ui.add_space(22.0);
            label(ui, tr!("What the House owns"));
            ui.add_space(6.0);
            super::pair_by(ui, |ui, i| {
                card(ui, |ui| owned(ui, m, art, i == 0));
            });
            ui.add_space(22.0);
            legacy(ui, m, art);
            ui.add_space(22.0);
            label(ui, tr!("The House's history"));
            ui.add_space(8.0);
            history(ui, m, st, art, page, &acc);
            ui.add_space(24.0);
        });
}

fn first_name(c: &Character) -> &str {
    c.name.split(' ').next().unwrap_or(&c.name)
}

fn default_name(head: &Character) -> String {
    match house::surname(head) {
        s if s.is_empty() => tr!("The Household").to_string(),
        s => tr!("House {name}", name = s),
    }
}

// ---- the header ----

fn header(ui: &mut Ui, m: &Model, art: &mut Art, head: &Character) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 214.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 10.0, PANEL);
    let class = head.class_file.to_lowercase();
    if let Some(tex) = art.get(ui.ctx(), &format!("banner-{class}.jpg")) {
        let [tw, th] = tex.size().map(|v| v as f32);
        let scale = (rect.width() / tw).max(rect.height() / th);
        let (uw, uh) = (rect.width() / (tw * scale), rect.height() / (th * scale));
        let uv = Rect::from_min_size(pos2(1.0 - uw, (1.0 - uh) * 0.3), vec2(uw, uh));
        p.image(tex.id(), rect, uv, Color32::from_gray(150));
    }
    // Night from the left, where the banner and the name stand.
    fade(&p, rect, 0.72);
    p.rect_stroke(rect, 10.0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);

    let settings = crate::config::get();
    let emblem = Some(settings.house_emblem)
        .filter(|e| EMBLEMS.contains(e))
        .unwrap_or(EMBLEMS[0]);
    let banner = Rect::from_min_size(rect.left_top() + vec2(34.0, 0.0), vec2(96.0, 192.0));
    let r = ui
        .interact(banner, ui.id().with("house-banner"), Sense::click())
        .on_hover_text(tr!("Click for another emblem"));
    if r.clicked() {
        let at = EMBLEMS.iter().position(|e| *e == emblem).unwrap_or(0);
        let next = EMBLEMS[(at + 1) % EMBLEMS.len()];
        let _ = crate::config::update(|s| s.house_emblem = next);
    }
    draw_banner(ui, art, banner, theme::class_color(&head.class_file), emblem, r.hovered());

    let x = banner.right() + 30.0;
    let text = Rect::from_min_max(
        pos2(x, rect.top() + 30.0),
        pos2(rect.right() - 24.0, rect.bottom() - 20.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let ui = &mut child;
    ui.spacing_mut().item_spacing.y = 4.0;
    name(ui, head, &settings.house_name);
    let chars = &m.memory.characters;
    let since = chars
        .iter()
        .filter_map(|c| c.sessions.first().map(|s| s.start))
        .min()
        .map(|t| diary::pretty_day(&diary::day_of(t)));
    let mut line = match chars.len() {
        1 => tr!("{name}, alone so far", name = head.name),
        2 => tr!("Headed by {name}, with one other", name = head.name),
        n => tr!("Headed by {name}, with {n} others", name = head.name, n = n - 1),
    };
    if let Some(day) = since {
        line += &tr!(", since {day}", day = day);
    }
    ui.label(RichText::new(line).size(17.0).color(INK));
    ui.add_space(8.0);
    // The members' crests in a row, like seals under the name.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for c in chars {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
            crest(ui, art, c, r, 1.0);
            resp.on_hover_text(&c.name);
        }
    });
}

/// A dark fade over art from the left edge to `share` of the width.
fn fade(p: &egui::Painter, rect: Rect, share: f32) {
    let mut mesh = egui::Mesh::default();
    let dark = Color32::from_rgba_unmultiplied(10, 14, 31, 240);
    let clear = Color32::from_rgba_unmultiplied(10, 14, 31, 40);
    let mid = rect.left() + rect.width() * share;
    for (a, b, ca, cb) in [
        (rect.left(), mid, dark, clear),
        (mid, rect.right(), clear, clear),
    ] {
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(a, rect.top()), ca);
        mesh.colored_vertex(pos2(b, rect.top()), cb);
        mesh.colored_vertex(pos2(b, rect.bottom()), cb);
        mesh.colored_vertex(pos2(a, rect.bottom()), ca);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    p.add(mesh);
}

/// The House's banner: the cloth in the head's colour, darkened to a dye,
/// a brass edge and pole, and the emblem.
fn draw_banner(ui: &Ui, art: &mut Art, r: Rect, color: Color32, emblem: i64, hot: bool) {
    let p = ui.painter();
    let dye = Color32::from_rgb(
        (color.r() as f32 * 0.55) as u8,
        (color.g() as f32 * 0.55) as u8,
        (color.b() as f32 * 0.55) as u8,
    );
    let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let pole = Rect::from_min_max(
        pos2(r.left() - 10.0, r.top() + 6.0),
        pos2(r.right() + 10.0, r.top() + 12.0),
    );
    p.rect_filled(pole.translate(vec2(0.0, 2.0)), 3.0, Color32::from_black_alpha(120));
    p.rect_filled(pole, 3.0, BRASS);
    let cloth = Rect::from_min_max(pos2(r.left(), r.top() + 10.0), r.max);
    if let Some(t) = art.icon(ui.ctx(), Some(BANNER)) {
        p.image(t.id(), cloth.translate(vec2(3.0, 4.0)), full, Color32::from_black_alpha(140));
        p.image(t.id(), cloth, full, dye);
    } else {
        p.rect_filled(cloth, 4.0, dye);
    }
    if let Some(t) = art.icon(ui.ctx(), Some(BANNER_EDGE)) {
        p.image(t.id(), cloth, full, if hot { GOLD } else { BRASS });
    }
    if let Some(t) = art.icon(ui.ctx(), Some(emblem)) {
        let e = Rect::from_center_size(
            pos2(cloth.center().x - 2.0, cloth.top() + cloth.height() * 0.36),
            Vec2::splat(r.width() * 0.78),
        );
        p.image(t.id(), e, full, Color32::from_rgb(0xf3, 0xe2, 0xb4));
    }
    for x in [pole.left(), pole.right()] {
        p.circle_filled(pos2(x, pole.center().y), 5.0, GOLD.gamma_multiply(0.8));
    }
}

/// The House's name, renamed in place with a click.
fn name(ui: &mut Ui, head: &Character, stored: &str) {
    let edit_id = egui::Id::new("house-name-edit");
    let field_id = egui::Id::new("house-name-field");
    let current = if stored.trim().is_empty() {
        default_name(head)
    } else {
        stored.to_string()
    };
    let font = theme::display_font(46.0);
    let mut editing: Option<String> = ui.data(|d| d.get_temp(edit_id));
    if let Some(buf) = &mut editing {
        let r = ui.add(
            egui::TextEdit::singleline(buf)
                .id(field_id)
                .font(font)
                .text_color(GOLD)
                .desired_width(ui.available_width().min(640.0))
                .hint_text(default_name(head)),
        );
        if r.lost_focus() {
            if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                let new = buf.trim().to_string();
                let keep = if new == default_name(head) { String::new() } else { new };
                let _ = crate::config::update(|s| s.house_name = keep);
            }
            editing = None;
        }
    } else {
        let r = ui
            .add(egui::Label::new(RichText::new(&current).font(font).color(GOLD)).sense(Sense::click()))
            .on_hover_text(tr!("Click to rename the House"));
        if r.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if r.clicked() {
            editing = Some(current);
            ui.memory_mut(|mem| mem.request_focus(field_id));
        }
    }
    ui.data_mut(|d| match editing {
        Some(e) => {
            d.insert_temp(edit_id, e);
        }
        None => d.remove::<String>(edit_id),
    });
}

/// A character's class crest in a brass-edged frame.
fn crest(ui: &Ui, art: &mut Art, c: &Character, r: Rect, stroke: f32) {
    let p = ui.painter();
    p.rect_filled(r, 5.0, Color32::from_rgb(5, 7, 15));
    if let Some(t) = art.get(ui.ctx(), &format!("class-{}.png", c.class_file.to_lowercase())) {
        p.image(
            t.id(),
            r.shrink(2.0),
            Rect::from_min_max(pos2(0.07, 0.07), pos2(0.93, 0.93)),
            Color32::WHITE,
        );
    }
    p.rect_stroke(r, 5.0, Stroke::new(stroke, BRASS), egui::StrokeKind::Inside);
}

/// Seconds as days and hours once there is a day of them.
fn long_duration(secs: i64) -> String {
    if secs < 86400 {
        return theme::duration(secs as f64);
    }
    tr!("{d}d {h}h", d = secs / 86400, h = secs % 86400 / 3600)
}

fn totals(ui: &mut Ui, m: &Model, art: &mut Art, acc: &Account) {
    let chars = &m.memory.characters;
    let levels: i64 = chars.iter().map(|c| c.level).sum();
    let played: i64 = chars.iter().map(Character::total_play).sum();
    let mounts = m.memory.account.iter().filter(|a| a.kind == "mounts").count();
    let pets = m.memory.account.iter().filter(|a| a.kind != "mounts").count();
    let legacy = house::legacy_source(m).and_then(house::legacy_points);
    let deeds: usize = acc.deeds.iter().sum();
    ui.set_width(ui.available_width());
    widgets::figure_row(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 26.0;
        let n = chars.len().to_string();
        widgets::figure_text(ui, art, icons::GROUP, &n, tr!("members"));
        let levels = theme::thousands(levels);
        widgets::figure_text(ui, art, icons::SPIRIT, &levels, tr!("levels in all"));
        widgets::figure_text(ui, art, icons::WATCH, &long_duration(played), tr!("played"));
        widgets::figure_text(ui, art, RIBBON, &deeds.to_string(), tr!("deeds earned"));
        if let Some((_, total)) = legacy {
            let total = total.to_string();
            widgets::figure_text(ui, art, EMBLEMS[0], &total, tr!("Legacy points"));
        }
        widgets::figure_text(ui, art, MOUNT, &mounts.to_string(), tr!("mounts"));
        widgets::figure_text(ui, art, PET, &pets.to_string(), tr!("companions"));
        let letters = acc.letters.to_string();
        widgets::figure_text(ui, art, LETTER, &letters, tr!("letters"));
    });
}

// ---- the members ----

fn members(
    ui: &mut Ui,
    m: &Model,
    st: &mut State,
    art: &mut Art,
    page: &mut Page,
    acc: &Account,
    head: usize,
) {
    let chars = &m.memory.characters;
    // The head first, then by time given to the world.
    let mut order: Vec<usize> = (0..chars.len()).collect();
    order.sort_by_key(|&i| (i != head, std::cmp::Reverse(chars[i].total_play()), i));
    let avail = ui.available_width();
    let cols = (((avail + GAP) / (MEMBER_W + GAP)).floor() as usize).clamp(1, chars.len().max(1));
    let w = ((avail - GAP * (cols - 1) as f32) / cols as f32).min(MEMBER_W * 1.4);
    for row in order.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for &i in row {
                let deeds = acc.deeds.get(i).copied().unwrap_or(0);
                if member(ui, m, art, i, i == head, deeds, w) {
                    super::select(st, i);
                    *page = Page::Overview;
                }
            }
        });
        ui.add_space(GAP);
    }
}

/// With a single member, a word on who else may come.
fn alone(ui: &mut Ui, art: &mut Art, c: &Character) {
    ui.horizontal(|ui| {
        super::icon(ui, art, Some(icons::GROUP), BRASS, 40.0);
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let title = RichText::new(tr!("A house of one")).font(theme::display_font(20.0));
            ui.label(title.color(INK));
            ui.label(
                RichText::new(tr!(
                    "For now {name} keeps the House alone. Whoever else takes up the road with the addon will join them here.",
                    name = first_name(c)
                ))
                .color(MUTED),
            );
        });
    });
}

/// What a member says for themselves on their card: the latest diary
/// words, else their personality note or preset; and where it comes from.
fn words(c: &Character) -> Option<(String, String)> {
    if let Some((day, w)) = house::latest_words(c, 150) {
        let from = tr!("from the diary, {day}", day = diary::pretty_day(&day));
        return Some((format!("“{w}”"), from));
    }
    let note = c.personality.trim();
    if !note.is_empty() {
        return Some((diary::clip(note, 150), tr!("as the player sees them").into()));
    }
    let preset = crate::data::presets::for_character(c, crate::i18n::current())?;
    Some((diary::clip(&preset, 150), tr!("as their kind are known").into()))
}

/// A member's portrait card; true when clicked.
fn member(ui: &mut Ui, m: &Model, art: &mut Art, i: usize, head: bool, deeds: usize, w: f32) -> bool {
    let c = &m.memory.characters[i];
    let (rect, resp) = ui.allocate_exact_size(vec2(w, MEMBER_H), Sense::click());
    let color = theme::class_color(&c.class_file);
    let hot = resp.hovered();
    let p = ui.painter_at(rect.expand(4.0));
    p.rect_filled(rect.translate(vec2(0.0, 4.0)), 10.0, Color32::from_black_alpha(80));
    p.rect_filled(rect, 10.0, PANEL);
    // The class's art across the top, fading into the card.
    let band = Rect::from_min_size(rect.min, vec2(rect.width(), 100.0)).shrink(1.0);
    if let Some(tex) = art.get(ui.ctx(), &format!("banner-{}.jpg", c.class_file.to_lowercase())) {
        let [tw, th] = tex.size().map(|v| v as f32);
        let uh = (band.height() / (th * band.width() / tw)).min(1.0);
        let uv = Rect::from_min_size(pos2(0.0, (1.0 - uh) * 0.35), vec2(1.0, uh));
        let tint = Color32::from_gray(if hot { 150 } else { 115 });
        let mut mesh = egui::Mesh::with_texture(tex.id());
        for (pos, uv, color) in [
            (band.left_top(), uv.left_top(), tint),
            (band.right_top(), uv.right_top(), tint),
            (band.right_bottom(), uv.right_bottom(), Color32::TRANSPARENT),
            (band.left_bottom(), uv.left_bottom(), Color32::TRANSPARENT),
        ] {
            mesh.vertices.push(egui::epaint::Vertex { pos, uv, color });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(mesh);
    }
    let edge = if hot {
        color.gamma_multiply(0.9)
    } else {
        Color32::from_rgb(0x34, 0x3a, 0x62)
    };
    let width = if hot { 1.5 } else { 1.0 };
    p.rect_stroke(rect, 10.0, Stroke::new(width, edge), egui::StrokeKind::Inside);

    let crest_r = Rect::from_min_size(rect.min + vec2(18.0, 20.0), Vec2::splat(62.0));
    p.rect_filled(crest_r.translate(vec2(0.0, 3.0)), 6.0, Color32::from_black_alpha(120));
    crest(ui, art, c, crest_r, 2.0);
    let tx = crest_r.right() + 16.0;
    p.text(
        pos2(tx, crest_r.top() + 2.0),
        egui::Align2::LEFT_TOP,
        &c.name,
        theme::display_font(25.0),
        color,
    );
    p.text(
        pos2(tx, crest_r.top() + 38.0),
        egui::Align2::LEFT_TOP,
        tr!("Level {level} {race} {class}", level = c.level, race = c.race, class = c.class),
        FontId::proportional(16.0),
        INK,
    );
    if head {
        let text = tr!("Head of the House").to_uppercase();
        let g = p.layout_no_wrap(text, FontId::proportional(11.5), GOLD);
        let pill = Rect::from_min_size(
            pos2(rect.right() - g.size().x - 26.0, rect.top() + 12.0),
            g.size() + vec2(14.0, 6.0),
        );
        p.rect_filled(pill, 4.0, Color32::from_rgba_unmultiplied(5, 7, 15, 200));
        p.rect_stroke(pill, 4.0, Stroke::new(1.0, BRASS), egui::StrokeKind::Inside);
        p.galley(pill.min + vec2(7.0, 3.0), g, GOLD);
    }

    let body = Rect::from_min_max(
        pos2(rect.left() + 18.0, crest_r.bottom() + 16.0),
        pos2(rect.right() - 18.0, rect.bottom() - 40.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(body)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    {
        let ui = &mut child;
        ui.spacing_mut().item_spacing.y = 5.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let seen = house::last_seen(c);
        let when = if seen > 0 {
            tr!("last seen {when}", when = theme::ago(seen as f64))
        } else {
            tr!("not yet seen in the world").to_string()
        };
        let place = if c.zone.is_empty() {
            when
        } else {
            format!("{} · {when}", c.zone)
        };
        ui.label(RichText::new(place).size(14.5).color(MUTED));
        let played = c.total_play();
        let played = match c.sessions.len() {
            _ if played <= 0 => tr!("No time in the world recorded yet").to_string(),
            1 => tr!("{time} played over one session", time = long_duration(played)),
            n => tr!("{time} played over {n} sessions", time = long_duration(played), n = n),
        };
        ui.label(RichText::new(played).size(14.5).color(MUTED));
        let profs = house::professions(c);
        if !profs.is_empty() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for s in &profs {
                    chip(ui, &s.name, s.rank, s.max);
                }
            });
        }
        if let Some((w, from)) = words(c) {
            ui.add_space(2.0);
            let mut job = LayoutJob::single_section(
                w,
                TextFormat::simple(FontId::new(15.5, theme::italic()), INK),
            );
            job.wrap.max_width = body.width();
            job.wrap.max_rows = 2;
            let g = ui.fonts_mut(|f| f.layout_job(job));
            ui.label(g);
            ui.label(RichText::new(from).size(12.5).color(MUTED));
        }
    }

    // The foot: small counts, and where a click goes.
    let foot = Rect::from_min_max(
        pos2(rect.left() + 18.0, rect.bottom() - 36.0),
        pos2(rect.right() - 18.0, rect.bottom() - 10.0),
    );
    let p = ui.painter_at(rect);
    p.line_segment(
        [pos2(foot.left(), foot.top() - 4.0), pos2(foot.right(), foot.top() - 4.0)],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xb0, 0x8a, 0x2e, 50)),
    );
    let deaths = c.events.iter().filter(|e| e.e == "death").count();
    let counts = [
        (RIBBON, match deeds {
            1 => tr!("1 deed").to_string(),
            n => tr!("{n} deeds", n = n),
        }),
        (icons::FEIGN, match deaths {
            1 => tr!("1 death").to_string(),
            n => tr!("{n} deaths", n = n),
        }),
        (icons::SCROLL, match c.diary.len() {
            1 => tr!("1 diary entry").to_string(),
            n => tr!("{n} diary entries", n = n),
        }),
    ];
    let mut x = foot.left();
    for (icon, text) in counts {
        let r = Rect::from_min_size(pos2(x, foot.center().y - 10.0), Vec2::splat(20.0));
        if let Some(t) = art.icon(ui.ctx(), Some(icon)) {
            let uv = Rect::from_min_max(pos2(0.07, 0.07), pos2(0.93, 0.93));
            p.image(t.id(), r, uv, Color32::WHITE);
        }
        let g = p.layout_no_wrap(text, FontId::proportional(14.0), INK);
        let right = r.right() + 6.0 + g.size().x;
        p.galley(pos2(r.right() + 6.0, foot.center().y - g.size().y / 2.0), g, INK);
        x = right + 18.0;
    }
    if hot {
        p.text(
            pos2(foot.right(), foot.center().y),
            egui::Align2::RIGHT_CENTER,
            tr!("Open →"),
            FontId::proportional(14.0),
            GOLD,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// A profession and its rank, brass once it reaches 150.
fn chip(ui: &mut Ui, name: &str, rank: i64, max: i64) {
    let master = rank >= 150;
    let text = if max > 0 {
        format!("{name} {rank}/{max}")
    } else {
        name.to_string()
    };
    let color = if master { TREASURE } else { INK };
    let g = ui.painter().layout_no_wrap(text, FontId::proportional(13.5), color);
    let (r, _) = ui.allocate_exact_size(g.size() + vec2(14.0, 8.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 4.0, Color32::from_rgb(0x0c, 0x10, 0x22));
    let edge = if master { BRASS } else { EDGE };
    p.rect_stroke(r, 4.0, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    p.galley(r.min + vec2(7.0, 4.0), g, color);
}

// ---- what the House owns ----

fn owned(ui: &mut Ui, m: &Model, art: &mut Art, mounts: bool) {
    ui.set_width(ui.available_width());
    let things: Vec<&Collected> = m
        .memory
        .account
        .iter()
        .filter(|a| (a.kind == "mounts") == mounts)
        .collect();
    ui.horizontal(|ui| {
        let title = if mounts { tr!("The stable") } else { tr!("The menagerie") };
        ui.label(RichText::new(title).font(theme::display_font(19.0)).color(INK));
        ui.label(RichText::new(things.len().to_string()).color(MUTED));
    });
    ui.add_space(6.0);
    if things.is_empty() {
        let text = if mounts {
            tr!("No mounts yet. The first one brought home will stand here, for all of them to ride.")
        } else {
            tr!("No companions yet. The first one brought home will wait here for all of them.")
        };
        ui.label(RichText::new(text).color(MUTED));
        return;
    }
    let all_id = ui.id().with(("owned-all", mounts));
    let all = ui.data(|d| d.get_temp::<bool>(all_id)).unwrap_or(false);
    const FEW: usize = 8;
    let chars = &m.memory.characters;
    for a in things.iter().take(if all { things.len() } else { FEW }) {
        ui.horizontal(|ui| {
            super::icon(ui, art, a.icon, BRASS, 44.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.add_space(2.0);
                ui.label(RichText::new(&a.name).size(17.0).color(TREASURE));
                let by = chars.iter().find(|c| !a.by.is_empty() && c.guid == a.by);
                let day = (a.first > 0).then(|| theme::day(a.first as f64));
                let job = match (by, &day) {
                    (Some(c), Some(day)) => rich(
                        tr!("brought home by {name}, {day}"),
                        &[
                            ("name", first_name(c), theme::class_color(&c.class_file)),
                            ("day", day, MUTED),
                        ],
                        14.0,
                    ),
                    (Some(c), None) => rich(
                        tr!("brought home by {name}"),
                        &[("name", first_name(c), theme::class_color(&c.class_file))],
                        14.0,
                    ),
                    (None, Some(day)) => rich(tr!("came home {day}"), &[("day", day, MUTED)], 14.0),
                    (None, None) => rich(tr!("in the household"), &[], 14.0),
                };
                ui.label(job);
            });
        });
        ui.add_space(4.0);
    }
    if things.len() > FEW {
        let text = if all {
            tr!("Show fewer").to_string()
        } else {
            tr!("Show all {n}", n = things.len())
        };
        if ui.link(RichText::new(text).color(GOLD)).clicked() {
            ui.data_mut(|d| d.insert_temp(all_id, !all));
        }
    }
}

/// A translated line with its {placeholders} filled in their own colours.
fn rich(template: &str, parts: &[(&str, &str, Color32)], size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let plain = TextFormat::simple(FontId::proportional(size), MUTED);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|c| open + c) else {
            break;
        };
        job.append(&rest[..open], 0.0, plain.clone());
        match parts.iter().find(|(k, _, _)| *k == &rest[open + 1..close]) {
            Some((_, text, color)) => job.append(
                text,
                0.0,
                TextFormat::simple(FontId::proportional(size), *color),
            ),
            None => job.append(&rest[open..=close], 0.0, plain.clone()),
        }
        rest = &rest[close + 1..];
    }
    job.append(rest, 0.0, plain);
    job
}

// ---- the Legacy ----

fn legacy(ui: &mut Ui, m: &Model, art: &mut Art) {
    label(ui, tr!("The Legacy"));
    let src = house::legacy_source(m);
    if let Some((spent, total)) = src.and_then(house::legacy_points) {
        let text = if total > 0 {
            tr!(
                "{total} points earned together by the whole House, {spent} of them spent",
                total = total,
                spent = spent
            )
        } else {
            tr!("No points earned yet").into()
        };
        ui.label(RichText::new(text).color(MUTED));
    }
    ui.add_space(8.0);
    card(ui, |ui| crafts(ui, m, art));
    ui.add_space(14.0);
    match src {
        Some(c) => super::armory::legacy_trees(ui, c, art),
        None => {
            ui.label(
                RichText::new(tr!(
                    "No Legacy recorded yet. It shows once a character logs in with the addon."
                ))
                .color(MUTED),
            );
        }
    }
}

/// Each member's level and crafts: what they bring to the House.
fn crafts(ui: &mut Ui, m: &Model, art: &mut Art) {
    ui.set_width(ui.available_width());
    ui.label(
        RichText::new(tr!("What each brings"))
            .font(theme::display_font(19.0))
            .color(INK),
    );
    ui.label(
        RichText::new(tr!("Their crafts as last seen; those taken to 150 are marked in brass."))
            .small()
            .color(MUTED),
    );
    ui.add_space(6.0);
    for c in &m.memory.characters {
        ui.horizontal(|ui| {
            ui.set_min_height(30.0);
            let (r, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            crest(ui, art, c, r, 1.0);
            let color = theme::class_color(&c.class_file);
            column(ui, 190.0, RichText::new(&c.name).size(16.0).color(color));
            column(ui, 80.0, RichText::new(tr!("level {n}", n = c.level)).color(MUTED));
            let profs = house::professions(c);
            if profs.is_empty() {
                ui.label(RichText::new(tr!("no crafts yet")).color(MUTED));
            }
            for s in &profs {
                chip(ui, &s.name, s.rank, s.max);
            }
        });
    }
}

/// A label in a fixed width, so rows line up.
fn column(ui: &mut Ui, w: f32, text: RichText) {
    ui.allocate_ui_with_layout(
        vec2(w, 24.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_width(w);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.label(text);
        },
    );
}

// ---- the history ----

const DAY_W: f32 = 170.0;
const RULE_W: f32 = 48.0;

fn history(
    ui: &mut Ui,
    m: &Model,
    st: &mut State,
    art: &mut Art,
    page: &mut Page,
    acc: &Account,
) {
    if acc.timeline.is_empty() {
        ui.label(RichText::new(tr!("Nothing has happened yet. It will.")).color(MUTED));
        return;
    }
    let mut days: Vec<(String, Vec<&Moment>)> = vec![];
    for mo in &acc.timeline {
        let day = diary::day_of(mo.t);
        match days.last_mut() {
            Some((d, v)) if *d == day => v.push(mo),
            _ => days.push((day, vec![mo])),
        }
    }
    let shown_id = ui.id().with("house-days");
    let shown = ui.data(|d| d.get_temp::<usize>(shown_id)).unwrap_or(6);
    let rule_x = ui.cursor().left() + DAY_W + RULE_W / 2.0;
    let rule = ui.painter().add(egui::Shape::Noop);
    let top = ui.cursor().top();
    for (_, moments) in days.iter().take(shown) {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                vec2(DAY_W, 30.0),
                egui::Layout::top_down(egui::Align::Max),
                |ui| {
                    ui.set_width(DAY_W - 16.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add_space(4.0);
                    let t = moments[0].t as f64;
                    let day = RichText::new(theme::day(t)).font(theme::display_font(18.0));
                    ui.label(day.color(GOLD));
                    let year = theme::local(t).format("%Y").to_string();
                    ui.label(RichText::new(year).small().color(MUTED));
                },
            );
            ui.add_space(RULE_W - ui.spacing().item_spacing.x);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for mo in moments {
                    moment(ui, m, st, art, page, mo, rule_x);
                }
            });
        });
        ui.add_space(16.0);
    }
    let bottom = ui.cursor().top() - 16.0;
    ui.painter().set(
        rule,
        egui::Shape::line_segment(
            [pos2(rule_x, top + 6.0), pos2(rule_x, bottom)],
            Stroke::new(2.0, Color32::from_rgba_unmultiplied(0xb0, 0x8a, 0x2e, 90)),
        ),
    );
    if days.len() > shown {
        ui.horizontal(|ui| {
            ui.add_space(DAY_W + RULE_W);
            let more = days.len() - shown;
            let text = if more == 1 {
                tr!("Show the day before").to_string()
            } else {
                tr!("Show earlier days ({n} more)", n = more)
            };
            if ui.link(RichText::new(text).color(GOLD)).clicked() {
                ui.data_mut(|d| d.insert_temp(shown_id, shown + 10));
            }
        });
    }
}

/// One moment: its line, a glimpse for letters, and its mark on the rule.
fn moment(
    ui: &mut Ui,
    m: &Model,
    st: &mut State,
    art: &mut Art,
    page: &mut Page,
    mo: &Moment,
    rule_x: f32,
) {
    let chars = &m.memory.characters;
    let who = mo.who.and_then(|i| chars.get(i));
    let color = |c: Option<&Character>| c.map(|c| theme::class_color(&c.class_file)).unwrap_or(INK);
    let n = ("name", who.map(first_name).unwrap_or_default(), color(who));
    let size = 16.5;
    let (job, icon, glimpse): (LayoutJob, Option<i64>, Option<String>) = match &mo.what {
        What::Joined { zone, first } => {
            let t = match (first, zone.is_empty()) {
                (true, false) => tr!("{name} first set foot in {zone}, and the House began."),
                (true, true) => tr!("{name} set out, and the House began."),
                (false, false) => tr!("{name} joined the House, in {zone}."),
                (false, true) => tr!("{name} joined the House."),
            };
            (rich(t, &[n, ("zone", zone, INK)], size), None, None)
        }
        What::Level(l) => {
            let icon = LEVEL
                .iter()
                .find(|(x, _)| x == l)
                .map(|(_, i)| *i)
                .unwrap_or(icons::SPIRIT);
            let lv = l.to_string();
            let job = rich(tr!("{name} reached level {n}."), &[n, ("n", &lv, GOLD)], size);
            (job, Some(icon), None)
        }
        What::FirstDeath { zone } => {
            let t = if zone.is_empty() {
                tr!("{name} died for the first time, and came back.")
            } else {
                tr!("{name} died for the first time, in {zone}, and came back.")
            };
            (rich(t, &[n, ("zone", zone, INK)], size), Some(FIRST_DEATH), None)
        }
        What::Mount { name: what, icon } => {
            let t = if who.is_some() {
                tr!("{name} brought {what} home to the stable.")
            } else {
                tr!("{what} came to the stable.")
            };
            (rich(t, &[n, ("what", what, TREASURE)], size), *icon, None)
        }
        What::Pet { name: what, icon } => {
            let t = if who.is_some() {
                tr!("{name} brought {what} home to the menagerie.")
            } else {
                tr!("{what} joined the menagerie.")
            };
            (rich(t, &[n, ("what", what, TREASURE)], size), *icon, None)
        }
        What::Letter { to, opening, .. } => {
            let other = chars.get(*to);
            let to = ("to", other.map(first_name).unwrap_or_default(), color(other));
            let job = rich(tr!("{name} wrote to {to}."), &[n, to], size);
            (job, Some(LETTER), Some(format!("“{opening}”")))
        }
        What::Deed { name: deed, icon } => {
            let job = rich(tr!("{name} earned the deed {deed}."), &[n, ("deed", deed, GOLD)], size);
            (job, Some(*icon), None)
        }
        What::Slew { name: foe } => {
            let job = rich(tr!("{name} brought down {foe}."), &[n, ("foe", foe, FOE)], size);
            (job, Some(icons::SKULL), None)
        }
    };
    let r = ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label(job);
                let clock = RichText::new(theme::clock(mo.t as f64)).small();
                ui.label(clock.color(MUTED.gamma_multiply(0.8)));
            });
            if let Some(g) = glimpse {
                let mut job = LayoutJob::single_section(
                    g,
                    TextFormat::simple(FontId::new(15.0, theme::italic()), MUTED),
                );
                job.wrap.max_width = ui.available_width().min(760.0);
                job.wrap.max_rows = 1;
                let g = ui.fonts_mut(|f| f.layout_job(job));
                ui.label(g);
            }
        });
    });
    // The moment's mark on the rule.
    let mark = Rect::from_center_size(
        pos2(rule_x, r.response.rect.top() + 17.0),
        Vec2::splat(30.0),
    );
    let p = ui.painter();
    p.circle_filled(mark.center(), 19.0, NIGHT);
    match (icon, who) {
        (Some(i), _) => {
            p.rect_filled(mark, 5.0, Color32::from_rgb(5, 7, 15));
            if let Some(t) = art.icon(ui.ctx(), Some(i)) {
                let uv = Rect::from_min_max(pos2(0.07, 0.07), pos2(0.93, 0.93));
                p.image(t.id(), mark.shrink(1.5), uv, Color32::WHITE);
            }
            p.rect_stroke(mark, 5.0, Stroke::new(1.2, BRASS), egui::StrokeKind::Inside);
        }
        (None, Some(c)) => crest(ui, art, c, mark, 1.2),
        (None, None) => {
            p.circle_filled(mark.center(), 5.0, BRASS);
        }
    }
    if let What::Letter { from_slug, to_slug, .. } = &mo.what {
        let resp = ui
            .interact(r.response.rect, ui.id().with(("letter", mo.t, from_slug)), Sense::click())
            .on_hover_text(tr!("Read the letter"));
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            st.letters.from = Some(from_slug.clone());
            st.letters.to = Some(to_slug.clone());
            *page = Page::Letters;
        }
    }
}
