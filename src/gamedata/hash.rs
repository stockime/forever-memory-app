//! The root manifest's path hash: Bob Jenkins' hashlittle2 over the
//! upper-cased path with backslashes, as (primary << 32 | secondary).

pub fn name_hash(path: &str) -> u64 {
    let k = path.replace('/', "\\").to_ascii_uppercase().into_bytes();
    let word = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let mut a = 0xdeadbeef_u32.wrapping_add(k.len() as u32);
    let (mut b, mut c) = (a, a);
    let mut k = &k[..];
    while k.len() > 12 {
        a = a.wrapping_add(word(&k[0..]));
        b = b.wrapping_add(word(&k[4..]));
        c = c.wrapping_add(word(&k[8..]));
        a = a.wrapping_sub(c) ^ c.rotate_left(4);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a) ^ a.rotate_left(6);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b) ^ b.rotate_left(8);
        b = b.wrapping_add(a);
        a = a.wrapping_sub(c) ^ c.rotate_left(16);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a) ^ a.rotate_left(19);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b) ^ b.rotate_left(4);
        b = b.wrapping_add(a);
        k = &k[12..];
    }
    if k.is_empty() {
        return (c as u64) << 32 | b as u64;
    }
    let mut t = [0u8; 12];
    t[..k.len()].copy_from_slice(k);
    a = a.wrapping_add(word(&t[0..]));
    b = b.wrapping_add(word(&t[4..]));
    c = c.wrapping_add(word(&t[8..]));
    c = (c ^ b).wrapping_sub(b.rotate_left(14));
    a = (a ^ c).wrapping_sub(c.rotate_left(11));
    b = (b ^ a).wrapping_sub(a.rotate_left(25));
    c = (c ^ b).wrapping_sub(b.rotate_left(16));
    a = (a ^ c).wrapping_sub(c.rotate_left(4));
    b = (b ^ a).wrapping_sub(a.rotate_left(14));
    c = (c ^ b).wrapping_sub(b.rotate_left(24));
    (c as u64) << 32 | b as u64
}

#[cfg(test)]
mod tests {
    use super::name_hash;

    #[test]
    fn matches_the_go_implementation() {
        // Values from stru.ci/wow/casc.NameHash.
        let cases = [
            ("", 0xdeadbeefdeadbeef),
            ("a", 0x01014ba110786e8c),
            ("abcdefghijkl", 0x4dcc6ecf4f3dc944),
            ("abcdefghijklm", 0xef91ecec95071727),
            ("interface/icons/classicon_paladin.blp", 0x0a32217b081d2064),
            (
                "Interface\\PaperDoll\\UI-PaperDoll-Slot-Head.blp",
                0x67ae0b571b0daf7c,
            ),
        ];
        for (path, want) in cases {
            assert_eq!(name_hash(path), want, "{path}");
        }
    }

    #[test]
    fn ignores_case_and_slash_direction() {
        assert_eq!(
            name_hash("interface/icons/inv_misc_questionmark.blp"),
            name_hash("INTERFACE\\ICONS\\INV_MISC_QUESTIONMARK.BLP")
        );
    }
}
