#!/usr/bin/env python3
"""Per-function instruction counts, opcode digests, alloca bytes and aliases in one LLVM IR file.

`ircmp.py` answers "are these two modules the same", and fails if any function differs; this answers
"how big is each function, what is its opcode sequence, and is it an alias of another one". The §36
tail-`?` obligation needs both halves at once in one module pair: an equal opcode sequence (or an
alias) for the same-type tail, and *no more* instructions than the baseline for the converting tail.
Symbol names are reduced to their last path segment, length-prefixed as the v0 mangling writes them,
so two crates of the same name can be compared.

usage: irstat.py <file.ll> [name …]      (default: every defined function)
output: `name<TAB>instructions<TAB>alloca_bytes<TAB>opcode_digest` per function, then
        `alias <name> -> <name>` for each alias
"""
import hashlib
import re
import sys


def last_segment(sym):
    """The last identifier of a mangled symbol, parsed by its length prefixes."""
    sym = sym.strip('"')
    out, i = [], 0
    while i < len(sym):
        if sym[i].isdigit():
            j = i
            while j < len(sym) and sym[j].isdigit():
                j += 1
            n = int(sym[i:j])
            if j + n <= len(sym):
                out.append(sym[j:j + n])
                i = j + n
                continue
        i += 1
    for part in reversed(out):
        if not re.fullmatch(r"h[0-9a-f]{16}", part) and not part.startswith("_"):
            return part
    return sym


def main():
    path = sys.argv[1]
    wanted = set(sys.argv[2:])
    name, ops, allocs = None, [], []
    rows, aliases = [], []
    for line in open(path):
        m = re.match(r'define .*?@("?[^("]+"?)\(', line)
        if m:
            name, ops, allocs = last_segment(m.group(1)), [], []
            continue
        m = re.match(r'@("?[^"= ]+"?) = .*\balias\b.*@("?[^"()\s,]+"?)', line)
        if m:
            aliases.append((last_segment(m.group(1)), last_segment(m.group(2))))
            continue
        if name is None:
            continue
        if line.startswith("}"):
            rows.append((name, len(ops), sum(allocs), hashlib.sha1(",".join(ops).encode()).hexdigest()[:12]))
            name = None
            continue
        s = line.split(";", 1)[0].strip()
        if not s or re.match(r'^[A-Za-z0-9_.$"]+:$', s):
            continue
        m = re.match(r"(?:%[^ ]+ = )?(\w+)", s)
        op = m.group(1) if m else s.split()[0]
        ops.append(op)
        if op == "alloca":
            am = re.search(r"alloca \[(\d+) x i8\]", s)
            allocs.append(int(am.group(1)) if am else 0)
    for n, count, ab, digest in rows:
        if not wanted or n in wanted:
            print("%s\t%d\t%d\t%s" % (n, count, ab, digest))
    for a, b in aliases:
        if not wanted or a in wanted or b in wanted:
            print("alias %s -> %s" % (a, b))


main()
