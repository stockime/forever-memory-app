//! Help: what Forever Memory is, how it records, and where to find what.
//! It opens on the very first start, before the setup.

use super::{card, label};
use crate::art::Art;
use crate::theme::{self, GOLD, INK, MUTED};
use crate::tr;
use crate::Page;
use egui::{Color32, Rect, RichText, Sense, Ui, Vec2, pos2, vec2};

pub fn show(ui: &mut Ui, art: &mut Art, page: &mut Page, characters: usize) {
    let s = crate::config::get();
    if !s.help_seen {
        let _ = crate::config::update(|s| s.help_seen = true);
    }
    let ready = characters > 0 && s.onboarded;
    egui::ScrollArea::vertical()
        .id_salt("help")
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.set_max_width(1040.0);
            hero(ui, art, page, ready);
            ui.add_space(22.0);
            label(ui, tr!("How it works"));
            ui.add_space(4.0);
            steps(ui, art);
            ui.add_space(22.0);
            label(ui, tr!("Where to find what"));
            ui.add_space(4.0);
            tour(ui, art, page, characters > 0);
            ui.add_space(22.0);
            super::pair(
                ui,
                |ui| {
                    card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        label(ui, tr!("Who it's for"));
                        para(ui, tr!("Roleplayers who want their character's story kept, and anyone who likes to look back on the road so far."));
                        para(ui, tr!("Streamers: \"Previously on…\" on the Diary page reads the last entry aloud in your character's voice while the stream is starting."));
                    });
                },
                |ui| {
                    card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        label(ui, tr!("Your data stays yours"));
                        para(ui, tr!("Everything is kept on this computer, in a folder you choose. The diary is written by the AI tool you already use, from the recorded facts alone; other players' chat is never sent."));
                        para(ui, tr!("Voices need an ElevenLabs key and backups your own storage. Both are optional."));
                    });
                },
            );
            ui.add_space(14.0);
            ui.label(
                RichText::new(tr!("Ctrl+K searches quests, players and items from anywhere. F5 reads everything again. This page stays under Help, next to Settings."))
                    .family(theme::italic())
                    .size(15.5)
                    .color(MUTED),
            );
            ui.add_space(24.0);
        });
}

fn para(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(16.0).color(INK));
    ui.add_space(6.0);
}

/// The banner: the Eastern Kingdoms, the name, what it is, and the way on.
fn hero(ui: &mut Ui, art: &mut Art, page: &mut Page, ready: bool) {
    let w = ui.available_width();
    let h = 300.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 10.0, Color32::from_rgb(4, 6, 14));
    if let Some(tex) = art.icon(ui.ctx(), Some(7963776)) {
        // The art's band, cut to the banner's shape from its right side.
        let aspect = 2992.0 / 1152.0;
        let vw = ((w / h) / aspect).min(1.0);
        let uv = Rect::from_min_max(pos2(1.0 - vw, 0.2), pos2(1.0, 0.8));
        p.image(tex.id(), rect, uv, Color32::from_gray(170));
    }
    // Night falls from the left, where the words are.
    let mut mesh = egui::Mesh::default();
    let mid = rect.left() + w * 0.62;
    for (x, a) in [(rect.left(), 245), (mid, 150), (rect.right(), 30)] {
        mesh.colored_vertex(pos2(x, rect.top()), Color32::from_rgba_unmultiplied(4, 6, 14, a));
        mesh.colored_vertex(pos2(x, rect.bottom()), Color32::from_rgba_unmultiplied(4, 6, 14, a));
    }
    for i in [0u32, 2] {
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i + 1, i + 2, i + 3);
    }
    p.add(mesh);
    p.rect_stroke(rect, 10.0, egui::Stroke::new(1.0, Color32::from_rgb(0x34, 0x3a, 0x62)), egui::StrokeKind::Inside);

    let inner = rect.shrink2(vec2(34.0, 30.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(inner.min, vec2(inner.width().min(620.0), inner.height()))));
    child.label(RichText::new("Forever Memory").font(theme::display_font(46.0)).color(GOLD));
    child.label(
        RichText::new(tr!("Your character remembers."))
            .family(theme::italic())
            .size(22.0)
            .color(INK),
    );
    child.add_space(14.0);
    child.label(
        RichText::new(tr!("A chronicler for your World of Warcraft characters. While you play, it keeps what they live: where they went, whom they met, what they fought, found and said. Afterwards it gives it back as their story, in a diary they write themselves, and as a record you can look through day by day."))
            .size(16.5)
            .color(INK),
    );
    child.add_space(16.0);
    let (text, to) = if ready {
        (tr!("Show my characters"), Page::Overview)
    } else {
        (tr!("Set it up"), Page::Settings)
    };
    let button = egui::Button::new(RichText::new(text).size(17.0).color(GOLD))
        .min_size(vec2(0.0, 36.0))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x8a, 0x6d, 0x2c)));
    if child.add(button).clicked() {
        *page = to;
    }
}

/// The three things that happen, in the order they happen.
fn steps(ui: &mut Ui, art: &mut Art) {
    let steps = [
        (
            super::widgets::icons::NOTE,
            tr!("While you play"),
            tr!("The armory addon notes what your character does: quests, loot, gold, places, the people met and what they said. The game itself logs every fight."),
        ),
        (
            super::widgets::icons::CHEST,
            tr!("When the game saves"),
            tr!("Forever Memory copies it into your archive, a folder that keeps every day for good, with its history and an optional backup. It keeps doing so while the app is closed, if you let it."),
        ),
        (
            super::widgets::icons::SCROLL,
            tr!("Whenever you like"),
            tr!("Look back: the day in your character's own words, read aloud if you wish, their chronicle and letters, and every number behind them."),
        ),
    ];
    let narrow = ui.available_width() < 900.0;
    let gap = 14.0;
    let w = if narrow { ui.available_width() } else { (ui.available_width() - 2.0 * gap) / 3.0 };
    let draw = |ui: &mut Ui, art: &mut Art, i: usize| {
        let (icon, title, text) = steps[i];
        ui.allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            card(ui, |ui| {
                ui.set_width(w - 38.0);
                ui.set_min_height(if narrow { 0.0 } else { 150.0 });
                ui.horizontal(|ui| {
                    frame_icon(ui, art, icon, 36.0);
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("{}", i + 1)).color(MUTED).small());
                        ui.label(RichText::new(title).font(theme::display_font(20.0)).color(INK));
                    });
                });
                ui.add_space(8.0);
                ui.label(RichText::new(text).size(15.5).color(INK));
            });
        });
    };
    if narrow {
        for i in 0..3 {
            draw(ui, art, i);
            ui.add_space(gap);
        }
    } else {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for i in 0..3 {
                draw(ui, art, i);
            }
        });
    }
}

/// Every page, grouped by what it is for; each one opens on a click once
/// there is a character to show.
fn tour(ui: &mut Ui, art: &mut Art, page: &mut Page, open: bool) {
    let groups: [(&str, &[(Page, &str)]); 3] = [
        (
            tr!("Their story"),
            &[
                (Page::Diary, tr!("A daily entry your character writes about what they lived, and can read aloud.")),
                (Page::Chronicle, tr!("The diary bound as a book, a chapter for each zone.")),
                (Page::Letters, tr!("Your characters write to each other about what they share.")),
                (Page::Dead, tr!("Every death, its last seconds, and an epitaph.")),
                (Page::Deeds, tr!("Feats earned along the way, some for their class alone.")),
            ],
        ),
        (
            tr!("Their record"),
            &[
                (Page::Overview, tr!("Levels, gold and time at a glance.")),
                (Page::Journal, tr!("Every session, moment by moment.")),
                (Page::Quests, tr!("Every quest with its text and rewards, and what each NPC said.")),
                (Page::Map, tr!("Where they walked, fought and fell.")),
                (Page::Combat, tr!("Fights, abilities and the seconds before each death.")),
                (Page::Economy, tr!("Where the money came from and where it went.")),
                (Page::Armory, tr!("Gear, talents and the character sheet.")),
            ],
        ),
        (
            tr!("Their world"),
            &[
                (Page::House, tr!("All your characters as one household.")),
                (Page::Players, tr!("Everyone met, their roleplay profiles, the fellowship and the nemeses.")),
                (Page::Standing, tr!("Where they stand with every faction.")),
            ],
        ),
    ];
    let narrow = ui.available_width() < 900.0;
    let gap = 14.0;
    let w = if narrow { ui.available_width() } else { (ui.available_width() - 2.0 * gap) / 3.0 };
    let mut draw = |ui: &mut Ui, (title, pages): (&str, &[(Page, &str)])| {
        ui.allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            card(ui, |ui| {
                ui.set_width(w - 38.0);
                ui.label(RichText::new(title).font(theme::display_font(20.0)).color(INK));
                ui.add_space(6.0);
                for &(p, text) in pages {
                    let r = ui
                        .scope_builder(egui::UiBuilder::new().sense(if open { Sense::click() } else { Sense::hover() }), |ui| {
                            ui.horizontal_top(|ui| {
                                frame_icon(ui, art, p.icon(), 28.0);
                                ui.add_space(4.0);
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(p.label()).size(16.0).color(GOLD));
                                    ui.label(RichText::new(text).size(14.5).color(MUTED));
                                });
                            });
                        })
                        .response;
                    if open && r.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if open && r.clicked() {
                        *page = p;
                    }
                    ui.add_space(6.0);
                }
            });
        });
    };
    if narrow {
        for g in groups {
            draw(ui, g);
            ui.add_space(gap);
        }
    } else {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for g in groups {
                draw(ui, g);
            }
        });
    }
}

fn frame_icon(ui: &mut Ui, art: &mut Art, icon: i64, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 5.0, Color32::from_rgb(5, 7, 15));
    if let Some(t) = art.icon(ui.ctx(), Some(icon)) {
        p.image(
            t.id(),
            rect.shrink(1.5),
            Rect::from_min_max(pos2(0.07, 0.07), pos2(0.93, 0.93)),
            Color32::WHITE,
        );
    }
    p.rect_stroke(rect, 5.0, egui::Stroke::new(1.0, Color32::from_rgb(0x8a, 0x6d, 0x2c)), egui::StrokeKind::Outside);
}
