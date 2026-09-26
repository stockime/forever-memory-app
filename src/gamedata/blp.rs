//! The first mip level of BLP2 textures (palette, DXT1/3/5 and raw BGRA),
//! which covers the game's icons and UI art.

use super::{bytes, le32};
use image::{Rgba, RgbaImage};

pub fn decode(b: &[u8]) -> Result<RgbaImage, String> {
    if b.len() < 148 || &b[0..4] != b"BLP2" {
        return Err("not a BLP2 file".into());
    }
    let (encoding, alpha_depth, alpha_type) = (b[8], b[9], b[10]);
    let (w, h) = (le32(b, 12)?, le32(b, 16)?);
    let (off, size) = (le32(b, 20)? as usize, le32(b, 84)? as usize);
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return Err(format!("bad BLP size {w}x{h}"));
    }
    let data = bytes(b, off, size).map_err(|_| "truncated BLP")?;
    let mut img = RgbaImage::new(w, h);
    match encoding {
        1 => palette(&mut img, data, bytes(b, 148, 1024)?, alpha_depth),
        2 => match alpha_type {
            0 => dxt(&mut img, data, 1, alpha_depth > 0),
            1 => dxt(&mut img, data, 3, true),
            7 => dxt(&mut img, data, 5, true),
            t => return Err(format!("unsupported DXT alpha type {t}")),
        },
        3 => {
            for (px, bgra) in img.pixels_mut().zip(data.as_chunks::<4>().0) {
                *px = Rgba([bgra[2], bgra[1], bgra[0], bgra[3]]);
            }
        }
        e => return Err(format!("unsupported BLP encoding {e}")),
    }
    Ok(img)
}

/// One index byte per pixel, then the alpha plane at 1, 4 or 8 bits.
fn palette(img: &mut RgbaImage, data: &[u8], palette: &[u8], alpha_depth: u8) {
    let n = (img.width() * img.height()) as usize;
    for (i, px) in img.pixels_mut().enumerate().take(n.min(data.len())) {
        let p = data[i] as usize * 4;
        let a = match alpha_depth {
            1 => match data.get(n + i / 8) {
                Some(v) if v >> (i % 8) & 1 == 0 => 0,
                _ => 255,
            },
            4 => data
                .get(n + i / 2)
                .map_or(255, |v| (v >> (4 * (i % 2)) & 0xf) * 17),
            8 => data.get(n + i).copied().unwrap_or(255),
            _ => 255,
        };
        *px = Rgba([palette[p + 2], palette[p + 1], palette[p], a]);
    }
}

fn rgb565(v: u16) -> Rgba<u8> {
    let (r, g, b) = (v >> 11 & 31, v >> 5 & 63, v & 31);
    Rgba([
        (r << 3 | r >> 2) as u8,
        (g << 2 | g >> 4) as u8,
        (b << 3 | b >> 2) as u8,
        255,
    ])
}

fn mix(a: Rgba<u8>, b: Rgba<u8>, wa: u32, wb: u32, d: u32) -> Rgba<u8> {
    let c = |i: usize| ((a[i] as u32 * wa + b[i] as u32 * wb) / d) as u8;
    Rgba([c(0), c(1), c(2), 255])
}

/// DXT1/3/5: 4x4 blocks of two RGB565 endpoints and 2-bit indices, with
/// explicit 4-bit alpha (DXT3) or interpolated alpha (DXT5) in front.
fn dxt(img: &mut RgbaImage, data: &[u8], kind: u8, alpha: bool) {
    let (w, h) = (img.width(), img.height());
    let block_size = if kind == 1 { 8 } else { 16 };
    let mut blocks = data.chunks_exact(block_size);
    for by in 0..h.div_ceil(4) {
        for bx in 0..w.div_ceil(4) {
            let Some(block) = blocks.next() else {
                return;
            };
            let mut alphas = [255u8; 16];
            let colors = match kind {
                3 => {
                    for (i, a) in alphas.iter_mut().enumerate() {
                        *a = (block[i / 2] >> (4 * (i % 2)) & 0xf) * 17;
                    }
                    &block[8..]
                }
                5 => {
                    let (a0, a1) = (block[0] as u32, block[1] as u32);
                    let mut table = [a0 as u8, a1 as u8, 0, 0, 0, 0, 0, 0];
                    if a0 > a1 {
                        for i in 1..7 {
                            table[i + 1] = (((7 - i as u32) * a0 + i as u32 * a1) / 7) as u8;
                        }
                    } else {
                        for i in 1..5 {
                            table[i + 1] = (((5 - i as u32) * a0 + i as u32 * a1) / 5) as u8;
                        }
                        table[7] = 255;
                    }
                    let bits = block[2..8]
                        .iter()
                        .rev()
                        .fold(0u64, |acc, &v| acc << 8 | v as u64);
                    for (i, a) in alphas.iter_mut().enumerate() {
                        *a = table[(bits >> (3 * i) & 7) as usize];
                    }
                    &block[8..]
                }
                _ => block,
            };
            let c0v = u16::from_le_bytes([colors[0], colors[1]]);
            let c1v = u16::from_le_bytes([colors[2], colors[3]]);
            let (c0, c1) = (rgb565(c0v), rgb565(c1v));
            let pal = if c0v > c1v || kind != 1 {
                [c0, c1, mix(c0, c1, 2, 1, 3), mix(c0, c1, 1, 2, 3)]
            } else {
                [
                    c0,
                    c1,
                    mix(c0, c1, 1, 1, 2),
                    Rgba([0, 0, 0, if alpha { 0 } else { 255 }]),
                ]
            };
            let idx = u32::from_le_bytes([colors[4], colors[5], colors[6], colors[7]]);
            for i in 0..16 {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x >= w || y >= h {
                    continue;
                }
                let mut c = pal[(idx >> (2 * i) & 3) as usize];
                if kind != 1 {
                    c[3] = alphas[i as usize];
                }
                img.put_pixel(x, y, c);
            }
        }
    }
}
