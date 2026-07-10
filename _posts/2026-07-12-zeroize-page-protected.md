---
title: "Seal the Key Behind PROT_NONE"
subtitle: "A page-isolated secret that turns stray reads in your own code into crashes"
published: false
permalink: "/zeroize-page-protected"
tags: [rust, security, cryptography, zeroize, linux]
readtime: true
comments: false
---

*Part three of a series. [Part one](/zeroize) showed what `zeroize` does and doesn't wipe; [part two](/zeroize-os-hardening) built a `HardenedKey` that keeps the secret out of swap, core dumps, and other processes' reach. This post adds a control for a threat those two never touched: your own code.*

Everything in part two was about *external* threats — the OS paging memory out, the kernel dumping it on a crash, another process attaching. There's one more category worth knowing about: a stray pointer, out-of-bounds read, or use-after-free *in your own code* that accidentally reads the secret's bytes. The control for that is `mprotect(PROT_NONE)`: deny every access to the page when the key isn't actively in use, so any stray touch SIGSEGVs instead of silently picking up the bytes.

That's a genuine threat-model shift, and it's why this got its own post rather than a section in part two: everything there hardened the key against the world outside your process; this hardens it against accidents inside.

## A page of your own

The catch is that `mprotect` works at page granularity, and part two's heap `Box` shares its page with allocator metadata and neighbouring allocations — flipping the whole page to `PROT_NONE` would crash anything that touched a neighbour. To use `protect` safely you need a *dedicated* page. That's what `region::alloc` (the `mmap`/`VirtualAlloc` row of [part two's `region` table](/zeroize-os-hardening)) is for. Once you own a whole page, `region::lock`, `MADV_DONTDUMP`, *and* `region::protect` can all be applied to it cleanly — including the per-region dump exclusion part two had to defer, because `madvise` demands a page-aligned address that a `Box` can't give.

Drop-order discipline matters here too, but the field order is *reversed* from part two's `HardenedKey` — and for good reason. `HardenedKey` puts `bytes` first so the `Zeroizing` wipe runs while the lock is still held. `PageProtectedKey` doesn't need that: it has a custom `Drop` that re-opens the page and wipes *before* any field drops. What matters after the custom `Drop` is that `_lock` (`munlock`) drops before `alloc` (`munmap`), so the unlock lands on still-mapped memory. That's why `_lock` comes first here. Reverse the field order and the unlock fires on a region that's already gone.

```rust
use std::ffi::c_void;
use std::io;
use region::Protection;
use zeroize::Zeroize;

const KEY_LEN: usize = 32;

struct PageProtectedKey {
    _lock: region::LockGuard,
    alloc: region::Allocation, // dedicated, page-aligned; PROT_NONE at rest
}

fn io_err<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

/// Zeroes `len` bytes at `ptr` on drop. Covers any early return before
/// `PageProtectedKey` is fully constructed — a panic in `init`, or a failed
/// `region::protect` call — by wiping before `_lock`/`alloc` unwind. Only
/// `mem::forget`'d once the page is sealed and construction is about to
/// succeed, handing wipe duty back to `PageProtectedKey`'s own `Drop`.
struct WipeGuard { ptr: *mut u8, len: usize }

impl Drop for WipeGuard {
    fn drop(&mut self) {
        // SAFETY: only alive while `alloc` below still owns a live, writable
        // mapping of at least `len` bytes at `ptr`.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len).zeroize(); }
    }
}

impl PageProtectedKey {
    fn load(init: impl FnOnce(&mut [u8])) -> io::Result<Self> {
        // 1. Whole page, initially writable so we can fill it.
        let mut alloc = region::alloc(KEY_LEN, Protection::READ_WRITE)
            .map_err(io_err)?;
        let ptr: *const u8 = alloc.as_ptr::<u8>();
        let len: usize = alloc.len(); // page-rounded (typically 4096)

        // 2. Pin, exclude from dumps, and wipe-on-fork. The first two are the
        //    same controls as part two, just on a whole page instead of a
        //    corner of one; the third is new — and only possible here, because
        //    madvise needs the page-aligned address a Box can't give us.
        let _lock = region::lock(ptr, len).map_err(io_err)?;
        // SAFETY: ptr/len describe a live, owned, page-aligned mmap region.
        unsafe { os_memlock::madvise_dontdump(ptr as *mut c_void, len)?; }
        // Linux ≥ 4.14: a forked child sees this page zero-filled, so the
        // secret never exists in any child address space. This matters because
        // mlock is NOT inherited across fork(2) — a child's copy-on-write copy
        // of the page would be swappable even while the parent's stays pinned.
        // SAFETY: same live, owned, page-aligned mapping.
        unsafe {
            if libc::madvise(ptr as *mut c_void, len, libc::MADV_WIPEONFORK) != 0 {
                return Err(io::Error::last_os_error());
            }
        }

        // 2.5. Guard the unwind path: if `init` panics before `Self` exists,
        // ordinary local drop order would unlock and unmap the page without
        // ever wiping the partial secret it already holds. This guard runs
        // first on unwind (it's the most recently declared local) and wipes
        // before `_lock`/`alloc` let go of the page.
        let wipe_on_unwind = WipeGuard { ptr: alloc.as_mut_ptr::<u8>(), len: KEY_LEN };

        // 3. Write the secret in through the only window we'll allow.
        // SAFETY: alloc is page-rounded and at least KEY_LEN bytes.
        {
            let bytes = unsafe {
                std::slice::from_raw_parts_mut(alloc.as_mut_ptr::<u8>(), KEY_LEN)
            };
            init(bytes);
        }

        // 4. Seal: no access at all until something deliberately opens it.
        // The guard is still armed here — if this call fails, `?` returns
        // early and `wipe_on_unwind` still wipes before `_lock`/`alloc`
        // release the page.
        // SAFETY: same live mapping; the writable slice above has gone out of
        // scope, so flipping to PROT_NONE doesn't dangle any reference.
        unsafe { region::protect(ptr, len, Protection::NONE).map_err(io_err)?; }

        // Sealed, construction about to succeed — disarm the guard now that
        // `PageProtectedKey::drop` owns the wipe path.
        std::mem::forget(wipe_on_unwind);
        Ok(Self { _lock, alloc })
    }

    /// Briefly flip to read-only, hand the bytes to `f`, then re-seal.
    /// `&mut self` so the borrow checker enforces the single-threaded use
    /// the prose talks about — concurrent calls would race the seal/unseal.
    fn with_readable<R>(&mut self, f: impl FnOnce(&[u8]) -> R) -> io::Result<R> {
        // SAFETY: same live mapping; the returned guard restores PROT_NONE on drop.
        let _open = unsafe {
            region::protect_with_handle(
                self.alloc.as_ptr::<u8>(),
                self.alloc.len(),
                Protection::READ,
            )
            .map_err(io_err)?
        };
        // SAFETY: page is now PROT_READ for the duration of `_open`.
        let bytes = unsafe {
            std::slice::from_raw_parts(self.alloc.as_ptr::<u8>(), KEY_LEN)
        };
        Ok(f(bytes))
    }
}

impl Drop for PageProtectedKey {
    fn drop(&mut self) {
        // Re-open for writing so the wipe can land, then let _lock unlock and
        // alloc unmap. Best-effort: if mprotect fails, the page may still be
        // PROT_NONE, and writing through it would SIGSEGV during Drop — so we
        // only wipe if reopening actually succeeded. The worst case is then
        // that the page wasn't wiped before munmap reclaimed it, not a crash.
        // SAFETY: same live mapping; we only write if `protect` reports success.
        unsafe {
            let reopened = region::protect(
                self.alloc.as_ptr::<u8>(),
                self.alloc.len(),
                Protection::READ_WRITE,
            )
            .is_ok();
            if reopened {
                let bytes = std::slice::from_raw_parts_mut(
                    self.alloc.as_mut_ptr::<u8>(), KEY_LEN,
                );
                bytes.zeroize();
            }
        }
    }
}
```

Usage follows the same bracket pattern libsodium uses with `sodium_mprotect_noaccess` / `sodium_mprotect_readonly`:

```rust
let mut key = PageProtectedKey::load(|buf| load_key_into(buf))?;
let sig = key.with_readable(|bytes| sign(bytes, &digest))?;
// page is PROT_NONE again until the next call.
```

## Trying it

The companion example ([`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening)) includes a `live-page-protected` mode that lets you watch the bracket in action — running the `/proc/<pid>/mem` scanner from parts one and two at each phase to see the mapping flip between `---p` and readable in `/proc/<pid>/maps`; the scanner skips the page while it's sealed, but the mapping itself never goes away.

## What this buys — and what it doesn't

A few honest things to note about this layer:

- **It defends against bugs, not against in-process attackers.** Any code running inside your process — a compromised dependency, an injected shared object, a supply-chain payload — shares your address space and can call `mprotect` itself to unseal the page whenever it likes, or simply wait for `with_readable` to open the window. The kernel enforces page permissions per-*process*, not per-library. The win here is against *accidents*: stray pointers, OOB reads, an errant log statement. Defending against malicious code in your own process requires keeping the key out of that process entirely — via privilege separation or hardware boundaries.
- **The protection state is per-page and process-global.** That's why `with_readable` takes `&mut self`: two threads calling it on the same key would otherwise race — one's guard re-sealing the page to `PROT_NONE` while the other is still mid-read, SIGSEGVing the reader. `&mut self` forces the borrow checker to serialize calls per key for you. And note the closure can still copy the bytes into its return value — the seal only protects the page, not whatever you carry out of it.
- **You pay a whole page per key.** `region::alloc(32, ...)` rounds up to the system page size (typically 4 KiB). Fine for a handful of long-lived keys; very much not for thousands of short-lived ones.
- **It doesn't help with the threats part two already covered.** Swap, core dumps, and `ptrace` all bypass page protections — the bytes-on-disk and bytes-via-tracer paths read the underlying memory regardless of `PROT_*` flags. The `region::lock` and `madvise_dontdump` calls inside `load` are still the load-bearing controls there; the `protect` toggle is *added* on top, not a replacement.
- **Part two's `harden_process()` still applies unchanged.** Per-key and process-level controls compose freely.

## Don't roll your own

The pattern itself isn't novel, and **if you're reaching for this in production, use [`secrets`](https://crates.io/crates/secrets)**. It packages the same bracket (`PROT_NONE` at rest, `PROT_READ`/`PROT_WRITE` on borrow) behind an ergonomic Rust API (`SecretBox<T>` / `SecretVec<T>`), with protected borrows, guard pages, canaries, `mlock`, and zero-on-free that the hand-rolled version here doesn't have. Its docs describe it as mostly wrapping libsodium's memory-protection utilities, but the Rust API and the feature/build configuration are part of the guarantee too — not something inherited wholesale. The `PageProtectedKey` above is here to *show* the pattern, not to compete with it.

A couple of other crates worth knowing about in the same neighbourhood:

- **[`memsec`](https://docs.rs/memsec)** — pure-Rust port of libsodium's `utils.c`, lower-level (free functions, no borrow-API). Worth knowing about in the same space, though it doesn't expose `memfd_secret(2)`. That syscall (Linux ≥ 5.14) is a stronger Linux-specific primitive than `mlock`+`mprotect`: pages are removed from the kernel's direct map, ordinary `get_user_pages` access is blocked, the memory is locked like `mlock`, and hibernation is inhibited while it's in use — a natural sequel to this step, but you'd call it via `libc::syscall` or a dedicated wrapper today. It's still not a magic root boundary, though: the man page itself notes that a sufficiently privileged attacker can spawn a process and exfiltrate the contents via `ptrace`. (It's also off by default before Linux 6.5, when it needed the `secretmem.enable=y` kernel parameter; enabled by default from 6.5 on.)
- **[`secmem-alloc`](https://crates.io/crates/secmem-alloc)** — different shape: a custom allocator you plug into `Box::new_in` / `Vec::new_in` so every allocation in a secret-bearing region gets `mlock` + zeroize-on-dealloc by default. No `mprotect` bracket, but a much lower per-key cost than a dedicated page.

And then the one *not* to reach for here: **[`secrecy`](https://docs.rs/secrecy)** is explicitly *only* zeroize + a `Debug`-redacted `ExposeSecret` wrapper — no `mlock`, no `mprotect`, no `MADV_DONTDUMP`. It's a good ergonomic top-layer over any of the above, but it doesn't replace them.

A working version of `PageProtectedKey` (with the real `unsafe { ... }` blocks the `region` crate's `protect` API actually requires, and pinned crate versions) is in [`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening), alongside part two's `HardenedKey`.

## Where this leaves us

With this layer on top of part two's stack, the secret is off the swap file, out of crash dumps, away from same-user snooping, and — at the cost of a page per key — out of reach of accidental in-process reads. For a lot of keys that's enough.

But every knob in this series so far quietly assumes the plaintext key is sitting in *your* process's RAM — and as the first caveat above made clear, any code sharing your address space can undo your protections. The next post makes that assumption false without special hardware: privilege separation moves the key into a separate process that your dependencies can't reach, and the Linux kernel keyring keeps it in kernel memory where no pointer can follow.

← Previous: [Secrets in Memory: More Exposed Than You Think](/zeroize-os-hardening)
<!-- → Next: TODO -->

---

*Further reading:*

**Crates**

- `secrets` — guarded-memory API with libsodium integration (`SecretBox` / `SecretVec`, the recommended way to use this pattern in production) — <https://crates.io/crates/secrets>
- `memsec` — a Rust port of libsodium's secure-allocation primitives (`malloc`, `mlock`, `mprotect`, `memzero`) — <https://docs.rs/memsec>
- `secmem-alloc` — secret-memory custom allocator for `Box::new_in` / `Vec::new_in` — <https://crates.io/crates/secmem-alloc>
- `secrecy` — ergonomic `Secret<T>` / `ExposeSecret` (zeroize + `Debug` redaction *only* — no mlock/mprotect/MADV) — <https://docs.rs/secrecy>
- `region` — cross-platform virtual memory: `alloc`, `lock`, `protect`, `query` — <https://docs.rs/region>

**Kernel and libc docs**

- `mprotect(2)` — <https://man7.org/linux/man-pages/man2/mprotect.2.html>
- `madvise(2)` — <https://man7.org/linux/man-pages/man2/madvise.2.html>
- `memfd_secret(2)` — Linux ≥ 5.14, kernel-level secret memory beyond what `mlock`/`mprotect` reach — <https://man7.org/linux/man-pages/man2/memfd_secret.2.html>
- libsodium guarded heap allocation (the production-grade version of this pattern, with guard pages and a canary) — <https://doc.libsodium.org/memory_management> · source: [`src/libsodium/sodium/utils.c`](https://github.com/jedisct1/libsodium/blob/master/src/libsodium/sodium/utils.c) (`sodium_malloc` / `sodium_mprotect_noaccess` / `sodium_mprotect_readonly` / `sodium_mprotect_readwrite`)
