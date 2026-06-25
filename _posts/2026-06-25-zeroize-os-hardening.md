---
title: "Where `zeroize` stops: hardening keys at the OS level"
published: true
permalink: "/zeroize-os-hardening"
tags: [rust, security, cryptography, zeroize, linux]
readtime: true
comments: false
---

*This follows on from [I Zeroized My Secret. Or Did I?](/zeroize), where we found that `zeroize` reliably wipes a secret you can name — but copies still leak past it through register spills, moves, and `Vec` reallocation. Those leftover copies live in your process's memory, and the operating system is free to move that memory around. This post is about taking those moves away from it.*

There are really three things the OS can do with a secret sitting in your RAM, and each one needs a different knob:

1. **page it to swap** (or capture it in a hibernation image),
2. **write it into a core dump** when you crash,
3. **hand it to another process** that reads your live memory.

Rather than touring the three controls separately, let's build a single hardened key buffer one piece at a time — adding each line only when the previous version is missing something — and then harden the process around it. By the end we'll have a small `HardenedKey` type and a one-shot `harden_process()` call.

## Starting point: just `Zeroizing`

After part 1, this is where we ended up:

```rust
use zeroize::Zeroizing;

let key = Zeroizing::new(load_key()); // [u8; 32] on the stack
crypto_op(&key);
// drops here, wiped
```

It compiles, it wipes on drop, and for low-value secrets it's enough. But every problem from part 1 is still here — moves, spills, `mem::forget` — and now the OS is allowed to make things harder on top of that: the page the stack sits on can be swapped to disk, copied into a core dump, or read by another same-user process via `ptrace`. Let's close those gaps one at a time.

## Step 1: put it on the heap for a stable address

The stale-copies problem from part 1 has a particularly nasty form once the OS gets involved. Every time the array moves on the stack — a function return, an assignment, an argument pass — Rust memcopies the 32 bytes into a new slot. `zeroize` only wipes the *last* one. Every other stack slot that ever held those bytes can be paged to swap or captured in a core dump, which is exactly what the next two steps are trying to prevent.

The fix is the same one we'd reach for if we cared about the move hazard alone: heap-allocate, so the *pointer* moves but the *bytes* stay put.

```rust
use zeroize::Zeroizing;

let key = Box::new(Zeroizing::new(load_key()));
```

Now the only stack slot that holds anything sensitive holds a pointer, and the 32 bytes live at a fixed heap address that doesn't change just because the owning variable moves around. That stable address matters for everything below: the lock from Step 2 will be pinned to it, the `madvise` from Step 3 will be applied to it, and neither would survive if the bytes themselves moved.

## Step 2: pin it in RAM so it can't be swapped

A heap allocation is still pageable memory. The kernel might page your secret out to the swap file, leaving a copy sitting on disk long after the process is gone. The fix is `mlock(2)`: tell the kernel "don't swap these pages out."

A few mechanical details about `mlock` worth pinning down, because they change how you call it:

- **It operates on whole pages.** `mlock(addr, len)` takes a byte range, but the kernel rounds it out to every page that range touches — typically 4 KiB each. You can't pin just 32 bytes; the kernel pins the page those 32 bytes live on, and if your secret straddles a page boundary, both pages get locked.
- **There's a budget.** Locked memory counts against `RLIMIT_MEMLOCK` (`ulimit -l`). Exceed it and you get `ENOMEM` — the call doesn't half-succeed, but it also doesn't yell at you unless you check the return value.
- **The undo is `munlock`.** Same signature, symmetric. Forget it and the pages stay locked for the lifetime of the process.
- **Windows has a different name for the same idea.** `VirtualLock` keeps the region in the process's working set; `VirtualUnlock` releases it.

`mlock` is a raw libc call. You *can* reach it from Rust via `libc::mlock` directly, but it's `unsafe`, platform-specific, and you have to remember the `munlock` yourself.

That's where the [`region`](https://docs.rs/region) crate comes in. It's worth being precise about what it actually is, because its [own docs](https://docs.rs/region/) describe it as a *cross-platform virtual memory API*, not an `mlock` library. It wraps a whole family of platform primitives:

| `region` API | Unix | Windows | What it does |
|---|---|---|---|
| `region::query` | `/proc/self/maps` | `VirtualQuery` | inspect a memory region |
| `region::alloc` | `mmap` | `VirtualAlloc` | reserve/commit pages |
| `region::protect` | `mprotect` | `VirtualProtect` | change R/W/X permissions |
| `region::lock` | `mlock` | `VirtualLock` | pin pages so they can't be swapped |

For our purposes — keeping a secret out of swap — the one we care about is `region::lock`. It picks the right primitive per OS and hands you an RAII guard so the unlock (`munlock` / `VirtualUnlock`) fires automatically when the guard drops:

```rust
use zeroize::Zeroizing;

let key = Box::new(Zeroizing::new(load_key()));
let _lock = region::lock(key.as_ptr(), key.len())?;
// pages stay resident until `_lock` drops, which calls munlock/VirtualUnlock
```

The `Box` from Step 1 is doing real work here: the lock points at a heap address that *won't move* if `key` is later passed to another function or stored in a struct. If we'd locked a stack `[0u8; 32]` instead, then moved `key`, the lock would still be pinning the original (now stale) stack page and the live bytes would sit on an unlocked one.

A note on the sibling call: `region::protect` (i.e. `mprotect`/`VirtualProtect`) is a *different* control. It changes the read/write/execute flags on a page — useful for things like marking a buffer no-access between uses, or making JIT pages executable — but it doesn't stop the kernel from paging that memory out. For "don't swap my secret," reach for `lock`; `protect` is a separate axis. (This is also why `secrecy` says it deliberately does neither — it's leaving the OS-level posture to you.)

If you'd rather go lower-level, [`nix`](https://docs.rs/nix) gives you `nix::sys::mman::mlock`, and [`os-memlock`](https://docs.rs/os-memlock) offers thin `mlock`/`munlock` wrappers. Man page: [`mlock(2)`](https://man7.org/linux/man-pages/man2/mlock.2.html).

A couple of honest heads-ups, because `mlock` has sharp edges:

- **It can silently fail.** A process may only lock up to its `RLIMIT_MEMLOCK` limit (`ulimit -l`), which on many systemd setups is a modest few megabytes. Go over it and `mlock` returns `ENOMEM`. So check the return value — a "locked" buffer that didn't actually lock is worse than knowing you couldn't. You can read or raise the limit (within your hard cap) with the `rlimit` crate.
- **It stops swapping, not hibernation.** Suspend-to-disk snapshots *all* of RAM — locked pages included — into the hibernation image. Covering that means an encrypted hibernation image or no hibernation at all; it isn't something your process can fix on its own.
- **Windows differs.** `VirtualLock` keeps pages in the working set, but the working-set model has its own quirks; treat the cross-platform wrapper as best-effort and test on each target.

## Step 3: keep it out of core dumps

If the process crashes, the OS can write your whole address space — secrets and all — into a core file or hand it to a crash reporter. The per-region knob for that is `madvise(MADV_DONTDUMP)`: tell the kernel that a specific memory range should be excluded from any core dump. The kernel's [`core(5)`](https://man7.org/linux/man-pages/man5/core.5.html) confirms a dump will leave out any region you mark this way.

Before we reach for it though, there's a *real* gotcha worth knowing — and it's the reason the working example for this post ([`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening)) takes a slightly different shape than you might expect:

> **Linux's `madvise` requires the address to be page-aligned.** `mlock` doesn't (it rounds down internally), but `madvise` does — feed it a non-aligned address and you get `EINVAL`. The `Box` we built in Step 1 hands out *allocator-aligned* memory (8 or 16 bytes), not page-aligned. So a literal "`MADV_DONTDUMP` on the Box" doesn't compile-then-work — it compiles and then fails at runtime.

That leaves two real options:

1. **Defer per-region dump-exclusion until you have page-aligned memory.** That's what Step 6 will give us, via `region::alloc`. Until then, dump coverage for the Box-based key comes from Step 5's process-wide `RLIMIT_CORE = 0` + `PR_SET_DUMPABLE`.
2. **Round the pointer down to the page boundary and `madvise` the whole page.** Works, but you're now telling the kernel not to dump *all* neighbours sharing that heap page too — which has its own caveats (debuggability, surprise).

The example pursues option 1 for the heap-Box buffer and option "do it for real" for the page-isolated buffer in Step 6. For completeness, the `madvise` call you'd make on a page-aligned region looks like this:

```rust
// Only correct when `ptr` is a page-aligned address you own —
// i.e. NOT a Box, but the result of `region::alloc` (Step 6) or mmap.
unsafe {
    os_memlock::madvise_dontdump(ptr as *mut _, len)?;
}
```

(`nix` also exposes `madvise` with `MmapAdvise::MADV_DONTDUMP`. On FreeBSD the equivalent is `MADV_NOCORE`. Man page: [`madvise(2)`](https://man7.org/linux/man-pages/man2/madvise.2.html).)

It's *per-region*, not process-wide — it marks the page(s) you point at as "don't dump." That's the right granularity once we can satisfy the alignment requirement; the process-wide knob is the blunter Step 5.

## Step 4: bundle it into a `HardenedKey` type

We've now got two things that have to live and die together: the bytes and the lock guard. If the lock guard drops before the bytes, the page becomes swappable while the secret is still in it. If the bytes are freed without us knowing, the lock points at memory we no longer own. Holding both correctly across every early return and `?` is the kind of thing that's easy to get wrong by hand.

The Rust answer is "make them one type and let RAII enforce the ordering."

```rust
use zeroize::Zeroizing;

struct HardenedKey {
    bytes: Box<Zeroizing<[u8; 32]>>,
    _lock: region::LockGuard,
}

impl HardenedKey {
    fn load(init: impl FnOnce(&mut [u8; 32])) -> std::io::Result<Self> {
        let mut bytes = Box::new(Zeroizing::new([0u8; 32]));
        init(&mut bytes);
        let (ptr, len) = (bytes.as_ptr(), bytes.len());
        let lock = region::lock(ptr, len)?;   // pin: no swap
        // NOTE: no per-buffer MADV_DONTDUMP here — the Box isn't page-aligned.
        // For this buffer, dump coverage comes from `harden_process()` in Step 5.
        Ok(Self { bytes, _lock: lock })
    }
}
```

What this earns us:

- **Field-order drop semantics.** Rust drops a struct's fields in declaration order. By putting `bytes` first and `_lock` second, the `Zeroizing` wipe runs *while* the page is still locked — so the wipe lands on resident memory, not on memory the kernel just paged out under our feet. (Reversing the field order would be a quiet bug.)
- **You can't accidentally leak one of the two.** Bytes and lock are wired together — there's no path where one outlives the other or gets forgotten on an early return.
- **The move hazard from part 1 is genuinely gone.** Moving a `HardenedKey` moves the `Box` pointer and the `LockGuard` handle, not the 32 bytes; the heap allocation stays put, and the lock keeps pointing at the same page.

(Types and crate versions are simplified — see [`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening) for a buildable version pinned to real crate versions.)

## Step 5: harden the process around it

Everything above is per-key. There's a second category of fixes that don't belong on the buffer at all — they apply once, at process startup, and cover the gaps a per-page approach can't.

**Turn off core dumps for the whole process.** `MADV_DONTDUMP` is a hint on a specific region; a dump path can still capture *other* regions that happened to hold a copy (a spill, a stale stack slot, a `Vec` that grew and reallocated). The blunt instrument is `setrlimit(RLIMIT_CORE, 0)`:

```rust
use rlimit::{setrlimit, Resource};
setrlimit(Resource::CORE, 0, 0)?;   // soft = hard = 0
```

Here's the gotcha that's genuinely worth knowing: `core(5)` says **`RLIMIT_CORE` gets ignored when dumps are piped to a program** — which is exactly what `systemd-coredump` does on most modern Linux boxes. So `RLIMIT_CORE = 0` on its own can still quietly ship your crash to the journal. The fix is to also call `prctl(PR_SET_DUMPABLE, 0)`:

```rust
let _ = prctl::set_dumpable(false);   // also quiets systemd-coredump
```

Man page: [`prctl(2)`](https://man7.org/linux/man-pages/man2/prctl.2.html).

**Block live-memory snooping.** This is the threat the first post admitted `zeroize` can't touch: an attacker who reads your memory *while the process is running*. On Linux the usual route is `ptrace` (or reading `/proc/<pid>/mem`), and a process running as the same user can do it by default. In a world of compromised dependencies and shared hosts, that's not exotic.

Good news: the `prctl(PR_SET_DUMPABLE, 0)` call above pulls double duty. Marking the process non-dumpable also makes it **non-attachable by `ptrace` for anyone who isn't root** — the same single call that quiets core dumps also slams the easy door on live-memory snooping.

Wrap both in one startup helper, called *before* any secrets are loaded:

```rust
fn harden_process() {
    let _ = rlimit::setrlimit(rlimit::Resource::CORE, 0, 0); // no core dumps
    let _ = prctl::set_dumpable(false);                      // + quiet systemd, + block ptrace
}

fn main() -> std::io::Result<()> {
    harden_process();                            // once, at startup
    let key = HardenedKey::load(load_key_from_kms)?;
    crypto_op(key.as_bytes());
    Ok(())
}
```

For a system-wide policy there's the Yama LSM, via `/proc/sys/kernel/yama/ptrace_scope`:

- `0` — classic behaviour: any same-user process can attach.
- `1` — restricted (the common default): only a parent, or a tracer the target explicitly allows via `prctl(PR_SET_PTRACER, ...)`.
- `2` — admin-only (needs `CAP_SYS_PTRACE`).
- `3` — no attaching at all, for anyone, until reboot.

The honest caveat, same as always: none of this stops a root / `CAP_SYS_PTRACE` attacker, who can lift any of these. What it does is raise the bar a lot against the realistic same-user case — and that's most of the value.

## Step 6 (going further): a page-isolated variant

Every step above has been about *external* threats — the OS paging memory out, the kernel dumping it on a crash, another process attaching. There's one more category worth knowing about: a stray pointer, out-of-bounds read, or use-after-free *in your own code* that accidentally reads the secret's bytes. The control for that is `mprotect(PROT_NONE)`: deny every access to the page when the key isn't actively in use, so any stray touch SIGSEGVs instead of silently picking up the bytes.

The catch is that `mprotect` works at page granularity, and the heap `Box` from earlier shares its page with allocator metadata and neighbouring allocations — flipping the whole page to `PROT_NONE` would crash anything that touched a neighbour. To use `protect` safely you need a *dedicated* page. That's what `region::alloc` (the `mmap`/`VirtualAlloc` row of the table back in Step 2) is for. Once you own a whole page, `region::lock`, `MADV_DONTDUMP`, *and* `region::protect` can all be applied to it cleanly:

```rust
use std::ffi::c_void;
use region::Protection;
use zeroize::Zeroize;

const KEY_LEN: usize = 32;

struct PageProtectedKey {
    // Field order matters: _lock must drop *after* the wipe in our Drop impl,
    // and *before* `alloc` is unmapped. Custom Drop runs first; then fields
    // drop in declaration order, so list _lock before alloc.
    _lock: region::LockGuard,
    alloc: region::Allocation, // dedicated, page-aligned; PROT_NONE at rest
}

impl PageProtectedKey {
    fn load(init: impl FnOnce(&mut [u8])) -> std::io::Result<Self> {
        // 1. Whole page, initially writable so we can fill it.
        let mut alloc = region::alloc(KEY_LEN, Protection::READ_WRITE)?;
        let ptr: *const u8 = alloc.as_ptr::<u8>();
        let len: usize = alloc.len(); // page-rounded (typically 4096)

        // 2. Pin and exclude-from-dumps — same as Steps 2 and 3, just on a
        //    whole page instead of a corner of one.
        let _lock = region::lock(ptr, len)?;
        // SAFETY: ptr/len describe a live, owned, page-aligned mmap region.
        unsafe { os_memlock::madvise_dontdump(ptr as *mut c_void, len)?; }

        // 3. Write the secret in through the only window we'll allow.
        // SAFETY: alloc is page-rounded and at least KEY_LEN bytes.
        {
            let bytes = unsafe {
                std::slice::from_raw_parts_mut(alloc.as_mut_ptr::<u8>(), KEY_LEN)
            };
            init(bytes);
        }

        // 4. Seal: no access at all until something deliberately opens it.
        // SAFETY: same live mapping; the writable slice above has gone out of
        // scope, so flipping to PROT_NONE doesn't dangle any reference.
        unsafe { region::protect(ptr, len, Protection::NONE)?; }
        Ok(Self { _lock, alloc })
    }

    /// Briefly flip to read-only, hand the bytes to `f`, then re-seal.
    fn with_readable<R>(&self, f: impl FnOnce(&[u8]) -> R) -> std::io::Result<R> {
        // SAFETY: same live mapping; the returned guard restores PROT_NONE on drop.
        let _open = unsafe {
            region::protect_with_handle(
                self.alloc.as_ptr::<u8>(),
                self.alloc.len(),
                Protection::READ,
            )?
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
        // alloc unmap. (Custom Drop runs *before* the fields drop.)
        // SAFETY: same live mapping; we're about to write KEY_LEN bytes.
        unsafe {
            let _ = region::protect(
                self.alloc.as_ptr::<u8>(),
                self.alloc.len(),
                Protection::READ_WRITE,
            );
            let bytes = std::slice::from_raw_parts_mut(
                self.alloc.as_mut_ptr::<u8>(), KEY_LEN,
            );
            bytes.zeroize();
        }
    }
}
```

Usage follows the same bracket pattern libsodium uses with `sodium_mprotect_noaccess` / `sodium_mprotect_readonly`:

```rust
let key = PageProtectedKey::load(|buf| load_key_into(buf))?;
let sig = key.with_readable(|bytes| sign(bytes, &digest))?;
// page is PROT_NONE again until the next call.
```

A few honest things to note about this layer:

- **It defends against bugs, not against a determined attacker.** The page is readable *exactly* when you're using the key — which is also when a deliberate attacker would catch you. The win is against *accidents*: stray pointers, OOB reads, an errant log statement.
- **You pay a whole page per key.** `region::alloc(32, ...)` rounds up to the system page size (typically 4 KiB). Fine for a handful of long-lived keys; very much not for thousands of short-lived ones.
- **It doesn't help with the threats Steps 1–5 already covered.** Swap, core dumps, and `ptrace` all bypass page protections — the bytes-on-disk and bytes-via-tracer paths read the underlying memory regardless of `PROT_*` flags. The `region::lock` and `madvise_dontdump` calls inside `load` are still the load-bearing controls there; the `protect` toggle is *added* on top, not a replacement.
- **`harden_process()` from Step 5 still applies unchanged.** Per-key and process-level controls compose freely.

The pattern itself isn't novel — libsodium and a handful of Rust crates do exactly this. The reason it's tucked behind *"going further"* rather than the recommended baseline is the threat-model shift: it's a tool for a different problem than the rest of this post.

A working version of both `HardenedKey` and `PageProtectedKey` (with the real `unsafe { ... }` blocks the `region` crate's `protect` API actually requires, and pinned crate versions) is in [`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening). `cargo run --release` should print two matching checksums.

## Where this leaves us

Stacked up, these controls shrink the window in which your secret is recoverable: off the swap file, out of crash dumps, away from same-user snooping, and — at the cost of a page per key — out of reach of accidental in-process reads. For a lot of keys that's enough.

But every knob here quietly assumes the plaintext key is sitting in *your* process's RAM. The next layer makes that assumption false: HSMs, threshold signing, and enclaves arrange for the key never to be in your process at all.

← Previous: [I Zeroized My Secret. Or Did I?](/zeroize)
<!-- → Next: [Don't hold the key: architecture for secrets you can't afford to lose](./part-3-architecture.md) -->

---

*Further reading:*

- `region` (cross-platform virtual memory: `alloc`, `lock`, `protect`, `query`) — https://docs.rs/region
- `os-memlock` (mlock + MADV_DONTDUMP) — https://docs.rs/os-memlock
- `rlimit` — https://docs.rs/rlimit · `prctl` — https://docs.rs/prctl
- `core(5)` — https://man7.org/linux/man-pages/man5/core.5.html
- `mlock(2)` — https://man7.org/linux/man-pages/man2/mlock.2.html · `madvise(2)` — https://man7.org/linux/man-pages/man2/madvise.2.html · `mprotect(2)` — https://man7.org/linux/man-pages/man2/mprotect.2.html · `prctl(2)` — https://man7.org/linux/man-pages/man2/prctl.2.html
- Yama / `ptrace_scope` — https://docs.kernel.org/admin-guide/LSM/Yama.html · `ptrace(2)` — https://man7.org/linux/man-pages/man2/ptrace.2.html
- libsodium guarded heap allocation (the production-grade version of Step 6, with guard pages and a canary) — https://doc.libsodium.org/memory_management · source: [`src/libsodium/sodium/utils.c`](https://github.com/jedisct1/libsodium/blob/master/src/libsodium/sodium/utils.c) (`sodium_malloc` / `sodium_mprotect_noaccess` / `sodium_mprotect_readonly` / `sodium_mprotect_readwrite`)
- `memsec` — a Rust port of libsodium's secure-allocation primitives — https://docs.rs/memsec
