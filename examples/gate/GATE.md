# Phase 0 architectural acceptance gate — record (2026-09-11)

The gate of `docs/UREDO_LANGUAGE_SPEC_v0.4.md` §34, run as executable checks in
`compiler/tests/gate.rs` (items 1, 2, 3, 5, 6, 7), `compiler/tests/cli.rs` and
`compiler/tests/diagnostics.rs` (item 4). Every check runs the real toolchain; a check that
cannot run (missing target, missing `python3`) fails rather than skips. Thresholds were fixed
before running: exact expected output for items 1–3 and 6, exact consumer output for item 5,
mapped Uredo line and rewritten message for item 4, `same_opcodes` for every function and no
one-sided function for item 7.

| Item | What was run | Result |
|---|---|---|
| 1 | `labels(n)` calls the generic Rust callee `route<T: Mode>` (`src/route.rs`) with `n` and `&n` | prints `owned borrowed`; `uredo explain` reports both calls as "arguments passed verbatim (callee is a Rust item)" and no inserted borrow |
| 2 | `@derive(Debug, Clone, PartialEq, Eq, Hash, Default)` on an Uredo struct; `.clone()`, `==`, `{:?}`, `Default::default()` called from Uredo | `true Job { id: 1, payload: "payload" } 0` |
| 3 | `thread::spawn(move () => job.payload.len())` with `job: take Job` | `7`; the thread owns `job` |
| 4 | use after `take` (E0382) and a `self: inout` method on a non-`var` binding (E0596), from `compiler/tests/fixtures/moved`; an immutable argument to `inout`, rejected by Uredo itself | both rustc errors rewritten in Uredo terms on `src/main.ure` lines 20 and 22 with the callee's Uredo declaration; the `inout` case is a Uredo diagnostic before rustc runs |
| 5 | `uredo package examples/gate` (a `.ure` library and binary plus the hand-written `route.rs`), consumed by the plain-Rust project `compiler/tests/fixtures/gate_consumer` | `owned borrowed 6`; the exported manifest carries `rust-version`, no workspace table, no compiler artefacts; the explicit `pub mod route` is kept and not duplicated |
| 6 | `uredo run --features fancy` selects the `@cfg(feature = "fancy")` item; `uredo check --target x86_64-unknown-linux-musl` | `hello (fancy)`; the cross-target check passes |
| 7 | `kernels/uredo` (Uredo) and `kernels/rust` (hand-written) built with `--emit=llvm-ir -C codegen-units=1` in release, compared per function by `docs/corpus-study/roundtrip/tools/ircmp.py` | 3 functions (`dot`, `count_words`, `keep::__keep`): opcode sequences identical, alloca bytes identical, none one-sided |

## What the gate found

- **Cross-file callees were treated as Rust items.** `main.ure` calling `derive_roundtrip(job)`
  declared in `lib.ure` received verbatim arguments and failed with E0308. The compiler now
  indexes every `.ure` file's declarations under its module path before lowering, and `use`
  items resolve into the local tables, so §10.3 applies across the crate (`compiler/src/lower.rs`,
  `CrateIndex`). The translated E0308 message pointed at the exact call.
- **Automatic module declarations are private (§20.3)**, so a `keep.rs` helper module
  referenced by nothing was dropped from the rlib and two of three kernels vanished from the
  IR on the Uredo side. The kernels crate declares `pub mod keep` explicitly, as the spec says.
- **Parallel test runs raced on `target/`**; the gate checks now serialise on a lock.

## Record required by §34

| Measure | Value |
|---|---|
| Explicit-annotation count (`&`, `mut`/`var`, `take`, `inout`, lifetime ticks; tests excluded) | `src/lib.ure` 4 (generated Rust 5) · `src/main.ure` 0 (1) · kernels 3 (hand-written Rust twin 6) |
| Raw-Rust share of the gate sources | 31 of 108 lines (`route.rs` 22, `keep.rs` 9): 29% by line, all of it the generic callee the item requires and the codegen anchor |
| Diagnostic mapping rate on the fixtures | 3 of 3 primary spans mapped to Uredo lines (E0382, E0596, E0308); 3 of 3 secondary spans mapped; 2 rustc children ("aborting", "failure-note") dropped by design |
| Cold build (`rm -rf target`, `uredo build`) | 0.44 s |
| Warm build (no change) | 0.26 s |
| Lowering only (`uredo rust src/lib.ure`, rustfmt included) | 0.03 s |

Machine: the development laptop, debug build of `uredo`, cargo warm cache for std.

## Verdict

All seven items pass. Per §34, §10 and §12.3 were frozen with the v0.3 spec (§0.1). Failures kept in the
corpus: the cross-file resolution gap (fixed) and the private-module codegen trap (spec
behaviour, documented in the kernels crate).
