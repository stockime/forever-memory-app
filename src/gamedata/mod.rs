//! Game data read in-process from a local World of Warcraft install: CASC
//! storage, BLTE blobs and BLP textures, rendered into the images the app
//! shows. Works the same on Linux, macOS and Windows; no external tools.

#![allow(dead_code, unused_imports)]

mod blp;
mod blte;
mod casc;
mod cdn;
mod hash;
mod render;

pub use casc::{Storage, products};
pub use render::render;

/// Bounds-checked little- and big-endian reads over game files, so damaged
/// data turns into an error instead of a panic.
fn bytes(b: &[u8], at: usize, n: usize) -> Result<&[u8], String> {
    at.checked_add(n)
        .and_then(|end| b.get(at..end))
        .ok_or_else(|| format!("truncated data at {at}"))
}

fn le32(b: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(bytes(b, at, 4)?.try_into().unwrap()))
}

fn be32(b: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(bytes(b, at, 4)?.try_into().unwrap()))
}

fn be16(b: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_be_bytes(bytes(b, at, 2)?.try_into().unwrap()))
}
