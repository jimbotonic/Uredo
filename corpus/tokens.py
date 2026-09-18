#!/usr/bin/env python3
"""How much does Uredo save, and on what does that depend?

`run.py` measures the twenty paired programs and reports the figure §37 gates. This asks the
wider question that a single figure invites and cannot answer: the twenty programs were written
for this project, so what is the range across everything that has ever been measured, and what
puts a given piece of code at one end of it or the other?

Every pair below is scored with ONE tokenizer — the round-trip study's — so the numbers are
comparable to each other. Comments and blank lines are excluded on both sides.

A pair is counted **whole**: if the Uredo side ships a hand-written `.rs` module beside the
`.ure`, that module is counted, because the question is what the program cost and not what its
Uredo-written part cost. `run.py` asks the narrower question and excludes the shared module from
both sides, which is why program 19 reads -11.6% there and -3.8% here. Neither is wrong; the gap
between them is this study's whole point.

Usage: python3 corpus/tokens.py [--tsv]
"""
import sys
import pathlib
from collections import Counter

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT / "docs/corpus-study/roundtrip/tools"))
import metrics  # noqa: E402


def tokens(paths):
    out = []
    for p in paths:
        p = pathlib.Path(p)
        lang = "ure" if p.suffix == ".ure" else "rs"
        out += metrics.TOK.findall(metrics.strip_comments(p.read_text(), lang))
    return out


def pairs():
    """Every (name, uredo files, rust files) this repository can measure."""
    found = []
    for d in sorted(ROOT.glob("corpus/[0-9][0-9]_*")):
        u = sorted((d / "src").glob("*.ure")) + sorted((d / "src").glob("*.rs"))
        r = sorted((d / "idiomatic" / "src").glob("*.rs"))
        if u and r:
            found.append(("corpus/" + d.name, u, r))
    for d in sorted(ROOT.glob("docs/corpus-study/roundtrip/p[0-9]_*")):
        u = sorted(d.glob("*.ure"))
        r = d / "original.rs"
        if u and r.exists():
            found.append(("roundtrip/" + d.name, u, [r]))
    demo = ROOT / "examples/restdemo"
    if (demo / "src").is_dir():
        found.append(("restdemo (whole service)", sorted((demo / "src").glob("*.ure")), sorted((demo / "idiomatic/src").glob("*.rs"))))
        edge = ["lib", "router", "wire", "main"]
        found.append(("restdemo (HTTP edge)", [demo / "src" / f"{n}.ure" for n in edge], [demo / "idiomatic/src" / f"{n}.rs" for n in edge]))
        if (demo / "axum-ure").is_dir():
            found.append(("restdemo (axum edge)", [demo / "axum-ure/src/lib.ure", demo / "axum-ure/src/main.ure"], [demo / "axum/src/lib.rs", demo / "axum/src/main.rs"]))
    return found


def main():
    rows = []
    total_u, total_r = Counter(), Counter()
    for name, u, r in pairs():
        tu, tr = tokens(u), tokens(r)
        if not tr:
            continue
        total_u.update(tu)
        total_r.update(tr)
        layout = 100 * sum(1 for t in tr if t in ("{", "}", ";")) / len(tr)
        rows.append((name, len(tr), len(tu), 100 * (len(tr) - len(tu)) / len(tr), layout))
    rows.sort(key=lambda x: -x[3])

    if "--tsv" in sys.argv:
        print("pair\trust_tokens\turedo_tokens\tsaving_pct\tlayout_share_pct")
        for n, r, u, s, l in rows:
            print(f"{n}\t{r}\t{u}\t{s:.1f}\t{l:.1f}")
        return

    print(f"{'pair':<28}{'rust':>7}{'uredo':>7}{'saving':>9}{'{ } ; of rust':>15}")
    for n, r, u, s, l in rows:
        print(f"{n:<28}{r:>7}{u:>7}{s:>8.1f}%{l:>14.1f}%")

    savings = sorted(x[3] for x in rows)
    n = len(savings)
    rt, ut = sum(total_r.values()), sum(total_u.values())
    print(f"\n  {n} pairs.  range {savings[0]:.1f}% to {savings[-1]:.1f}%,  median {savings[n // 2]:.1f}%,"
          f"  token-weighted {100 * (rt - ut) / rt:.1f}%")

    # Pearson correlation of the saving against the Rust twin's brace-and-semicolon share
    xs = [x[4] for x in rows]
    ys = [x[3] for x in rows]
    mx, my = sum(xs) / n, sum(ys) / n
    cov = sum((a - mx) * (b - my) for a, b in zip(xs, ys))
    vx = sum((a - mx) ** 2 for a in xs)
    vy = sum((b - my) ** 2 for b in ys)
    print(f"  saving vs the twin's `{{`/`}}`/`;` share: r = {cov / (vx * vy) ** 0.5:+.2f}")

    print("\n  where the saving comes from, summed over every pair (+ removed by Uredo, - added):")
    delta = {t: total_r[t] - total_u[t] for t in set(total_r) | set(total_u)}
    gap = rt - ut
    for t, d in sorted(delta.items(), key=lambda kv: -abs(kv[1]))[:10]:
        print(f"    {t!r:<12} {d:>+6}   {100 * d / gap:>+6.1f}% of the saving")
    layout_tokens = delta.get("{", 0) + delta.get("}", 0) + delta.get(";", 0)
    print(f"\n    braces and semicolons alone: {layout_tokens} tokens = {100 * layout_tokens / gap:.0f}% of everything removed")


if __name__ == "__main__":
    main()
