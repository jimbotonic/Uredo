#!/usr/bin/env python3
"""Score the `@value use` question (§38) on concrete parameters only.

The first pass at this classified parameter types textually, so `impl Into<String>`, `F` and `Self`
appeared as concrete by-value types although a type parameter's mode is already settled (P8, D52).
This pass reads the syn records from `valparams` — which mark a parameter whose type mentions a
type parameter, `impl Trait` or `dyn Trait` — and counts only the rest, using the same known-Copy
predicate as `optionrule.py` (a mirror of `compiler/src/lower.rs::is_known_copy`).

usage: valueuse.py <corpus-dir> <valparams.jsonl>
"""
import collections, json, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from knowncopy import copy_types_per_repo, known_copy, last_segment, split_top


def main():
    corpus, records = sys.argv[1], sys.argv[2]
    copy_local = copy_types_per_repo(corpus)
    shapes = collections.Counter()
    concrete = collections.Counter()
    need = collections.Counter()
    for line in open(records):
        r = json.loads(line)
        shapes[(r["shape"], r["generic"])] += 1
        if r["generic"] or r["shape"] != "value":
            continue
        ty = r["type"]
        if known_copy(ty, copy_local.get(r["repo"], set())):
            concrete["known-Copy already by value in Lumo"] += 1
        else:
            concrete["would need `take` or `@value use`"] += 1
            need[last_segment(ty)] += 1
    print("all parameters, by shape (generic = the type mentions a type parameter, impl or dyn):")
    for (shape, gen), n in sorted(shapes.items()):
        print("  %-7s generic=%-5s %6d" % (shape, gen, n))
    tot = sum(n for (s, g), n in shapes.items() if s == "value" and not g)
    print("\nconcrete by-value parameters (the only ones `@value use` could affect): %d" % tot)
    for k, v in concrete.most_common():
        print("  %-38s %6d  (%.0f%%)" % (k, v, 100 * v / tot))
    print("\ntop concrete non-Copy types passed by value:")
    for k, v in need.most_common(20):
        print("  %5d  %s" % (v, k))


main()
