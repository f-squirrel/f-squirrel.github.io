//! Working companion to the blog post
//! "Where `zeroize` stops: hardening keys at the OS level".
//!
//! Two key buffers:
//!
//! * `HardenedKey` — Step 4: heap Box + `mlock`. No `MADV_DONTDUMP`: it
//!   requires a page-aligned address, which a `Box` doesn't provide. Dump
//!   coverage comes from Step 5's process-wide `RLIMIT_CORE = 0` +
//!   `PR_SET_DUMPABLE`.
//! * `PageProtectedKey` — Step 6: dedicated page via `region::alloc`, locked,
//!   `MADV_DONTDUMP`'d, kept `PROT_NONE` at rest, briefly flipped to
//!   `PROT_READ` inside `with_readable`.
//!
//! And one process-level startup helper:
//!
//! * `harden_process()` — Step 5: `RLIMIT_CORE = 0` + `PR_SET_DUMPABLE`, which
//!   also blocks `ptrace` for non-root.
//!
//! Run:  cargo run --release
//!
//! Linux/FreeBSD: full MADV_DONTDUMP path. Other Unixes: os-memlock returns
//! Unsupported and we treat it as best-effort. Linux is the well-trodden case.

use std::ffi::c_void;
use std::io;

use region::Protection;
use zeroize::{Zeroize, Zeroizing};

const KEY_LEN: usize = 32;

// --------------------------------------------------------------------------
// Step 5: process-level hardening.
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
// Step 4: Box-backed buffer.
// --------------------------------------------------------------------------

/// A 32-byte key on the heap, wiped on drop and pinned in RAM. Stable address
/// survives moves of `Self`. Core-dump exclusion for *this* buffer is not
/// per-region (the Box isn't page-aligned, so no `MADV_DONTDUMP`) — it comes
/// from the process-wide `harden_process()` knobs in Step 5.
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
        // For *this* buffer the dump coverage comes from Step 5's
        // process-wide RLIMIT_CORE=0 + PR_SET_DUMPABLE.

        Ok(Self { bytes, _lock: lock })
    }

    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.bytes
    }
}

// --------------------------------------------------------------------------
// Step 6: page-isolated buffer (region::alloc + mprotect bracket).
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
        // Step 6.1 — Whole page, initially writable so we can fill it.
        let mut alloc =
            region::alloc(KEY_LEN, Protection::READ_WRITE).map_err(io_err)?;
        let ptr: *const u8 = alloc.as_ptr::<u8>();
        let len: usize = alloc.len(); // page-rounded (typically 4096)

        // Step 6.2 — Pin the whole page in RAM, exclude it from dumps.
        let lock = region::lock(ptr, len).map_err(io_err)?;
        // SAFETY: `ptr`/`len` describe a live, owned, page-aligned mmap region.
        unsafe {
            os_memlock::madvise_dontdump(ptr as *mut c_void, len)?;
        }

        // Step 6.3 — Fill in the secret through the only window we'll allow.
        // SAFETY: alloc is page-rounded and at least KEY_LEN bytes.
        {
            let bytes = unsafe {
                std::slice::from_raw_parts_mut(alloc.as_mut_ptr::<u8>(), KEY_LEN)
            };
            init(bytes);
        }

        // Step 6.4 — Seal: no access until something deliberately opens it.
        // SAFETY: `ptr`/`len` describe the same live mapping; the slice from
        // 6.3 has gone out of scope, so flipping to PROT_NONE doesn't
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
// Demo helpers.
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

fn main() -> io::Result<()> {
    harden_process();

    println!("=== Step 4: HardenedKey  (Box + mlock) ===");
    {
        let key = HardenedKey::load(|buf| load_demo_key(buf))?;
        let sum = fake_sign(key.as_bytes());
        println!("  checksum = 0x{sum:016x}");
        // Drops here: Zeroizing wipes the 32 bytes while the page is still
        // locked, then LockGuard drops and munlocks the page.
    }

    println!(
        "=== Step 6: PageProtectedKey  (region::alloc + mlock + MADV_DONTDUMP + mprotect) ==="
    );
    {
        let mut key = PageProtectedKey::load(load_demo_key)?;
        let sum = key.with_readable(fake_sign)?;
        println!("  checksum = 0x{sum:016x}");
        // Drops here: custom Drop re-opens R/W, zeroizes the 32 bytes, then
        // LockGuard munlocks the page, then Allocation munmaps it.
    }

    println!("done.");
    Ok(())
}
