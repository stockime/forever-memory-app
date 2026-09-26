//! BLTE, the container every CASC blob is stored in: one or more chunks,
//! each plain (N), zlib (Z), nested BLTE (F) or encrypted (E).

use super::{be32, bytes};
use std::io::Read;

pub fn decode(b: &[u8]) -> Result<Vec<u8>, String> {
    if b.len() < 8 || &b[0..4] != b"BLTE" {
        return Err("not BLTE".into());
    }
    let header_size = be32(b, 4)? as usize;
    if header_size == 0 {
        let mut out = Vec::new();
        chunk(&b[8..], &mut out)?;
        return Ok(out);
    }
    let count = (be32(b, 8)? & 0xffffff) as usize;
    let total: usize = (0..count)
        .map(|i| be32(b, 12 + i * 24 + 4).map(|n| n as usize))
        .sum::<Result<_, _>>()?;
    let mut out = Vec::with_capacity(total.min(1 << 28)); // a damaged table must not abort on allocation
    let mut pos = header_size;
    for i in 0..count {
        let size = be32(b, 12 + i * 24)? as usize;
        chunk(bytes(b, pos, size)?, &mut out)?;
        pos += size;
    }
    Ok(out)
}

fn chunk(c: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
    match c.first() {
        Some(b'N') => out.extend_from_slice(&c[1..]),
        Some(b'Z') => {
            flate2::read::ZlibDecoder::new(&c[1..])
                .read_to_end(out)
                .map_err(|e| format!("BLTE zlib: {e}"))?;
        }
        Some(b'F') => out.extend(decode(&c[1..])?),
        Some(b'E') => return Err("encrypted BLTE chunk".into()),
        Some(m) => return Err(format!("unknown BLTE chunk mode {:?}", *m as char)),
        None => return Err("empty BLTE chunk".into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::decode;
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    /// A BLTE blob with a chunk table: header size, flags + count, then
    /// (compressed size, decompressed size, md5) per chunk.
    fn framed(chunks: &[(Vec<u8>, usize)]) -> Vec<u8> {
        let header = 12 + chunks.len() * 24;
        let mut b = b"BLTE".to_vec();
        b.extend((header as u32).to_be_bytes());
        b.extend((0x0f00_0000 | chunks.len() as u32).to_be_bytes());
        for (c, n) in chunks {
            b.extend((c.len() as u32).to_be_bytes());
            b.extend((*n as u32).to_be_bytes());
            b.extend([0; 16]);
        }
        for (c, _) in chunks {
            b.extend(c);
        }
        b
    }

    #[test]
    fn single_plain_chunk() {
        let mut b = b"BLTE\0\0\0\0N".to_vec();
        b.extend(b"hello");
        assert_eq!(decode(&b).unwrap(), b"hello");
    }

    #[test]
    fn chunk_table_with_zlib_and_nested() {
        let mut z = b"Z".to_vec();
        z.extend(zlib(b"world, "));
        let mut inner = b"BLTE\0\0\0\0N".to_vec();
        inner.extend(b"again");
        let mut f = b"F".to_vec();
        f.extend(&inner);
        let b = framed(&[(b"Nhello ".to_vec(), 6), (z, 7), (f, 5)]);
        assert_eq!(decode(&b).unwrap(), b"hello world, again");
    }

    #[test]
    fn rejects_encrypted_and_damaged_data() {
        assert!(
            decode(b"BLTE\0\0\0\0Exxxx")
                .unwrap_err()
                .contains("encrypted")
        );
        assert!(decode(b"BLTX\0\0\0\0Nhi").is_err());
        let mut b = framed(&[(b"Nhello".to_vec(), 5)]);
        b.truncate(b.len() - 2);
        assert!(decode(&b).is_err());
    }
}
