pub mod armory;
pub mod chronicle;
pub mod combat;
pub mod dead;
pub mod deeds;
pub mod diary;
pub mod economy;
pub mod house;
pub mod journal;
pub mod letters;
pub mod map;
pub mod overview;
pub mod players;
pub mod quests;
pub mod rp;
pub mod settings;
pub mod standing;
pub mod story;
pub mod widgets;

use crate::art::Art;
use crate::data::Model;
use crate::data::memory::{Character, QuestStatus, parse_link};
use crate::theme::{self, EDGE, GOLD, INK, MUTED, PANEL, RAISED};
use crate::tr;
use crate::{Page, State};
use egui::{Color32, RichText, Stroke, Ui, Vec2};
use serde_json::Value;

pub fn character<'a>(m: &'a Model, st: &State) -> &'a Character {
    &m.memory.characters[st.character.min(m.memory.characters.len() - 1)]
}

pub fn top_bar(
    ui: &mut Ui,
    m: &Model,
    st: &mut State,
    page: &mut Page,
    art: &mut Art,
    loading: bool,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Forever Memory")
                .font(theme::display_font(22.0))
                .color(GOLD),
        );
        ui.add_space(28.0);
        for (i, c) in m.memory.characters.iter().enumerate() {
            let selected = i == st.character;
            if let Some(tex) = art.get(
                ui.ctx(),
                &format!("class-{}.png", c.class_file.to_lowercase()),
            ) {
                ui.add(
                    egui::Image::new(&tex)
                        .fit_to_exact_size(Vec2::splat(22.0))
                        .corner_radius(4),
                );
            }
            let text = RichText::new(format!("{}  {}", c.name, c.level))
                .size(16.0)
                .color(if selected {
                    theme::class_color(&c.class_file)
                } else {
                    MUTED
                });
            if ui
                .add(egui::Button::new(text).fill(if selected {
                    RAISED
                } else {
                    Color32::TRANSPARENT
                }))
                .clicked()
            {
                st.character = i;
                st.quest = None;
                st.session = None;
                st.map_zone = None;
                st.fight = None;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let status = if loading {
                tr!("reading…").to_string()
            } else {
                tr!(
                    "read {when}",
                    when = theme::ago(
                        m.loaded_at
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs_f64())
                            .unwrap_or(0.0)
                    )
                )
            };
            ui.label(RichText::new(status).small().color(MUTED))
                .on_hover_text(tr!("Reloads on its own after each save; F5 reloads now"));
            ui.add_space(12.0);
            search(ui, m, st, page);
        });
    });
}

/// One box that finds quests, players, items and zones anywhere (Ctrl+K).
/// A search result: its kind, its text, and what picking it does.
type SearchHit = (String, String, Box<dyn Fn(&mut State, &mut Page)>);

fn search(ui: &mut Ui, m: &Model, st: &mut State, page: &mut Page) {
    let id = egui::Id::new("global-search");
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::K)) {
        ui.memory_mut(|mem| mem.request_focus(id));
    }
    let resp = ui.add(
        egui::TextEdit::singleline(&mut st.search)
            .id(id)
            .hint_text(tr!("Search quests, players, items  (Ctrl+K)"))
            .desired_width(320.0),
    );
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        st.search.clear();
    }
    // Results stay open while there is a query: clicking one takes the focus
    // from the field before the click lands.
    let q = st.search.trim().to_lowercase();
    if q.len() < 2 {
        return;
    }
    let c = character(m, st);
    let mut hits: Vec<SearchHit> = vec![];
    for quest in &c.quests {
        if quest.title.to_lowercase().contains(&q) {
            let id = quest.id;
            let tab = match quest.status {
                QuestStatus::Active => 0,
                QuestStatus::Completed => 1,
                _ => 2,
            };
            hits.push((
                tr!("Quest").into(),
                quest.title.clone(),
                Box::new(move |s, p| {
                    s.quest = Some(id);
                    s.quest_tab = tab;
                    *p = Page::Quests;
                }),
            ));
        }
    }
    for p in &m.players {
        if p.name.to_lowercase().contains(&q) {
            let name = p.name.clone();
            hits.push((
                tr!("Player").into(),
                p.name.clone(),
                Box::new(move |s, pg| {
                    s.player = Some(name.clone());
                    *pg = Page::Players;
                }),
            ));
        }
    }
    let mut items: Vec<_> = m
        .memory
        .items
        .values()
        .filter(|i| i.name.to_lowercase().contains(&q))
        .map(|i| i.name.clone())
        .collect();
    items.sort();
    items.dedup();
    for name in items.into_iter().take(6) {
        let n = name.clone();
        hits.push((
            tr!("Item").into(),
            name,
            Box::new(move |s, p| {
                s.loot_search = n.clone();
                *p = Page::Economy;
            }),
        ));
    }
    hits.truncate(12);
    let area = egui::Area::new(id.with("results"))
        .order(egui::Order::Foreground)
        .fixed_pos(resp.rect.left_bottom() + egui::vec2(0.0, 6.0));
    area.show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).fill(PANEL).show(ui, |ui| {
            ui.set_width(resp.rect.width().max(320.0));
            if hits.is_empty() {
                ui.label(RichText::new(tr!("Nothing found")).color(MUTED));
            }
            for (kind, label, go) in &hits {
                let r = ui.add(
                    egui::Button::new(RichText::new(label.as_str()))
                        .fill(Color32::TRANSPARENT)
                        .right_text(RichText::new(kind.as_str()).small().color(MUTED))
                        .min_size(egui::vec2(ui.available_width(), 28.0)),
                );
                if r.clicked() {
                    go(st, page);
                    st.search.clear();
                }
            }
        });
    });
}

pub fn empty(ui: &mut Ui, text: &str) {
    ui.add_space(40.0);
    ui.label(RichText::new(text).color(MUTED).size(17.0));
}

pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(theme::display_font(24.0))
            .color(INK),
    );
}

/// A section title in the client's label gold, with a fading rule.
pub fn label(ui: &mut Ui, text: &str) {
    widgets::title(ui, text);
}

/// Two equal columns; each card inside fills its column, padding included.
pub fn pair(ui: &mut Ui, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    // Narrow windows (a tiled half screen) stack the two instead.
    if ui.available_width() < 900.0 {
        left(ui);
        ui.add_space(14.0);
        right(ui);
        return;
    }
    ui.columns(2, |c| {
        c[0].spacing_mut().item_spacing.x = 14.0;
        left(&mut c[0]);
        right(&mut c[1]);
    });
}

/// Two equal columns drawn by one closure (column 0 or 1), for content that
/// needs the same mutable state on both sides.
pub fn pair_by(ui: &mut Ui, mut add: impl FnMut(&mut Ui, usize)) {
    if ui.available_width() < 900.0 {
        add(ui, 0);
        ui.add_space(14.0);
        add(ui, 1);
        return;
    }
    ui.columns(2, |c| {
        let (a, b) = c.split_at_mut(1);
        add(&mut a[0], 0);
        add(&mut b[0], 1);
    });
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, Color32::from_rgb(0x34, 0x3a, 0x62)))
        .corner_radius(10)
        .inner_margin(egui::Margin::same(18))
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(90),
        })
        .show(ui, add)
        .inner
}

/// An item or spell icon with a quality-coloured border.
pub fn icon(
    ui: &mut Ui,
    art: &mut Art,
    icon: Option<i64>,
    border: Color32,
    size: f32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 5.0, Color32::from_rgb(5, 7, 15));
    if let Some(tex) = art.icon(ui.ctx(), icon) {
        p.image(
            tex.id(),
            rect.shrink(2.0),
            egui::Rect::from_min_max(egui::pos2(0.07, 0.07), egui::pos2(0.93, 0.93)),
            Color32::WHITE,
        );
    }
    p.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.5, border),
        egui::StrokeKind::Inside,
    );
    resp
}

/// Tooltip lines as the game draws them ({l, lc, r, rc} rows).
pub fn tooltip_lines(ui: &mut Ui, lines: &Value) {
    for (i, (_, line)) in crate::data::memory::entries(lines).into_iter().enumerate() {
        let get = |k: &str| line.get(k).and_then(Value::as_str).unwrap_or("");
        let color = |k: &str| hex(get(k)).unwrap_or(INK);
        if get("l").trim().is_empty() && get("r").is_empty() {
            continue;
        }
        ui.horizontal(|ui| {
            let mut left = RichText::new(get("l")).color(color("lc"));
            if i == 0 {
                left = left.size(17.0);
            }
            ui.label(left);
            if !get("r").is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(get("r")).color(color("rc")));
                });
            }
        });
    }
}

pub fn hex(s: &str) -> Option<Color32> {
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// An item link as icon + quality-coloured name.
pub fn item_link(ui: &mut Ui, m: &Model, art: &mut Art, link: &str, size: f32) -> egui::Response {
    let l = parse_link(link).unwrap_or_default();
    let info = m.memory.items.get(&l.id);
    let name = if l.name.is_empty() {
        info.map(|i| i.name.clone())
            .unwrap_or_else(|| tr!("Item {id}", id = l.id))
    } else {
        l.name.clone()
    };
    let q = l.quality.or(info.and_then(|i| i.quality));
    ui.horizontal(|ui| {
        if let Some(i) = info.and_then(|i| i.icon) {
            icon(ui, art, Some(i), theme::quality(q), size);
        }
        ui.label(RichText::new(name).color(theme::quality(q)));
    })
    .response
}

/// Charts share one quiet style: no dragging or zooming, recessive grid.
pub fn plot(id: &str) -> egui_plot::Plot<'static> {
    egui_plot::Plot::new(id.to_string())
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .show_background(false)
        .grid_color(Color32::from_rgba_unmultiplied(0x28, 0x30, 0x5a, 110))
        .y_axis_min_width(36.0)
}

/// A row of toggles; `hidden` holds the ones switched off.
pub fn chips(
    ui: &mut Ui,
    all: &[&'static str],
    hidden: &mut std::collections::HashSet<&'static str>,
) {
    ui.horizontal_wrapped(|ui| {
        for &c in all {
            let on = !hidden.contains(c);
            let text = RichText::new(crate::i18n::t(c)).color(if on { INK } else { MUTED });
            if ui
                .add(
                    egui::Button::new(text)
                        .fill(if on { RAISED } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, EDGE)),
                )
                .clicked()
            {
                if on {
                    hidden.insert(c);
                } else {
                    hidden.remove(c);
                }
            }
        }
    });
}

/// The plot point under the pointer, near a data point or not.
/// Hover text for a bar chart as a real tooltip: the chart's own label is
/// drawn inside the plot and cut off at its edges above tall bars.
pub fn bar_tip<R>(
    r: &egui_plot::PlotResponse<R>,
    text: impl FnOnce(egui_plot::PlotPoint) -> Option<String>,
) {
    let Some(pos) = r.response.hover_pos() else {
        return;
    };
    if let Some(t) = text(r.transform.value_from_position(pos)) {
        r.response.clone().on_hover_text_at_pointer(t);
    }
}

pub fn hover(h: &egui_plot::HoverPosition<'_>) -> egui_plot::PlotPoint {
    match h {
        egui_plot::HoverPosition::NearDataPoint { position, .. } => *position,
        egui_plot::HoverPosition::Elsewhere { position } => *position,
    }
}
