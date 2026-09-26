//! Colours, fonts and small formatting helpers in the game's look: tooltip
//! navy, the client's label gold, item quality and class colours.

use crate::i18n::{self, Lang};
use crate::tr;
use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::path::{Path, PathBuf};
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
        "WARRIOR" => tr!("Warrior"),
        "PALADIN" => tr!("Paladin"),
        "HUNTER" => tr!("Hunter"),
        "ROGUE" => tr!("Rogue"),
        "PRIEST" => tr!("Priest"),
        "SHAMAN" => tr!("Shaman"),
        "MAGE" => tr!("Mage"),
        "WARLOCK" => tr!("Warlock"),
        "DRUID" => tr!("Druid"),
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

pub const FONT_DISPLAY: &[u8] = include_bytes!("../assets/fonts/marcellus.ttf");
pub const FONT_ITALIC: &[u8] = include_bytes!("../assets/fonts/alegreya-sans-400i.ttf");

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("marcellus", FONT_DISPLAY),
        (
            "alegreya",
            &include_bytes!("../assets/fonts/alegreya-sans-400.ttf")[..],
        ),
        (
            "alegreya-bold",
            &include_bytes!("../assets/fonts/alegreya-sans-700.ttf")[..],
        ),
        ("alegreya-italic", FONT_ITALIC),
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
    // Chinese text, and CJK player names in any language, from a system font.
    if let Some(cjk) = cjk_font() {
        fonts.font_data.insert("cjk".into(), Arc::new(cjk));
        for family in fonts.families.values_mut() {
            family.push("cjk".into());
        }
    }
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
        v.window_fill = Color32::from_rgb(0x07, 0x0b, 0x1c);
        v.extreme_bg_color = Color32::from_rgb(0x07, 0x0a, 0x18);
        // Table stripes, and tooltips in the game's own navy.
        v.faint_bg_color = Color32::from_rgb(0x15, 0x1c, 0x3a);
        v.override_text_color = Some(INK);
        v.window_stroke = egui::Stroke::new(1.0, Color32::from_rgb(0x5d, 0x64, 0x85));
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

/// A system font with Chinese glyphs (Simplified forms where the file holds
/// several), or None; Marcellus and Alegreya are Latin only.
fn cjk_font() -> Option<FontData> {
    const KNOWN: [&str; 20] = [
        // Windows
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        // Linux
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJKsc-Regular.otf",
        "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
        "/usr/share/fonts/adobe-source-han-sans/SourceHanSansCN-Regular.otf",
        "/usr/share/fonts/adobe-source-han-sans/SourceHanSans-Regular.ttc",
        "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/wenquanyi/wqy-zenhei/wqy-zenhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/usr/share/fonts/truetype/arphic/uming.ttc",
    ];
    if let Some(f) = KNOWN.iter().find_map(|p| load_cjk(Path::new(p), None)) {
        return Some(f);
    }
    // Whatever fontconfig knows, for other distributions and user fonts.
    let out = std::process::Command::new("fc-match")
        .args([
            "-f",
            "%{file}\n%{index}",
            "sans-serif:lang=zh-cn:charset=4e2d",
        ])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut lines = text.lines();
    let file = PathBuf::from(lines.next().filter(|f| !f.is_empty())?);
    load_cjk(&file, lines.next().and_then(|i| i.trim().parse().ok()))
}

/// The font at `path` if one of its faces has Han glyphs, preferring the
/// given face, then a Simplified Chinese one.
fn load_cjk(path: &Path, index: Option<u32>) -> Option<FontData> {
    let bytes = std::fs::read(path).ok()?;
    let faces = ttf_parser::fonts_in_collection(&bytes).unwrap_or(1);
    let has_han =
        |i: u32| ttf_parser::Face::parse(&bytes, i).is_ok_and(|f| f.glyph_index('中').is_some());
    let simplified = |i: u32| {
        ttf_parser::Face::parse(&bytes, i).is_ok_and(|f| {
            f.names().into_iter().any(|n| {
                n.name_id == ttf_parser::name_id::FAMILY
                    && n.to_string().is_some_and(|s| {
                        let s = s.to_lowercase();
                        (s.ends_with(" sc") || s.contains(" sc ") || s.contains("gb"))
                            && !s.contains("mono")
                    })
            })
        })
    };
    let index = index
        .filter(|i| has_han(*i))
        .or_else(|| (0..faces).find(|i| simplified(*i) && has_han(*i)))
        .or_else(|| (0..faces).find(|i| has_han(*i)))?;
    let mut data = FontData::from_owned(bytes);
    data.index = index;
    Some(data)
}

// ---- formatting ----

pub fn duration(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    let (h, m) = (s / 3600, s % 3600 / 60);
    if h > 0 {
        tr!("{h}h {m}m", h = h, m = format!("{m:02}"))
    } else if m > 0 {
        tr!("{m}m {s}s", m = m, s = format!("{:02}", s % 60))
    } else {
        tr!("{s}s", s = s % 60)
    }
}

pub fn money(copper: i64) -> String {
    let neg = copper < 0;
    let c = copper.abs();
    let (g, s, cu) = (c / 10000, c / 100 % 100, c % 100);
    let mut out = String::new();
    if g > 0 {
        out += &tr!("{n}g", n = g);
        out += " ";
    }
    if g > 0 || s > 0 {
        out += &tr!("{n}s", n = s);
        out += " ";
    }
    out += &tr!("{n}c", n = cu);
    if neg { format!("−{out}") } else { out }
}

/// Money without the parts that are zero, for chart axes: "2g", "1g 50s", "40c".
pub fn money_short(copper: i64) -> String {
    let c = copper.abs();
    let (g, s, cu) = (c / 10000, c / 100 % 100, c % 100);
    let mut parts = vec![];
    if g > 0 {
        parts.push(tr!("{n}g", n = g));
    }
    if s > 0 {
        parts.push(tr!("{n}s", n = s));
    }
    if cu > 0 || parts.is_empty() {
        parts.push(tr!("{n}c", n = cu));
    }
    let out = parts.join(" ");
    if copper < 0 { format!("−{out}") } else { out }
}

pub fn local(t: f64) -> chrono::DateTime<chrono::Local> {
    chrono::DateTime::from_timestamp(t as i64, 0)
        .unwrap_or_default()
        .with_timezone(&chrono::Local)
}

fn chrono_locale(lang: Lang) -> chrono::Locale {
    match lang {
        Lang::En => chrono::Locale::en_US,
        Lang::De => chrono::Locale::de_DE,
        Lang::Fr => chrono::Locale::fr_FR,
        Lang::Es => chrono::Locale::es_ES,
        Lang::Pt => chrono::Locale::pt_BR,
        Lang::Zh => chrono::Locale::zh_CN,
    }
}

/// A date (and time) in the current language's own order and names.
fn date(t: f64, with_time: bool) -> String {
    let lang = i18n::current();
    let pattern = match lang {
        Lang::En | Lang::Fr | Lang::Es => "%a %-d %b",
        Lang::De => "%a, %-d. %b",
        Lang::Pt => "%a, %-d %b",
        Lang::Zh => "%-m月%-d日 周%a",
    };
    let mut out = local(t)
        .format_localized(pattern, chrono_locale(lang))
        .to_string();
    if with_time {
        out += if lang == Lang::Zh { " " } else { ", " };
        out += &clock(t);
    }
    out
}

/// A full date, "Saturday, 26 September 2026", in the current language.
pub fn long_day(d: chrono::NaiveDate) -> String {
    let lang = i18n::current();
    let pattern = match lang {
        Lang::En => "%A, %-d %B %Y",
        Lang::De => "%A, %-d. %B %Y",
        Lang::Fr => "%A %-d %B %Y",
        Lang::Es | Lang::Pt => "%A, %-d de %B de %Y",
        Lang::Zh => "%Y年%-m月%-d日 %A",
    };
    d.format_localized(pattern, chrono_locale(lang)).to_string()
}

pub fn clock(t: f64) -> String {
    local(t).format("%H:%M").to_string()
}
pub fn day(t: f64) -> String {
    date(t, false)
}
pub fn when(t: f64) -> String {
    date(t, true)
}
pub fn ago(t: f64) -> String {
    let d = chrono::Local::now().timestamp() as f64 - t;
    if d < 60.0 {
        tr!("just now").into()
    } else if d < 3600.0 {
        tr!("{n} min ago", n = (d / 60.0) as i64)
    } else if d < 86400.0 * 2.0 {
        tr!("{n} h ago", n = (d / 3600.0) as i64)
    } else {
        day(t)
    }
}

pub fn thousands(n: i64) -> String {
    let sep = match i18n::current() {
        Lang::De | Lang::Es | Lang::Pt => '.',
        Lang::Fr => '\u{a0}',
        Lang::En | Lang::Zh => ',',
    };
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    if n < 0 { format!("−{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_in_each_language() {
        let t = 1790000000.0; // Monday 21 September 2026
        for l in Lang::ALL {
            i18n::set(l);
            println!(
                "{l:?}: {} | {} | {} | {}",
                day(t),
                when(t),
                duration(3725.0),
                money(123456)
            );
        }
        i18n::set(Lang::Zh);
        let zh = day(t);
        i18n::set(Lang::En);
        assert!(zh.contains('月') && zh.contains('周'), "{zh}");
    }
}
