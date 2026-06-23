// Try this on https://godbolt.org with rustc + -O2
// Look for mov [rsp+...] instructions that store secret bytes
// to the stack — those are register spills you can't name or wipe.

use std::hint::black_box;

#[inline(never)]
fn process_secret(secret: [u8; 32]) -> u64 {
    let mut a = u32::from_le_bytes([secret[0], secret[1], secret[2], secret[3]]);
    let mut b = u32::from_le_bytes([secret[4], secret[5], secret[6], secret[7]]);
    let mut c = u32::from_le_bytes([secret[8], secret[9], secret[10], secret[11]]);
    let mut d = u32::from_le_bytes([secret[12], secret[13], secret[14], secret[15]]);
    let mut e = u32::from_le_bytes([secret[16], secret[17], secret[18], secret[19]]);
    let mut f = u32::from_le_bytes([secret[20], secret[21], secret[22], secret[23]]);
    let mut g = u32::from_le_bytes([secret[24], secret[25], secret[26], secret[27]]);
    let mut h = u32::from_le_bytes([secret[28], secret[29], secret[30], secret[31]]);

    for _ in 0..10 {
        a = a.wrapping_add(b).wrapping_mul(0x9e3779b9).rotate_left(5);
        b = b.wrapping_add(c).wrapping_mul(0x517cc1b7).rotate_left(7);
        c = c.wrapping_add(d).wrapping_mul(0x6a09e667).rotate_left(11);
        d = d.wrapping_add(e).wrapping_mul(0xbb67ae85).rotate_left(13);
        e = e.wrapping_add(f).wrapping_mul(0x3c6ef372).rotate_left(17);
        f = f.wrapping_add(g).wrapping_mul(0xa54ff53a).rotate_left(19);
        g = g.wrapping_add(h).wrapping_mul(0x510e527f).rotate_left(23);
        h = h.wrapping_add(a).wrapping_mul(0x1f83d9ab).rotate_left(29);
    }

    (a as u64)
        ^ ((b as u64) << 1)
        ^ ((c as u64) << 2)
        ^ ((d as u64) << 3)
        ^ ((e as u64) << 4)
        ^ ((f as u64) << 5)
        ^ ((g as u64) << 6)
        ^ ((h as u64) << 7)
}

fn main() {
    let secret = black_box([0xDEu8; 32]);
    let result = process_secret(secret);
    println!("{result}");
}
