//! Colours, fonts and small formatting helpers, shared with wow.stru.ci's look:
//! tooltip navy, the client's label gold, item quality and class colours.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::sync::Arc;

pub const NIGHT: Color32 = Color32::from_rgb(0x0a, 0x0e, 0x1f);
pub const PANEL: Color32 = Color32::from_rgb(0x10, 0x16, 0x2e);
pub const RAISED: Color32 = Color32::from_rgb(0x18, 0x1f, 0x3d);
pub const EDGE: Color32 = Color32::from_rgb(0x28, 0x30, 0x5a);
pub const INK: Color32 = Color32::from_rgb(0xec, 0xe6, 0xd6);
pub const MUTED: Color32 = Color32::from_rgb(0x9b, 0x94, 0x7f);
pub const GOLD: Color32 = Color32::from_rgb(0xff, 0xd1, 0x00);

// Chart series, validated on PANEL (dark band, all pairs): gold, blue, aqua.
pub const SERIES: [Color32; 3] = [
    Color32::from_rgb(0xc9, 0x85, 0x00),
    Color32::from_rgb(0x39, 0x87, 0xe5),
    Color32::from_rgb(0x19, 0x9e, 0x70),
];
pub const DANGER: Color32 = Color32::from_rgb(0xe6, 0x67, 0x67);
pub const GOOD: Color32 = Color32::from_rgb(0x1e, 0xff, 0x00);

pub fn quality(q: Option<i64>) -> Color32 {
    match q {
        Some(0) => Color32::from_rgb(0x9d, 0x9d, 0x9d),
        Some(2) => Color32::from_rgb(0x1e, 0xff, 0x00),
        Some(3) => Color32::from_rgb(0x3d, 0x9b, 0xff),
        Some(4) => Color32::from_rgb(0xc0, 0x70, 0xff),
        Some(5) => Color32::from_rgb(0xff, 0x80, 0x00),
        Some(6) => Color32::from_rgb(0xe6, 0xcc, 0x80),
        Some(7) => Color32::from_rgb(0x00, 0xcc, 0xff),
        _ => INK,
    }
}

pub fn class_color(class_file: &str) -> Color32 {
    match class_file {
        "WARRIOR" => Color32::from_rgb(0xc6, 0x9b, 0x6d),
        "PALADIN" => Color32::from_rgb(0xf4, 0x8c, 0xba),
        "HUNTER" => Color32::from_rgb(0xaa, 0xd3, 0x72),
        "ROGUE" => Color32::from_rgb(0xff, 0xf4, 0x68),
        "PRIEST" => Color32::from_rgb(0xff, 0xff, 0xff),
        "SHAMAN" => Color32::from_rgb(0x00, 0x70, 0xdd),
        "MAGE" => Color32::from_rgb(0x3f, 0xc7, 0xeb),
        "WARLOCK" => Color32::from_rgb(0x87, 0x88, 0xee),
        "DRUID" => Color32::from_rgb(0xff, 0x7c, 0x0a),
        _ => Color32::from_rgb(0xd4, 0xb4, 0x5a),
    }
}

pub fn class_name(class_file: &str) -> &'static str {
    match class_file {
        "WARRIOR" => "Warrior",
        "PALADIN" => "Paladin",
        "HUNTER" => "Hunter",
        "ROGUE" => "Rogue",
        "PRIEST" => "Priest",
        "SHAMAN" => "Shaman",
        "MAGE" => "Mage",
        "WARLOCK" => "Warlock",
        "DRUID" => "Druid",
        _ => "",
    }
}

pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}
pub fn italic() -> FontFamily {
    FontFamily::Name("italic".into())
}
pub fn display_font(size: f32) -> FontId {
    FontId::new(size, display())
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            "marcellus",
            &include_bytes!("../assets/fonts/marcellus.ttf")[..],
        ),
        (
            "alegreya",
            &include_bytes!("../assets/fonts/alegreya-sans-400.ttf")[..],
        ),
        (
            "alegreya-bold",
            &include_bytes!("../assets/fonts/alegreya-sans-700.ttf")[..],
        ),
        (
            "alegreya-italic",
            &include_bytes!("../assets/fonts/alegreya-sans-400i.ttf")[..],
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let with = |first: &str| {
        let mut v = vec![first.to_string()];
        v.extend(fallback.iter().cloned());
        v
    };
    fonts
        .families
        .insert(FontFamily::Proportional, with("alegreya"));
    fonts.families.insert(display(), with("marcellus"));
    fonts.families.insert(bold(), with("alegreya-bold"));
    fonts.families.insert(italic(), with("alegreya-italic"));
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (TextStyle::Heading, FontId::new(26.0, display())),
            (TextStyle::Body, FontId::proportional(16.0)),
            (TextStyle::Button, FontId::proportional(15.5)),
            (TextStyle::Small, FontId::proportional(13.5)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        // Tooltips like the game's: right away, even while the pointer drifts.
        // Labels aren't selectable, so hovering an item's name counts as
        // hovering the item.
        s.interaction.show_tooltips_only_when_still = false;
        s.interaction.tooltip_delay = 0.08;
        s.interaction.selectable_labels = false;
        // Long text wraps, also inside horizontal layouts.
        s.wrap_mode = Some(egui::TextWrapMode::Wrap);
        let v = &mut s.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = NIGHT;
        v.window_fill = PANEL;
        v.extreme_bg_color = Color32::from_rgb(0x07, 0x0a, 0x18);
        v.faint_bg_color = PANEL;
        v.override_text_color = Some(INK);
        v.window_stroke = egui::Stroke::new(1.0, EDGE);
        v.selection.bg_fill = Color32::from_rgb(0x4a, 0x3b, 0x0c);
        v.selection.stroke = egui::Stroke::new(1.0, GOLD);
        v.hyperlink_color = GOLD;
        for (w, fill) in [
            (&mut v.widgets.noninteractive, PANEL),
            (&mut v.widgets.inactive, RAISED),
            (&mut v.widgets.hovered, Color32::from_rgb(0x22, 0x2a, 0x50)),
            (&mut v.widgets.active, Color32::from_rgb(0x2a, 0x33, 0x60)),
            (&mut v.widgets.open, RAISED),
        ] {
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.corner_radius = egui::CornerRadius::same(6);
        }
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, EDGE);
        v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, EDGE);
        v.window_corner_radius = egui::CornerRadius::same(8);
        v.popup_shadow = egui::Shadow {
            offset: [0, 6],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(140),
        };
    });
}

// ---- formatting ----

pub fn duration(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    let (h, m) = (s / 3600, s % 3600 / 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {:02}s", s % 60)
    } else {
        format!("{}s", s % 60)
    }
}

pub fn money(copper: i64) -> String {
    let neg = copper < 0;
    let c = copper.abs();
    let (g, s, cu) = (c / 10000, c / 100 % 100, c % 100);
    let mut out = String::new();
    if g > 0 {
        out += &format!("{g}g ");
    }
    if g > 0 || s > 0 {
        out += &format!("{s}s ");
    }
    out += &format!("{cu}c");
    if neg { format!("−{out}") } else { out }
}

pub fn local(t: f64) -> chrono::DateTime<chrono::Local> {
    chrono::DateTime::from_timestamp(t as i64, 0)
        .unwrap_or_default()
        .with_timezone(&chrono::Local)
}
pub fn clock(t: f64) -> String {
    local(t).format("%H:%M").to_string()
}
pub fn day(t: f64) -> String {
    local(t).format("%a %-d %b").to_string()
}
pub fn when(t: f64) -> String {
    local(t).format("%a %-d %b, %H:%M").to_string()
}
pub fn ago(t: f64) -> String {
    let d = chrono::Local::now().timestamp() as f64 - t;
    if d < 60.0 {
        "just now".into()
    } else if d < 3600.0 {
        format!("{} min ago", (d / 60.0) as i64)
    } else if d < 86400.0 * 2.0 {
        format!("{} h ago", (d / 3600.0) as i64)
    } else {
        day(t)
    }
}

pub fn thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("−{out}") } else { out }
}
