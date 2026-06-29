# zeroize-os-hardening — runnable demo

Companion to the blog post
[I Hardened My Secret. Or Did I?](https://ddanilov.me/zeroize-os-hardening).
Demonstrates what each OS-level hardening control actually blocks — and what
still gets through.

**Platform:** Linux only (uses `/proc`, `prctl`, `mlock`, `madvise`,
`RLIMIT_CORE`, Yama `ptrace_scope`).

## Build

```bash
cd examples/zeroize-os-hardening
cargo build --release
```

## Modes

| Command | What it does |
| --- | --- |
| *(none)* | Run both `HardenedKey` and `PageProtectedKey`, print matching checksums |
| `live` | Load key **unhardened**, sleep 30 s for `/proc/pid/mem` scanning |
| `live-hardened` | Load key **hardened** (`RLIMIT_CORE=0`, `PR_SET_DUMPABLE=0`), sleep 30 s |
| `crash` | Load key **unhardened**, abort immediately (core dump demo) |
| `crash-hardened` | Load key **hardened**, abort immediately (core dump demo) |

## One-time system setup

The demos touch two system settings.  Save the originals, change them for the
demo, and restore them when you're done.

### 1. Core dump path

Most distros pipe core dumps to a crash reporter (`apport`, `systemd-coredump`).
For the demo we need core files written to the current directory.

```bash
# save the original
cat /proc/sys/kernel/core_pattern          # e.g. |/usr/share/apport/apport ...
sudo sh -c 'echo "core.%p" > /proc/sys/kernel/core_pattern'
```

Also allow core dumps in the current shell:

```bash
ulimit -c unlimited
```

### 2. Yama ptrace_scope

The default `ptrace_scope=1` already blocks non-parent same-user reads of
`/proc/pid/mem`.  To show the *unhardened* case allowing same-user scanning
(so the contrast with the hardened case is visible), temporarily set it to 0:

```bash
cat /proc/sys/kernel/yama/ptrace_scope     # probably 1
sudo sh -c 'echo 0 > /proc/sys/kernel/yama/ptrace_scope'
```

## Walkthrough

Run every command from this directory (`examples/zeroize-os-hardening`).

### Demo 1 — core dump: unhardened

```bash
cargo run --release -- crash
# → "UNHARDENED: key loaded …, PID <PID>"
# → "Aborted (core dumped)"

python3 scan_core.py core.<PID>
# →   hit at offset 0x…
# → Found 1 occurrence(s) of the 32-byte DEADBEEF pattern

rm core.<PID>                              # clean up
```

The unhardened process crashes, the kernel writes a core file, and the secret
is inside it.

### Demo 2 — core dump: hardened

```bash
cargo run --release -- crash-hardened
# → "HARDENED: key loaded …, PID <PID>"
# → "Aborted"              ← note: no "(core dumped)"

ls core.<PID>
# → ls: cannot access 'core.<PID>': No such file or directory
```

`RLIMIT_CORE=0` + `PR_SET_DUMPABLE=0` prevented the core file entirely.

### Demo 3 — live memory: unhardened, same-user scan

```bash
# Terminal 1
cargo run --release -- live
# → "Process NOT hardened."
# → "PID <PID>"
# → "  VmLck:        4 kB"     ← mlock worked
# → "Sleeping 30 s …"

# Terminal 2 (same user, no sudo)
python3 scan_mem.py <PID>
# →   hit at 0x… in [heap]
# → Found 1 occurrence(s) of the 32-byte DEADBEEF pattern in PID <PID>
```

With `ptrace_scope=0`, any same-user process can read your memory through
`/proc/<pid>/mem`.

### Demo 4 — live memory: hardened, same-user scan

```bash
# Terminal 1
cargo run --release -- live-hardened
# → "Process hardened (RLIMIT_CORE=0, PR_SET_DUMPABLE=0)."
# → "PID <PID>"
# → "  VmLck:        4 kB"
# → "Sleeping 30 s …"

# Terminal 2 (same user, no sudo)
python3 scan_mem.py <PID>
# → Permission denied reading /proc/<PID>/maps
# → (Process is likely non-dumpable — PR_SET_DUMPABLE is off.)
```

`PR_SET_DUMPABLE=0` blocked same-user access.

### Demo 5 — live memory: hardened, root scan

```bash
# against the still-running live-hardened process from Demo 4:
sudo python3 scan_mem.py <PID>
# →   hit at 0x… in [heap]
# → Found 1 occurrence(s) of the 32-byte DEADBEEF pattern in PID <PID>
```

Root / `CAP_SYS_PTRACE` bypasses everything.  That's the honest limit.

### Demo 6 — key gone after drop

Wait for any `live` or `live-hardened` run to print "key DROPPED", then
scan again:

```bash
sudo python3 scan_mem.py <PID>
# → Found 0 occurrence(s) …
```

`Zeroizing` wiped the heap buffer on drop — the secret is gone.

## Restore system settings

```bash
# restore core dump handler (paste your original from step 1)
sudo sh -c 'echo "|/usr/share/apport/apport -p%p -s%s -c%c -d%d -P%P -u%u -g%g -- %E" > /proc/sys/kernel/core_pattern'

# restore ptrace scope
sudo sh -c 'echo 1 > /proc/sys/kernel/yama/ptrace_scope'
```

Adjust the `core_pattern` value to whatever your system had originally (the
`cat` output you saved earlier).
