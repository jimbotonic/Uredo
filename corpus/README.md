# The §43 corpus

The twenty representative programs of spec §43, each as a Uredo package (`NN_name/src/main.ure`,
runnable with `uredo run NN_name`) beside its hand-written idiomatic Rust twin
(`NN_name/idiomatic/`, plain Cargo). `run.py` runs both, checks that their output is identical,
and computes the §37 metrics into `RESULTS.md` and `results.json`:

```bash
cargo build --manifest-path compiler/Cargo.toml
cargo build --release --manifest-path docs/corpus-study/analyzer/Cargo.toml
python3 corpus/run.py
```

Programs 1–15 are application-style (the population §37's token threshold names); 16–18 need
tokio or rayon; 19 calls a hand-written `.rs` module; 20 uses `rust { }` for an unchecked loop.
The generated Rust of each program is kept under `NN_name/target/uredo/src/main.rs` after a run.

What the corpus found while being written (log #53): three compiler gaps — writes through
pattern-bound names of unknown type were rejected instead of left to rustc, `for … in inout
param` double-borrowed an `inout` parameter, and `throws` before `->` was silently mis-parsed —
and one language observation: foreign trait methods that take their argument by value
(`From::from`) still need a written `take` (G11, held).

## Allocation, memory and size fixtures (§36)

`corpus/perf.py` exports the Uredo side with `uredo package`, copies the twin, appends a counting
global allocator to both, renames each `main` so a wrapper can report the totals, builds both in
release and runs each three times. A program whose own counts move between runs is reported as
unstable rather than compared.

| Measure | Gate |
|---|---|
| allocation count, total bytes | `uredo <= twin` (§36's words); measured result: identical on all twenty |
| peak live bytes | reported; one program differs, because D12 iterates a place by shared reference where the twin's loop consumes it |
| release binary size | against the checked-in limits in `size_baseline.json`, on a matching toolchain; *not* against the twin, since at this granularity the paired figure carries the crate-name length and the build paths |

Results: `PERF.md`, `perf.json`. Naming programs prints a comparison without touching the committed
report; `--update-baseline` rewrites the size limits, which is the reviewed change §36 asks for.
