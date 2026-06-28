//! Companion to part 1's "The fix: put the bytes on the heap".
//!
//! The leak demo in `main.rs` moves a secret around on the stack and leaves
//! several DEADBEEF fossils behind that survive the `Zeroizing` wipe. This one
//! does the opposite: it allocates the secret on the heap and fills it *in
//! place*, so the 32-byte pattern only ever exists at a single fixed address —
//! one copy to wipe, and no stack fossils to find.
//!
//! Run:   cargo run --bin heap_fix
//! Scan:  sudo python3 scan_mem.py <PID>   (once per window the program prints)
//!
//! Expect ONE hit while the key is live, and ZERO after it drops — versus the
//! four the stack-move demo leaves behind.

use std::hint::black_box;
use zeroize::Zeroizing;

/// Fill the secret straight into the caller's buffer. Because it takes
/// `&mut [u8; 32]` and never returns the array by value, the 32-byte pattern
/// is never materialized in a stack temporary the way `load_key() -> [u8; 32]`
/// (in `main.rs`) is.
fn load_key_into(buf: &mut [u8; 32]) {
    for (i, chunk) in buf.chunks_exact_mut(4).enumerate() {
        chunk.copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        black_box(i); // prevent the loop from being const-folded
    }
}

fn main() {
    let pid = std::process::id();

    // Allocate the heap slot first (zeros — harmless on the stack), then fill
    // it in place. The secret only ever lives at this one fixed heap address.
    let mut key = Box::new(Zeroizing::new([0u8; 32]));
    load_key_into(&mut key);
    black_box(&*key); // pretend we used it; keep the allocation observed/live

    eprintln!("PID {pid} — secret is LIVE on the heap.");
    eprintln!("  Scan now — exactly ONE copy, no stack fossils:");
    eprintln!("  sudo python3 scan_mem.py {pid}");
    eprintln!("Sleeping 20 s …");
    std::thread::sleep(std::time::Duration::from_secs(20));

    // Wipe the single heap copy via Zeroizing's Drop.
    drop(key);

    eprintln!("PID {pid} — secret dropped (the one heap copy was zeroized).");
    eprintln!("  Scan again — now ZERO copies:");
    eprintln!("  sudo python3 scan_mem.py {pid}");
    eprintln!("Sleeping 20 s …");
    std::thread::sleep(std::time::Duration::from_secs(20));
}
