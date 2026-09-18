#!/usr/bin/env python3
"""Score the candidate `T?` passing-mode rules (§38) against idiomatic Rust signatures.

Reads the syn-based per-parameter records (`genparams.jsonl`, produced by `genparams`) and the
corpus sources, and classifies each `Option`-outer parameter under each candidate rule. The
known-Copy predicate here mirrors `compiler/src/lower.rs::is_known_copy` — scalars, shared
references, tuples, fixed arrays, the prelude table with its generic-wrapper rule, the project's
own `Copy` derives, and (for rule C) the `Option`/`Result` payload recursion — so the hit rates
are computed against what the compiler actually decides, not an approximation of it.

usage: optionrule.py <corpus-dir> <genparams.jsonl>
"""
import collections, json, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from knowncopy import copy_types_per_repo, known_copy, last_segment, split_top


def main():
    corpus, records = sys.argv[1], sys.argv[2]
    copy_local = copy_types_per_repo(corpus)
    print("types deriving Copy, per repo:",
          {r: len(v) for r, v in sorted(copy_local.items())})

    pop = []           # (repo, shape, type text, payload text)
    for line in open(records):
        r = json.loads(line)
        shape = r["shape"]
        if shape not in ("Option<X>", "&Option<X>", "Option<T>", "Option<&X>", "Option<&mut X>"):
            continue
        ty = r["type"]
        if shape.startswith("&mut"):
            continue
        inner = ty[ty.index("<") + 1:ty.rindex(">")] if "<" in ty else ""
        if ty.startswith("&"):
            inner = inner
        tp_copy = shape == "Option<T>" and "Copy" in r["bounds"]
        pop.append((r["repo"], shape, ty, inner, shape == "Option<T>", tp_copy))

    shapes = collections.Counter()
    for line in open(records):
        r = json.loads(line)
        if r["shape"].replace("&mut ", "").startswith(("Option", "&Option")) or r["shape"] in ("Option<&X>", "Option<&mut X>"):
            shapes[r["shape"]] += 1
    print("\nOption-ish parameters as idiomatic Rust writes them:", sum(shapes.values()))
    for shape, k in shapes.most_common():
        print("  %-16s %4d" % (shape, k))
    print("  (`Option<&X>` is written `&T?` in Uredo; `Option<&mut X>` needs `take &mut T?`,"
          " since a bare `&mut T?` parameter is rejected under D54)")

    hits = collections.Counter()
    narrow = collections.Counter()
    misses = collections.Counter()
    per_repo = collections.defaultdict(lambda: collections.Counter())
    for repo, shape, ty, inner, is_tp, tp_copy in pop:
        rust_by_value = not ty.startswith("&")
        local = copy_local.get(repo, set())
        c_by_value = known_copy("Option<%s>" % inner, local, True)
        # the adopted rule (D54): as C, and a type parameter whose bounds include `Copy` is
        # itself known-Copy, so `T?` with `T: Copy` passes by value
        c_bound = c_by_value or tp_copy
        c_tp = c_by_value or is_tp      # rejected variant: any type-parameter payload by value (P8)
        for rule, pred in (("A", False), ("B", True), ("C", c_by_value), ("D54", c_bound), ("C+tp", c_tp)):
            if pred == rust_by_value:
                hits[rule] += 1
                if shape in ("Option<X>", "&Option<X>", "Option<T>"):
                    narrow[rule] += 1
            elif rule == "C":
                misses[last_segment(inner) or inner] += 1
        per_repo[repo]["n"] += 1
        per_repo[repo]["byval"] += rust_by_value
        per_repo[repo]["c"] += (c_by_value == rust_by_value)

    n = len(pop)
    nn = sum(1 for r in pop if r[1] in ("Option<X>", "&Option<X>", "Option<T>"))
    print("\npopulation: every Option-outer parameter except `&mut Option` (which is `inout`):", n)
    print("  of which the container-mode question (Option<X> / &Option<X> / Option<T>):", nn)
    for rule, label in (("A", "always borrow (status quo)          "),
                        ("B", "always by value                     "),
                        ("C", "by value iff payload known-Copy     "),
                        ("D54", "as C, plus a `Copy`-bounded payload  "),
                        ("C+tp", "as C, type-parameter payload by value")):
        print("  rule %-4s %s  all %4d  %5.1f%%   |  container question only %4d  %5.1f%%"
              % (rule, label, hits[rule], 100 * hits[rule] / n, narrow[rule], 100 * narrow[rule] / nn))
    print("\nrule C's misses, by payload:")
    for k, v in misses.most_common(14):
        print("  %4d  %s" % (v, k))
    print("\nthe parameters idiomatic Rust borrows as `&Option<X>`, in full:")
    for line in open(records):
        r = json.loads(line)
        if r["shape"] == "&Option<X>":
            print("  %-10s %-24s %s" % (r["repo"], r["fn"], r["type"]))

    print("\nper repo: n / Rust passes by value / rule C agrees")
    for repo in sorted(per_repo):
        c = per_repo[repo]
        print("  %-12s %4d / %4d / %4d  (%.0f%%)" % (repo, c["n"], c["byval"], c["c"], 100 * c["c"] / c["n"]))


main()
