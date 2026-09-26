//! Small pieces that make the app feel like part of the game: coin money,
//! share bars, sortable full-width tables, figures with icons, quest colours.

use crate::art::Art;
use crate::theme::{self, GOLD, INK, MUTED};
use egui::{Align, Color32, Layout, RichText, Sense, Stroke, Ui, Vec2};
use egui_extras::{Column, TableBuilder};

/// File IDs of game art used as UI icons (all in this client).
pub mod icons {
    pub const GOLD: i64 = 237618;
    pub const SILVER: i64 = 237620;
    pub const COPPER: i64 = 237617;
    pub const WATCH: i64 = 134376;
    pub const SPIRIT: i64 = 135946;
    pub const NOTE: i64 = 134327;
    pub const SKULL: i64 = 133730;
    pub const FEIGN: i64 = 132293;
    pub const GROUP: i64 = 134149;
    pub const COIN: i64 = 133784;
    pub const COINS: i64 = 133785;
    pub const CHEST: i64 = 132627;
    pub const BOOK: i64 = 133743;
    pub const SCROLL: i64 = 134939;
    pub const MAP: i64 = 134269;
    pub const SWORDS: i64 = 132147;
    pub const BAG: i64 = 133639;
    pub const HEAL: i64 = 135942;
    pub const ALL: &[i64] = &[
        GOLD, SILVER, COPPER, WATCH, SPIRIT, NOTE, SKULL, FEIGN, GROUP, COIN, COINS, CHEST, BOOK,
        SCROLL, MAP, SWORDS, BAG, HEAL,
    ];
}

fn icon_image(ui: &mut Ui, art: &mut Art, id: i64, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if let Some(t) = art.icon(ui.ctx(), Some(id)) {
        ui.painter().image(
            t.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

/// Money as the game shows it: 12 [gold] 34 [silver] 5 [copper].
pub fn coins(ui: &mut Ui, art: &mut Art, copper: i64, size: f32) -> egui::Response {
    let neg = copper < 0;
    let c = copper.abs();
    let parts = [
        (c / 10000, icons::GOLD),
        (c / 100 % 100, icons::SILVER),
        (c % 100, icons::COPPER),
    ];
    let first = parts.iter().position(|(v, _)| *v > 0).unwrap_or(2);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        if neg {
            ui.label(RichText::new("−").size(size).color(INK));
        }
        for &(v, icon) in &parts[first..] {
            ui.label(RichText::new(v.to_string()).size(size).color(INK));
            icon_image(ui, art, icon, size * 0.8);
            ui.add_space(size * 0.25);
        }
    })
    .response
}

/// A thin share bar with rounded ends; `share` in 0..=1.
pub fn bar(ui: &mut Ui, share: f32, color: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 16.0), Sense::hover());
    let track = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 6.0));
    ui.painter()
        .rect_filled(track, 3.0, Color32::from_rgb(0x07, 0x0a, 0x18));
    let mut fill = track;
    fill.set_width((track.width() * share.clamp(0.0, 1.0)).max(if share > 0.0 {
        6.0
    } else {
        0.0
    }));
    ui.painter().rect_filled(fill, 3.0, color);
}

/// A number with a caption, and a game icon to its left.
pub fn figure(
    ui: &mut Ui,
    art: &mut Art,
    icon: i64,
    value: impl FnOnce(&mut Ui, &mut Art),
    caption: &str,
) {
    // A fixed-height row, everything centred in it, so figures line up
    // whatever their value (text or coins) and caption look like.
    const H: f32 = 44.0;
    ui.horizontal(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.set_min_height(H);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 6.0, Color32::from_rgb(5, 7, 15));
        if let Some(t) = art.icon(ui.ctx(), Some(icon)) {
            p.image(
                t.id(),
                rect.shrink(2.0),
                egui::Rect::from_min_max(egui::pos2(0.07, 0.07), egui::pos2(0.93, 0.93)),
                Color32::WHITE,
            );
        }
        p.rect_stroke(
            rect,
            6.0,
            Stroke::new(1.0, Color32::from_rgb(0x6b, 0x5a, 0x2e)),
            egui::StrokeKind::Inside,
        );
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                value(ui, art)
            });
            ui.label(RichText::new(caption).small().color(MUTED));
        });
    });
}

/// A wrapping row of figures. Items are top-aligned: all figures are the
/// same height, so they line up exactly (centring against a growing row
/// height would make them step downwards).
pub fn figure_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.with_layout(
        Layout::left_to_right(Align::Min).with_main_wrap(true),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(34.0, 12.0);
            add(ui)
        },
    );
}

pub fn figure_text(ui: &mut Ui, art: &mut Art, icon: i64, value: &str, caption: &str) {
    figure(
        ui,
        art,
        icon,
        |ui, _| {
            ui.label(
                RichText::new(value)
                    .font(theme::display_font(24.0))
                    .color(INK),
            );
        },
        caption,
    );
}

pub fn figure_money(ui: &mut Ui, art: &mut Art, icon: i64, copper: i64, caption: &str) {
    figure(
        ui,
        art,
        icon,
        |ui, art| {
            coins(ui, art, copper, 20.0);
        },
        caption,
    );
}

/// The quest log's difficulty colours: red, orange, yellow, green, grey.
pub fn quest_color(quest: i64, player: i64) -> Color32 {
    let d = quest - player;
    let grey_below = if player <= 5 {
        0
    } else if player <= 39 {
        player - player / 10 - 5
    } else {
        player - player / 5 - 1
    };
    if d >= 5 {
        Color32::from_rgb(0xff, 0x20, 0x20)
    } else if d >= 3 {
        Color32::from_rgb(0xff, 0x80, 0x40)
    } else if d >= -2 {
        Color32::from_rgb(0xff, 0xff, 0x00)
    } else if quest > grey_below {
        Color32::from_rgb(0x40, 0xc0, 0x40)
    } else {
        Color32::from_rgb(0x80, 0x80, 0x80)
    }
}

// ---- tables ----

/// A value to sort a column by.
pub enum Key {
    Num(f64),
    Text(String),
}

pub struct Col {
    pub title: &'static str,
    /// Grows to fill the table; otherwise sized to content (at least `min`).
    pub grow: bool,
    pub min: f32,
    pub right: bool,
}

impl Col {
    pub fn grow(title: &'static str) -> Self {
        Col {
            title,
            grow: true,
            min: 140.0,
            right: false,
        }
    }
    pub fn fit(title: &'static str, min: f32) -> Self {
        Col {
            title,
            grow: false,
            min,
            right: false,
        }
    }
    pub fn num(title: &'static str, min: f32) -> Self {
        Col {
            title,
            grow: false,
            min,
            right: true,
        }
    }
}

/// A full-width table: click a header to sort by it, again to flip.
/// `key(row, col)` gives the sort value, `cell(ui, row, col)` draws a cell.
pub fn table<R>(
    ui: &mut Ui,
    id: &str,
    cols: &[Col],
    rows: &[R],
    default: (usize, bool),
    row_h: f32,
    key: impl Fn(&R, usize) -> Key,
    mut cell: impl FnMut(&mut Ui, &R, usize),
) {
    let state_id = ui.id().with(("table-sort", id));
    let (sort, desc) = ui
        .data(|d| d.get_temp::<(usize, bool)>(state_id))
        .unwrap_or(default);
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| {
        let o = match (key(&rows[a], sort), key(&rows[b], sort)) {
            (Key::Num(x), Key::Num(y)) => x.total_cmp(&y),
            (Key::Text(x), Key::Text(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
            _ => std::cmp::Ordering::Equal,
        };
        if desc { o.reverse() } else { o }
    });
    ui.push_id(id, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let mut t = TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .vscroll(false)
            .cell_layout(Layout::left_to_right(Align::Center));
        for c in cols {
            t = t.column(if c.grow {
                Column::remainder().at_least(c.min).clip(true)
            } else {
                Column::exact(c.min).clip(true)
            }); // fixed widths: no resizing as rows arrive
        }
        let mut clicked = None;
        t.header(26.0, |mut h| {
            for (i, c) in cols.iter().enumerate() {
                h.col(|ui| {
                    let arrow = if i == sort {
                        if desc { " ▼" } else { " ▲" }
                    } else {
                        ""
                    };
                    let layout = if c.right {
                        Layout::right_to_left(Align::Center)
                    } else {
                        Layout::left_to_right(Align::Center)
                    };
                    ui.with_layout(layout, |ui| {
                        let text = RichText::new(format!("{}{arrow}", c.title))
                            .small()
                            .color(if i == sort { GOLD } else { MUTED });
                        if ui
                            .add(egui::Label::new(text).sense(Sense::click()))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            clicked = Some(i);
                        }
                    });
                });
            }
        })
        .body(|body| {
            body.rows(row_h, order.len(), |mut row| {
                let r = &rows[order[row.index()]];
                for (i, c) in cols.iter().enumerate() {
                    row.col(|ui| {
                        if c.right {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                cell(ui, r, i)
                            });
                        } else {
                            cell(ui, r, i);
                        }
                    });
                }
            });
        });
        if let Some(i) = clicked {
            let next = if i == sort {
                (i, !desc)
            } else {
                (i, cols[i].right)
            };
            ui.data_mut(|d| d.insert_temp(state_id, next));
        }
    });
}

/// A section title in label gold with a faint rule running out to the right.
pub fn title(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(text)
                .font(theme::display_font(18.0))
                .color(GOLD),
        );
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 10.0), Sense::hover());
        let y = rect.center().y;
        let mut mesh = egui::Mesh::default();
        let a = Color32::from_rgba_unmultiplied(0xb0, 0x8a, 0x2e, 90);
        let z = Color32::from_rgba_unmultiplied(0xb0, 0x8a, 0x2e, 0);
        let r = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 6.0, y - 0.5),
            egui::pos2(rect.right(), y + 0.5),
        );
        mesh.colored_vertex(r.left_top(), a);
        mesh.colored_vertex(r.right_top(), z);
        mesh.colored_vertex(r.right_bottom(), z);
        mesh.colored_vertex(r.left_bottom(), a);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        ui.painter().add(mesh);
    });
}

pub const INK_BROWN: Color32 = Color32::from_rgb(0x2e, 0x1d, 0x0e);
pub const INK_RED: Color32 = Color32::from_rgb(0x6b, 0x1e, 0x0a);

/// Content on the Classic quest log's parchment (the paper fills the
/// top-left 301x409 of that 512x512 texture). Write on it in INK_BROWN.
pub fn parchment(ui: &mut Ui, art: &mut Art, max_width: f32, add: impl FnOnce(&mut Ui)) {
    let bg = ui.painter().add(egui::Shape::Noop);
    let frame = egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 30,
            right: 34,
            top: 26,
            bottom: 30,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width().min(max_width));
            add(ui);
        });
    let rect = frame.response.rect;
    let shape = match art.icon(ui.ctx(), Some(3450737)) {
        Some(tex) => {
            let mut mesh = egui::Mesh::with_texture(tex.id());
            mesh.add_rect_with_uv(
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.012), egui::pos2(0.586, 0.796)),
                Color32::WHITE,
            );
            egui::Shape::mesh(mesh)
        }
        None => egui::Shape::rect_filled(rect, 6.0, Color32::from_rgb(0xe3, 0xc9, 0x93)),
    };
    ui.painter().set(bg, shape);
}
