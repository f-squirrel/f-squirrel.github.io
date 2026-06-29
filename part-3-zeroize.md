# Improvement ideas for the zeroize series

## Series structure

1. **Part 1** — Wipe it (`zeroize`): move hazards, stack vs heap, register spills, Vec reallocation
2. **Part 2** — Stop the OS from copying it: mlock, dumps, ptrace, page-isolated variant
3. **Part 3** (new) — Keep it out of your process: privilege separation + Linux kernel keyring. Demonstrable on any Linux box, no special hardware.
4. **Part 4** (existing draft in `_posts/part-3-architecture.md`) — Keep it out of your machine: HSMs, KMS, MPC, TEEs

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

Core idea: two ways to keep the key out of your process without special hardware.

### Privilege separation

- The OpenSSH model: fork a minimal privileged child that holds the key and exposes only "sign this" / "decrypt this" over IPC. The unprivileged parent handles networking, parsing, and all the dependency-heavy work.
- Other examples: qmail (split into ~8 processes), Postfix, Chrome (renderer vs broker).
- Strongest defence against supply-chain attacks without hardware — a compromised crate owns the unprivileged process but can't read the key because it's in a different address space.
- Demo idea: a two-process Rust example — parent loads key, forks, parent drops the key, child holds it and serves signing requests over a Unix socket or pipe. Show `scan_mem.py` finding nothing in the parent.
- Compose with part 2 controls: the privileged child should also `harden_process()`, `mlock` the key, and optionally use `seccomp` to drop unnecessary syscalls after init.
- `seccomp` after init: once the privileged child has loaded the key and set up crypto, lock down the syscall surface. A compromised dependency can't call `ptrace`, `mprotect`, or `process_vm_readv` if seccomp blocks them. Relevant crate: `seccompiler` (from Firecracker).
- Capability dropping: `prctl(PR_SET_NO_NEW_PRIVS, 1)` + drop `CAP_SYS_PTRACE` after startup. Cheap, composable.
- The cost: IPC overhead, two binaries (or fork-based), protocol design. The privileged side must validate every request.

### Linux kernel keyring

- Keys live in kernel memory, not process address space. Process holds a handle (serial ID), not the bytes.
- `linux-keyutils` crate (v0.2.5) wraps `add_key` / `keyctl` / `request_key`.
- A compromised dependency can't pointer-walk to the key — must call `keyctl(KEYCTL_READ)`, which the kernel checks against the key's permission mask.
- Demo idea: store the DEADBEEF pattern in the session keyring, show `scan_mem.py` finding nothing in process memory. Read it back via the handle, use it, then `key.invalidate()`.
- Limitations: the key *is* readable via `keyctl(KEYCTL_READ)` if the process has `KEY_POS_READ` — so a compromised dependency in the same process can still call the syscall. Kernel keyring protects against accidental leaks and pointer-based reads, not against deliberate `keyctl` calls from in-process code. Privsep composes with it: the unprivileged process never gets the key serial.
- Man page: `keyrings(7)`, `keyctl(2)`, `add_key(2)`.

### Closing for part 3

- Tie back: part 2's `PageProtectedKey` protects the page but any in-process code can unseal it. Privsep + keyring move the key across a process boundary where page-table tricks are no longer the defence — the kernel's process isolation is.
- Tease part 4: "For the keys you truly can't afford to lose, even a separate process on the same machine may not be enough — HSMs, MPC, and enclaves arrange for the key to never exist on your host at all."

## Part 4 (existing draft `_posts/part-3-architecture.md`) — issues to fix

- **Rename file** to match the new numbering (part 4, not part 3).
- **Relative links are broken.** `./part-1-zeroize.md` and `./part-2-os-hardening.md` won't resolve in Jekyll — use permalinks (`/zeroize`, `/zeroize-os-hardening`).
- **Missing Jekyll front matter.** No `title:`, `permalink:`, `tags:`, `published:` etc.
- **Table separators violate MD060.** `|---|---|---|` should be `| --- | --- | --- |`.
- **Add a row for privsep and kernel keyring** to the "which layer stops what" table, now that part 3 covers them.

## Ideas that could go in any post or a future addendum

- **Encrypted-at-rest in RAM.** Keep the key AES-wrapped in process memory; unwrap into a local variable only for the operation, then zeroize the local. Reduces the window where plaintext exists. Some crypto libraries (libsodium's `crypto_secretbox`) support this pattern natively.

- **Testing your hardening.** How to verify each control actually works: check `VmLck` in `/proc/self/status`, try `ptrace` attach from another process, trigger a crash and confirm no core file, scan `/proc/pid/maps` for expected protection flags. The demo modes in the example do some of this, but a "hardening self-test" checklist would be useful standalone.

- **Cost/benchmark of each control.** `mlock` has a budget (`RLIMIT_MEMLOCK`), `mprotect` flipping has syscall overhead, `region::alloc` wastes a page per key, HSM calls add network latency. A rough table of "what each control costs" would help readers make trade-off decisions.

- **Windows coverage.** Part 2 is Linux-only in practice (`prctl`, `RLIMIT_CORE`, `/proc`). The `region` crate is cross-platform, but `harden_process()` is not. What's the Windows equivalent? `SetProcessMitigationPolicy`, `MiniDumpWriteDump` control, job objects?

- **Real-world leak case studies.** Heartbleed (buffer over-read), Cloudbleed (uninitialised memory), the Debian OpenSSL RNG disaster — each maps to a specific row in the part 4 table. Connecting real incidents to the controls would make the series more concrete.
