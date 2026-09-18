#!/usr/bin/env python3
"""Mechanical checks over the prose, before a reader checks it for us.

`spec_audit.py` does this for the specification. This does it for everything else a newcomer
actually reads — the READMEs, the manual, the decision record — and asks only the questions that
have one right answer derivable from the repository:

  * a sentence that announces a count and is followed by a list must announce the list's length;
  * a numbered list must be numbered 1, 2, 3 …;
  * a relative link must point at a file that exists;
  * the counts the README states about the repository — tests, corpus programs, portfolio topics,
    ecosystem fixtures — must be the counts the repository has.

None of these judge the prose. They exist because four of them were wrong on 2026-09-18, one of
them introduced the previous day by an edit that took a number from a working tally rather than
from the record, and because a reader who finds "167 tests" beside a suite of 186 has been given a
reason to check everything else by hand.

usage: docs/claims_audit.py
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
problems = []


def report(kind, detail):
    problems.append("%-24s %s" % (kind, detail))


NUMBER = {
    "one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6, "seven": 7, "eight": 8,
    "nine": 9, "ten": 10, "eleven": 11, "twelve": 12, "thirteen": 13, "fourteen": 14,
    "fifteen": 15, "sixteen": 16, "seventeen": 17, "eighteen": 18, "nineteen": 19, "twenty": 20,
    "twenty-five": 25, "thirty": 30,
}


def as_number(word):
    w = word.lower().replace(",", "")
    if w.isdigit():
        return int(w)
    return NUMBER.get(w)


def tracked(pattern):
    out = subprocess.run(["git", "ls-files", pattern], cwd=ROOT, capture_output=True, text=True)
    return [f for f in out.stdout.split("\n") if f]


def audited_markdown():
    """Everything a newcomer reads. The specification has its own audit; the corpus study and the
    panel reviews are dated records of what was true when they were written, and are left alone."""
    skip = ("docs/UREDO_LANGUAGE_SPEC", "docs/oracle-reviews/", "docs/corpus-study/")
    return [f for f in tracked("*.md") if not f.startswith(skip)]


# ---- a sentence that announces a count, followed by a list of that many things ----------------

LEAD = re.compile(r"^\*{0,2}([A-Za-z-]+|\d+)\*{0,2} [a-z][^.!?]{0,90}:$")
ITEM = re.compile(r"^\s*(?:[-*] |\d+\. )")
NUMBERED = re.compile(r"^(\d+)\. ")


def count_list_after(lines, i):
    """The number of top-level items in the list that follows line `i`, or None if none does.

    A numbered list may be interrupted by a paragraph and resume — `examples/restdemo/README.md`
    breaks its thirteen between 10 and 11 to say that 4, 5 and 6 were one misunderstanding — and
    that is still one list, so the numbering is what is followed rather than the blank lines. A
    bullet list has no such thread, so prose at the left margin ends it.
    """
    j = i + 1
    while j < len(lines) and not lines[j].strip():
        j += 1
    if j >= len(lines) or not ITEM.match(lines[j]):
        return None
    numbered = NUMBERED.match(lines[j])
    if numbered:
        expect, count = int(numbered.group(1)), 0
        while j < len(lines) and not lines[j].startswith("#"):
            m = NUMBERED.match(lines[j])
            if m and not lines[j].startswith((" ", "\t")):
                if int(m.group(1)) != expect:
                    break
                count += 1
                expect += 1
            j += 1
        return count
    count = 0
    while j < len(lines):
        line = lines[j]
        if line.startswith("#"):
            break
        if not line.strip() or line.startswith((" ", "\t")):
            j += 1
            continue
        if ITEM.match(line):
            count += 1
            j += 1
            continue
        break
    return count


def check_declared_counts(path, lines):
    for i, line in enumerate(lines):
        m = LEAD.match(line.rstrip())
        if not m:
            continue
        declared = as_number(m.group(1))
        if declared is None or declared > 30:
            continue
        actual = count_list_after(lines, i)
        if actual is None or actual == declared:
            continue
        report("count vs its list", "%s:%d says %d, the list has %d — %r" % (path, i + 1, declared, actual, line.strip()[:64]))


def check_numbering(path, lines):
    run, start = [], 0
    for i, line in enumerate(lines + [""]):
        m = NUMBERED.match(line)
        if m:
            if not run:
                start = i
            run.append(int(m.group(1)))
            continue
        if run:
            if run != list(range(run[0], run[0] + len(run))):
                report("numbering", "%s:%d runs %s" % (path, start + 1, run))
            run = []


def check_links(path, lines):
    text = "\n".join(lines)
    for target in re.findall(r"\]\(([^)#\s]+)(?:#[^)\s]*)?\)", text):
        if target.startswith(("http://", "https://", "mailto:")):
            continue
        resolved = os.path.normpath(os.path.join(ROOT, os.path.dirname(path), target))
        if not os.path.exists(resolved):
            report("dangling link", "%s -> %s" % (path, target))


# ---- the counts the README states about the repository ----------------------------------------

def check_repository_counts():
    readme = open(os.path.join(ROOT, "README.md")).read()

    declared = re.search(r"`cargo test` runs ([\d,]+) tests", readme)
    if declared:
        n = 0
        for f in tracked("compiler/src/*.rs") + tracked("compiler/tests/*.rs"):
            n += len(re.findall(r"#\[(?:tokio::)?test\]", open(os.path.join(ROOT, f)).read()))
        if as_number(declared.group(1)) != n:
            report("test count", "README says %s, the sources declare %d" % (declared.group(1), n))

    for pattern, glob_expr, what in (
        (r"\| `corpus/` \| ([A-Za-z]+) paired programs", "corpus/[0-9][0-9]_*", "corpus programs"),
        (r"\| `docs/portfolio/` \| (\d+) topics", "docs/portfolio/[0-9]*", "portfolio topics"),
    ):
        m = re.search(pattern, readme)
        if not m:
            report("claim missing", "README no longer states the %s" % what)
            continue
        actual = len({f.split("/")[1] if what == "corpus programs" else f.split("/")[2] for f in tracked(glob_expr)})
        if as_number(m.group(1)) != actual:
            report(what, "README says %s, there are %d" % (m.group(1), actual))


# ---- a count asserted about a list that lives in another file ---------------------------------

CROSS = re.compile(r"([A-Za-z-]+) more from\s+writing `([^`]+)`")


def check_cross_document_counts(path, lines):
    """The manual's §17 says how many mistakes another document records. It said sixteen when that
    document recorded thirteen, and nothing here could have known: the number and the list it
    counts are in different files, which is precisely why this one drifted."""
    for m in CROSS.finditer("\n".join(lines)):
        declared, where = as_number(m.group(1)), m.group(2)
        if declared is None:
            continue
        other = os.path.join(ROOT, where, "README.md")
        if not os.path.exists(other):
            report("cross-document claim", "%s points at %s, which has no README" % (path, where))
            continue
        target = open(other, encoding="utf-8").read().split("\n")
        found = None
        for i, line in enumerate(target):
            if not LEAD.match(line.rstrip()):
                continue
            n = count_list_after(target, i)
            if n and NUMBERED.match(target[i + 1] if i + 1 < len(target) else ""):
                found = n
                break
            if n and any(NUMBERED.match(t) for t in target[i + 1:i + 4]):
                found = n
                break
        if found is None:
            report("cross-document claim", "%s says %d from %s, which has no numbered list to count" % (path, declared, where))
        elif found != declared:
            report("cross-document claim", "%s says %s (%d) from %s, whose list has %d" % (path, m.group(1), declared, where, found))


def main():
    for path in audited_markdown():
        lines = open(os.path.join(ROOT, path), encoding="utf-8").read().split("\n")
        check_declared_counts(path, lines)
        check_numbering(path, lines)
        check_links(path, lines)
        check_cross_document_counts(path, lines)
    check_repository_counts()

    if problems:
        print("\n".join(sorted(set(problems))))
        print("\n%d problem(s)" % len(set(problems)))
        return 1
    print("no problems found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
