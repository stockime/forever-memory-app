//! Game textures turned into the images the app shows: icons, empty
//! equipment slots, class icons, Classic talent tree backgrounds, the class
//! banner and scene, and world maps. Every image has a key that doubles as
//! its file name, e.g. "icon-135274.png" or "map-1420.jpg".

use super::{Storage, blp};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ImageEncoder, RgbImage, RgbaImage, imageops};

/// The Classic talent frame backgrounds per class, in the order of the
/// class's talent groups.
#[rustfmt::skip]
const TREE_TEXTURES: [(&str, [&str; 3]); 9] = [
    ("warrior", ["warriorarms", "warriorfury", "warriorprotection"]),
    ("paladin", ["paladinholy", "paladinprotection", "paladincombat"]),
    ("hunter", ["hunterbeastmastery", "huntermarksmanship", "huntersurvival"]),
    ("rogue", ["rogueassassination", "roguecombat", "roguesubtlety"]),
    ("priest", ["priestdiscipline", "priestholy", "priestshadow"]),
    ("shaman", ["shamanelementalcombat", "shamanenhancement", "shamanrestoration"]),
    ("mage", ["magearcane", "magefire", "magefrost"]),
    ("warlock", ["warlockcurses", "warlocksummoning", "warlockdestruction"]),
    ("druid", ["druidbalance", "druidferalcombat", "druidrestoration"]),
];

/// Classic uiMapIDs (what the addon records with positions) and their world
/// map texture folders.
#[rustfmt::skip]
const MAP_FOLDERS: [(u32, &str); 48] = [
    (1411, "durotar"), (1412, "mulgore"), (1413, "barrens"), (1414, "kalimdor"),
    (1415, "azeroth"), (1416, "alterac"), (1417, "arathi"), (1418, "badlands"),
    (1419, "blastedlands"), (1420, "tirisfal"), (1421, "silverpine"),
    (1422, "westernplaguelands"), (1423, "easternplaguelands"), (1424, "hilsbrad"),
    (1425, "hinterlands"), (1426, "dunmorogh"), (1427, "searinggorge"),
    (1428, "burningsteppes"), (1429, "elwynn"), (1430, "deadwindpass"),
    (1431, "duskwood"), (1432, "lochmodan"), (1433, "redridge"), (1434, "stranglethorn"),
    (1435, "swampofsorrows"), (1436, "westfall"), (1437, "wetlands"), (1438, "teldrassil"),
    (1439, "darkshore"), (1440, "ashenvale"), (1441, "thousandneedles"),
    (1442, "stonetalonmountains"), (1443, "desolace"), (1444, "feralas"),
    (1445, "dustwallow"), (1446, "tanaris"), (1447, "aszhara"), (1448, "felwood"),
    (1449, "ungorocrater"), (1450, "moonglade"), (1451, "silithus"), (1452, "winterspring"),
    (1453, "stormwind"), (1454, "ogrimmar"), (1455, "ironforge"), (1456, "thunderbluff"),
    (1457, "darnassis"), (1458, "undercity"),
];

/// Produces the encoded image for a key.
pub fn render(s: &Storage, key: &str) -> Result<Vec<u8>, String> {
    if let Some(id) = key
        .strip_prefix("icon-")
        .and_then(|k| k.strip_suffix(".png"))
    {
        let id: i64 = id.parse().map_err(|_| format!("bad key {key}"))?;
        return png(texture(s, &id.to_string())?);
    }
    let name = key.strip_suffix(".png").unwrap_or(key);
    let name = name.strip_suffix(".jpg").unwrap_or(name);
    let (kind, rest) = name
        .split_once('-')
        .ok_or_else(|| format!("unknown image key {key}"))?;
    match kind {
        "slot" => png(texture(
            s,
            &format!("interface/paperdoll/ui-paperdoll-slot-{rest}.blp"),
        )?),
        "class" => png(texture(
            s,
            &format!("interface/icons/classicon_{rest}.blp"),
        )?),
        "map" => {
            let id: u32 = rest.parse().map_err(|_| format!("bad key {key}"))?;
            let (_, folder) = MAP_FOLDERS
                .iter()
                .find(|(m, _)| *m == id)
                .ok_or_else(|| format!("no world map for uiMap {id}"))?;
            // Twelve 256x256 tiles, four across and three down; the map fills 1002x668.
            let mut out = RgbaImage::new(1024, 768);
            for i in 0..12 {
                let tile = texture(
                    s,
                    &format!("interface/worldmap/{folder}/{folder}{}.blp", i + 1),
                )?;
                imageops::replace(&mut out, &tile, (i % 4 * 256) as i64, (i / 4 * 256) as i64);
            }
            jpeg(&imageops::crop_imm(&out, 0, 0, 1002, 668).to_image(), 88)
        }
        "tree" => {
            let (class, i) = rest
                .rsplit_once('-')
                .ok_or_else(|| format!("bad key {key}"))?;
            let i: usize = i.parse().map_err(|_| format!("bad key {key}"))?;
            let tex = TREE_TEXTURES
                .iter()
                .find(|(c, _)| c.eq_ignore_ascii_case(class))
                .and_then(|(_, t)| t.get(i))
                .ok_or_else(|| format!("no tree art for {key}"))?;
            png(tree_background(s, tex)?)
        }
        "banner" | "scene" => {
            let img = texture(
                s,
                &format!("interface/talentframe/talentsclassbackground{rest}1.blp"),
            )?;
            // The texture is an atlas of two class scenes stacked in the top-left
            // 1612x1548: the banner is the first, the scene behind the gear the second.
            let (x0, y0, x1, y1) = if kind == "scene" {
                (8, 784, 1604, 1540)
            } else {
                (8, 8, 1604, 756)
            };
            let (x1, y1) = (x1.min(img.width()), y1.min(img.height()));
            if x1 <= x0 || y1 <= y0 {
                return Err(format!(
                    "{key}: texture is only {}x{}",
                    img.width(),
                    img.height()
                ));
            }
            jpeg(
                &imageops::crop_imm(&img, x0, y0, x1 - x0, y1 - y0).to_image(),
                86,
            )
        }
        _ => Err(format!("unknown image key {key}")),
    }
}

/// Stitches the four quarters of a Classic talent background.
fn tree_background(s: &Storage, name: &str) -> Result<RgbaImage, String> {
    let mut out = RgbaImage::new(320, 384);
    for (suffix, x, y) in [
        ("topleft", 0, 0),
        ("topright", 256, 0),
        ("bottomleft", 0, 256),
        ("bottomright", 256, 256),
    ] {
        let img = texture(s, &format!("interface/talentframe/{name}-{suffix}.blp"))?;
        imageops::replace(&mut out, &img, x, y);
    }
    // The painted area ends before the texture does; the rest is padding.
    Ok(imageops::crop_imm(&out, 0, 0, 298, 336).to_image())
}

/// A texture by file data ID or game path.
fn texture(s: &Storage, r: &str) -> Result<RgbaImage, String> {
    let id = match r.parse::<u32>() {
        Ok(id) => id,
        Err(_) => s
            .lookup(r)
            .ok_or_else(|| format!("{r}: not in this build"))?,
    };
    blp::decode(&s.read(id)?)
}

/// Opaque images are written as RGB and at the default zlib level, like Go's
/// image/png does.
fn png(img: RgbaImage) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let enc =
        PngEncoder::new_with_quality(&mut buf, CompressionType::Default, FilterType::Adaptive);
    let (w, h) = img.dimensions();
    let res = if img.pixels().all(|p| p[3] == 255) {
        let rgb: RgbImage = image::DynamicImage::ImageRgba8(img).to_rgb8();
        enc.write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
    } else {
        enc.write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgba8)
    };
    res.map_err(|e| e.to_string())?;
    Ok(buf)
}

/// JPEG has no alpha: pixels are premultiplied, i.e. laid over black, which
/// is what Go's image/jpeg does with a non-opaque image.
fn jpeg(img: &RgbaImage, quality: u8) -> Result<Vec<u8>, String> {
    let rgb = RgbImage::from_fn(img.width(), img.height(), |x, y| {
        let p = img.get_pixel(x, y);
        let a = p[3] as u32;
        let c = |v: u8| ((v as u32 * 0x101 * a / 0xff) >> 8) as u8;
        image::Rgb([c(p[0]), c(p[1]), c(p[2])])
    });
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, quality)
        .encode_image(&rgb)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Renders from the real install; set GAMEDATA_OUT to keep the images.
    #[test]
    #[ignore = "needs a local World of Warcraft install"]
    fn renders_from_the_local_install() {
        let home = std::env::var("HOME").unwrap_or_default();
        let dir = PathBuf::from(home)
            .join("Games/battlenet/drive_c/Program Files (x86)/World of Warcraft");
        let t = std::time::Instant::now();
        let s = Storage::open(&dir, "wow_classic_beta").expect("open");
        eprintln!("opened {} files in {:?}", s.count(), t.elapsed());
        assert!(
            super::super::products(&dir)
                .contains(&("wow_classic_beta".into(), "_classic_beta_".into()))
        );
        let out = std::env::var_os("GAMEDATA_OUT").map(PathBuf::from);
        for (key, w, h) in [
            ("icon-135274.png", 64, 64),
            ("slot-head.png", 64, 64),
            ("tree-paladin-0.png", 298, 336),
            ("banner-paladin.jpg", 1596, 748),
            ("map-1420.jpg", 1002, 668),
        ] {
            let t = std::time::Instant::now();
            let b = render(&s, key).unwrap_or_else(|e| panic!("{key}: {e}"));
            let img = image::load_from_memory(&b).unwrap();
            eprintln!(
                "{key}: {}x{} {} bytes in {:?}",
                img.width(),
                img.height(),
                b.len(),
                t.elapsed()
            );
            assert_eq!((img.width(), img.height()), (w, h), "{key}");
            if let Some(dir) = &out {
                std::fs::write(dir.join(key), &b).unwrap();
            }
        }
    }
}
