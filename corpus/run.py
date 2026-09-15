#!/usr/bin/env python3
"""The §43 corpus runner: for every program, run the Uredo version and the idiomatic Rust twin,
compare their output, and compute the §37 metrics; write RESULTS.md and results.json.

usage: python3 corpus/run.py   (from the repository root; needs the compiler built in
compiler/target/debug and the analyzer tools in docs/corpus-study/analyzer/target/release)
"""
import json, os, re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORPUS = ROOT / "corpus"
UREDO = ROOT / "compiler/target/debug/uredo"
METRICS = ROOT / "docs/corpus-study/roundtrip/tools/metrics.py"
PARAMSHAPE = ROOT / "docs/corpus-study/analyzer/target/release/paramshape"
ENV = dict(os.environ, CARGO_TARGET_DIR=str(CORPUS / "target"))

THRESHOLDS = {"tokens": 10.0, "annotation_ratio": 0.5, "hit_rate": 90.0, "mapped": 80.0}


def run(cmd, cwd=None, timeout=600):
    p = subprocess.run(cmd, cwd=cwd, env=ENV, capture_output=True, text=True, timeout=timeout)
    return p.returncode, p.stdout, p.stderr


def metrics(path):
    rc, out, err = run([sys.executable, str(METRICS), str(path)])
    rows = out.strip().splitlines()
    head = rows[0].split("\t")
    vals = rows[1].split("\t")
    m = dict(zip(head, vals))
    return {k: int(m[k]) for k in ("lines", "nonws_chars", "tokens", "annotations", "fns")}


def shapes(path):
    rc, out, err = run([str(PARAMSHAPE), str(path)])
    res = {}
    for line in out.splitlines():
        fn, idx, shape, generic = line.split("\t")
        res[(fn, int(idx))] = (shape, generic == "1")
    return res


def signature_annotations(text, lang):
    """§37's *parameter* annotations: `take`, `inout`, `&`, `var`/`mut` and lifetime ticks inside
    function signatures (parameter lists and return types), excluding receivers' own spelling."""
    n = 0
    for m in re.finditer(r"\bfn\s+[A-Za-z_][A-Za-z0-9_]*\s*(<[^>]*>)?\s*\(", text):
        start = m.end() - 1
        depth = 0
        i = start
        while i < len(text):
            if text[i] == "(":
                depth += 1
            elif text[i] == ")":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        params = text[start + 1:i]
        rest = text[i + 1:]
        end = rest.find("{") if lang == "rs" else rest.find(":")
        nl = rest.find("\n")
        cut = min(x for x in [end if end >= 0 else 10**9, nl if nl >= 0 else 10**9])
        ret = rest[:cut]
        sig = params + " " + ret
        n += len(re.findall(r"&", sig))
        n += len(re.findall(r"'[a-z_]+\b", sig))
        if lang == "rs":
            n += len(re.findall(r"\bmut\b", sig))
        else:
            n += len(re.findall(r"\b(take|inout|var)\b", sig))
    return n


def raw_rust_share(ure_text, extra_rs_files):
    """Lines inside `rust { }` blocks plus lines of hand-written .rs modules, over all source lines."""
    lines = ure_text.split("\n")
    total = sum(1 for l in lines if l.strip() and not l.strip().startswith("#"))
    raw = 0
    depth = 0
    for l in lines:
        s = l.strip()
        if depth == 0 and re.search(r"\brust\s*\{", s):
            depth = s.count("{") - s.count("}")
            raw += 1
            continue
        if depth > 0:
            raw += 1
            depth += s.count("{") - s.count("}")
    for f in extra_rs_files:
        n = sum(1 for l in f.read_text().split("\n") if l.strip() and not l.strip().startswith("//"))
        raw += n
        total += n
    return raw, total


def diagnostics_mapping():
    """Mapped share of barrier diagnostics on the negative fixtures (§27)."""
    total = 0
    mapped = 0
    for fx in ["moved", "mismatch", "moved_generic"]:
        d = ROOT / "compiler/tests/fixtures" / fx
        rc, out, err = run([str(UREDO), "check", str(d)])
        text = re.sub(r"\x1b\[[0-9;]*m", "", err)
        for m in re.finditer(r"^error\[E\d+\]:.*\n  --> (\S+)", text, re.M):
            total += 1
            if m.group(1).endswith(".ure") or ".ure:" in m.group(1):
                mapped += 1
    return mapped, total


def main():
    results = []
    programs = sorted(p for p in CORPUS.iterdir() if p.is_dir() and p.name[:2].isdigit())
    for prog in programs:
        name = prog.name
        # No wall-clock time is recorded. It was, and because it changes on every run `results.json`
        # showed a diff whenever it was regenerated — so it kept being reverted, and drifted away
        # from `RESULTS.md`, the other half of the same record, by a whole correction (2026-09-13).
        # Nothing read the field. §36 is where timing lives.
        rc_u, out_u, err_u = run([str(UREDO), "run", str(prog)])
        rc_r, out_r, err_r = run(["cargo", "run", "-q"], cwd=prog / "idiomatic")
        same_output = (rc_u == 0 and rc_r == 0 and out_u == out_r)
        ure = prog / "src/main.ure"
        gen = prog / "target/uredo/src/main.rs"
        idi = prog / "idiomatic/src/main.rs"
        mu = metrics(ure)
        mr = metrics(idi)
        mg = metrics(gen) if gen.exists() else None
        # inference hit rate: non-generic parameters whose generated shape equals the idiomatic one
        sg = shapes(gen) if gen.exists() else {}
        si = shapes(idi)
        hits = 0
        total = 0
        misses = []
        for key, (shape_i, generic) in si.items():
            if generic or key not in sg:
                continue
            total += 1
            if sg[key][0] == shape_i:
                hits += 1
            else:
                misses.append(f"{key[0]}#{key[1]}: uredo {sg[key][0]} vs rust {shape_i}")
        sig_u = signature_annotations(ure.read_text(), "ure")
        sig_r = signature_annotations(idi.read_text(), "rs")
        extra_rs = [f for f in (prog / "src").glob("*.rs")]
        raw, total_lines = raw_rust_share(ure.read_text(), extra_rs)
        results.append({
            "program": name, "same_output": same_output, "uredo_rc": rc_u, "rust_rc": rc_r,
            "ure": mu, "rust": mr, "generated": mg,
            "token_reduction_pct": round(100 * (mr["tokens"] - mu["tokens"]) / mr["tokens"], 1),
            "char_reduction_pct": round(100 * (mr["nonws_chars"] - mu["nonws_chars"]) / mr["nonws_chars"], 1),
            "line_reduction_pct": round(100 * (mr["lines"] - mu["lines"]) / mr["lines"], 1),
            "annotations_ure": mu["annotations"], "annotations_rust": mr["annotations"],
            "sig_annotations_ure": sig_u, "sig_annotations_rust": sig_r,
            "hit": hits, "params": total, "misses": misses,
            "raw_rust_lines": raw, "source_lines": total_lines,
            "stderr_head": "" if same_output else (err_u + err_r)[:400],
        })
        status = "ok" if same_output else "MISMATCH"
        print(f"{name:20s} {status:9s} tokens {mu['tokens']:4d} vs {mr['tokens']:4d} ({results[-1]['token_reduction_pct']:+.1f}%)  annot {mu['annotations']:3d} vs {mr['annotations']:3d}  hit {hits}/{total}")
    mapped, dtotal = diagnostics_mapping()

    # totals
    tu = sum(r["ure"]["tokens"] for r in results)
    tr = sum(r["rust"]["tokens"] for r in results)
    cu = sum(r["ure"]["nonws_chars"] for r in results)
    cr = sum(r["rust"]["nonws_chars"] for r in results)
    lu = sum(r["ure"]["lines"] for r in results)
    lr = sum(r["rust"]["lines"] for r in results)
    au = sum(r["annotations_ure"] for r in results)
    ar = sum(r["annotations_rust"] for r in results)
    su = sum(r["sig_annotations_ure"] for r in results)
    sr = sum(r["sig_annotations_rust"] for r in results)
    hits = sum(r["hit"] for r in results)
    params = sum(r["params"] for r in results)
    raw = sum(r["raw_rust_lines"] for r in results)
    src = sum(r["source_lines"] for r in results)
    app = [r for r in results if int(r["program"][:2]) <= 15]  # application-style: 1–15
    tua = sum(r["ure"]["tokens"] for r in app)
    tra = sum(r["rust"]["tokens"] for r in app)
    summary = {
        "programs": len(results), "all_same_output": all(r["same_output"] for r in results),
        "tokens_ure": tu, "tokens_rust": tr, "token_reduction_pct": round(100 * (tr - tu) / tr, 1),
        "token_reduction_app_pct": round(100 * (tra - tua) / tra, 1),
        "chars_ure": cu, "chars_rust": cr, "char_reduction_pct": round(100 * (cr - cu) / cr, 1),
        "lines_ure": lu, "lines_rust": lr, "line_reduction_pct": round(100 * (lr - lu) / lr, 1),
        "annotations_ure": au, "annotations_rust": ar, "annotation_ratio": round(au / ar, 3),
        "sig_annotations_ure": su, "sig_annotations_rust": sr, "sig_annotation_ratio": round(su / sr, 3) if sr else None,
        "hit": hits, "params": params, "hit_rate_pct": round(100 * hits / params, 1) if params else None,
        "diagnostics_mapped": mapped, "diagnostics_total": dtotal, "mapped_pct": round(100 * mapped / dtotal, 1) if dtotal else None,
        "raw_rust_lines": raw, "source_lines": src, "raw_rust_share_pct": round(100 * raw / src, 1),
    }
    checks = {
        "token reduction ≥ 10% on application-style programs (1–15)": summary["token_reduction_app_pct"] >= THRESHOLDS["tokens"],
        "parameter annotations ≤ ½ of idiomatic Rust's (signatures)": (summary["sig_annotation_ratio"] or 1) <= THRESHOLDS["annotation_ratio"],
        "inference hit rate ≥ 90% on non-generic parameters": (summary["hit_rate_pct"] or 0) >= THRESHOLDS["hit_rate"],
        "≥ 80% of barrier diagnostics mapped": (summary["mapped_pct"] or 0) >= THRESHOLDS["mapped"],
    }
    (CORPUS / "results.json").write_text(json.dumps({"summary": summary, "checks": checks, "programs": results}, indent=2))

    md = ["# §43 corpus — results (generated by `corpus/run.py`)", "",
          f"{len(results)} programs; every Uredo program and its idiomatic Rust twin produce identical output: **{summary['all_same_output']}**.",
          "", "| # | program | output | tokens ure/rust | Δtok | Δchars | Δlines | signature annotations ure/rust | all annotations ure/rust | hit rate | raw Rust |", "|---|---|---|---|---|---|---|---|---|---|---|"]
    for r in results:
        md.append(f"| {r['program'][:2]} | {r['program'][3:]} | {'same' if r['same_output'] else 'DIFFERENT'} | {r['ure']['tokens']}/{r['rust']['tokens']} | {r['token_reduction_pct']:+.1f}% | {r['char_reduction_pct']:+.1f}% | {r['line_reduction_pct']:+.1f}% | {r['sig_annotations_ure']}/{r['sig_annotations_rust']} | {r['annotations_ure']}/{r['annotations_rust']} | {r['hit']}/{r['params']} | {r['raw_rust_lines']}/{r['source_lines']} |")
    md += ["", f"**Totals.** tokens {tu} vs {tr} ({summary['token_reduction_pct']:+.1f}%; application-style 1–15: {summary['token_reduction_app_pct']:+.1f}%), non-whitespace characters {cu} vs {cr} ({summary['char_reduction_pct']:+.1f}%), lines {lu} vs {lr} ({summary['line_reduction_pct']:+.1f}%), signature annotations {su} vs {sr} (ratio {summary['sig_annotation_ratio']}; all annotations incl. bodies {au} vs {ar}, ratio {summary['annotation_ratio']}), inference hit rate {hits}/{params} = {summary['hit_rate_pct']}% on non-generic parameters, barrier diagnostics mapped {mapped}/{dtotal} = {summary['mapped_pct']}% (fixtures), raw-Rust share {raw}/{src} lines = {summary['raw_rust_share_pct']}%.", "",
           "## §37 thresholds", "", "| Threshold | Result |", "|---|---|"]
    for k, v in checks.items():
        md.append(f"| {k} | {'**pass**' if v else '**fail**'} |")
    misses = [(r["program"], m) for r in results for m in r["misses"]]
    md += ["", "## Inference misses", ""]
    if misses:
        for p, m in misses:
            md.append(f"- {p}: {m}")
    else:
        md.append("none")
    md += ["", "Method: one tokenizer for both versions (`docs/corpus-study/roundtrip/tools/metrics.py`; comments and tests excluded). Signature annotations (the §37 threshold) count `take`, `inout`, `&`, `var` and lifetime ticks in parameter lists and return types, against `&`, `mut` and lifetime ticks in the Rust twin's signatures; all-annotations additionally counts bodies, where Uredo keeps Rust's `&`/`*` for Rust items and `var` for `mut`. Inference hit rate: for every non-generic parameter of the idiomatic twin, the generated Rust's parameter shape (value / `&` / `&mut`) must equal it (`paramshape`, syn-based). Raw-Rust share: lines inside `rust { }` blocks plus hand-written `.rs` modules over all non-comment source lines. Diagnostics mapping: rustc errors on the compiler's negative fixtures whose primary span was translated to a `.ure` location.", ""]
    (CORPUS / "RESULTS.md").write_text("\n".join(md))
    print("\n".join(md[-12:]))
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()
