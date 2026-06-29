#!/usr/bin/env python3
"""Scan a core dump file for the 32-byte DEADBEEF pattern."""
import sys

if len(sys.argv) < 2:
    sys.exit("usage: scan_core.py <core-file>")

pattern = b"\xde\xad\xbe\xef" * 8  # 32 bytes
data = open(sys.argv[1], "rb").read()
count = 0
pos = 0

while True:
    pos = data.find(pattern, pos)
    if pos == -1:
        break
    print(f"  hit at offset 0x{pos:x}")
    count += 1
    pos += len(pattern)

print(f"\nFound {count} occurrence(s) of the 32-byte DEADBEEF pattern")
