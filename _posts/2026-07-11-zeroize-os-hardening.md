---
title: "Secrets in Memory: More Exposed Than You Think"
subtitle: "Where zeroize stops: keeping keys out of swap, dumps, and other processes"
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

Rather than touring the three controls separately, let's build a hardened key buffer one piece at a time — adding each control only when the previous version is missing something — and then harden the process around it. By the end we'll have a small `HardenedKey` type and a one-shot `harden_process()` call. (A fourth concern — your *own* code reading the secret by accident — needs a different tool and gets the next post to itself.)

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

## Pin it in RAM so it can't be swapped

A heap allocation is still pageable memory. The kernel might page your secret out to the swap file, leaving a copy sitting on disk long after the process is gone. The fix is `mlock(2)`: tell the kernel "don't swap these pages out."

Note that `mlock` — like every kernel control in this post — operates on a *specific address range*. That's why part 1's "allocate, then fill in place" `Box` matters here and not just as a tidiness point: it hands us a fixed heap address that won't wander off after we pin it. Lock a stack array instead, let the value move, and you've pinned an empty page while the live secret sits on an unlocked one.

Two mechanical details worth knowing before the first call: `mlock(addr, len)` takes a byte range but the kernel rounds it out to every page that range touches (typically 4 KiB each) — you can't pin just 32 bytes, and a secret straddling a page boundary locks both pages. And the undo is `munlock`, same signature; forget it and the pages stay locked for the lifetime of the process.

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
let _lock = region::lock(key.as_ptr(), key.len())?;  // pin the zeroed page first…
load_key_into(&mut key);                             // …then fill it, into locked memory
// pages stay resident until `_lock` drops, which calls munlock/VirtualUnlock
```

The heap `Box` is doing real work here: the lock points at a heap address that *won't move* if `key` is later passed to another function or stored in a struct. If we'd locked a stack `[0u8; 32]` instead, then moved `key`, the lock would still be pinning the original (now stale) stack page and the live bytes would sit on an unlocked one.

A note on the sibling call: `region::protect` (i.e. `mprotect`/`VirtualProtect`) is a *different* control. It changes the read/write/execute flags on a page — useful for things like marking a buffer no-access between uses, or making JIT pages executable — but it doesn't stop the kernel from paging that memory out. For "don't swap my secret," reach for `lock`; `protect` is a separate axis. (This is also why `secrecy` says it deliberately does neither — it's leaving the OS-level posture to you.)

If you'd rather go lower-level, [`nix`](https://docs.rs/nix) gives you `nix::sys::mman::mlock`, and [`os-memlock`](https://docs.rs/os-memlock) offers thin `mlock`/`munlock` wrappers. Man page: [`mlock(2)`](https://man7.org/linux/man-pages/man2/mlock.2.html).

A couple of honest heads-ups, because `mlock` has sharp edges:

- **It can silently fail.** Locked memory counts against `RLIMIT_MEMLOCK` (`ulimit -l`), which on many systemd setups is a modest few megabytes. Go over it and `mlock` returns `ENOMEM` — the call doesn't half-succeed, but it also doesn't yell at you unless you check the return value. So check it: a "locked" buffer that didn't actually lock is worse than knowing you couldn't. You can read or raise the limit (within your hard cap) with the `rlimit` crate.
- **It stops swapping, not hibernation.** Suspend-to-disk snapshots *all* of RAM — locked pages included — into the hibernation image. Covering that means an encrypted hibernation image or no hibernation at all; it isn't something your process can fix on its own.
- **Locks don't survive `fork`.** Memory locks are not inherited by a child created via `fork(2)` — the child's copy-on-write copy of the secret page is *swappable*, even while the parent's stays pinned. Any dependency that spawns a subprocess the classic way (`fork` + `exec`) briefly duplicates your secret into an unlocked address space. So load keys *after* any daemonizing or forking setup, and treat "fork while a key is live" as a leak — or use the page-isolated variant in the next post, which can tell the kernel to hand the child a zero-filled page instead (`MADV_WIPEONFORK`).
- **On a swapless host it buys less secrecy value.** Plenty of servers and containers run with no disk swap at all, or only compressed `zram` that never touches disk; there, `mlock`'s swap protection has no disk path to block. It still pins pages, consumes the `RLIMIT_MEMLOCK` budget, and can fail — so keep checking the return value — and it's cheap insurance where swap *does* exist. Just don't read "I called `mlock`" as "my secret can't reach disk," because hibernation (above) still can.
- **Windows differs.** `VirtualLock` keeps pages in the working set, but the working-set model has its own quirks; treat the cross-platform wrapper as best-effort and test on each target.

## Keep it out of core dumps

If the process crashes, the OS can write your whole address space — secrets and all — into a core file or hand it to a crash reporter. The per-region knob for that is `madvise(MADV_DONTDUMP)`: tell the kernel that a specific memory range should be excluded from any core dump. The kernel's [`core(5)`](https://man7.org/linux/man-pages/man5/core.5.html) confirms a dump will leave out any region you mark this way.

Before we reach for it though, there's a *real* gotcha worth knowing — and it's the reason the working example for this post ([`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening)) takes a slightly different shape than you might expect:

> **Linux's `madvise` requires the address to be page-aligned.** `mlock` doesn't (it rounds down internally), but `madvise` does — feed it a non-aligned address and you get `EINVAL`. The heap `Box` we started from hands out *allocator-aligned* memory (8 or 16 bytes), not page-aligned. So a literal "`MADV_DONTDUMP` on the Box" doesn't compile-then-work — it compiles and then fails at runtime.

That leaves two real options:

1. **Defer per-region dump-exclusion until you have page-aligned memory.** That's what the page-isolated variant in the next post gives us, via `region::alloc`. Until then, dump coverage for the Box-based key comes from the process-wide `RLIMIT_CORE = 0` + `PR_SET_DUMPABLE` in "Harden the process around it."
2. **Round the pointer down to the page boundary and `madvise` the whole page.** It works, but you've now told the kernel to exclude *unrelated heap data* — whatever neighbouring allocations happen to share that page — from your own core dumps. The next time something crashes for a reason that has nothing to do with the secret, the data you'd want for the post-mortem is silently missing.

The example pursues option 1 for the heap-Box buffer (and does the per-region call for real on the page-isolated buffer, next post). For completeness, the `madvise` call you'd make on a page-aligned region looks like this:

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
        let (ptr, len) = (bytes.as_ptr(), bytes.len());
        // Pin the (zeroed) page *before* writing the secret, so the key bytes
        // never sit in swappable memory even briefly. (`io_err` maps
        // region::Error into io::Error — a one-liner shown in the example.)
        let lock = region::lock(ptr, len).map_err(io_err)?;  // pin: no swap
        init(&mut bytes);                                    // fill in place, into locked memory
        // NOTE: no per-buffer MADV_DONTDUMP here — the Box isn't page-aligned.
        // For this buffer, dump coverage comes from harden_process().
        Ok(Self { bytes, _lock: lock })
    }
}
```

What this earns us:

- **Field-order drop semantics.** Rust drops a struct's fields in declaration order, so `bytes` (first) is wiped before `_lock` (second) releases the pin. For `HardenedKey` this is tidiness more than safety — `munlock` doesn't evict the page, and the `Box` stays mapped until it drops last — but keep `bytes` first as a clean default; the ordering becomes load-bearing in the page-isolated variant next post. (One corner case: if `init` panics before `Self` exists, field order doesn't apply — `load`'s locals unwind instead, and the page can be unlocked a moment before the wipe. Treat the closure as non-panicking, or add a construction-time wipe guard if that unwind window matters to your threat model.)
- **You can't accidentally leak one of the two.** Bytes and lock are wired together — there's no path where one outlives the other or gets forgotten on an early return.
- **The move hazard from part 1 is genuinely gone.** Moving a `HardenedKey` moves the `Box` pointer and the `LockGuard` handle, not the 32 bytes; the heap allocation stays put, and the lock keeps pointing at the same page.
- **Locks don't stack, and the page isn't exclusively ours.** Linux tracks `mlock` as a `VM_LOCKED` bit on the VMA covering the (page-rounded) range, not a per-page refcount per caller — so if two `HardenedKey`s happen to land on the same heap page, dropping one can `munlock` the page while the other key is still live. The page may also hold allocator metadata or unrelated heap data that gets pinned along for the ride. That's an acceptable baseline cost for a *single* 32-byte key — but with a modern allocator packing small allocations, two keys sharing a page is the likely case, not a corner case, and the failure is silent: `VmLck` drops to zero and nothing in the program can tell. If your process holds more than one `HardenedKey` at a time, assume they collide, and use the page-isolated variant from the next post (or the [`secrets`](https://crates.io/crates/secrets) crate) instead — there every key owns its page, so one key's unlock can't strip another's protection.

(Types and crate versions are simplified — see [`examples/zeroize-os-hardening`](https://github.com/f-squirrel/f-squirrel.github.io/tree/master/examples/zeroize-os-hardening) for a buildable version pinned to real crate versions.)

## Harden the process around it

Everything above is per-key. There's a second category of fixes that don't belong on the buffer at all — they apply once, at process startup, and cover the gaps a per-page approach can't.

**Turn off core dumps for the whole process.** `MADV_DONTDUMP` is a hint on a specific region; a dump path can still capture *other* regions that happened to hold a copy (a spill, a stale stack slot, a `Vec` that grew and reallocated). The blunt instrument is `setrlimit(RLIMIT_CORE, 0)`:

```rust
use rlimit::{setrlimit, Resource};
setrlimit(Resource::CORE, 0, 0)?;   // soft = hard = 0
```

This is application startup policy, not something a library should do as a side effect — setting the *hard* limit to zero here is intentionally irreversible for the rest of the process's life (a process can lower its own hard limit but generally can't raise it back without privilege). That's the right call for a hardening entry point, but it would be a surprising thing for library code or test tooling to do on your behalf.

Here's the gotcha that's genuinely worth knowing: `core(5)` says **`RLIMIT_CORE` gets ignored when dumps are piped to a program** — which is exactly what `systemd-coredump` does on most modern Linux boxes. So `RLIMIT_CORE = 0` on its own can still quietly ship your crash to the journal. The fix is to also call `prctl(PR_SET_DUMPABLE, 0)`:

```rust
let _ = prctl::set_dumpable(false);   // also quiets systemd-coredump
```

Man page: [`prctl(2)`](https://man7.org/linux/man-pages/man2/prctl.2.html).

**Block live-memory snooping.** This is the threat the first post admitted `zeroize` can't touch: an attacker who reads your memory *while the process is running*. On Linux the usual route is `ptrace` (or reading `/proc/<pid>/mem`), and under the classic ptrace policy a same-user process can do it by default — though, as the Yama table below shows, most modern distros already restrict this out of the box. In a world of compromised dependencies and shared hosts, the same-user case is still not exotic.

Good news: the `prctl(PR_SET_DUMPABLE, 0)` call above pulls double duty. Marking the process non-dumpable also makes it **non-attachable by `ptrace` for anyone without `CAP_SYS_PTRACE`** (in practice, anyone but root) — the same single call that quiets core dumps also slams the easy door on live-memory snooping.

Wrap both in one startup helper, called *before* any secrets are loaded. It's deliberately best-effort — a hardening helper shouldn't refuse to run because a container's seccomp profile forbids one syscall — but "best-effort" has to mean more than logging and moving on. The shape to avoid is the silent `let _ = ...` *or* an unconditional "Process hardened" print regardless of outcome: in a restricted container or seccomp profile, one or both syscalls may fail, and a boolean success label at that point is a false-positive hardening state. So the helper returns a status the caller can act on instead of a bare `()`:

```rust
struct HardenStatus {
    core_dumps_disabled: bool,
    non_dumpable: bool,
}

impl HardenStatus {
    fn fully_hardened(&self) -> bool {
        self.core_dumps_disabled && self.non_dumpable
    }
}

fn harden_process() -> HardenStatus {
    let core_dumps_disabled = match rlimit::setrlimit(rlimit::Resource::CORE, 0, 0) {
        Ok(()) => true,
        Err(e) => { eprintln!("warn: failed to disable core dumps: {e}"); false }
    };
    let non_dumpable = match prctl::set_dumpable(false) {
        Ok(()) => true,
        Err(e) => { eprintln!("warn: failed to mark process non-dumpable: {e}"); false }
    };
    HardenStatus { core_dumps_disabled, non_dumpable }
}

fn main() -> std::io::Result<()> {
    let status = harden_process();                // once, at startup
    if !status.fully_hardened() {
        // Fail closed: don't load secrets into a process that couldn't harden.
        return Err(std::io::Error::other(format!(
            "hardening failed (core_dumps_disabled={}, non_dumpable={}); refusing to load key",
            status.core_dumps_disabled, status.non_dumpable
        )));
    }
    let key = HardenedKey::load(load_key_from_kms)?;
    crypto_op(key.as_bytes());
    Ok(())
}
```

A misconfigured environment now produces a process that *refuses to load the key*, instead of one that silently looks fine and isn't. Whether to fail closed or to warn and continue is a deployment-policy call — just make it deliberately, in one visible place, rather than drifting into a warning line nobody reads followed by business as usual.

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

**Same-user memory scan.** One caveat first: a stock box defaults to `ptrace_scope=1`, which *already* blocks a non-parent same-user process from reading `/proc/<pid>/mem` — the unhardened process below isn't as exposed as this looks. The README lowers it to `0` so the attack is visible at all. With that in place, a same-user process can walk `/proc/<pid>/mem` for the DEADBEEF pattern — the `scan_mem.py` attack from part 1:

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

## Where this leaves us

Stacked up, these controls shrink the window in which your secret is recoverable: off the swap file, out of crash dumps, away from same-user snooping. For a lot of keys that's enough.

Two gaps remain, and they each get a post of their own. First, nothing above stops *your own process* from reading the key by accident — a stray pointer, an out-of-bounds read, an errant log statement all walk right past `mlock` and `prctl`. The next post gives the key a dedicated page and seals it behind `mprotect(PROT_NONE)`, so a stray touch crashes instead of leaking. And further out: every knob here quietly assumes the plaintext key is sitting in *your* process's RAM, where any code sharing your address space can undo your protections. Privilege separation and the Linux kernel keyring make that assumption false — that's where the series ends up.

← Previous: [I Zeroized My Secret. Or Did I?](/zeroize)
<!-- → Next: [Seal the Key Behind PROT_NONE](/zeroize-page-protected) — uncomment when published -->

---

*Further reading:*

**Crates**

- `region` — cross-platform virtual memory: `alloc`, `lock`, `protect`, `query` — <https://docs.rs/region>
- `os-memlock` — mlock + MADV_DONTDUMP — <https://docs.rs/os-memlock>
- `rlimit` — <https://docs.rs/rlimit>
- `prctl` — <https://docs.rs/prctl>
- `secrecy` — ergonomic `Secret<T>` / `ExposeSecret` (zeroize + `Debug` redaction *only* — no mlock/mprotect/MADV) — <https://docs.rs/secrecy>

**Kernel and libc docs**

- `core(5)` — <https://man7.org/linux/man-pages/man5/core.5.html>
- `mlock(2)` — <https://man7.org/linux/man-pages/man2/mlock.2.html>
- `madvise(2)` — <https://man7.org/linux/man-pages/man2/madvise.2.html>
- `prctl(2)` — <https://man7.org/linux/man-pages/man2/prctl.2.html>
- `ptrace(2)` — <https://man7.org/linux/man-pages/man2/ptrace.2.html>
- Yama / `ptrace_scope` — <https://docs.kernel.org/admin-guide/LSM/Yama.html>
