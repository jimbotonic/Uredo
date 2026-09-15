# §36 performance fixtures

## `tail_ure` and `tail_rust` — the `throws` tail-`?` obligation

The round-trip study found the one codegen regression of the whole corpus here: a tail `e?` in a
`throws` function lowered to `Ok(e?)`, which on a 32-byte error payload left a stack temporary and a
`memcpy` (`corpus-study/roundtrip/README.md`, finding G1). D37 replaced that lowering with an explicit
`match` through an identity call (§15.3), and §36 requires a fixture proving it.

`tail_ure/src/lib.ure` is the Uredo side and `tail_rust/src/lib.rs` the hand-written twin. Both crates
are named `tailfix` so the same public names can be compared function by function. The error payload is
exactly 32 bytes, asserted at compile time on both sides, and the callee both tails call is
`@inline(never)`, so what the comparison sees is the tail lowering and not an inlined body.

| Function | Uredo side | Rust side | Obligation | Measured |
|---|---|---|---|---|
| `inner` | the shared opaque callee | the same | identical, or the comparison means nothing | identical opcodes, 17 instructions |
| `archive` | tail `inner(n)?`, same error type | `inner(n)`, the direct return | the tail is emitted as the direct return | identical opcodes, 2 instructions, no stack temporary |
| `convert` | tail `inner(n)?`, converting error type | `Ok(inner(n)?)`, the form D37 replaced | no more instructions than `Ok(e?)` | 16 against 18 |

Run by `compiler/tests/perf.rs`, which builds both crates with `--emit=llvm-ir -C codegen-units=1 -C
debuginfo=0` in release and compares them with `corpus-study/roundtrip/tools/irstat.py`.

## The allocation, memory and size obligations

They live with the programs they measure: `corpus/perf.py`, reported in `corpus/PERF.md`, with the
checked-in size limits in `corpus/size_baseline.json`. One program of that sweep also runs in
`compiler/tests/perf.rs` so the harness stays exercised.
