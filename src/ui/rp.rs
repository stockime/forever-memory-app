//! A roleplay profile (Total RP 3, MyRolePlay, XRP) as a card: the name the
//! character goes by, their title, what the eye sees, where they call home,
//! their motto and what they're up to, and TRP3's personality sliders.

use crate::data::rp::{Profile, Trait};
use crate::theme::{self, GOLD, INK, MUTED};
use crate::tr;
use egui::{Color32, RichText, Sense, Ui};

/// The realm a character lives on, for finding who they met.
pub fn realm(c: &crate::data::memory::Character) -> &str {
    c.snapshot
        .get("realm")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
}

/// A TRP3 preset trait in the app's language; custom ones stay as written.
pub fn trait_name(en: &str) -> String {
    match en {
        "Chaotic" => tr!("Chaotic"),
        "Lawful" => tr!("Lawful"),
        "Chaste" => tr!("Chaste"),
        "Lustful" => tr!("Lustful"),
        "Forgiving" => tr!("Forgiving"),
        "Vindictive" => tr!("Vindictive"),
        "Altruistic" => tr!("Altruistic"),
        "Selfish" => tr!("Selfish"),
        "Truthful" => tr!("Truthful"),
        "Deceitful" => tr!("Deceitful"),
        "Gentle" => tr!("Gentle"),
        "Brutal" => tr!("Brutal"),
        "Superstitious" => tr!("Superstitious"),
        "Rational" => tr!("Rational"),
        "Renegade" => tr!("Renegade"),
        "Paragon" => tr!("Paragon"),
        "Cautious" => tr!("Cautious"),
        "Impulsive" => tr!("Impulsive"),
        "Ascetic" => tr!("Ascetic"),
        "Bon vivant" => tr!("Bon vivant"),
        "Valorous" => tr!("Valorous"),
        "Spineless" => tr!("Spineless"),
        other => other,
    }
    .to_string()
}

/// The name and full title, large.
fn names(ui: &mut Ui, p: &Profile, size: f32, color: Color32) {
    let name = p.full_name();
    if !name.is_empty() {
        ui.label(
            RichText::new(name)
                .font(theme::display_font(size))
                .color(color),
        );
    }
    let size = (size * 0.55).max(15.0);
    let title = match (p.full_title.as_str(), p.nickname.as_str()) {
        ("", "") => return,
        (t, "") => t.to_string(),
        ("", n) => format!("“{n}”"),
        (t, n) => format!("{t} · “{n}”"),
    };
    ui.label(
        RichText::new(title)
            .family(theme::italic())
            .size(size)
            .color(INK),
    );
}

/// "Age 34 · Eyes grey · …": a caption and its value, each pair kept on
/// one line and the pairs wrapping.
fn facts(ui: &mut Ui, rows: &[(&str, &str)]) {
    use egui::text::{LayoutJob, TextFormat};
    let rows: Vec<_> = rows.iter().filter(|(_, v)| !v.trim().is_empty()).collect();
    if rows.is_empty() {
        return;
    }
    let font = egui::FontId::proportional(16.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(14.0, 4.0);
        for (k, v) in rows {
            let mut job = LayoutJob::default();
            job.append(k, 0.0, TextFormat::simple(font.clone(), MUTED));
            job.append(&clip(v, 60), 5.0, TextFormat::simple(font.clone(), INK));
            ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Extend));
        }
    });
}

fn clip(s: &str, n: usize) -> String {
    crate::data::diary::clip(s, n)
}

fn motto(ui: &mut Ui, p: &Profile) {
    if !p.motto.is_empty() {
        ui.label(
            RichText::new(format!("“{}”", clip(&p.motto, 160)))
                .family(theme::italic())
                .size(17.0)
                .color(GOLD),
        );
    }
}

fn currently(ui: &mut Ui, p: &Profile) {
    if !p.currently.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.label(RichText::new(tr!("Currently:")).color(MUTED));
            ui.label(RichText::new(clip(&p.currently, 240)).color(INK));
        });
    }
}

/// A slider between two traits: the bar splits where the character stands,
/// and the side they lean to is lit.
fn slider(ui: &mut Ui, t: &Trait, width: f32) {
    const LEFT: Color32 = Color32::from_rgb(0xc9, 0x85, 0x00);
    const RIGHT: Color32 = Color32::from_rgb(0x39, 0x87, 0xe5);
    let side = 96.0_f32.min(width * 0.3);
    let (row, _) = ui.allocate_exact_size(egui::vec2(width, 22.0), Sense::hover());
    let p = ui.painter_at(row.expand(4.0));
    let font = egui::FontId::proportional(15.0);
    let lean_left = t.left_share >= 0.5;
    let strong = |on: bool| if on { INK } else { MUTED };
    p.text(
        egui::pos2(row.left() + side - 8.0, row.center().y),
        egui::Align2::RIGHT_CENTER,
        trait_name(&t.left),
        font.clone(),
        strong(lean_left),
    );
    p.text(
        egui::pos2(row.right() - side + 8.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        trait_name(&t.right),
        font,
        strong(!lean_left),
    );
    let track = egui::Rect::from_min_max(
        egui::pos2(row.left() + side, row.center().y - 4.0),
        egui::pos2(row.right() - side, row.center().y + 4.0),
    );
    p.rect_filled(track.expand(1.0), 5.0, Color32::from_rgb(0x07, 0x0a, 0x18));
    let split = track.left() + track.width() * t.left_share.clamp(0.0, 1.0);
    let dim = |c: Color32, on: bool| if on { c } else { c.gamma_multiply(0.45) };
    let l = egui::Rect::from_min_max(track.left_top(), egui::pos2(split, track.bottom()));
    let r = egui::Rect::from_min_max(egui::pos2(split, track.top()), track.right_bottom());
    p.rect_filled(l, 4.0, dim(LEFT, lean_left));
    p.rect_filled(r, 4.0, dim(RIGHT, !lean_left));
    // A small gold diamond where they stand.
    let c = egui::pos2(split, track.center().y);
    let d = 6.5;
    p.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(c.x, c.y - d),
            egui::pos2(c.x + d, c.y),
            egui::pos2(c.x, c.y + d),
            egui::pos2(c.x - d, c.y),
        ],
        GOLD,
        egui::Stroke::new(1.0, Color32::from_rgb(0x2e, 0x1d, 0x0e)),
    ));
}

pub fn traits(ui: &mut Ui, traits: &[Trait], width: f32) {
    for t in traits {
        slider(ui, t, width);
        ui.add_space(2.0);
    }
}

/// The whole profile, for the character's own Overview.
pub fn card(ui: &mut Ui, p: &Profile, color: Color32) {
    ui.set_width(ui.available_width());
    ui.horizontal(|ui| {
        super::label(ui, tr!("Roleplay profile"));
    });
    // Traits and first glances to the right; the history when there are none.
    let history = p.traits.is_empty() && p.glances.is_empty();
    let wide = ui.available_width() >= 760.0 && !(history && p.history.is_empty());
    let left = |ui: &mut Ui| {
        names(ui, p, 30.0, color);
        ui.add_space(6.0);
        facts(
            ui,
            &[
                (tr!("Age"), p.age.as_str()),
                (tr!("Eyes"), p.eyes.as_str()),
                (tr!("Height"), p.height.as_str()),
                (tr!("Build"), p.build.as_str()),
            ],
        );
        facts(
            ui,
            &[
                (tr!("Lives in"), p.residence.as_str()),
                (tr!("Born in"), p.birthplace.as_str()),
            ],
        );
        ui.add_space(6.0);
        motto(ui, p);
        currently(ui, p);
        let about = if p.appearance.is_empty() {
            &p.about
        } else {
            &p.appearance
        };
        if !about.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(clip(about, 420)).color(INK));
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(tr!(
                "From {addon}, as you wrote it. The diary, letters and epitaphs know it too.",
                addon = p.addon
            ))
            .small()
            .color(MUTED),
        );
    };
    let right = |ui: &mut Ui, w: f32| {
        traits(ui, &p.traits, w.min(460.0));
        for (t, x) in p.glances.iter().take(3) {
            ui.add_space(8.0);
            if !t.is_empty() {
                ui.label(
                    RichText::new(clip(t, 60))
                        .font(theme::display_font(16.0))
                        .color(GOLD),
                );
            }
            if !x.is_empty() {
                ui.label(RichText::new(clip(x, 200)).color(INK));
            }
        }
        if history && !p.history.is_empty() {
            ui.label(
                RichText::new(tr!("History"))
                    .font(theme::display_font(16.0))
                    .color(GOLD),
            );
            ui.label(RichText::new(clip(&p.history, 420)).color(INK));
        }
    };
    if wide {
        ui.horizontal_top(|ui| {
            let w = ui.available_width();
            ui.vertical(|ui| {
                ui.set_width(w * 0.55);
                left(ui);
            });
            ui.add_space(24.0);
            ui.vertical(|ui| {
                let w = ui.available_width();
                ui.add_space(4.0);
                right(ui, w);
            });
        });
    } else {
        left(ui);
        ui.add_space(8.0);
        let w = ui.available_width().min(520.0);
        right(ui, w);
    }
}

/// A shorter card for someone else's profile on the Players page.
pub fn other(ui: &mut Ui, p: &Profile, color: Color32) {
    ui.set_width(ui.available_width());
    super::label(ui, tr!("Roleplay profile"));
    names(ui, p, 26.0, color);
    ui.add_space(4.0);
    facts(
        ui,
        &[
            (tr!("Age"), p.age.as_str()),
            (tr!("Eyes"), p.eyes.as_str()),
            (tr!("Lives in"), p.residence.as_str()),
        ],
    );
    motto(ui, p);
    currently(ui, p);
    for (text, n) in [(&p.appearance, 360), (&p.about, 360)] {
        if !text.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(clip(text, n)).color(INK));
        }
    }
    if !p.traits.is_empty() {
        ui.add_space(6.0);
        let w = ui.available_width().min(520.0);
        traits(ui, &p.traits, w);
    }
    ui.add_space(4.0);
    ui.label(
        RichText::new(tr!(
            "From their {addon} profile, as your game last saw it.",
            addon = p.addon
        ))
        .small()
        .color(MUTED),
    );
}

/// A short note to start "Who X is" from, made of the profile.
pub fn note(p: &Profile) -> String {
    let mut parts = vec![];
    let mut head = p.full_name();
    if !p.full_title.is_empty() {
        head = if head.is_empty() {
            p.full_title.clone()
        } else {
            format!("{head}, {}", p.full_title)
        };
    }
    if !head.is_empty() {
        parts.push(format!("{head}."));
    }
    for (text, n) in [(&p.about, 500), (&p.appearance, 240), (&p.history, 400)] {
        if !text.is_empty() {
            parts.push(clip(text, n));
        }
    }
    let words = p.trait_words(trait_name, tr!("very"));
    if !words.is_empty() {
        parts.push(format!("{}.", capitalise(&words.join(", "))));
    }
    if !p.motto.is_empty() {
        parts.push(format!("“{}”", clip(&p.motto, 160)));
    }
    parts.join("\n\n")
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}
