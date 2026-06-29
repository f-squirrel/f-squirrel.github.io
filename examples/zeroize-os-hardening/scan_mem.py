#!/usr/bin/env python3
"""Scan a process's readable memory for a 32-byte DEADBEEF pattern."""
import sys

pid = sys.argv[1] if len(sys.argv) > 1 else sys.exit("usage: scan_mem.py <PID>")
pattern = b"\xde\xad\xbe\xef" * 8  # 32 bytes
count = 0

try:
    maps = open(f"/proc/{pid}/maps")
except PermissionError:
    print(f"Permission denied reading /proc/{pid}/maps")
    print("(Process is likely non-dumpable — PR_SET_DUMPABLE is off.)")
    sys.exit(1)

for line in maps:
    parts = line.split()
    perms = parts[1]
    if "r" not in perms:
        continue

    addr_range = parts[0]
    start_s, end_s = addr_range.split("-")
    start = int(start_s, 16)
    end = int(end_s, 16)

    try:
        with open(f"/proc/{pid}/mem", "rb") as mem:
            mem.seek(start)
            data = mem.read(end - start)
    except (OSError, ValueError):
        continue

    # count non-overlapping occurrences
    pos = 0
    while True:
        pos = data.find(pattern, pos)
        if pos == -1:
            break
        region = parts[-1] if len(parts) > 5 else "(anonymous)"
        offset_hex = hex(start + pos)
        print(f"  hit at {offset_hex} in {region}")
        count += 1
        pos += len(pattern)

print(f"\nFound {count} occurrence(s) of the 32-byte DEADBEEF pattern in PID {pid}")
