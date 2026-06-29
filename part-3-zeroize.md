# Improvement ideas for the zeroize series

## Series structure

1. **Part 1** — Wipe it (`zeroize`): move hazards, stack vs heap, register spills, Vec reallocation
2. **Part 2** — Stop the OS from copying it: mlock, dumps, ptrace, page-isolated variant
3. **Part 3** (new) — Keep it out of your process: `memfd_secret`, privilege separation, sandboxing (seccomp, Landlock). Demonstrable on any Linux box, no special hardware.
4. **Part 4** (existing draft in `_posts/part-3-architecture.md`) — Keep it off your machine: kernel keyring, HSMs, KMS, MPC, TEEs

## Part 2 (OS hardening) — gaps to close

- ~~**Explicitly call out the in-process attacker limit on `PageProtectedKey`.**~~ Done (50b614e). Rewrote the "defends against bugs" bullet to name supply-chain attacks, explain why `mprotect` can't help (kernel enforces per-process), and point to privsep/hardware as the real answer.

- ~~**Stale `memsec` / `memfd_secret` claim.**~~ Done (45a9787, 50b614e). Fixed in both the body and the "Further reading" section. `memsec` doesn't expose `memfd_secret`.

- ~~**Orphaned "Steps 2 and 3" reference in `PageProtectedKey` code.**~~ Done (45a9787). Changed to "same controls as earlier."

- ~~**Non-compiling `PageProtectedKey` code.**~~ Done (45a9787). Replaced with real code from examples (added `io_err` helper, `.map_err(io_err)?`).

- ~~**Drop comment framing mismatch.**~~ Done (45a9787). Aligned with actual code's best-effort framing.

- ~~**Drop-order explanation didn't acknowledge reversed field order.**~~ Done (45a9787). Rewritten to explain why `_lock` comes first in `PageProtectedKey` vs `bytes` first in `HardenedKey`.

- ~~**Part 2 closing teased HSMs/TEEs instead of part 3.**~~ Done (9b8a4b5). Now teases privsep + kernel keyring.

- **`memfd_secret(2)` deserves its own section, not a parenthetical.** It's the one Linux primitive that actually removes pages from the kernel's direct map — even `/proc/<pid>/mem` and most kernel-side reads can't reach them. Currently buried in the `memsec` bullet. No Rust crate wraps it yet; a raw `libc::syscall` example would be valuable.

## Part 3 (new) — privilege separation + kernel keyring

Core idea: `memfd_secret` as the strongest single-process control, then privilege separation as the real answer to in-process attackers, with seccomp + Landlock to harden the privileged child.

### `memfd_secret(2)` — kernel-sealed memory

- Part 2 buried this in a `memsec` parenthetical. It deserves a full section because it's the one Linux primitive (≥ 5.14) that removes pages from the kernel's direct map — even `/proc/<pid>/mem` and most kernel-side reads can't reach them. Strictly stronger than `mlock` + `mprotect`.
- No Rust crate wraps it today. Show a raw `libc::syscall(SYS_memfd_secret, 0)` + `mmap` example: create the fd, mmap a page, write the secret, use it, munmap.
- Demo: `scan_mem.py` against a process using `memfd_secret` — zero hits even with `sudo`.
- Limitations: requires `CONFIG_SECRETMEM=y` in the kernel (not all distros enable it), costs a whole page, and the secret is still visible to code *inside* the process (same in-process attacker caveat as `PageProtectedKey`). The real payoff comes when composed with privsep: the privileged child uses `memfd_secret`, so even root reading `/proc/<child>/mem` gets nothing.
- Transition: `memfd_secret` is the strongest single-process control, but any code sharing the address space can still call `munmap` or just read the fd. To actually keep the key away from your dependencies, you need a process boundary.

### Privilege separation

- The OpenSSH model: fork a minimal privileged child that holds the key and exposes only "sign this" / "decrypt this" over IPC. The unprivileged parent handles networking, parsing, and all the dependency-heavy work.
- Other examples: qmail (split into ~8 processes), Postfix, Chrome (renderer vs broker).
- Strongest defence against supply-chain attacks without hardware — a compromised crate owns the unprivileged process but can't read the key because it's in a different address space.
- Demo idea: a two-process Rust example — parent loads key, forks, parent drops the key, child holds it and serves signing requests over a Unix domain socket. Show `scan_mem.py` finding nothing in the parent.

#### IPC design

The plan currently says "Unix socket or pipe" — a reader trying to build this needs more:
- **Why Unix domain sockets over pipes** — bidirectional, and `SO_PEERCRED` / `SCM_CREDENTIALS` give the privileged child the PID/UID of whoever's asking it to sign. The child can reject requests from unexpected peers.
- **Message framing and validation** — the privileged side must treat the unprivileged side as untrusted. Length-prefixed messages, bounded max size, explicit command enum (`Sign`, `Shutdown`, nothing else). A serde-less, allocation-free protocol for the privileged side keeps the attack surface minimal.
- **What *not* to expose** — the privileged child should never have an "export key" or "read key" command. The API surface *is* the security boundary.
- This is where the post stops being "here's how privsep works conceptually" and becomes practically useful.

#### Sandboxing the privileged child

- Compose with part 2 controls: the privileged child should also `harden_process()`, `mlock` the key (or use `memfd_secret`).
- `seccomp` after init: once the privileged child has loaded the key and set up crypto, lock down the syscall surface. A compromised dependency can't call `ptrace`, `mprotect`, or `process_vm_readv` if seccomp blocks them. Relevant crate: `seccompiler` (from Firecracker).
  - Gotcha worth noting: `io_uring` can bypass seccomp filters in some kernel versions. If the privileged child doesn't need `io_uring`, block `io_uring_setup` in the seccomp filter.
- Capability dropping: `prctl(PR_SET_NO_NEW_PRIVS, 1)` + drop `CAP_SYS_PTRACE` after startup. Cheap, composable.
- **Landlock** (Linux ≥ 5.13): complements seccomp. Seccomp restricts *which syscalls*; Landlock restricts *which filesystem paths*. The unprivileged parent could Landlock itself to deny reading `/proc/*/mem`, `/proc/*/maps`, and other sensitive paths. The privileged child could Landlock itself to only its socket path after startup. Crate: `landlock` (`landlock-abi`).

#### Signal handling and clean shutdown

- Part 1 noted that `Drop` doesn't run on abort/SIGKILL. In the privsep model this becomes a design question: how does the privileged child zeroize the key on SIGTERM/SIGINT?
- A signal handler that sets an atomic flag, checked in the IPC loop, triggering orderly `drop()` before `_exit`. Ties back to part 1's "Drop doesn't always run" caveat and makes the privsep model feel complete rather than demo-only.
- SIGKILL is still uncatchable — the key bytes survive in RAM until the page is reused. `memfd_secret` pages are scrubbed by the kernel on process exit, which is one more reason to prefer it.

#### The cost

- IPC overhead, two binaries (or fork-based), protocol design. The privileged side must validate every request.

### Testing the hardening — verification section

Continue the `scan_mem.py` tradition from parts 1 and 2:
- Run `scan_mem.py` against the **unprivileged parent** — zero hits (it dropped the key after fork).
- Run it against the **privileged child** — hits if using plain `mlock`, zero hits if using `memfd_secret`.
- Try `ptrace` across the process boundary — fails (different process, non-dumpable).
- Try a forbidden syscall from the privileged child after seccomp is applied — blocked.
- Hands the reader a concrete "did my hardening actually work?" checklist.

### The full stack: one annotated startup sequence

A single code block showing the privileged child's `main()` that layers everything from parts 1–3:

```
harden_process()           // part 2: no dumps, no ptrace
load key into memfd_secret // part 3: kernel-sealed memory
mlock the page             // part 2: no swap (redundant with memfd_secret but defense-in-depth)
set PR_SET_NO_NEW_PRIVS    // part 3: no escalation
apply seccomp filter       // part 3: minimal syscall surface
apply landlock             // part 3: minimal fs access
enter IPC loop             // part 3: serve sign requests
// on SIGTERM: zeroize, exit
```

This "composition" block is what makes the series feel like it builds to something rather than being three independent posts.

### Closing for part 3

- Tie back: part 2's `PageProtectedKey` protects the page but any in-process code can unseal it. `memfd_secret` hides it from the kernel but not from in-process code. Privsep moves the key across a process boundary where page-table tricks are no longer the defence — the kernel's process isolation is.
- Tease part 4: "Even with privsep, the key still lives on your machine — in the privileged child's RAM, or in the kernel keyring. For the keys you truly can't afford to lose, the next step is to arrange for the plaintext to never exist on your host at all."

### Out of scope for part 3 (footnotes / further reading)

- **Namespace isolation** (`CLONE_NEWUSER` / `CLONE_NEWPID` via `clone3`) — interesting but adds enough complexity to dilute the post. Mention as a possible next step.
- **`pidfd_open`** (Linux 5.3+) — race-free process monitoring for the privsep model. Useful but not load-bearing for the security story.

## Part 4 (existing draft `_posts/part-3-architecture.md`) — issues to fix

- **Rename file** to match the new numbering (part 4, not part 3).
- **Relative links are broken.** `./part-1-zeroize.md` and `./part-2-os-hardening.md` won't resolve in Jekyll — use permalinks (`/zeroize`, `/zeroize-os-hardening`).
- **Missing Jekyll front matter.** No `title:`, `permalink:`, `tags:`, `published:` etc.
- **Table separators violate MD060.** `|---|---|---|` should be `| --- | --- | --- |`.
- **Add rows for `memfd_secret`, privsep, seccomp, Landlock, and kernel keyring** to the "which layer stops what" table.

### New section to add: Linux kernel keyring (bridge from part 3 to HSMs)

Moved here from part 3 — it's a natural bridge between "keep it out of your process" (privsep) and "keep it off your machine" (HSMs/KMS). The key lives in kernel memory, not your address space, but still on the same host.

- Keys live in kernel memory, not process address space. Process holds a handle (serial ID), not the bytes.
- `linux-keyutils` crate (v0.2.5) wraps `add_key` / `keyctl` / `request_key`.
- A compromised dependency can't pointer-walk to the key — must call `keyctl(KEYCTL_READ)`, which the kernel checks against the key's permission mask.
- Demo idea: store the DEADBEEF pattern in the session keyring, show `scan_mem.py` finding nothing in process memory. Read it back via the handle, use it, then `key.invalidate()`.
- Limitations: the key *is* readable via `keyctl(KEYCTL_READ)` if the process has `KEY_POS_READ` — so a compromised dependency in the same process can still call the syscall. Kernel keyring protects against accidental leaks and pointer-based reads, not against deliberate `keyctl` calls from in-process code. Privsep (part 3) composes with it: the unprivileged process never gets the key serial.
- Positioning: kernel keyring keeps the key on the machine but out of your address space. HSMs/KMS (next section) keep it off the machine entirely. The keyring is the stepping stone.
- Man page: `keyrings(7)`, `keyctl(2)`, `add_key(2)`.

## Ideas that could go in any post or a future addendum

- **Encrypted-at-rest in RAM.** Keep the key AES-wrapped in process memory; unwrap into a local variable only for the operation, then zeroize the local. Reduces the window where plaintext exists. Some crypto libraries (libsodium's `crypto_secretbox`) support this pattern natively.

- **Testing your hardening.** How to verify each control actually works: check `VmLck` in `/proc/self/status`, try `ptrace` attach from another process, trigger a crash and confirm no core file, scan `/proc/pid/maps` for expected protection flags. The demo modes in the example do some of this, but a "hardening self-test" checklist would be useful standalone.

- **Cost/benchmark of each control.** `mlock` has a budget (`RLIMIT_MEMLOCK`), `mprotect` flipping has syscall overhead, `region::alloc` wastes a page per key, HSM calls add network latency. A rough table of "what each control costs" would help readers make trade-off decisions.

- **Windows coverage.** Part 2 is Linux-only in practice (`prctl`, `RLIMIT_CORE`, `/proc`). The `region` crate is cross-platform, but `harden_process()` is not. What's the Windows equivalent? `SetProcessMitigationPolicy`, `MiniDumpWriteDump` control, job objects?

- **Real-world leak case studies.** Heartbleed (buffer over-read), Cloudbleed (uninitialised memory), the Debian OpenSSL RNG disaster — each maps to a specific row in the part 4 table. Connecting real incidents to the controls would make the series more concrete.
