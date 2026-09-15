# `tools/` — the toolchain's own tools, written in Uredo

This package exists twice over. It does real work — these are the scripts the repository runs on
every change — and it is the first Uredo code written to *work* rather than to illustrate a rule.
Everything else in the repository was built to demonstrate the language: the twenty corpus programs
have idiomatic Rust twins to be compared against, the 25 portfolio topics are one per language
area, and the compatibility fixtures are one per ecosystem crate. None of them had to survive
someone changing their mind halfway through a function.

That matters because every open measurement in the specification ends with the same sentence: no
Uredo project of the size needed exists. §37's ergonomic evaluation, §38's two conditions for
reversing the by-value rule (D52), and §29.1's trigger for a source-level lint suppression are all
waiting for code like this.

| Tool | Ports | Status |
|---|---|---|
| `src/bin/spec-audit.ure` | `docs/spec_audit.py` | byte-identical output on the clean document and on a broken one, exit codes included |
| `src/bin/compat-record.ure` | `docs/compat/record.py` | byte-identical `COMPATIBILITY.md`, and the same line on stdout |
| `src/bin/corpus-run.ure` | `corpus/run.py` | byte-identical `RESULTS.md`; `results.json` equal field for field once the wall-clock timings are set aside |
| `src/bin/metrics.ure` | `docs/corpus-study/roundtrip/tools/metrics.py` | byte-identical over **every one of the 174 `.ure` and `.rs` files in the repository**, the compiler's own sources included |
| `src/bin/ir-compare.ure` | `docs/corpus-study/roundtrip/tools/ircmp.py` | identical output and exit code on four real IR module pairs and on a module against itself |
| `src/bin/ir-stat.ure` | `docs/corpus-study/roundtrip/tools/irstat.py` | identical output on eight IR modules, with and without a name filter |
| `src/pieces.ure` (library) | `docs/corpus-study/roundtrip/tools/pieces.py` | byte-identical reconstructed trees and the same twelve-line listing |
| `src/bin/difftest.ure` | `docs/corpus-study/roundtrip/tools/difftest.py` | identical generated manifest and identical output; 200,000 inputs per piece agree, exit code included |
| `src/bin/perf.ure` | `corpus/perf.py` | byte-identical `PERF.md`, `perf.json` **and** `size_baseline.json` — including Python's one-space JSON indent and its `\u00a7` escaping |
| `src/bin/runtime.ure` | `docs/corpus-study/roundtrip/tools/runtime.py` | byte-identical `RUNTIME.md` on the deterministic `--report-only` path; the measuring path cannot be byte-compared (see below) |
| `src/bin/status-check.ure` | — | new: recomputes every measured figure §0.1 publishes, from the artefacts, and fails when two records of one measurement disagree |

The Python originals stay. They are the reference the port is diffed against, and a port that
cannot be checked against something is not evidence of anything.

**One tool cannot be diffed exactly, and the reason is in the statistics rather than the port.**
`runtime`'s confidence bound is a bootstrap over the measured pairs, and Python's `random.Random` is
a Mersenne Twister seeded Python's way. Reproducing that stream in Rust to make the last digits
match would be work spent on the wrong thing, so the port uses a small documented xorshift and says
so in the source. What *is* comparable is checked: the point estimate is a plain median with no
randomness in it, the `--report-only` path re-renders the report from the saved record and is
byte-identical, and the measuring path's own output is a well-formed record of the same shape.

Porting `runtime` also found a function that was dead in the original: `t_quantile` and its table of
t quantiles are never called, the bound having become a bootstrap at some point without the table
being removed. It is not in the port.

```bash
uredo build tools
cargo run -q --manifest-path tools/target/uredo/Cargo.toml --bin spec-audit
```

## What the ports measured

Ten binaries and one shared library module, 1,786 lines by the metrics tool's own count (comments
and tests excluded on both sides).

| | Uredo | generated Rust | |
|---|---|---|---|
| **total** | **1,786 lines, 17,972 tokens, 457 annotations** | **2,784, 20,743, 621** | **−13.4%** |

D39 makes the generator write `::std::println!` and `::std::format!` by absolute path where a person
would not; those cost 560 tokens across the ten, and removing them puts the honest figure at
**−11.0%**, above §37's 10% threshold. Cyclomatic complexity and function count are identical on
both sides of every file, as they must be. The corpus's own figure, corrected the same day, is
−13.0%; the two agree to within half a point on code written for entirely different reasons.

**The annotation ratio is 0.736 against the corpus's 0.571, and D2 is the whole of the gap.**
`uredo lint --measure` counts the references written because a callee is a Rust item, which Uredo
would have inserted for its own: **102 across these files, against 35 across the corpus, the
portfolio and the examples together** — 5.7 per 100 lines against 1.8. This code lives on `Path`,
`Command`, `Regex`, `fs` and `serde`, and D2 makes every argument to a Rust item Rust's to spell, so
Uredo saves on its own declarations and nothing else. §37 predicted exactly this shape —
"break-even on framework code whose types are by-value wrappers" — and tooling is that kind of code.
The token saving survives contact with foreign APIs; the annotation saving does not. §37's *gated*
measure, signatures only, is untouched at 0.477.

**Lifetimes written: four — every one of them `'static`.** `uredo lint --measure` counts them now
rather than a grep: four in reference position, none as a bound, none as a generic argument, none a
loop label, **1.9 per 1,000 non-blank lines over 2,137 of them**. All four are D48: a
`str` return with nothing to elide from must write its lifetime, and a function returning a fixed
string or a `Vec` of them is the shape that keeps arriving (`verdict_of`, `pieces`, `gated`,
`python_bool`). One that looked necessary was not — `const PUNCT: str` already lowers to
`&'static str` by §8.3. §38 reopens the notation question at more than 3 per 1,000 lines over at
least 2,300 lines; this is 1.9 over 2,137, so it is under the threshold *and* 163 lines short of the
sample. No named lifetime has been needed yet.

## What it cost to write

Six errors, all the author's, all reported in Uredo terms at Uredo line numbers:

- `and` for `&&`. Uredo's layout is Python's; its operators are Rust's (§7.1), and the habit does
  not transfer.
- `inout problems` at a call site. The mode is on the declaration; the caller writes the argument
  and Uredo inserts the `&mut` (§10.3).
- `.captures_iter(row.body)` where the callee is a Rust item, so the argument is Rust's to spell:
  `&row.body` (D2). This is the rule that costs the most attention in practice.
- `.collect()` into a `str` parameter, which wants a turbofish and a `String` (D10).
- a match arm that assigns — `Some(up): here = up.to_path_buf()` — which reads as a binding, and
  needs the indented block form the diagnostic named.
- a zero-argument closure where `unwrap_or_else` wants one.

The second and third ports added four more, none of them repeats:

- `for p in programs` over a `&Vec` behind `as_array()`. §16 borrows a `for` source that is a
  place, and a reference to an iterator is not one; the diagnostic named both ways out.
- `fn or_dash(s: String) -> String` returning `s`. A `String` parameter is a borrow (P3), so there
  was nothing to return; `take String` says what was meant.
- `items: [str]` for a list of strings — a slice of the unsized `str`, which cannot exist. `str`
  means `&str` where a parameter *is* one, not inside a slice of them: `[&str]`.
- a `*held` deref on a `bool` already copied out by a tuple pattern.

The `'static` above is the only diagnostic in the whole exercise that arrived as a *rule* rather
than a mistake — Uredo mapped it to D48 and said which spelling to write.

The last four ports added four more mistakes and one rule:

- `crate` as a variable name. It is one of the four Rust keywords with no raw form, so there was no
  spelling for it in the generated Rust — and Uredo said nothing until rustc did. It has its own
  diagnostic now (see below).
- `root.join(piece)` where `join` takes its argument by value, moving a `String` the loop still
  needed. D2 again, in its third disguise.
- `a, b => …` for a two-parameter closure, which parses as two arguments. §17 writes it `(a, b) => …`.
- `var arms: usize` with no initialiser. Uredo has no deferred initialisation — D6 binds a name to a
  value — so the three measures became expressions instead, which reads better anyway.
- and the rule: `Option::unwrap_or_else` takes a closure of no arguments, where `Result`'s takes one.
  Rust's distinction, reported by rustc, mapped to the Uredo line.

And four more defects in the toolchain, all found by building these four:

- **`src/bin/` was not a Cargo target directory to `uredo`** — fixed in the first port, but the
  library layout these four need exposed the other half: a module shared between two binaries has to
  live in the library, and §20.3's automatic declaration is private, so `pub mod pieces` is written
  by hand in `src/lib.ure`. That is the rule working as specified, and it is now exercised.
- **a Rust keyword naming a *field* was not escaped.** The access was (`p.r#gen`), the declaration
  and the struct literal were not, so `gen` — reserved in Rust 2024 — reached rustc bare in two of
  the three places it appears.
- **`crate`, `self`, `Self` and `super` naming a binding** produced uncompilable Rust with no Uredo
  diagnostic. §6.1 already said these four cannot be raw; it did not say what happens when one is
  used anyway, and the answer was "nothing, until rustc".
- **a diagnostic on an import named the wrong import.** rustfmt sorts `use` declarations, and the
  provenance map aligns the formatted text to the raw text by walking tokens — which cannot follow a
  permutation. Every import in a reordered run pointed at a neighbour's line. The `use` lines are now
  matched by text after the walk.

The last three ports added two more mistakes, and the same rule twice:

- `token.find_iter(code)` and `literal.replace_all(code, …)`, both needing `&code`. D2 again, and
  it is the only error in this exercise that recurred across every port.
- `items: [(usize, usize, &str)]` iterated with `for (start, end, text) in spans` — the tuple's
  `usize` fields arrive behind a reference and need `*start`, where `&str` does not.

And three more defects in the toolchain and its measurements:

- **the lexer**: `'\''` ended one character early, the escaped quote being taken for the closing
  one, and the stray quote then lexed as a lifetime. Every escape form is tested now.
- **the annotation counter** read string literals as code, so an English possessive counted as a
  lifetime and an `&` in a message as a borrow.
- **the fix for that** paired quotes with a pattern, which gets `r#"…"#` wrong — its body may hold
  a quote of its own — and swallowed whole spans of code. The tokenizer, which already knows every
  literal form, now decides where a literal ends. The corpus's all-annotations ratio moves from
  0.563 to 0.552 as a result; its gated figures do not move at all.
- **and the same scanner cut a `.ure` line at the `#` of a raw string**, so `const X = r#"…"#` lost
  its literal and the body was read as code. This one moved a *published* number: corpus program 08
  embeds a JSON document as a raw string, which the Rust twin counted as one token and the Uredo
  side counted as forty-odd. The corpus's token reduction is **−13.0%** overall and **−13.4%** on
  the application-style programs, not the −14.0% / −14.6% reported until now. The stripper is one
  scan over the whole text now, because a raw string spans lines and its `#` and `//` are not
  comments.

And three defects in the toolchain, each found by building this rather than by reading anything:

- `src/bin/` was treated as an ordinary module directory, so the automatic declarations of §20.3
  wrote `src/bin/mod.rs` — which Cargo then built as a binary named `mod`.
- files beside the source were not copied into the generated crate, so `include_str!("data.txt")`
  could not find its own neighbour, and `uredo package` would have exported an incomplete package.
- `CARGO_MANIFEST_DIR` names the generated crate rather than the authored one. That is correct and
  cannot be otherwise without breaking the exported package, but §25 said nothing about it.
