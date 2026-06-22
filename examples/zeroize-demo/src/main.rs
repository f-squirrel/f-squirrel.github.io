use std::hint::black_box;
use zeroize::Zeroizing;

fn load_key() -> [u8; 32] {
    // Simulate loading a secret — built at runtime so it doesn't
    // end up in .rodata the way a const would.
    let mut key = [0u8; 32];
    for (i, chunk) in key.chunks_exact_mut(4).enumerate() {
        chunk.copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        black_box(i); // prevent the loop from being const-folded
    }
    key
}

fn main() {
    let pid = std::process::id();

    // Create a Zeroizing secret at stack slot A.
    let a = Zeroizing::new(load_key());

    // Move to slot B — the compiler bitwise-copies the bytes.
    let b = black_box(a);

    // Wipes slot B via Zeroizing's Drop.
    drop(b);

    eprintln!("PID {pid} — secret dropped. Scan memory now:");
    eprintln!("  sudo python3 scan_mem.py {pid}");
    eprintln!("Sleeping 30 s …");
    std::thread::sleep(std::time::Duration::from_secs(30));
}
