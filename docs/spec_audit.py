#!/usr/bin/env python3
"""Mechanical checks over the language specification, before a human reads it.

None of these judge the prose. They ask the questions a reader cannot answer without a script and a
copy of the compiler: does every cross-reference point at a section that exists, does every decision
row cite one, are the rows and the revision history in order, do the examples still lex, and — the
one that matters most — does every section number the *compiler* prints in a diagnostic still exist.

usage: docs/spec_audit.py [spec.md]        (default: the current version)
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SPEC = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "docs", "UREDO_LANGUAGE_SPEC_v0.4.md")
text = open(SPEC).read()
lines = text.split("\n")
problems = []


def report(kind, detail):
    problems.append("%-22s %s" % (kind, detail))


# ---- the sections that exist -------------------------------------------------
sections = set()
for line in lines:
    m = re.match(r"#{2,4} (\d+(?:\.\d+)*)[. ]", line)
    if m:
        sections.add(m.group(1))
    # §38's numbered questions are rows of a table, and are referred to by number like a section
    m = re.match(r"\| (\d+\.\d+) \|", line)
    if m:
        sections.add(m.group(1))
print("sections: %d" % len(sections))

# ---- every reference resolves ------------------------------------------------
refs = {}
for i, line in enumerate(lines, 1):
    for m in re.finditer(r"§(\d+(?:\.\d+)*)", line):
        refs.setdefault(m.group(1), []).append(i)
missing = {r: ls for r, ls in refs.items() if r not in sections}
print("references: %d distinct, %d occurrences" % (len(refs), sum(len(v) for v in refs.values())))
for r, ls in sorted(missing.items()):
    report("dangling reference", "§%s (lines %s)" % (r, ", ".join(str(l) for l in ls[:4])))

# ---- decision rows: ordered, and each cites a section that exists -------------
rows = [(int(m.group(1)), m.group(2), i) for i, line in enumerate(lines, 1)
        for m in [re.match(r"\| D(\d+) \| (.*)$", line)] if m]
print("decision rows: %d" % len(rows))
numbers = [n for n, _, _ in rows]
if numbers != sorted(numbers, reverse=True) and numbers != sorted(numbers):
    out_of_order = [n for a, n in zip(numbers, numbers[1:]) if not (a - 1 == n or a + 1 == n)]
    report("decision rows", "not in order around D%s" % out_of_order[:3])
if len(set(numbers)) != len(numbers):
    report("decision rows", "duplicate numbers")
if numbers and sorted(numbers) != list(range(1, max(numbers) + 1)):
    gaps = sorted(set(range(1, max(numbers) + 1)) - set(numbers))
    report("decision rows", "no row for D%s" % gaps)
for n, body, i in rows:
    cited = re.findall(r"§(\d+(?:\.\d+)*)", body)
    if not cited:
        report("decision row", "D%d cites no section (line %d)" % (n, i))
    for c in cited:
        if c not in sections:
            report("decision row", "D%d cites §%s, which does not exist (line %d)" % (n, c, i))

# ---- the revision history is in one order ------------------------------------
history = [(i, m.group(1)) for i, line in enumerate(lines, 1)
           for m in [re.match(r"- \*\*Revision (\d{4}-\d{2}-\d{2}[a-z]?)", line)] if m]
dates = [d for _, d in history]
print("revision entries: %d" % len(dates))
# The history reads oldest first. Dates must not go backwards, and within one date the lettered
# entries must not either; an entry with no letter is a consolidation, whose place is stated in the
# entry itself, so it is not compared against its neighbours.
days = [d[:10] for d in dates]
if days != sorted(days):
    for (i, a), (_, b) in zip(history, history[1:]):
        if a[:10] > b[:10]:
            report("revision history", "out of order at line %d: %s before %s" % (i, a, b))
            break
lettered = [(i, d) for i, d in history if len(d) > 10]
if [d for _, d in lettered] != sorted(d for _, d in lettered):
    for (i, a), (_, b) in zip(lettered, lettered[1:]):
        if a > b:
            report("revision history", "out of order at line %d: %s before %s" % (i, a, b))
            break

# ---- the compiler's own citations still exist --------------------------------
cited_by_compiler = set()
for root, _dirs, files in os.walk(os.path.join(ROOT, "compiler", "src")):
    for f in files:
        if f.endswith(".rs"):
            for m in re.finditer(r"§(\d+(?:\.\d+)*)", open(os.path.join(root, f)).read()):
                cited_by_compiler.add(m.group(1))
print("sections cited by the compiler: %d" % len(cited_by_compiler))
for c in sorted(cited_by_compiler - sections):
    report("compiler cites", "§%s, which the specification does not have" % c)

# ---- every Uredo example lexes ------------------------------------------------
blocks = re.findall(r"```uredo\n(.*?)```", text, re.S)
binary = os.path.join(ROOT, "compiler", "target", "debug", "uredo")
lexed = compiled = 0
if os.path.exists(binary):
    import tempfile

    for b in blocks:
        with tempfile.NamedTemporaryFile("w", suffix=".ure", delete=False) as fh:
            fh.write(b)
            path = fh.name
        r = subprocess.run([binary, "rust", "--tokens", path], capture_output=True, text=True)
        if r.returncode == 0 and "error" not in r.stderr:
            lexed += 1
        else:
            report("example does not lex", (b.strip().split("\n") or [""])[0][:60])
        if subprocess.run([binary, "rust", path], capture_output=True).returncode == 0:
            compiled += 1
        os.unlink(path)
print("examples: %d, lexed %d, lowered to Rust that parses %d" % (len(blocks), lexed, compiled))
print("  (lowering is not type-checking: `compiler/tests/manual.rs` runs rustc over the manual's examples;\n   these are only lowered, which is what `uredo rust` proves)")

print()
if problems:
    print("%d problem(s):" % len(problems))
    for p in problems:
        print("  " + p)
else:
    print("no problems found")
sys.exit(1 if problems else 0)
