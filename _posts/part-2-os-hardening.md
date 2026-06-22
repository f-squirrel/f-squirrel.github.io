# Where `zeroize` stops: hardening keys at the OS level

*This follows on from [I Zeroized My Secret. Or Did I?](./part-1-zeroize.md), where we found that `zeroize` reliably wipes a secret you can name — but copies still leak past it through register spills, moves, and `Vec` reallocation. Those leftover copies live in your process's memory, and the operating system is free to move that memory around. This post is about taking those moves away from it.*

There are really three things the OS can do with a secret sitting in your RAM, and each one needs a different knob:

1. **page it to swap** (or capture it in a hibernation image),
2. **write it into a core dump** when you crash,
3. **hand it to another process** that reads your live memory.

Let's take them one at a time. All of these are cheap to add and worth it for high-value keys.

## 1. Memory can be swapped (and hibernated)

The kernel might page your secret out to the swap file, leaving a copy sitting on disk long after the process is gone. The fix is `mlock(2)`: pin the pages in RAM so they're never swapped.

The friendliest way to do this in Rust is the cross-platform [`region`](https://docs.rs/region) crate (it maps to `VirtualLock` on Windows and `mlock` on Unix):

```rust
// region = "3"
let secret = Zeroizing::new([0u8; 32]);
let _guard = region::lock(secret.as_ptr(), secret.len())?;
// pages stay resident until `_guard` drops
```

If you'd rather go lower-level, [`nix`](https://docs.rs/nix) gives you `nix::sys::mman::mlock`, and [`os-memlock`](https://docs.rs/os-memlock) offers thin `mlock`/`munlock` wrappers. Man page: [`mlock(2)`](https://man7.org/linux/man-pages/man2/mlock.2.html).

A couple of honest heads-ups, because `mlock` has sharp edges:

- **It can silently fail.** A process may only lock up to its `RLIMIT_MEMLOCK` limit (`ulimit -l`), which on many systemd setups is a modest few megabytes. Go over it and `mlock` returns `ENOMEM`. So check the return value — a "locked" buffer that didn't actually lock is worse than knowing you couldn't. You can read or raise the limit (within your hard cap) with the `rlimit` crate.
- **It stops swapping, not hibernation.** Suspend-to-disk snapshots *all* of RAM — locked pages included — into the hibernation image. Covering that means an encrypted hibernation image or no hibernation at all; it isn't something your process can fix on its own.
- **Windows differs.** `VirtualLock` keeps pages in the working set, but the working-set model has its own quirks; treat the cross-platform wrapper as best-effort and test on each target.

## 2. Memory can end up in a core dump

If the process crashes, the OS can write your whole address space — secrets and all — into a core file or hand it to a crash reporter. Two independent things to do here.

**(a) Keep the secret's pages out of dumps** with `madvise(MADV_DONTDUMP)`. The kernel's [`core(5)`](https://man7.org/linux/man-pages/man5/core.5.html) confirms a dump will leave out any region you mark this way.

```rust
// os-memlock = "..."   (Linux: MADV_DONTDUMP, FreeBSD: MADV_NOCORE)
unsafe { os_memlock::madvise_dontdump(ptr, len)?; }
```

`nix` also exposes `madvise` with `MmapAdvise::MADV_DONTDUMP`. Man page: [`madvise(2)`](https://man7.org/linux/man-pages/man2/madvise.2.html).

**(b) Turn off core dumps for the whole process.** The obvious move is `setrlimit(RLIMIT_CORE, 0)`:

```rust
// rlimit = "0.10"
use rlimit::{setrlimit, Resource};
setrlimit(Resource::CORE, 0, 0)?;   // soft = hard = 0
```

Here's the gotcha that's genuinely worth knowing: `core(5)` says **`RLIMIT_CORE` gets ignored when dumps are piped to a program** — which is exactly what `systemd-coredump` does on most modern Linux boxes. So `RLIMIT_CORE = 0` on its own can still quietly ship your crash to the journal. The fix is to also call `prctl(PR_SET_DUMPABLE, 0)`:

```rust
// prctl = "1"
let _ = prctl::set_dumpable(false);   // also quiets systemd-coredump
```

Man page: [`prctl(2)`](https://man7.org/linux/man-pages/man2/prctl.2.html).

## 3. Another process can read your live memory

This is the threat the first post admitted `zeroize` can't touch: an attacker who reads your memory *while the process is running*. On Linux the usual route is `ptrace` (or reading `/proc/<pid>/mem`), and a process running as the same user can do it by default. In a world of compromised dependencies and shared hosts, that's not exotic.

Good news: `prctl(PR_SET_DUMPABLE, 0)` from the last section pulls double duty. Marking the process non-dumpable also makes it **non-attachable by `ptrace` for anyone who isn't root** — so the single call that quiets core dumps also slams the easy door on live-memory snooping.

For a system-wide policy there's the Yama LSM, via `/proc/sys/kernel/yama/ptrace_scope`:

- `0` — classic behaviour: any same-user process can attach.
- `1` — restricted (the common default): only a parent, or a tracer the target explicitly allows via `prctl(PR_SET_PTRACER, ...)`.
- `2` — admin-only (needs `CAP_SYS_PTRACE`).
- `3` — no attaching at all, for anyone, until reboot.

The honest caveat, same as always: none of this stops a root / `CAP_SYS_PTRACE` attacker, who can lift any of these. What it does is raise the bar a lot against the realistic same-user case — and that's most of the value.

## Putting it in one buffer

These knobs are most useful stacked. Here's a sketch of a 32-byte key that is wiped on drop, pinned in RAM, and excluded from dumps — plus a one-time process-hardening call you'd run at startup:

```rust
use zeroize::Zeroizing;

/// Run once, early, before any secrets are loaded.
fn harden_process() {
    let _ = rlimit::setrlimit(rlimit::Resource::CORE, 0, 0); // no core dumps
    let _ = prctl::set_dumpable(false);                      // + quiet systemd, + block ptrace
}

/// A secret that is zeroized on drop, mlock'd (no swap), and MADV_DONTDUMP'd.
/// Boxed so its address is *stable*: remember from part 1 that moving the
/// owner would otherwise memcpy the bytes and leave the lock pointing at a
/// stale (unwiped!) location.
struct HardenedKey {
    bytes: Box<Zeroizing<[u8; 32]>>,
    _lock: region::LockGuard,
}

impl HardenedKey {
    fn load(init: impl FnOnce(&mut [u8; 32])) -> std::io::Result<Self> {
        let mut bytes = Box::new(Zeroizing::new([0u8; 32]));
        init(&mut bytes);
        let (ptr, len) = (bytes.as_ptr(), bytes.len());
        let lock = region::lock(ptr, len)?;                          // pin: no swap
        unsafe { os_memlock::madvise_dontdump(ptr as *mut _, len)?; } // exclude from dumps
        Ok(Self { bytes, _lock: lock })
    }
}
```

(Types and crate versions are simplified — check the exact `region::LockGuard` name and signatures against the versions you pin.) Notice that the move hazard from part 1 reappears here at the OS layer: the boxed buffer is the fix, because the *heap* allocation stays put even when the struct that owns it moves.

## Where this leaves us

Stacked up, these controls meaningfully shrink the window in which your secret is recoverable: off the swap file, out of crash dumps, and away from same-user snooping. That's a real improvement, and for a lot of keys it's enough.

But notice what every single knob here quietly assumes: that the plaintext key is sitting in *your* process's RAM in the first place. Each control is damage limitation around that fact. The strongest move isn't a better knob — it's making the assumption false, so there's nothing in your address space to protect.

That's the last layer: HSMs, threshold signing, and enclaves — arranging for the key never to be in your process at all.

← Previous: [I Zeroized My Secret. Or Did I?](./part-1-zeroize.md)
→ Next: [Don't hold the key: architecture for secrets you can't afford to lose](./part-3-architecture.md)

---

*Further reading:*

- `region` (mlock) — https://docs.rs/region
- `os-memlock` (mlock + MADV_DONTDUMP) — https://docs.rs/os-memlock
- `rlimit` — https://docs.rs/rlimit · `prctl` — https://docs.rs/prctl
- `core(5)` — https://man7.org/linux/man-pages/man5/core.5.html
- `mlock(2)` — https://man7.org/linux/man-pages/man2/mlock.2.html · `madvise(2)` — https://man7.org/linux/man-pages/man2/madvise.2.html · `prctl(2)` — https://man7.org/linux/man-pages/man2/prctl.2.html
- Yama / `ptrace_scope` — https://docs.kernel.org/admin-guide/LSM/Yama.html · `ptrace(2)` — https://man7.org/linux/man-pages/man2/ptrace.2.html
