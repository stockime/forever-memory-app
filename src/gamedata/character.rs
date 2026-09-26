//! A character as the game draws it: the race's HD model with its default
//! look (the first skin and hair colour, the body parts every character
//! starts with), rendered in software to a still with a transparent
//! background. No appearance choices are recorded yet, so everyone of a race
//! and gender looks the same for now.

use super::Storage;
use super::m2::{Model, Skin};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;

/// HD model, default skin colour and hair colour, per race and gender.
/// Only races whose default textures are known; the rest need the game's
/// customization tables to find theirs.
fn look(race_file: &str, female: bool) -> Option<(u32, u32, u32)> {
    Some(match (race_file.to_lowercase().as_str(), female) {
        ("scourge", false) => (959310, 3595458, 3457574),
        ("scourge", true) => (997378, 3594316, 3459239),
        _ => return None,
    })
}

/// Whether a character of this race can be drawn.
pub fn supported(race_file: &str) -> bool {
    look(race_file, false).is_some()
}

/// Which body parts show: the body, the first variant of each part
/// (bare hands and feet, no cape, the first hairstyle), the ears, and the
/// bare arms, shoulders and shins HD models keep in parts of their own.
fn shown(id: u16) -> bool {
    // Eye glow (17xx) blends two textures, which this renderer doesn't do yet.
    (id == 0 || id % 100 == 1 || matches!(id, 702 | 802 | 2902 | 3002)) && id / 100 != 17
}

struct Tex {
    img: RgbaImage,
}

impl Tex {
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let (w, h) = (self.img.width() as f32, self.img.height() as f32);
        let x = (u.rem_euclid(1.0) * w - 0.5).max(0.0);
        let y = (v.rem_euclid(1.0) * h - 0.5).max(0.0);
        let (x0, y0) = (x as u32 % self.img.width(), y as u32 % self.img.height());
        let (x1, y1) = ((x0 + 1) % self.img.width(), (y0 + 1) % self.img.height());
        let (fx, fy) = (x.fract(), y.fract());
        let p = |x, y| self.img.get_pixel(x, y).0;
        let mut out = [0.0; 4];
        for c in 0..4 {
            let top = p(x0, y0)[c] as f32 * (1.0 - fx) + p(x1, y0)[c] as f32 * fx;
            let bot = p(x0, y1)[c] as f32 * (1.0 - fx) + p(x1, y1)[c] as f32 * fx;
            out[c] = (top * (1.0 - fy) + bot * fy) / 255.0;
        }
        out
    }
}

/// Renders a character still, `w` by `h`, as PNG bytes.
pub fn render(s: &Storage, race_file: &str, female: bool, w: u32, h: u32) -> Result<Vec<u8>, String> {
    let (model_id, skin_id, hair_id) = look(race_file, female).ok_or_else(|| format!("no model for {race_file}"))?;
    let model = Model::parse(&s.read(model_id)?)?;
    let skin = Skin::parse(&s.read(*model.skins.first().ok_or("model without skins")?)?)?;

    // The triangles to draw, each with its texture and blend mode.
    struct Part {
        tex: u32,
        blend: u16,
        tris: Vec<[usize; 3]>,
    }
    let mut parts = vec![];
    for batch in &skin.batches {
        let Some(sec) = skin.sections.get(batch.section as usize) else { continue };
        if !shown(sec.id) {
            continue;
        }
        let tex_index = model.texture_lookup.get(batch.texture_combo as usize).copied().unwrap_or(0) as usize;
        let tex = match model.textures.get(tex_index).map(|t| (t.kind, t.file)) {
            Some((0, file)) if file != 0 => file,
            Some((1, _)) => skin_id,
            Some((6, _)) => hair_id,
            _ => continue, // capes, fur and extras aren't part of the default look
        };
        let blend = model.materials.get(batch.material as usize).copied().unwrap_or(0);
        let mut tris = vec![];
        let (start, count) = (sec.index_start as usize, sec.index_count as usize);
        for t in (start..start + count).step_by(3) {
            let idx = |k: usize| skin.indices.get(t + k).and_then(|&i| skin.vertices.get(i as usize)).map(|&v| v as usize);
            if let (Some(a), Some(b), Some(c)) = (idx(0), idx(1), idx(2)) {
                tris.push([a, b, c]);
            }
        }
        parts.push(Part { tex, blend, tris });
    }
    let mut textures: HashMap<u32, Tex> = HashMap::new();
    for part in &parts {
        if !textures.contains_key(&part.tex) {
            if let Some(img) = s.read(part.tex).ok().and_then(|b| super::blp::decode(&b).ok()) {
                textures.insert(part.tex, Tex { img });
            }
        }
    }
    // Opaque first, then alpha-tested, then blended.
    parts.sort_by_key(|p| match p.blend {
        0 => 0,
        1 => 1,
        _ => 2,
    });

    // A three-quarter view, like the character sheet: turned a little to the left.
    let yaw: f32 = 0.35;
    let (sy, cy) = yaw.sin_cos();
    let view = |p: [f32; 3]| -> [f32; 3] {
        // Model space: x forward, y left, z up. Screen: right, down, depth.
        let x = p[0] * cy - p[1] * sy;
        let y = p[0] * sy + p[1] * cy;
        [-y, -p[2], -x]
    };
    let used: Vec<usize> = parts.iter().flat_map(|p| p.tris.iter().flatten().copied()).collect();
    if used.is_empty() {
        return Err("nothing to draw".into());
    }
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for &i in &used {
        let v = view(model.vertices[i].pos);
        for k in 0..2 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    let ss = 2u32; // supersampling
    let (bw, bh) = (w * ss, h * ss);
    let margin = 0.06;
    let scale = ((bw as f32 * (1.0 - 2.0 * margin)) / (hi[0] - lo[0])).min((bh as f32 * (1.0 - 2.0 * margin)) / (hi[1] - lo[1]));
    let (cx, cyy) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
    let project = |p: [f32; 3]| -> [f32; 3] {
        let v = view(p);
        [bw as f32 / 2.0 + (v[0] - cx) * scale, bh as f32 / 2.0 + (v[1] - cyy) * scale, v[2]]
    };

    let mut color = vec![[0.0f32; 4]; (bw * bh) as usize];
    let mut depth = vec![f32::MAX; (bw * bh) as usize];
    let light = normalize([0.6, 0.35, 0.72]); // from the front, above, a little to the side
    for part in &parts {
        let tex = textures.get(&part.tex);
        for tri in &part.tris {
            let v: Vec<_> = tri.iter().map(|&i| &model.vertices[i]).collect();
            let p: Vec<[f32; 3]> = v.iter().map(|v| project(v.pos)).collect();
            let (minx, maxx) = (p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).max(0.0), p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).min(bw as f32 - 1.0));
            let (miny, maxy) = (p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).max(0.0), p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).min(bh as f32 - 1.0));
            let area = edge(p[0], p[1], p[2]);
            if area.abs() < 1e-6 || minx > maxx || miny > maxy {
                continue;
            }
            for y in miny as u32..=maxy as u32 {
                for x in minx as u32..=maxx as u32 {
                    let q = [x as f32 + 0.5, y as f32 + 0.5, 0.0];
                    let (w0, w1, w2) = (edge(p[1], p[2], q) / area, edge(p[2], p[0], q) / area, edge(p[0], p[1], q) / area);
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
                    let at = (y * bw + x) as usize;
                    if z >= depth[at] {
                        continue;
                    }
                    let uv = [
                        w0 * v[0].uv[0] + w1 * v[1].uv[0] + w2 * v[2].uv[0],
                        w0 * v[0].uv[1] + w1 * v[1].uv[1] + w2 * v[2].uv[1],
                    ];
                    let mut c = tex.map(|t| t.sample(uv[0], uv[1])).unwrap_or([0.6, 0.6, 0.6, 1.0]);
                    if part.blend == 1 && c[3] < 0.5 {
                        continue;
                    }
                    let n = normalize([
                        w0 * v[0].normal[0] + w1 * v[1].normal[0] + w2 * v[2].normal[0],
                        w0 * v[0].normal[1] + w1 * v[1].normal[1] + w2 * v[2].normal[1],
                        w0 * v[0].normal[2] + w1 * v[1].normal[2] + w2 * v[2].normal[2],
                    ]);
                    let lit = 0.58 + 0.55 * dot(n, light).max(0.0);
                    for k in 0..3 {
                        c[k] = (c[k] * lit).min(1.0);
                    }
                    let blended = part.blend >= 2;
                    if blended {
                        let dst = color[at];
                        let a = c[3];
                        color[at] = [c[0] * a + dst[0] * (1.0 - a), c[1] * a + dst[1] * (1.0 - a), c[2] * a + dst[2] * (1.0 - a), (a + dst[3] * (1.0 - a)).min(1.0)];
                    } else {
                        color[at] = [c[0], c[1], c[2], 1.0];
                        depth[at] = z;
                    }
                }
            }
        }
    }

    // Down to the output size, averaging the supersamples.
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 4];
            for dy in 0..ss {
                for dx in 0..ss {
                    let c = color[((y * ss + dy) * bw + x * ss + dx) as usize];
                    for k in 0..3 {
                        acc[k] += c[k] * c[3];
                    }
                    acc[3] += c[3];
                }
            }
            let n = (ss * ss) as f32;
            let a = acc[3] / n;
            let px = if a > 0.0 {
                [(acc[0] / acc[3] * 255.0) as u8, (acc[1] / acc[3] * 255.0) as u8, (acc[2] / acc[3] * 255.0) as u8, (a * 255.0) as u8]
            } else {
                [0, 0, 0, 0]
            };
            out.put_pixel(x, y, Rgba(px));
        }
    }
    let mut png = vec![];
    image::DynamicImage::ImageRgba8(out)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png)
}

fn edge(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    (c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

#[cfg(test)]
mod tests {
    /// FM_OUT=dir [FM_RACE=Scourge] [FM_FEMALE=1] cargo test --release character_png -- --ignored
    #[test]
    #[ignore]
    fn character_png() {
        let game = crate::platform::home().join("Games/battlenet/drive_c/Program Files (x86)/World of Warcraft");
        let s = super::super::Storage::open(&game, "wow_classic_beta").unwrap();
        let race = std::env::var("FM_RACE").unwrap_or("Scourge".into());
        let female = std::env::var("FM_FEMALE").is_ok();
        let t = std::time::Instant::now();
        let png = super::render(&s, &race, female, 600, 900).unwrap();
        println!("rendered in {:?}", t.elapsed());
        std::fs::write(std::path::Path::new(&std::env::var("FM_OUT").unwrap()).join(format!("char-{race}-{}.png", if female { "f" } else { "m" })), png).unwrap();
    }
}

#[cfg(test)]
mod sections {
    #[test]
    #[ignore]
    fn section_bounds() {
        let game = crate::platform::home().join("Games/battlenet/drive_c/Program Files (x86)/World of Warcraft");
        let s = super::super::Storage::open(&game, "wow_classic_beta").unwrap();
        let m = super::Model::parse(&s.read(959310).unwrap()).unwrap();
        let sk = super::Skin::parse(&s.read(m.skins[0]).unwrap()).unwrap();
        for sec in &sk.sections {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for t in sec.index_start..sec.index_start + sec.index_count {
                let v = m.vertices[sk.vertices[sk.indices[t as usize] as usize] as usize].pos;
                for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); }
            }
            println!("{:5} tris {:6} y {:6.2}..{:6.2} z {:5.2}..{:5.2}", sec.id, sec.index_count / 3, lo[1], hi[1], lo[2], hi[2]);
        }
    }
}
