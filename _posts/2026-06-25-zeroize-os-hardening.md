---
title: "I Hardened My Secret. Or Did I?"
subtitle: "Where `zeroize` stops: keeping keys out of swap, dumps, and other processes"
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

Rather than touring the three controls separately, let's build a hardened key buffer one piece at a time — adding each control only when the previous version is missing something — and then harden the process around it. By the end we'll have a small `HardenedKey` type and a one-shot `harden_process()` call, plus a page-isolated variant for a different threat model in the "going further" coda.

## Starting point: the heap `Box` from part 1

Part 1's move-hazard fix ended by moving the secret onto the heap and filling it in place, precisely so no stale copies were left behind:

```rust
use zeroize::Zeroizing;

let mut key = Box::new(Zeroizing::new([0u8; 32])); // allocate first…
load_key_into(&mut key);                           // …then fill in place
crypto_op(&key);
// drops here, wiped
```

Within the language, that's solid: the secret lives at one fixed heap address, moving `key` only moves the pointer, and `Zeroizing` wipes the single remaining copy on drop. (`mem::forget` and aborts can still skip that wipe — part 1's best-effort caveat hasn't gone away — but the *move*-driven leaks are handled.)

What the heap `Box` does *not* touch is the operating system. Those 32 bytes still sit on an ordinary, pageable heap page, and the OS is free to copy that page somewhere `zeroize` will never follow: out to the swap file, into a core dump, or into another same-user process that reads your memory via `ptrace`. Closing those three gaps is the whole job of this post — and we'll do it one knob at a time, building up a small `HardenedKey` type as we go.

## Lean on the stable address

Everything below hangs on one property of that `Box`: its address doesn't move. The kernel controls we're about to reach for — `mlock` to pin the page, `madvise` to exclude it from dumps — all operate on a *specific address range*, and they'd be worse than useless if the bytes wandered off to a new location afterwards. Pin one page, then let the secret move to another, and you've locked an empty page while the live secret sits exposed.

That's exactly why part 1's "allocate, then fill in place" shape matters here and not just as a tidiness point: it hands us a fixed heap address we can pin and protect, with no stack copy left over to leak first. The `HardenedKey::load` we arrive at below wraps that same pattern behind an `init` closure — but conceptually, the foundation is already in your hand the moment the secret is in a `Box`. So: `region::lock` pins this address, `madvise` marks it, and neither would survive if the bytes themselves moved. They don't — so we can build.

## Pin it in RAM so it can't be swapped

A heap allocation is still pageable memory. The kernel might page your secret out to the swap file, leaving a copy sitting on disk long after the process is gone. The fix is `mlock(2)`: tell the kernel "don't swap these pages out."

A few mechanical details about `mlock` worth pinning down, because they change how you call it:

- **It operates on whole pages.** `mlock(addr, len)` takes a byte range, but the kernel rounds it out to every page that range touches — typically 4 KiB each. You can't pin just 32 bytes; the kernel pins the page those 32 bytes live on, and if your secret straddles a page boundary, both pages get locked.
- **There's a budget.** Locked memory counts against `RLIMIT_MEMLOCK` (`ulimit -l`). Exceed it and you get `ENOMEM` — the call doesn't half-succeed, but it also doesn't yell at you unless you check the return value.
- **The undo is `munlock`.** Same signature, symmetric. Forget it and the pages stay locked for the lifetime of the process.
- **Windows has a different name for the same idea.** `VirtualLock` keeps the region in the process's working set; `VirtualUnlock` releases it.

`mlock` is a raw libc call. You *can* reach it from Rust via `libc::mlock` directly, but it's `unsafe`, platform-specific, and you have to remember the `munlock` yourself.

That's where the [`region`](https://docs.rs/region) crate comes in. It's worth being precise about what it actually is, because its [own docs](https://docs.rs/region/) describe it as a *cross-platform virtual memory API*, not an `mlock` library. It wraps a whole family of platform primitives:

| `region` API | Unix | Windows | What it does |
| --- | --- | --- | --- |
| `region::query` | `/proc/self/maps` | `VirtualQuery` | inspect a memory region |
| `region::alloc` | `mmap` | `VirtualAlloc` | reserve/commit pages |
| `region::protect` | `mprotect` | `VirtualProtect` | change R/W/X permissions |
| `region::lock` | `mlock` | `VirtualLock` | pin pages so they can't be swapped |

For our purposes — keeping a secret out of swap — the one we care about is `region::lock`. It picks the right primitive per OS and hands you an RAII guard so the unlock (`munlock` / `VirtualUnlock`) fires automatically when the guard drops:

```rust
use zeroize::Zeroizing;

let mut key = Box::new(Zeroizing::new([0u8; 32]));
load_key_into(&mut key);
let _lock = region::lock(key.as_ptr(), key.len())?;
// pages stay resident until `_lock` drops, which calls munlock/VirtualUnlock
```

The heap `Box` is doing real work here: the lock points at a heap address that *won't move* if `key` is later passed to another function or stored in a struct. If we'd locked a stack `[0u8; 32]` instead, then moved `key`, the lock would still be pinning the original (now stale) stack page and the live bytes would sit on an unlocked one.

A note on the sibling call: `region::protect` (i.e. `mprotect`/`VirtualProtect`) is a *different* control. It changes the read/write/execute flags on a page — useful for things like marking a buffer no-access between uses, or making JIT pages executable — but it doesn't stop the kernel from paging that memory out. For "don't swap my secret," reach for `lock`; `protect` is a separate axis. (This is also why `secrecy` says it deliberately does neither — it's leaving the OS-level posture to you.)

If you'd rather go lower-level, [`nix`](https://docs.rs/nix) gives you `nix::sys::mman::mlock`, and [`os-memlock`](https://docs.rs/os-memlock) offers thin `mlock`/`munlock` wrappers. Man page: [`mlock(2)`](https://man7.org/linux/man-pages/man2/mlock.2.html).

A couple of honest heads-ups, because `mlock` has sharp edges:

- **It can silently fail.** A process may only lock up to its `RLIMIT_MEMLOCK` limit (`ulimit -l`), which on many systemd setups is a modest few megabytes. Go over it and `mlock` returns `ENOMEM`. So check the return value — a "locked" buffer that didn't actually lock is worse than knowing you couldn't. You can read or raise the limit (within your hard cap) with the `rlimit` crate.
- **It stops swapping, not hibernation.** Suspend-to-disk snapshots *all* of RAM — locked pages included — into the hibernation image. Covering that means an encrypted hibernation image or no hibernation at all; it isn't something your process can fix on its own.
- **Windows differs.** `VirtualLock` keeps pages in the working set, but the working-set model has its own quirks; treat the cross-platform wrapper as best-effort and test on each target.

## Keep it out of core dumps

If the process crashes, the OS can write your whole address space — secrets and all — into a core file or hand it to a crash reporter. The per-region knob for that is `madvise(MADV_DONTDUMP)`: tell the kernel that a specific memory range should be excluded from any core dump. The kernel's [`core(5)`](https://man7.org/linux/man-pages/man5/core.5.html) confirms a dump will leave out any region you mark this way.

Before we reach for it though, there's a *real* gotcha worth knowing — and it's the reason the working example for this post ([`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening)) takes a slightly different shape than you might expect:

> **Linux's `madvise` requires the address to be page-aligned.** `mlock` doesn't (it rounds down internally), but `madvise` does — feed it a non-aligned address and you get `EINVAL`. The heap `Box` we started from hands out *allocator-aligned* memory (8 or 16 bytes), not page-aligned. So a literal "`MADV_DONTDUMP` on the Box" doesn't compile-then-work — it compiles and then fails at runtime.

That leaves two real options:

1. **Defer per-region dump-exclusion until you have page-aligned memory.** That's what the page-isolated variant will give us, via `region::alloc`. Until then, dump coverage for the Box-based key comes from the process-wide `RLIMIT_CORE = 0` + `PR_SET_DUMPABLE` in "Harden the process around it."
2. **Round the pointer down to the page boundary and `madvise` the whole page.** It works, but you've now told the kernel to exclude *unrelated heap data* — whatever neighbouring allocations happen to share that page — from your own core dumps. The next time something crashes for a reason that has nothing to do with the secret, the data you'd want for the post-mortem is silently missing.

The example pursues option 1 for the heap-Box buffer and option "do it for real" for the page-isolated variant. For completeness, the `madvise` call you'd make on a page-aligned region looks like this:

```rust
// Only correct when `ptr` is a page-aligned address you own —
// i.e. NOT a Box, but the result of `region::alloc` or mmap.
unsafe {
    os_memlock::madvise_dontdump(ptr as *mut _, len)?;
}
```

(`nix` also exposes `madvise` with `MmapAdvise::MADV_DONTDUMP`. On FreeBSD the equivalent is `MADV_NOCORE`. Man page: [`madvise(2)`](https://man7.org/linux/man-pages/man2/madvise.2.html).)

It's *per-region*, not process-wide — it marks the page(s) you point at as "don't dump." That's the right granularity once we can satisfy the alignment requirement; the process-wide knob is the blunter `harden_process()` approach.

## Bundle it into a `HardenedKey` type

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
        // For this buffer, dump coverage comes from harden_process().
        Ok(Self { bytes, _lock: lock })
    }
}
```

What this earns us:

- **Field-order drop semantics.** Rust drops a struct's fields in declaration order. By putting `bytes` first and `_lock` second, the `Zeroizing` wipe runs *while* the page is still locked — so the wipe lands on resident memory, not on memory the kernel just paged out under our feet. (Reversing the field order would be a quiet bug.)
- **You can't accidentally leak one of the two.** Bytes and lock are wired together — there's no path where one outlives the other or gets forgotten on an early return.
- **The move hazard from part 1 is genuinely gone.** Moving a `HardenedKey` moves the `Box` pointer and the `LockGuard` handle, not the 32 bytes; the heap allocation stays put, and the lock keeps pointing at the same page.

(Types and crate versions are simplified — see [`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening) for a buildable version pinned to real crate versions.)

## Harden the process around it

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

Good news: the `prctl(PR_SET_DUMPABLE, 0)` call above pulls double duty. Marking the process non-dumpable also makes it **non-attachable by `ptrace` for anyone without `CAP_SYS_PTRACE`** (in practice, anyone but root) — the same single call that quiets core dumps also slams the easy door on live-memory snooping.

Wrap both in one startup helper, called *before* any secrets are loaded:

```rust
fn harden_process() {
    if let Err(e) = rlimit::setrlimit(rlimit::Resource::CORE, 0, 0) {
        eprintln!("warn: failed to disable core dumps: {e}");
    }
    if let Err(e) = prctl::set_dumpable(false) {
        eprintln!("warn: failed to mark process non-dumpable: {e}");
    }
}

fn main() -> std::io::Result<()> {
    harden_process();                            // once, at startup
    let key = HardenedKey::load(load_key_from_kms)?;
    crypto_op(key.as_bytes());
    Ok(())
}
```

This is deliberately best-effort — a startup hardening helper shouldn't refuse to run because a container's `seccomp` profile forbids one syscall — but it logs every failure. The shape to avoid is the silent `let _ = ...`: a misconfigured environment then quietly produces a process that *looks* hardened and isn't.

For a system-wide policy there's the Yama LSM, via `/proc/sys/kernel/yama/ptrace_scope`:

- `0` — classic behaviour: any same-user process can attach.
- `1` — restricted (the common default): only a parent, or a tracer the target explicitly allows via `prctl(PR_SET_PTRACER, ...)`.
- `2` — admin-only (needs `CAP_SYS_PTRACE`).
- `3` — no attaching at all, for anyone, until reboot.

For a server: pair `ptrace_scope=1` with the `PR_SET_DUMPABLE` call from above, and you've covered most of the same-user threat at minimal cost. For a CLI handling keys interactively, level `2` is worth it if your environment allows it. Level `3` tends to break debugging tooling enough that it's only worth reaching for on sealed appliances.

The honest caveat, same as always: none of this stops a root / `CAP_SYS_PTRACE` attacker, who can lift any of these. What it does is raise the bar a lot against the realistic same-user case — and that's most of the value.

## Trying it

The companion example ([`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening)) ships with demo modes that exercise each control and the same `/proc/<pid>/mem` scanner from part 1. The [`README`](https://github.com/f-squirrel/f-squirrel.github.io/blob/master/examples/zeroize-os-hardening/README.md) has the full walkthrough, including two one-time system changes needed to reproduce the demos below: setting `core_pattern` to write core files locally (instead of piping to `apport` or `systemd-coredump`) and temporarily lowering `ptrace_scope` to `0` so the unhardened same-user scan actually succeeds. The highlights:

**Core dumps.** Without hardening, crashing dumps the secret to disk:

```text
$ cargo run --release -- crash
UNHARDENED: key loaded (checksum 0x00000000000019c0), PID 2703676
Aborting — check for core.2703676 afterwards.
Aborted (core dumped)

$ python3 scan_core.py core.2703676
  hit at offset 0xab50
Found 1 occurrence(s) of the 32-byte DEADBEEF pattern
```

With `harden_process()` — `RLIMIT_CORE = 0` plus `PR_SET_DUMPABLE = 0` — no core file is produced at all:

```text
$ cargo run --release -- crash-hardened
HARDENED: key loaded (checksum 0x00000000000019c0), PID 2703885
Aborting — check for core.2703885 afterwards.
Aborted

$ ls core.2703885
ls: cannot access 'core.2703885': No such file or directory
```

**Same-user memory scan.** With `ptrace_scope = 0` (classic), a same-user process can walk `/proc/<pid>/mem` for the DEADBEEF pattern — exactly the `scan_mem.py` attack from part 1:

```text
# Terminal 1
$ cargo run --release -- live
Process NOT hardened.

=== UNHARDENED: key is LIVE (checksum 0x00000000000019c0) ===
PID 2704053
  VmLck:         4 kB

# Terminal 2 (same user, no sudo)
$ python3 scan_mem.py 2704053
  hit at 0x63abb893bb50 in [heap]
Found 1 occurrence(s) of the 32-byte DEADBEEF pattern in PID 2704053
```

That `VmLck: 4 kB` line confirms `mlock` did its job — one page is pinned and won't be swapped. But the secret is still visible to any same-user process that reads our memory.

After `harden_process()`, the same scan gets shut out:

```text
# Terminal 1
$ cargo run --release -- live-hardened
Process hardened (RLIMIT_CORE=0, PR_SET_DUMPABLE=0).

=== HARDENED: key is LIVE (checksum 0x00000000000019c0) ===
PID 2704258
  VmLck:         4 kB

# Terminal 2 (same user, no sudo)
$ python3 scan_mem.py 2704258
Permission denied reading /proc/2704258/maps
(Process is likely non-dumpable — PR_SET_DUMPABLE is off.)
```

**Root still walks right through.** Against the same hardened process:

```text
$ sudo python3 scan_mem.py 2704258
  hit at 0x63abb893bb50 in [heap]
Found 1 occurrence(s) of the 32-byte DEADBEEF pattern in PID 2704258
```

Five syscalls, and root reads the key in one line. What we've closed is the *same-user, non-root* window — which is most of the realistic threat surface for a compromised dependency or a nosy co-tenant.

**After drop, the secret is gone.** Once the `live` or `live-hardened` process prints "key DROPPED," scanning again confirms `Zeroizing` did its job:

```text
$ python3 scan_mem.py 2706794
Found 0 occurrence(s) of the 32-byte DEADBEEF pattern in PID 2706794
```

## Going further: a page-isolated variant

Every step above has been about *external* threats — the OS paging memory out, the kernel dumping it on a crash, another process attaching. There's one more category worth knowing about: a stray pointer, out-of-bounds read, or use-after-free *in your own code* that accidentally reads the secret's bytes. The control for that is `mprotect(PROT_NONE)`: deny every access to the page when the key isn't actively in use, so any stray touch SIGSEGVs instead of silently picking up the bytes.

The catch is that `mprotect` works at page granularity, and the heap `Box` from earlier shares its page with allocator metadata and neighbouring allocations — flipping the whole page to `PROT_NONE` would crash anything that touched a neighbour. To use `protect` safely you need a *dedicated* page. That's what `region::alloc` (the `mmap`/`VirtualAlloc` row of the table earlier) is for. Once you own a whole page, `region::lock`, `MADV_DONTDUMP`, *and* `region::protect` can all be applied to it cleanly.

Drop-order discipline matters here too, but the field order is *reversed* from `HardenedKey` — and for good reason. `HardenedKey` puts `bytes` first so the `Zeroizing` wipe runs while the lock is still held. `PageProtectedKey` doesn't need that: it has a custom `Drop` that re-opens the page and wipes *before* any field drops. What matters after the custom `Drop` is that `_lock` (`munlock`) drops before `alloc` (`munmap`), so the unlock lands on still-mapped memory. That's why `_lock` comes first here. Reverse the field order and the unlock fires on a region that's already gone.

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

impl PageProtectedKey {
    fn load(init: impl FnOnce(&mut [u8])) -> io::Result<Self> {
        // 1. Whole page, initially writable so we can fill it.
        let mut alloc = region::alloc(KEY_LEN, Protection::READ_WRITE)
            .map_err(io_err)?;
        let ptr: *const u8 = alloc.as_ptr::<u8>();
        let len: usize = alloc.len(); // page-rounded (typically 4096)

        // 2. Pin and exclude-from-dumps — same controls as earlier, just on a
        //    whole page instead of a corner of one.
        let _lock = region::lock(ptr, len).map_err(io_err)?;
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
        unsafe { region::protect(ptr, len, Protection::NONE).map_err(io_err)?; }
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
        // alloc unmap. Best-effort: if mprotect fails the program is already on
        // its way out; the worst case is the page wasn't wiped before munmap
        // reclaimed it.
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
let mut key = PageProtectedKey::load(|buf| load_key_into(buf))?;
let sig = key.with_readable(|bytes| sign(bytes, &digest))?;
// page is PROT_NONE again until the next call.
```

The companion example includes a `live-page-protected` mode that lets you watch the bracket in action — scanning at each phase to see the page appear and disappear from `/proc/<pid>/maps`.

A few honest things to note about this layer:

- **It defends against bugs, not against in-process attackers.** Any code running inside your process — a compromised dependency, an injected shared object, a supply-chain payload — shares your address space and can call `mprotect` itself to unseal the page whenever it likes, or simply wait for `with_readable` to open the window. The kernel enforces page permissions per-*process*, not per-library. The win here is against *accidents*: stray pointers, OOB reads, an errant log statement. Defending against malicious code in your own process requires keeping the key out of that process entirely — via privilege separation or hardware boundaries.
- **The protection state is per-page and process-global.** That's why `with_readable` takes `&mut self`: two threads calling it on the same key would otherwise race — one's guard re-sealing the page to `PROT_NONE` while the other is still mid-read, SIGSEGVing the reader. `&mut self` forces the borrow checker to serialize calls per key for you. And note the closure can still copy the bytes into its return value — the seal only protects the page, not whatever you carry out of it.
- **You pay a whole page per key.** `region::alloc(32, ...)` rounds up to the system page size (typically 4 KiB). Fine for a handful of long-lived keys; very much not for thousands of short-lived ones.
- **It doesn't help with the threats the earlier sections already covered.** Swap, core dumps, and `ptrace` all bypass page protections — the bytes-on-disk and bytes-via-tracer paths read the underlying memory regardless of `PROT_*` flags. The `region::lock` and `madvise_dontdump` calls inside `load` are still the load-bearing controls there; the `protect` toggle is *added* on top, not a replacement.
- **`harden_process()` still applies unchanged.** Per-key and process-level controls compose freely.

The pattern itself isn't novel, and **if you're reaching for this in production, don't roll your own — use [`secrets`](https://crates.io/crates/secrets)**. It's the same bracket (`PROT_NONE` at rest, `PROT_READ`/`PROT_WRITE` on borrow), but ergonomic (`SecretBox<T>` / `SecretVec<T>`), backed by libsodium's audited primitives, with guard pages and canaries that the hand-rolled version here doesn't have. The `PageProtectedKey` above is here to *show* the pattern, not to compete with it.

A couple of other crates worth knowing about in the same neighbourhood:

- **[`memsec`](https://docs.rs/memsec)** — pure-Rust port of libsodium's `utils.c`, lower-level (free functions, no borrow-API). Worth knowing about in the same space, though it doesn't expose `memfd_secret(2)`. That syscall (Linux ≥ 5.14) is *strictly stronger* than `mlock`+`mprotect`: the kernel itself can no longer map the page, so even `/proc/<pid>/mem` and most kernel-side reads can't reach it — a natural sequel to this step, but you'd call it via `libc::syscall` or a dedicated wrapper today.
- **[`secmem-alloc`](https://crates.io/crates/secmem-alloc)** — different shape: a custom allocator you plug into `Box::new_in` / `Vec::new_in` so every allocation in a secret-bearing region gets `mlock` + zeroize-on-dealloc by default. No `mprotect` bracket, but a much lower per-key cost than a dedicated page.

And then the one *not* to reach for here: **[`secrecy`](https://docs.rs/secrecy)** is explicitly *only* zeroize + a `Debug`-redacted `ExposeSecret` wrapper — no `mlock`, no `mprotect`, no `MADV_DONTDUMP`. It's a good ergonomic top-layer over any of the above, but it doesn't replace them.

The reason all of this is tucked behind *"going further"* rather than the recommended baseline is the threat-model shift: it's a tool for a different problem than the rest of this post.

A working version of both `HardenedKey` and `PageProtectedKey` (with the real `unsafe { ... }` blocks the `region` crate's `protect` API actually requires, and pinned crate versions) is in [`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening). `cargo run --release` prints two matching checksums; the `live`, `live-hardened`, `live-page-protected`, `crash`, and `crash-hardened` modes run the demos from "Trying it" above and this section.

## Where this leaves us

Stacked up, these controls shrink the window in which your secret is recoverable: off the swap file, out of crash dumps, away from same-user snooping, and — at the cost of a page per key — out of reach of accidental in-process reads. For a lot of keys that's enough.

But every knob here quietly assumes the plaintext key is sitting in *your* process's RAM. The next layer makes that assumption false: HSMs, threshold signing, and enclaves arrange for the key never to be in your process at all.

← Previous: [I Zeroized My Secret. Or Did I?](/zeroize)
<!-- → Next: [Don't hold the key: architecture for secrets you can't afford to lose](./part-3-architecture.md) -->

---

*Further reading:*

**Crates**

- `region` — cross-platform virtual memory: `alloc`, `lock`, `protect`, `query` — <https://docs.rs/region>
- `os-memlock` — mlock + MADV_DONTDUMP — <https://docs.rs/os-memlock>
- `rlimit` — <https://docs.rs/rlimit>
- `prctl` — <https://docs.rs/prctl>
- `secrets` — ergonomic Rust wrapper over libsodium's guarded allocation (`SecretBox` / `SecretVec`, the recommended way to use the page-isolated pattern in production) — <https://crates.io/crates/secrets>
- `memsec` — a Rust port of libsodium's secure-allocation primitives (`malloc`, `mlock`, `mprotect`, `memzero`) — <https://docs.rs/memsec>
- `secmem-alloc` — secret-memory custom allocator for `Box::new_in` / `Vec::new_in` — <https://crates.io/crates/secmem-alloc>
- `secrecy` — ergonomic `Secret<T>` / `ExposeSecret` (zeroize + `Debug` redaction *only* — no mlock/mprotect/MADV) — <https://docs.rs/secrecy>

**Kernel and libc docs**

- `core(5)` — <https://man7.org/linux/man-pages/man5/core.5.html>
- `mlock(2)` — <https://man7.org/linux/man-pages/man2/mlock.2.html>
- `madvise(2)` — <https://man7.org/linux/man-pages/man2/madvise.2.html>
- `mprotect(2)` — <https://man7.org/linux/man-pages/man2/mprotect.2.html>
- `prctl(2)` — <https://man7.org/linux/man-pages/man2/prctl.2.html>
- `ptrace(2)` — <https://man7.org/linux/man-pages/man2/ptrace.2.html>
- `memfd_secret(2)` — Linux ≥ 5.14, kernel-level secret memory beyond what `mlock`/`mprotect` reach — <https://man7.org/linux/man-pages/man2/memfd_secret.2.html>
- Yama / `ptrace_scope` — <https://docs.kernel.org/admin-guide/LSM/Yama.html>
- libsodium guarded heap allocation (the production-grade version of the page-isolated pattern, with guard pages and a canary) — <https://doc.libsodium.org/memory_management> · source: [`src/libsodium/sodium/utils.c`](https://github.com/jedisct1/libsodium/blob/master/src/libsodium/sodium/utils.c) (`sodium_malloc` / `sodium_mprotect_noaccess` / `sodium_mprotect_readonly` / `sodium_mprotect_readwrite`)
