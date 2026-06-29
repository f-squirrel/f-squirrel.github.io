//! Working companion to the blog post
//! "Where `zeroize` stops: keeping keys out of swap, dumps, and other processes".
//!
//! Two key buffers:
//!
//! * `HardenedKey` — heap Box + `mlock`. No `MADV_DONTDUMP`: it
//!   requires a page-aligned address, which a `Box` doesn't provide. Dump
//!   coverage comes from the process-wide `RLIMIT_CORE = 0` +
//!   `PR_SET_DUMPABLE`.
//! * `PageProtectedKey` — dedicated page via `region::alloc`, locked,
//!   `MADV_DONTDUMP`'d, kept `PROT_NONE` at rest, briefly flipped to
//!   `PROT_READ` inside `with_readable`.
//!
//! And one process-level startup helper:
//!
//! * `harden_process()` — `RLIMIT_CORE = 0` + `PR_SET_DUMPABLE`, which
//!   also blocks `ptrace` for non-root.
//!
//! Modes:
//!
//!   (no args)            Run both key types, print checksums (default)
//!   live                 Load key unhardened, sleep for /proc/pid/mem scanning
//!   live-hardened        Load key hardened, sleep for scanning
//!   live-page-protected  PageProtectedKey: PROT_NONE at rest, briefly unsealed
//!   crash                Load key unhardened, abort (core dump demo)
//!   crash-hardened       Load key hardened, abort (core dump demo)
//!
//! See README.md for system setup and full walkthrough.

use std::ffi::c_void;
use std::hint::black_box;
use std::io;

use region::Protection;
use zeroize::{Zeroize, Zeroizing};

const KEY_LEN: usize = 32;

// --------------------------------------------------------------------------
// Process-level hardening.
// --------------------------------------------------------------------------

fn harden_process() {
    // Best-effort: a startup hardening helper shouldn't refuse to run because
    // a container's seccomp profile forbids one syscall. But log every
    // failure so a hardened-looking process can't quietly be a soft target.

    // Disable core dumps (soft = hard = 0). systemd-coredump can ignore this
    // by piping crashes, which is exactly what the next call quiets.
    if let Err(e) = rlimit::setrlimit(rlimit::Resource::CORE, 0, 0) {
        eprintln!("warn: failed to disable core dumps: {e}");
    }

    // PR_SET_DUMPABLE = 0 makes the process non-dumpable (the systemd path
    // honours this) and as a bonus blocks ptrace attach by non-root callers.
    if let Err(e) = prctl::set_dumpable(false) {
        eprintln!("warn: failed to mark process non-dumpable: {e}");
    }
}

// --------------------------------------------------------------------------
// HardenedKey (Box + mlock).
// --------------------------------------------------------------------------

/// A 32-byte key on the heap, wiped on drop and pinned in RAM. Stable address
/// survives moves of `Self`. Core-dump exclusion for *this* buffer is not
/// per-region (the Box isn't page-aligned, so no `MADV_DONTDUMP`) — it comes
/// from the process-wide `harden_process()` knobs.
pub struct HardenedKey {
    bytes: Box<Zeroizing<[u8; KEY_LEN]>>,
    _lock: region::LockGuard,
}

impl HardenedKey {
    pub fn load(init: impl FnOnce(&mut [u8; KEY_LEN])) -> io::Result<Self> {
        let mut bytes = Box::new(Zeroizing::new([0u8; KEY_LEN]));
        init(&mut bytes);

        let ptr: *const u8 = bytes.as_ptr();
        let len: usize = bytes.len();

        // mlock is fine on a heap Box: Linux rounds the address down to the
        // page boundary internally. So this pins the (whole) page our 32-byte
        // secret happens to live on.
        let lock = region::lock(ptr, len).map_err(io_err)?;

        // NOTE: We don't call madvise(MADV_DONTDUMP) here. The kernel
        // requires the address to be page-aligned, and `Box`'s allocator
        // alignment isn't. Per-region MADV_DONTDUMP needs a page-aligned
        // allocation — that's `PageProtectedKey` below (via `region::alloc`).
        // For *this* buffer the dump coverage comes from process-wide
        // RLIMIT_CORE=0 + PR_SET_DUMPABLE.

        Ok(Self { bytes, _lock: lock })
    }

    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.bytes
    }
}

// --------------------------------------------------------------------------
// PageProtectedKey (region::alloc + mprotect bracket).
// --------------------------------------------------------------------------

/// A 32-byte key that owns a whole dedicated page, kept `PROT_NONE` while
/// idle. Briefly flipped to `PROT_READ` inside `with_readable`.
///
/// Field order matters: the custom `Drop` runs first (re-opens the page,
/// wipes), then fields drop in declaration order — `_lock` (munlock) before
/// `alloc` (munmap), so the unlock lands on still-mapped memory.
pub struct PageProtectedKey {
    _lock: region::LockGuard,
    alloc: region::Allocation,
}

impl PageProtectedKey {
    pub fn load(init: impl FnOnce(&mut [u8])) -> io::Result<Self> {
        // Whole page, initially writable so we can fill it.
        let mut alloc =
            region::alloc(KEY_LEN, Protection::READ_WRITE).map_err(io_err)?;
        let ptr: *const u8 = alloc.as_ptr::<u8>();
        let len: usize = alloc.len(); // page-rounded (typically 4096)

        // Pin the whole page in RAM, exclude it from dumps.
        let lock = region::lock(ptr, len).map_err(io_err)?;
        // SAFETY: `ptr`/`len` describe a live, owned, page-aligned mmap region.
        unsafe {
            os_memlock::madvise_dontdump(ptr as *mut c_void, len)?;
        }

        // Fill in the secret through the only window we'll allow.
        // SAFETY: alloc is page-rounded and at least KEY_LEN bytes.
        {
            let bytes = unsafe {
                std::slice::from_raw_parts_mut(alloc.as_mut_ptr::<u8>(), KEY_LEN)
            };
            init(bytes);
        }

        // Seal: no access until something deliberately opens it.
        // SAFETY: `ptr`/`len` describe the same live mapping; the slice from
        // above has gone out of scope, so flipping to PROT_NONE doesn't
        // dangle any reference.
        unsafe {
            region::protect(ptr, len, Protection::NONE).map_err(io_err)?;
        }

        Ok(Self { _lock: lock, alloc })
    }

    /// Briefly flip the page to read-only, hand the secret to `f`, then
    /// the returned `ProtectGuard` restores `PROT_NONE` at scope exit.
    /// `&mut self` so the borrow checker forbids concurrent calls on the
    /// same key — two callers would otherwise race the seal/unseal.
    pub fn with_readable<R>(&mut self, f: impl FnOnce(&[u8]) -> R) -> io::Result<R> {
        // SAFETY: same live mapping; the guard restores PROT_NONE on drop.
        let _open = unsafe {
            region::protect_with_handle(
                self.alloc.as_ptr::<u8>(),
                self.alloc.len(),
                Protection::READ,
            )
            .map_err(io_err)?
        };
        // SAFETY: page is now PROT_READ for the duration of `_open`.
        let bytes =
            unsafe { std::slice::from_raw_parts(self.alloc.as_ptr::<u8>(), KEY_LEN) };
        Ok(f(bytes))
    }
}

impl Drop for PageProtectedKey {
    fn drop(&mut self) {
        // Re-open writable so the wipe can land, then let `_lock` munlock
        // and `alloc` munmap. Best-effort: if mprotect fails the program is
        // already on its way out; the worst case is the page wasn't wiped
        // before munmap reclaimed it.
        // SAFETY: same live mapping; we're about to write KEY_LEN bytes.
        unsafe {
            let _ = region::protect(
                self.alloc.as_ptr::<u8>(),
                self.alloc.len(),
                Protection::READ_WRITE,
            );
            let bytes =
                std::slice::from_raw_parts_mut(self.alloc.as_mut_ptr::<u8>(), KEY_LEN);
            bytes.zeroize();
        }
    }
}

// --------------------------------------------------------------------------
// Helpers.
// --------------------------------------------------------------------------

fn io_err<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

/// Fill `buf` with the DEADBEEF pattern part 1's demo uses, so the
/// scan_mem.py helper in that example would also find this one.
fn load_demo_key(buf: &mut [u8]) {
    let pat = [0xDEu8, 0xAD, 0xBE, 0xEF];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = pat[i % 4];
    }
}

/// Stand in for a real crypto operation. The compiler can't elide the read
/// because the checksum is observed.
fn fake_sign(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0u64, |acc, &b| acc.wrapping_add(b as u64))
}

fn sleep(secs: u64) {
    std::thread::sleep(std::time::Duration::from_secs(secs));
}

/// Print the VmLck line from /proc/self/status to confirm mlock worked.
fn print_vmlck(pid: u32) {
    if let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) {
        for line in status.lines() {
            if line.starts_with("VmLck:") {
                println!("  {line}");
                return;
            }
        }
    }
    println!("  (VmLck not available on this platform)");
}

// --------------------------------------------------------------------------
// Demo modes.
// --------------------------------------------------------------------------

/// Default: run both key types, print matching checksums.
fn run_checksums() -> io::Result<()> {
    harden_process();

    println!("=== HardenedKey (Box + mlock) ===");
    {
        let key = HardenedKey::load(|buf| load_demo_key(buf))?;
        let sum = fake_sign(key.as_bytes());
        println!("  checksum = 0x{sum:016x}");
    }

    println!(
        "=== PageProtectedKey (region::alloc + mlock + MADV_DONTDUMP + mprotect) ==="
    );
    {
        let mut key = PageProtectedKey::load(load_demo_key)?;
        let sum = key.with_readable(fake_sign)?;
        println!("  checksum = 0x{sum:016x}");
    }

    println!("done.");
    Ok(())
}

/// Load key and sleep, with or without hardening.  The reader scans
/// /proc/<pid>/mem from another terminal.
fn run_live(harden: bool) -> io::Result<()> {
    let label = if harden { "HARDENED" } else { "UNHARDENED" };

    if harden {
        harden_process();
        println!("Process hardened (RLIMIT_CORE=0, PR_SET_DUMPABLE=0).");
    } else {
        println!("Process NOT hardened.");
    }
    println!();

    let key = HardenedKey::load(|buf| load_demo_key(buf))?;
    let sum = fake_sign(key.as_bytes());
    let pid = std::process::id();

    println!("=== {label}: key is LIVE (checksum 0x{sum:016x}) ===");
    println!("PID {pid}");
    print_vmlck(pid);
    println!();
    println!("Try in another terminal:");
    println!("  python3 scan_mem.py {pid}        # same-user scan");
    println!("  sudo python3 scan_mem.py {pid}   # root scan");
    println!();
    println!("Sleeping 30 s \u{2026}");
    sleep(30);

    drop(key);
    println!();
    println!("=== {label}: key DROPPED (zeroized) ===");
    println!("Scan again to verify it's gone:");
    println!("  sudo python3 scan_mem.py {pid}");
    println!();
    println!("Sleeping 15 s \u{2026}");
    sleep(15);

    Ok(())
}

/// Load key into a PageProtectedKey (PROT_NONE at rest) and sleep.
/// Even `sudo scan_mem.py` finds nothing — the page is `---p` in
/// /proc/pid/maps so the scanner skips it.  Then briefly unseal so
/// the reader can scan again and catch it while readable.
fn run_live_page_protected() -> io::Result<()> {
    // Deliberately no harden_process() here — we want to isolate the
    // PROT_NONE effect.  The live-hardened demo already showed PR_SET_DUMPABLE.
    println!("PageProtectedKey demo (PROT_NONE at rest, no process hardening).");
    println!();

    let mut key = PageProtectedKey::load(load_demo_key)?;
    let pid = std::process::id();

    println!("=== PAGE-PROTECTED: key is LIVE but page is PROT_NONE ===");
    println!("PID {pid}");
    print_vmlck(pid);
    println!();
    println!("Scan now — the page is sealed, scanner won't find it:");
    println!("  sudo python3 scan_mem.py {pid}");
    println!();
    println!("Sleeping 30 s …");
    sleep(30);

    // Briefly unseal to PROT_READ so the scanner can catch it.
    println!();
    println!("=== PAGE-PROTECTED: unsealing page to PROT_READ ===");
    println!("Scan now — the page is readable, scanner will find it:");
    println!("  sudo python3 scan_mem.py {pid}");
    println!();
    println!("Sleeping 30 s …");
    let _sum = key.with_readable(|bytes| {
        black_box(fake_sign(bytes));
        sleep(30);
    })?;

    // Page is re-sealed to PROT_NONE after with_readable returns.
    println!();
    println!("=== PAGE-PROTECTED: page re-sealed to PROT_NONE ===");
    println!("Scan again — sealed, scanner won't find it:");
    println!("  sudo python3 scan_mem.py {pid}");
    println!();
    println!("Sleeping 15 s …");
    sleep(15);

    Ok(())
}

/// Load key and abort immediately — produces a core dump (if enabled).
fn run_crash(harden: bool) -> io::Result<()> {
    let label = if harden { "HARDENED" } else { "UNHARDENED" };

    if harden {
        harden_process();
    }

    let key = HardenedKey::load(|buf| load_demo_key(buf))?;
    let sum = fake_sign(key.as_bytes());
    let pid = std::process::id();

    eprintln!("{label}: key loaded (checksum 0x{sum:016x}), PID {pid}");
    eprintln!("Aborting \u{2014} check for core.{pid} afterwards.");

    // Keep the key live at the crash point so the core dump captures it.
    // abort() diverges — Drop never runs, the secret stays in memory.
    black_box(&key);
    std::process::abort();
}

fn print_usage() {
    eprintln!("usage: zeroize-os-hardening [command]");
    eprintln!();
    eprintln!("commands:");
    eprintln!("  (none)              run both key types, print checksums");
    eprintln!("  live                load key unhardened, sleep for scanning");
    eprintln!("  live-hardened       load key hardened, sleep for scanning");
    eprintln!("  live-page-protected PageProtectedKey: PROT_NONE at rest");
    eprintln!("  crash               load key unhardened, abort (core dump demo)");
    eprintln!("  crash-hardened      load key hardened, abort (core dump demo)");
    eprintln!();
    eprintln!("see README.md for system setup and full walkthrough.");
}

fn main() -> io::Result<()> {
    match std::env::args().nth(1).as_deref() {
        None => run_checksums(),
        Some("live") => run_live(false),
        Some("live-hardened") => run_live(true),
        Some("live-page-protected") => run_live_page_protected(),
        Some("crash") => run_crash(false),
        Some("crash-hardened") => run_crash(true),
        _ => {
            print_usage();
            std::process::exit(2);
        }
    }
}
