//! Companion to part 1, "I Zeroized My Secret. Or Did I?".
//!
//! Two demos, selected by a command-line argument so each runs in its own
//! process (one PID, one behavior — which is what makes `scan_mem.py` clean):
//!
//!   cargo run -- stack   leak demo: the secret is moved around on the stack,
//!                        and several DEADBEEF fossils survive the wipe.
//!   cargo run -- heap    the fix: the secret is allocated on the heap and
//!                        filled in place — one copy while live, zero on drop.
//!
//! While either demo sleeps, scan it with:  sudo python3 scan_mem.py <PID>

use std::hint::black_box;
use zeroize::Zeroizing;

/// Build the secret *by value* — used by the stack demo. Returning `[u8; 32]`
/// means every hop (return, wrap, move) copies the bytes into a new stack slot.
fn load_key() -> [u8; 32] {
    // Built at runtime so it doesn't end up in .rodata the way a const would.
    let mut key = [0u8; 32];
    for (i, chunk) in key.chunks_exact_mut(4).enumerate() {
        chunk.copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        black_box(i); // prevent the loop from being const-folded
    }
    key
}

/// Fill the secret *in place* — used by the heap demo. Taking `&mut [u8; 32]`
/// (instead of returning by value) means the 32-byte pattern is never
/// materialized in a stack temporary.
fn load_key_into(buf: &mut [u8; 32]) {
    for (i, chunk) in buf.chunks_exact_mut(4).enumerate() {
        chunk.copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        black_box(i);
    }
}

fn sleep(secs: u64) {
    std::thread::sleep(std::time::Duration::from_secs(secs));
}

/// Stack-move leak: `zeroize` wipes only the last slot the bytes landed in;
/// the copies left along the way survive.
fn run_stack(pid: u32) {
    // Create a Zeroizing secret at stack slot A.
    let a = Zeroizing::new(load_key());
    // Move to slot B — the compiler bitwise-copies the bytes.
    let b = black_box(a);
    // Wipes slot B via Zeroizing's Drop.
    drop(b);

    eprintln!("PID {pid} — secret dropped. The stale stack copies survive the wipe:");
    eprintln!("  sudo python3 scan_mem.py {pid}");
    eprintln!("Sleeping 30 s …");
    sleep(30);
}

/// Heap fix: the secret lives at one fixed heap address, filled in place.
/// One copy while live, zero after drop — no stack fossils.
fn run_heap(pid: u32) {
    // Allocate the heap slot first (zeros — harmless on the stack), then fill
    // it in place. The secret only ever lives at this one fixed heap address.
    let mut key = Box::new(Zeroizing::new([0u8; 32]));
    load_key_into(&mut key);
    black_box(&*key); // pretend we used it; keep the allocation observed/live

    eprintln!("PID {pid} — secret is LIVE on the heap.");
    eprintln!("  Scan now — exactly ONE copy, no stack fossils:");
    eprintln!("  sudo python3 scan_mem.py {pid}");
    eprintln!("Sleeping 20 s …");
    sleep(20);

    // Wipe the single heap copy via Zeroizing's Drop.
    drop(key);

    eprintln!("PID {pid} — secret dropped (the one heap copy was zeroized).");
    eprintln!("  Scan again — now ZERO copies:");
    eprintln!("  sudo python3 scan_mem.py {pid}");
    eprintln!("Sleeping 20 s …");
    sleep(20);
}

fn main() {
    let pid = std::process::id();
    match std::env::args().nth(1).as_deref() {
        Some("stack") => run_stack(pid),
        Some("heap") => run_heap(pid),
        _ => {
            eprintln!("usage: zeroize-demo <stack|heap>");
            eprintln!("  stack  leak demo — secret moved on the stack; fossils survive the wipe");
            eprintln!("  heap   the fix — secret on the heap, filled in place; no fossils");
            std::process::exit(2);
        }
    }
}
