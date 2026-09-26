//! Deeds: the feats a character has earned, from what was recorded, and
//! how far along the rest are.

use super::widgets;
use super::{card, character, label};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::deeds::{self, Deed, Mine, Status, Unit};
use crate::theme::{self, EDGE, GOLD, INK, MUTED, RAISED};
use crate::tr;
use egui::{Color32, Rect, RichText, Stroke, Ui, Vec2};
use std::collections::HashMap;

const RIBBON: i64 = 134411;
const CARD_W: f32 = 320.0;
const CARD_H: f32 = 120.0;
const GAP: f32 = 12.0;
const BRASS: Color32 = Color32::from_rgb(0xb0, 0x8a, 0x2e);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    All,
    Earned,
    Progress,
    Class,
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    let list = deeds::for_class(&c.class_file);
    let status = super::story::memo(ui, m, c, "deeds", || {
        let mine = Mine::new(m, c);
        list.iter()
            .map(|d| (d.id, d.eval(&mine)))
            .collect::<HashMap<&'static str, Status>>()
    });
    let get = |d: &Deed| {
        status.get(d.id).copied().unwrap_or(Status::Progress {
            have: 0.0,
            need: 1.0,
        })
    };
    let earned = |d: &Deed| matches!(get(d), Status::Earned(_));
    let class = theme::class_name(&c.class_file);
    let tab_id = ui.id().with("deeds-tab");
    let mut tab = ui.data(|d| d.get_temp::<Tab>(tab_id)).unwrap_or(Tab::All);

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            let total = list.len();
            let done = list.iter().filter(|d| earned(d)).count();
            let own: Vec<&Deed> = list.iter().filter(|d| d.class.is_some()).collect();
            let own_done = own.iter().filter(|d| earned(d)).count();
            let latest = list
                .iter()
                .filter_map(|d| match get(d) {
                    Status::Earned(Some(t)) => Some((t, d)),
                    _ => None,
                })
                .max_by(|a, b| a.0.total_cmp(&b.0));
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::figure_row(ui, |ui| {
                    widgets::figure_text(
                        ui,
                        art,
                        RIBBON,
                        &tr!("{n} of {total}", n = done, total = total),
                        tr!("deeds earned"),
                    );
                    if let Some(first) = own.first() {
                        widgets::figure_text(
                            ui,
                            art,
                            first.icon,
                            &tr!("{n} of {total}", n = own_done, total = own.len()),
                            &tr!("{class} deeds", class = class),
                        );
                    }
                    if let Some((t, d)) = latest {
                        widgets::figure_text(
                            ui,
                            art,
                            d.icon,
                            d.name,
                            &tr!("most recent, {when}", when = theme::day(t)),
                        );
                    }
                });
                ui.add_space(6.0);
                widgets::bar(
                    ui,
                    done as f32 / total.max(1) as f32,
                    GOLD,
                    ui.available_width(),
                );
            });
            ui.add_space(14.0);
            let class_title = tr!("{class} deeds", class = class);
            ui.horizontal(|ui| {
                for (t, name) in [
                    (Tab::All, tr!("All")),
                    (Tab::Earned, tr!("Earned")),
                    (Tab::Progress, tr!("In progress")),
                    (Tab::Class, class_title.as_str()),
                ] {
                    let on = tab == t;
                    let text = RichText::new(name).color(if on { INK } else { MUTED });
                    let stroke = if on { EDGE } else { Color32::TRANSPARENT };
                    if ui
                        .add(
                            egui::Button::new(text)
                                .fill(if on { RAISED } else { Color32::TRANSPARENT })
                                .stroke(Stroke::new(1.0, stroke)),
                        )
                        .clicked()
                    {
                        tab = t;
                    }
                }
            });
            ui.add_space(10.0);

            let show = |d: &&Deed| match tab {
                Tab::All => true,
                Tab::Earned => earned(d),
                Tab::Progress => !earned(d),
                Tab::Class => d.class.is_some(),
            };
            // Earned first, oldest first; then the nearest to done.
            let key = |d: &Deed| match get(d) {
                Status::Earned(t) => (0, t.unwrap_or(0.0)),
                Status::Progress { have, need } => (1, -(have / need.max(1.0))),
            };
            let pick = |class: bool| {
                let mut v: Vec<&Deed> = list
                    .iter()
                    .filter(|d| d.class.is_some() == class)
                    .filter(show)
                    .collect();
                v.sort_by(|a, b| {
                    let (x, y) = (key(a), key(b));
                    x.0.cmp(&y.0).then(x.1.total_cmp(&y.1))
                });
                v
            };
            let sections = [
                (class_title.clone(), pick(true)),
                (tr!("Deeds for everyone").to_string(), pick(false)),
            ];
            if sections.iter().all(|(_, v)| v.is_empty()) {
                super::empty(
                    ui,
                    if tab == Tab::Earned {
                        tr!("No deeds yet. They come with time.")
                    } else {
                        tr!("Every deed here is done.")
                    },
                );
            }
            for (title, v) in sections {
                if v.is_empty() {
                    continue;
                }
                label(ui, &title);
                ui.add_space(4.0);
                grid(ui, art, &v, &get);
                ui.add_space(18.0);
            }
        });
    ui.data_mut(|d| d.insert_temp(tab_id, tab));
}

fn grid(ui: &mut Ui, art: &mut Art, v: &[&Deed], get: &dyn Fn(&Deed) -> Status) {
    let avail = ui.available_width();
    let cols = (((avail + GAP) / (CARD_W + GAP)).floor() as usize).max(1);
    let w = (avail - GAP * (cols - 1) as f32) / cols as f32;
    for row in v.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for d in row {
                deed_card(ui, art, d, get(d), w);
            }
        });
        ui.add_space(GAP - ui.spacing().item_spacing.y);
    }
}

fn deed_card(ui: &mut Ui, art: &mut Art, d: &Deed, s: Status, w: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, CARD_H), egui::Sense::hover());
    let done = matches!(s, Status::Earned(_));
    let p = ui.painter();
    if done {
        p.rect_filled(
            rect.translate(egui::vec2(0.0, 3.0)).expand(1.0),
            10.0,
            Color32::from_black_alpha(70),
        );
        p.rect_filled(rect, 10.0, Color32::from_rgb(0x1d, 0x1d, 0x30));
        // A warm light from the top, like a lit plaque.
        let r = Rect::from_min_max(
            rect.min + egui::vec2(1.0, 1.0),
            egui::pos2(rect.max.x - 1.0, rect.center().y),
        );
        let glow = Color32::from_rgba_unmultiplied(0xff, 0xd1, 0x00, 18);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(r.left_top(), glow);
        mesh.colored_vertex(r.right_top(), glow);
        mesh.colored_vertex(r.right_bottom(), Color32::TRANSPARENT);
        mesh.colored_vertex(r.left_bottom(), Color32::TRANSPARENT);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(mesh);
        p.rect_stroke(
            rect,
            10.0,
            Stroke::new(1.5, BRASS),
            egui::StrokeKind::Inside,
        );
    } else {
        p.rect_filled(rect, 10.0, Color32::from_rgb(0x0c, 0x10, 0x22));
        p.rect_stroke(
            rect,
            10.0,
            Stroke::new(1.0, Color32::from_rgb(0x1f, 0x25, 0x45)),
            egui::StrokeKind::Inside,
        );
    }

    let icon = Rect::from_min_size(rect.min + egui::vec2(14.0, 16.0), Vec2::splat(52.0));
    p.rect_filled(icon, 6.0, Color32::from_rgb(5, 7, 15));
    if let Some(t) = art.icon(ui.ctx(), Some(d.icon)) {
        p.image(
            t.id(),
            icon.shrink(2.0),
            Rect::from_min_max(egui::pos2(0.07, 0.07), egui::pos2(0.93, 0.93)),
            if done {
                Color32::WHITE
            } else {
                Color32::from_gray(80)
            },
        );
    }
    let edge = if done {
        GOLD
    } else {
        Color32::from_rgb(0x3a, 0x3f, 0x5a)
    };
    p.rect_stroke(icon, 6.0, Stroke::new(1.5, edge), egui::StrokeKind::Inside);

    let text = Rect::from_min_max(
        egui::pos2(icon.right() + 14.0, rect.top() + 12.0),
        egui::pos2(rect.right() - 14.0, rect.bottom() - 30.0),
    );
    let mut ui_text = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    {
        let ui = &mut ui_text;
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let name = if done {
            GOLD
        } else {
            Color32::from_rgb(0xb8, 0xb2, 0x9e)
        };
        ui.label(
            RichText::new(d.name)
                .font(theme::display_font(17.0))
                .color(name),
        );
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.label(
            RichText::new(d.text)
                .small()
                .color(if done { INK } else { MUTED }),
        );
    }
    // The foot of the card: when it was earned, or how far along it is.
    let foot = Rect::from_min_max(
        egui::pos2(text.left(), rect.bottom() - 30.0),
        egui::pos2(text.right(), rect.bottom() - 8.0),
    );
    let mut ui_foot = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(foot)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let ui = &mut ui_foot;
    match s {
        Status::Earned(t) => {
            let when = match t {
                Some(t) => tr!("Earned {when}", when = theme::when(t)),
                None => tr!("Earned before the records began").to_string(),
            };
            ui.label(RichText::new(when).small().color(BRASS));
        }
        Status::Progress { have, need } => {
            let fmt = |x: f64| match d.unit {
                Unit::Count => theme::thousands(x as i64),
                Unit::Copper => theme::money_short(x as i64),
                Unit::Seconds => theme::duration(x),
            };
            let caption = format!("{} / {}", fmt(have.min(need)), fmt(need));
            // The caption first, from the right, so the bar takes what is left.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(RichText::new(caption).small().color(MUTED));
                ui.add_space(4.0);
                widgets::bar(
                    ui,
                    (have / need.max(1.0)) as f32,
                    Color32::from_rgb(0x8a, 0x76, 0x3e),
                    ui.available_width().max(40.0),
                );
            });
        }
    }
}
