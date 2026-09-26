//! M2 models (the chunked MD21 format of modern clients) and their .skin
//! files: vertices, textures, materials and the submeshes ("geosets") a
//! character is assembled from.

use super::{bytes, le32};

pub struct Model {
    pub vertices: Vec<Vertex>,
    pub textures: Vec<Texture>,
    /// Texture lookup: batch texture combo index -> texture index.
    pub texture_lookup: Vec<u16>,
    /// Blend mode per material.
    pub materials: Vec<u16>,
    /// .skin files, by level of detail (SFID chunk).
    pub skins: Vec<u32>,
    pub bounds: ([f32; 3], [f32; 3]),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct Texture {
    /// 0: a file of its own (see `file`), 1: the character's body skin,
    /// 2: cape, 6: hair, 8: fur, and more.
    pub kind: u32,
    pub file: u32,
}

pub struct Skin {
    /// Model vertex index for each skin vertex.
    pub vertices: Vec<u16>,
    pub indices: Vec<u16>,
    pub sections: Vec<Section>,
    pub batches: Vec<Batch>,
}

#[derive(Clone, Copy, Debug)]
pub struct Section {
    /// Geoset: group * 100 + variant; 0 is the body.
    pub id: u16,
    pub index_start: u32,
    pub index_count: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Batch {
    pub section: u16,
    pub material: u16,
    pub texture_combo: u16,
    pub texture_count: u16,
}

fn f32le(b: &[u8], at: usize) -> Result<f32, String> {
    Ok(f32::from_bits(le32(b, at)?))
}

fn u16le(b: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(bytes(b, at, 2)?.try_into().unwrap()))
}

/// An M2Array: count and offset, relative to the start of MD20.
fn array(b: &[u8], at: usize) -> Result<(usize, usize), String> {
    Ok((le32(b, at)? as usize, le32(b, at + 4)? as usize))
}

impl Model {
    pub fn parse(file: &[u8]) -> Result<Model, String> {
        // Chunks: MD21 holds the classic MD20 body; SFID, TXID and others follow.
        let mut md20: &[u8] = &[];
        let mut skins = vec![];
        let mut txid = vec![];
        let mut pos = 0;
        while pos + 8 <= file.len() {
            let magic = bytes(file, pos, 4)?;
            let size = le32(file, pos + 4)? as usize;
            let body = bytes(file, pos + 8, size)?;
            match magic {
                b"MD21" => md20 = body,
                b"SFID" => skins = body.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect(),
                b"TXID" => txid = body.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<u32>>(),
                _ => {}
            }
            pos += 8 + size;
        }
        if !md20.starts_with(b"MD20") {
            return Err("not an MD21 model".into());
        }
        let b = md20;
        let (nv, ov) = array(b, 60)?;
        let mut vertices = Vec::with_capacity(nv);
        for i in 0..nv {
            let at = ov + i * 48;
            vertices.push(Vertex {
                pos: [f32le(b, at)?, f32le(b, at + 4)?, f32le(b, at + 8)?],
                normal: [f32le(b, at + 20)?, f32le(b, at + 24)?, f32le(b, at + 28)?],
                uv: [f32le(b, at + 32)?, f32le(b, at + 36)?],
            });
        }
        let (nt, ot) = array(b, 80)?;
        let mut textures = vec![];
        for i in 0..nt {
            let kind = le32(b, ot + i * 16)?;
            textures.push(Texture { kind, file: txid.get(i).copied().unwrap_or(0) });
        }
        let (nl, ol) = array(b, 128)?;
        let texture_lookup = (0..nl).map(|i| u16le(b, ol + i * 2)).collect::<Result<_, _>>()?;
        let (nm, om) = array(b, 112)?;
        let materials = (0..nm).map(|i| u16le(b, om + i * 4 + 2)).collect::<Result<_, _>>()?;
        let bounds = (
            [f32le(b, 160)?, f32le(b, 164)?, f32le(b, 168)?],
            [f32le(b, 172)?, f32le(b, 176)?, f32le(b, 180)?],
        );
        Ok(Model { vertices, textures, texture_lookup, materials, skins, bounds })
    }
}

impl Skin {
    pub fn parse(b: &[u8]) -> Result<Skin, String> {
        if !b.starts_with(b"SKIN") {
            return Err("not a .skin file".into());
        }
        let (nv, ov) = array(b, 4)?;
        let vertices = (0..nv).map(|i| u16le(b, ov + i * 2)).collect::<Result<_, _>>()?;
        let (ni, oi) = array(b, 12)?;
        let indices = (0..ni).map(|i| u16le(b, oi + i * 2)).collect::<Result<_, _>>()?;
        let (ns, os) = array(b, 28)?;
        let mut sections = vec![];
        for i in 0..ns {
            let at = os + i * 48;
            let level = u16le(b, at + 2)? as u32;
            sections.push(Section {
                id: u16le(b, at)?,
                index_start: u16le(b, at + 8)? as u32 + (level << 16),
                index_count: u16le(b, at + 10)? as u32,
            });
        }
        let (nb, ob) = array(b, 36)?;
        let mut batches = vec![];
        for i in 0..nb {
            let at = ob + i * 24;
            batches.push(Batch {
                section: u16le(b, at + 4)?,
                material: u16le(b, at + 12)?,
                texture_count: u16le(b, at + 14)?,
                texture_combo: u16le(b, at + 16)?,
            });
        }
        Ok(Skin { vertices, indices, sections, batches })
    }
}

#[cfg(test)]
mod tests {
    /// cargo test --release m2_dump -- --ignored --nocapture
    #[test]
    #[ignore]
    fn m2_dump() {
        let game = crate::platform::home().join("Games/battlenet/drive_c/Program Files (x86)/World of Warcraft");
        let s = super::super::Storage::open(&game, "wow_classic_beta").unwrap();
        let id: u32 = std::env::var("FM_M2").ok().and_then(|v| v.parse().ok()).unwrap_or(959310);
        let m = super::Model::parse(&s.read(id).unwrap()).unwrap();
        println!("vertices {} bounds {:?}", m.vertices.len(), m.bounds);
        for (i, t) in m.textures.iter().enumerate() {
            println!("texture {i}: kind {} file {}", t.kind, t.file);
        }
        println!("lookup {:?}", m.texture_lookup);
        println!("materials {:?}", m.materials);
        println!("skins {:?}", m.skins);
        let sk = super::Skin::parse(&s.read(m.skins[0]).unwrap()).unwrap();
        let ids: Vec<u16> = sk.sections.iter().map(|x| x.id).collect();
        println!("sections {:?}", ids);
        for b in &sk.batches {
            println!("batch section {} (geoset {}) material {} tex combo {} count {}", b.section, sk.sections[b.section as usize].id, b.material, b.texture_combo, b.texture_count);
        }
    }
}

#[cfg(test)]
mod dims {
    /// FM_IDS="1 2 3" cargo test --release blp_dims -- --ignored --nocapture
    #[test]
    #[ignore]
    fn blp_dims() {
        let game = crate::platform::home().join("Games/battlenet/drive_c/Program Files (x86)/World of Warcraft");
        let s = super::super::Storage::open(&game, "wow_classic_beta").unwrap();
        for id in std::env::var("FM_IDS").unwrap().split_whitespace() {
            let id: u32 = id.parse().unwrap();
            match s.read(id) {
                Ok(b) => println!("{id}: {}x{} type {} comp {} alpha {} {}", u32::from_le_bytes(b[12..16].try_into().unwrap()), u32::from_le_bytes(b[16..20].try_into().unwrap()), b[4], b[8], b[9], b[10]),
                Err(e) => println!("{id}: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod peek {
    /// FM_IDS="1 2" FM_OUT=dir cargo test --release blp_png -- --ignored
    #[test]
    #[ignore]
    fn blp_png() {
        let game = crate::platform::home().join("Games/battlenet/drive_c/Program Files (x86)/World of Warcraft");
        let s = super::super::Storage::open(&game, "wow_classic_beta").unwrap();
        let out = std::path::PathBuf::from(std::env::var("FM_OUT").unwrap());
        for id in std::env::var("FM_IDS").unwrap().split_whitespace() {
            let img = super::super::blp::decode(&s.read(id.parse().unwrap()).unwrap()).unwrap();
            img.save(out.join(format!("{id}.png"))).unwrap();
        }
    }
}
