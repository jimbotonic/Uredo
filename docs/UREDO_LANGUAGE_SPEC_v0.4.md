# Uredo Language Specification v0.4

> **Status:** **Core v0.1 surface frozen (2026-09-12)** — meaning no further rule may change what existing source means without a decision row and a migration; D54–D60 were taken after that date, and D54 did change the meaning of an existing `T?` parameter, which is why it carries a panel and a corpus behind it. The Phase 0 gate (§34) passed and the §43 corpus meets every §37 threshold. Every **language rule** below is implemented by the reference compiler in `compiler/` and exercised by the 25-topic portfolio (`portfolio/`) and the twenty corpus programs (`corpus/`); the **tooling** is built but for three commands and the **validation** is complete but for one certification — §0.1 is the single status record, and where a section describes a command or a measurement it does not qualify, §0.1 governs. Decisions D1–D59 in §0 record what was decided and why; **the sections carry the rules, and on any conflict the section text wins.**  
> **Provisional language name:** **Uredo**  
> **Target:** Rust-compatible, zero-overhead, progressively explicit programming language  
> **Primary implementation strategy:** source-to-source compilation to readable, deterministic Rust, followed by Cargo/rustc  
> **Supersedes:** v0.3, v0.2 and v0.1 (all kept for the record: v0.3 is where the compatibility suite, the performance fixtures and D54–D59 were taken; v0.2 is the working draft in which the first decisions were taken one by one; v0.1 is what the first panel reviewed)  

---

## 0. Decisions, status and history

*Reading order: this section is a record, not an introduction. A reader meeting the language should start at §1, then read §6 to §25 in order — the rules are there, and they are written to be read in sequence. Come back here for why a rule is what it is, what is implemented, and what is still open.*

The v0.1 draft stated its central mechanism ("ordinary parameter syntax means shared borrow, the compiler infers the rest") as if it were decidable from Uredo source. It is not: `Copy`-ness of foreign types, receiver kinds and call-site borrows all depend on Rust type facts the pipeline placed *after* the decision, and a generic Rust callee can accept both the borrowed and the owned lowering with different results (verified, `oracle-reviews/verification_log.md` #10). v0.2 makes the rules **syntactically decidable** and pays the resulting idiom cost openly.

The decisions, in the order they were taken (D1–D33 in the v0.2 revision, D34–D53 during implementation and measurement). This table records the **decisions that were taken** — where a rule was chosen over an alternative, and why. It is deliberately not an index of every rule: most of the language is ordinary Rust semantics under a different spelling and needed no decision, and duplicating those rules here would give them two homes. The sections are the rules. Codes used here and in the review record: **D***n* is a decision, listed below; **G***n* is a gap the round-trip study opened, since closed by a decision or held (§38); **A**/**B**/**C**/**E***n* label amendment groups in `oracle-reviews/decisions/`, and **P***n* a portfolio finding. Only D and G codes carry rules; the rest is provenance.

| # | Decision | Section |
|---|---|---|
| D1 | Passing modes are decided from the declaration alone (the complete table is §10.2): known-Copy types pass by value (scalars, `@derive(Copy)` types, the prelude table, `@copy use`, and known-Copy tuples and arrays); a type parameter or `impl Trait` passes by value unless `?Sized` (P8, D52); a pattern parameter by value (P7); an explicit `&T`/`&mut T` as written (P6); an `impl` of a listed std trait takes the trait's modes (D53); every other type, including foreign types, is a shared borrow unless `inout`/`take`. No rule consults a foreign type's `Copy`-ness. | §10 |
| D2 | Call-site auto-borrow applies only when the callee is a Uredo-declared function or method whose signature Uredo can resolve. Calls into Rust items use Rust argument conventions verbatim. | §10.3 |
| D3 | Receivers are explicit (`self`, `self: inout`, `self: take`) on every method and trait signature; no receiver means an associated function. No receiver inference. | §12.3 |
| D4 | `[…]` is a fixed-size array. A `Vec` requires `Vec::from([…])`, `vec![…]` or a `Vec<T>` annotation. | §9.4 |
| D5 | Interpolated string literals are legal only as the direct argument of a bare call to `print`, `eprint`, `format`, `panic`; they capture identifiers and, since D43, accept Rust's positional and named `format_args!` arguments; `format` returns an owned `String`. | §11.3 |
| D6 | `name = expr` binds when the name is unbound in the current *and* every enclosing scope; otherwise it assigns and requires `var`; `let name = expr` always creates a new binding (shadowing). | §8.1 |
| D7 | Paths use `::`; `.` is field access, method call and the postfix `.await` (D30) only. | §6.7, §21 |
| D8 | In a `throws` function, `return e` and the tail expression lower to `Ok(e)`; a unit `throws` function gets `Ok(())`. Bare `throws` means the crate's declared default error type and is not permitted on `pub` items. | §15 |
| D9 | Optional binding is Rust's `if let Some(u) = user:`; `as` remains the cast operator; `none` is dropped in favour of `None`. | §14, §7.3, §9.3 |
| D10 | Generic arguments in expression position use turbofish `::<…>` in v0.1. | §7.6 |
| D11 | `take`, `inout`, `move` and `rust` (before `{`) are contextual keywords; `throw`, `throws`, `var` are reserved (`try` is already a Rust keyword and has no Uredo meaning). Rust keywords used as Uredo identifiers are emitted as raw identifiers. | §6.1, §6.2 |
| D12 | `for x in xs` iterates by shared reference when `xs` is a place; `for x in inout xs` / `for x in take xs` for mutable / consuming iteration. | §16 |
| D13 | Closures use Rust's capture inference; forced by-value capture is spelled `move x => …`. | §17 |
| D14 | Every `pub` item has a materialised Rust signature derived only from its declaration; bodies, dependencies and inference never change it. | §4.4 |
| D15 | Distribution: `uredo package` emits a self-contained Cargo package containing generated Rust. Consumers need Cargo/rustc, not Uredo. | §4.6 |
| D16 | Phase 0 is an architectural acceptance gate, not a demo. | §34 |
| D17 | Primary persona: developers who accept Rust's ownership model and want less notation. Newcomers are served by diagnostics, not by hiding ownership. | §1.1 |
| D18 | Comments `#`; doc comments `##` → `///` and `##!` → `//!`; attributes `@`. | §6.4, §23 |
| D19 | The formatter hands Rust blocks to rustfmt. | §29 |
| D20 | Foreign `Copy` types: the prelude ships a versioned table of std `Copy` types; other crates' types are declared with `@copy use`; the generator asserts `Copy`. (38.1) | §10.1 |
| D21 | Struct construction uses Rust braces only: `User { id: 42, name, ..base }`. Struct variants are declared and matched with braces too. (38.2) | §12.1, §13 |
| D22 | Turbofish stays in expression position. (38.3) | §7.6 |
| D23 | Rust macros are invoked directly, `path!(…)`, in expression, statement and item position. (38.4) | §22.5 |
| D24 | Bare `throws` = `@!default_error(E)` declared at crate or module level; `pub` functions name their error type. (38.5) | §15.1 |
| D25 | `let name = expr` introduces a new binding and may shadow. (38.6) | §8.1 |
| D26 | Optional chaining stays deferred; §14 documents the idioms. (38.7) | §14 |
| D27 | Receivers explicit on every method (supersedes the private-method inference in D3's first draft). (38.8) | §12.3 |
| D28 | `rust { }` is the only Rust-block form. (38.9) | §6.6 |
| D29 | Debugging: line-preserving codegen now; Phase 2 exports the provenance map and ships map-aware GDB/LLDB/VS Code helpers; DWARF rewriting only on measured need. (38.10) | §28.2 |
| D30 | Error propagation and awaiting use Rust's postfix `?` and `.await`; the prefix `try`/`await` forms are withdrawn. `?` is legal in any function whose return type is `throws`, `T?`, or a verbatim `Result`/`Option`. (corpus study, amendment 1) Amended by D37 for the tail/returned `?` of a `throws` body. | §15.2, §21 |
| D31 | Mutable by-value bindings are spelled with `var`: `var self: take`, `var x: take T`, `var n: i32`. (amendment 4) | §10.2, §12.3 |
| D32 | Pattern parameters pass by value; pattern bindings `pat = expr` bind when every name is new and assign when every name is bound; `let pat = expr` forces a binding; `let PAT = expr else:` for refutable patterns. (amendment 5) | §8.1, §10.2 |
| D33 | A verbatim `Result`/`Option` return type is plain Rust: no implicit `Ok`; `?` is allowed in both, `throw` only in the `Result` case. `throws` remains sugar. (amendment 6) | §15.1 |
| D34 | Inline single-statement bodies: the colon of a function, method, `if`, `else`, `while` or `for` header, or of a `match` arm (the list in §6.3), may be followed on the same line by exactly one body; `else:` attaches by layout anchor. (final check, E1) A trailing `if` (D51) anchors at its header line. | §6.3, §13.1, §29 |
| D35 | Clarifications: colon-less item declarations have no body; bare `return` in a unit `throws` body; tail `rust { }` needs no annotation; single-expression arm lowering never adds a block; `use` in bodies; inline `mod name:`; `_ = expr` and other name-less patterns always bind; `() =>` closures; arm lowering never adds a block; receivers are outside the passing-modes table; intrinsics are bare-call names. (A1–A7, A9–A11) | §6.3, §8.1, §11.3, §12.3, §13.1, §15.3, §17, §20, §22.3 |
| D36 | A `match`/`if` header may follow `=`, `return` or `throw` and owns the indented block; as a trailing call argument it is governed by D51. (A8, D51) | §7.7 |
| D37 | In a `throws` function or closure, a tail or returned `e?` lowers to a `match` through `__uredo_owned` with qualified constructors; its operand must be an owned `Result`. Verified alias-identical to a direct return. (B1) | §15.3, §36 |
| D38 | Item declarations in blocks are lowered in place; nothing is hoisted. (B2) | §6.3 |
| D39 | Generated code names every type, trait, constructor and macro it introduces by absolute path. (B3) | §5.3 |
| D40 | Macro token streams are Rust source: no intrinsics, no auto-borrow, no `throws` elaboration inside; `macro_rules! name` also switches the lexer. (B4) | §22.5 |
| D41 | Bare field names are read-only sugar with lexical precedence, disabled by glob imports, nested items and `move` closures; every field write is spelled `self.field`. Amends §12.2. (C1) | §12.2 |
| D42 | Closure parameters are Rust's: an annotation is the exact type and `take`/`inout` do not apply — the stated exemption to the transfer rules of §38, which govern `fn`/method declarations. (C2) | §17, §38 |
| D43 | Intrinsic format literals accept positional and named arguments, elaborated as Uredo expressions. Amends D5. (C3) | §11.3 |
| D44 | Attributes may precede struct-literal fields; attributed block expressions stay in `rust { }`. (C4) | §23 |
| D45 | Trait impls may be nested in a `struct`/`enum` body for the enclosing type and are emitted after it with its generics and `@cfg`. (E4) | §12.2 |
| D46 | `const:`/`static:` declaration groups: an ordered list of ordinary items with one written visibility; group attributes distribute to every member. (E3) | §8.3 |
| D47 | Call-site auto-borrow emits a syntactically-reference argument verbatim (`&e`, string/byte-string literals, bindings declared with a reference type); `&&[T; N]` does not coerce to `&[T]`. P8 parameters receive the argument verbatim, so the instantiation is the argument's spelled type; the former G15 insertion (log #42) and its goal are withdrawn by D52. (portfolio P1) | §10.3, §9.9 |
| D48 | A `str`/`[T]` return type with nothing to elide from writes its lifetime: `-> &'static str`. (portfolio P2) | §9.7 |
| D49 | An inline `if` body may be followed on the same line by `else:` and one inline body; neither body may be an `if`; `else if` has no inline form. (portfolio P3) | §6.3, §29 |
| D50 | Configuration-invariant lowering: one lowering per file for all targets and feature sets; `@cfg` alternates of one item must agree in what Uredo elaborates from; `@derive(Copy)`, `@copy use`, `@!default_error` unconditional (enclosing guards count). Replaces §4.6's per-configuration lowering. (corpus: 13 of 46,258 fns alternate with differing types, mostly different items; panel `oracle-reviews/cfg/`) | §4.6 |
| D51 | Trailing block arguments: inside an unclosed `(` — a call's argument list or a grouping pair — a line ending in a closure `=>` or a `match`/`if` colon opens an indented block anchored at that line; the block ends at a line beginning with `)` at the anchor, which may carry the enclosing expression's suffix (`).collect()`, `))`, `)?`, `):`); the block is that parenthesis's last expression; a trailing `if` anchors its `else` at the header line; a closure block's value is its last expression. Resolves G9. (corpus 4,430/4,432 last-argument; panel `oracle-reviews/g9/`) | §6.3, §7.7, §17 |
| D52 | Type parameters and `impl Trait` pass by value (P8); a `?Sized` bound keeps the borrow; reversal conditions in §38; the visible signs of a non-Copy transfer are the `take` annotation, a pattern parameter (P7), the P8 type form and, in an `impl` of a listed std trait, the trait's own signature (D53); `take` remains the only *annotation*. (corpus 91% by value, hit rate 9.2% → 90.8%; panel `oracle-reviews/generics/`) | §10.2, §10.3, §18 |
| D53 | Std-trait parameter modes: in an `impl` of a std trait from the compiler's table, method parameters take the trait's modes (`From::from` by value, `PartialEq::eq` `&Rhs`, `Display::fmt` `&mut Formatter`, operator traits by value, …); other traits follow §10.2. Resolves G11. (corpus program 9, round-trip p4) | §12.3 |
| D54 | An optional parameter follows its payload: `Option<T>`/`Result<T, E>` are known-Copy when their payloads are (§10.1, recursively), so `x: u32?` is `Option<u32>` by value and `x: String?` stays `&Option<String>`; a `Copy`-bounded type parameter is known-Copy as a payload; `&T?` is `Option<&T>`, resolving G10; `&mut T?` is rejected with the two spellings that work; known-Copy-ness may not vary with the configuration (D50). By-value-always rejected: it moves out of borrowed places at call sites the compiler does not type. `@value use` stays held. (corpus 65.1% of Option-outer parameters, against 1.1% for the former rule; panel `oracle-reviews/optional/`) | §9.3, §10.1, §10.2 |
| D55 | An attribute on a field of a tuple struct or a tuple variant is written inline before the type (`Parse(@from io::Error)`), the same way D44 puts one on a struct-literal field; it lowers into that position. Found by the §35 forwarded-attribute fixture: without it thiserror's `#[from]` needs a `rust { }` block. | §23 |
| D56 | `@copy use` may name one instantiation of a generic foreign type (`@copy use nalgebra::Vector3<f64>`); that instantiation is known-Copy, others are not, the import drops the arguments and the `Copy` assertion keeps them. Found by the §35 numeric fixture: the bare form generated an assertion with a missing generic argument and did not compile. | §10.1 |
| D57 | Rust's name-value attribute has a name-value spelling: `@name = value`, the value being the rest of the line, lowering to `#[name = value]`. Found by the §35 proc-macro fixture, whose derive reads one. (corpus: 797) | §23 |
| D58 | A fenced block in a `##` comment is Uredo source: it is lowered and emitted as a doctest, so rustdoc compiles and runs it, with `rust` and `text`-style tags as the escapes; the rendered example is therefore Rust, which is the language of the generated crate its readers consume. (corpus: 3,210 doc examples in 562 files, almost all compiled by rustdoc) | §26.4 |
| D59 | A receiver may name its own type — `self: Pin<&mut Self>`, `self: Rc<Self>` — and is emitted as written; Uredo models none of them, so what may be done through one is rustc's to say. Found by the Phase 3 sweep: without it a hand-written `Future` could not be spelled. (corpus: 862 such receivers, 849 on a `poll`) | §12.3 |
| D60 | A trait declaration may carry supertraits, written as Rust writes them; the colon that opens the body is the last one on the line. Found by the v0.4 review: §21 said supertraits use Rust syntax and no spelling parsed, so 57% of the corpus's trait declarations could not be written at all. (corpus: 322 of 568) | §21 |
| D61 | Source files are `*.ure`; the extension is **not** shortened to `.ur`. GitHub Linguist assigns `.ur` to UrWeb, and Linguist will not take a new language until it has 2,000 indexed public files, so `.ur` would render every Uredo file as another language for years while `.ure` is merely unrecognised. Sharing an extension is possible — `.ex` is shared by Elixir and Euphoria under a content heuristic — so the bar is cost, not availability, and the cost falls on every user rather than on the author. (panel, `docs/oracle-reviews/extension/`) | §20.3 |
| D62 | The block-opening `:`, the `fn` keyword and the `->` of a return type stay mandatory. A colon-less declaration already means a body-less one (D35), and real code needs the colon to end a multi-line header; every comparable indentation-sensitive language keeps a header/body marker (Python `:`, F# `then`/`do`, Haskell `then`/`=`, Scala 3 `then`/`do`/`:`). Omitting `->` while keeping the return type, and omitting `fn` in declaration position, violate neither standing principle and were declined on price: 217 and 292 tokens, 0.72% and 0.97% of real Uredo, against a −13.0% headline. (panel, `docs/oracle-reviews/elision/`) | §6, §11 |
| — | **Under review, not applied** (owner's decision after the corpus study; generic `T` resolved by D52): by-value `T?` parameters, and a `@value use` by-value declaration for non-`Copy` foreign types. `take` stays the only *annotation* of a non-`Copy` ownership transfer until the corpus evidence is reconciled with visible move semantics. | §38 |

### 0.1 Milestone status (2026-09-13)

| Milestone | Status | Record |
|---|---|---|
| Phase 0 gate (§34), seven items | **passed** on 2026-09-11; two failures kept: cross-file callees were lowered as Rust items (fixed by a crate-wide declaration index), an automatically declared module is private (§20.3), so a module referenced by nothing was dropped before codegen and its anchor with it — the module system working as specified, not a lowering defect | `examples/gate/GATE.md`; `compiler/tests/gate.rs`, `cli.rs`, `diagnostics.rs` |
| Phase 1 success criterion (§34): §43 programs meet §37 | **met** on 2026-09-12, measured over all twenty programs and reported for the application-style set the threshold names: tokens −13.4% on programs 1–15 and −13.0% over all twenty (≥ 10%; both corrected on 2026-09-13 — the tokenizer had been cutting a `.ure` line at the `#` of a raw string, so program 08's JSON literal was counted as code on the Uredo side and as one token on the Rust side), signature annotations — parameter lists and return types, §37 — 52 vs 109 over all twenty (ratio 0.48 ≤ 0.5), inference hit rate 64/64 on non-generic parameters (≥ 90%), barrier diagnostics mapped 4/4 (≥ 80%); raw-Rust share 7.4% of lines, all in programs 19–20 by design; all twenty programs print exactly what their idiomatic Rust twins print. Counting annotations inside bodies too, where Uredo keeps Rust's `&` for Rust items and `var` for `mut`, the ratio is 0.57 (120 vs 210; corrected twice on 2026-09-13, once for a counter that read string literals as code and once for the raw-string cut above). Re-measured after D54, whose two new parameters in program 06 are both inference hits | `corpus/RESULTS.md`, `corpus/run.py` |
| Round-trip verification (§36) | **reproduced from source on 2026-09-13**, having rested on a stored record while the archive was flat and the pipeline expected crate directories it no longer had: all six pieces pass their originals' own tests on both sides (109 tests), all 191 functions are opcode-identical with equal instruction totals and alloca bytes, and the differential fuzz agrees on 200,000 random inputs per piece. Two gate kernels identical | `corpus-study/roundtrip/README.md`, `results.tsv`, `examples/gate/kernels/` |
| Portfolio | 25 topics, generated byte for byte by the compiler (golden test), all build with no warnings and run | `portfolio/README.md` |
| Tooling (§28.1) | **every command §28.1 names is implemented**: `new`/`init`, `rust`, `check`, `check --api`, `build`, `run`, `test`, `doc`, `package`, `publish`, `explain`, `fmt`, `lint`, `lint --measure`, `fix`, `lsp`, `report`. Two carry a stated limit rather than a gap: `publish` is exercised to a `--dry-run` and nothing has been published to crates.io, and `lsp` answers a file that does not parse from the items that still did, which is §28.2's *tolerant* half; the *lossless* half — trivia and error nodes, enough to reconstruct the source — is not there, and rules out renames and code actions until it is. `fix` has no migrations to run, the surface being frozen; it applies the findings whose rewrite leaves the generated Rust byte-identical, verified per edit | `compiler/README.md` |
| §36 allocation, memory and size fixtures over the §43 programs | **met** on 2026-09-12: all twenty programs allocate no more than their idiomatic twins, and in fact exactly as much — identical allocation counts and bytes, each side stable over three runs, measured with a counting global allocator in release. Peak live bytes is also recorded and is identical on nineteen; the twentieth holds 15 bytes more because D12 iterates a place by shared reference where the twin's loop consumes it. Binary size gates against checked-in per-program limits rather than against the twin, since at that granularity the paired figure carries the crate-name length and the build paths | `corpus/PERF.md`, `corpus/perf.py`, `corpus/size_baseline.json` |
| §36 `throws` tail fixture | **met** on 2026-09-12: with the same 32-byte error type the tail `?` is emitted as the hand-written direct return (identical opcodes, 2 instructions, no stack temporary); with a converting error type it uses 16 instructions against 18 for the `Ok(e?)` form D37 replaced | `examples/perf/`, `compiler/tests/perf.rs` |
| §36 runtime regression budget | **closed as not enforceable by this method**, 2026-09-13, and that is the finding rather than a deferral. The twin control compiles the original's own source as a second crate, so its ratio is 1.0 by construction; on the last run its *median* was 1.0248 on `p5_sanitize` — a systematic 2.5%, larger than the 2% budget and larger than that piece's own reading of 1.0189. A +1.9% measurement from an instrument that reads +2.5% on a null difference is not evidence of anything. That is bias, not noise: the machine's own control resolved 2% on 4 pieces of 4, the cross-crate control on 2, and samples tighten the first while leaving the second where it is, code placement between two crates not being a random quantity. **What stands in its place is stronger and already passing**: every function of every round-trip piece compiles to the same instruction sequence as its original (191 of 191), which is what a wall-clock budget was a proxy for. A timing budget could be enforced again by comparing within one crate, or by setting the budget above the measured floor; neither is worth building while the IR is identical. Previously recorded as **measurable** since 2026-09-12 (`corpus-study/roundtrip/tools/runtime.py`, results `RUNTIME.md`): both sides of four round-trip pieces timed in one process, each comparison back to back with the order swapped every other pair, the median paired ratio bounded by a bootstrap. Two controls run on every measurement — the original against itself, and against its own source compiled as a second crate — and a run in which either control cannot resolve 2% is reported inconclusive, which §36 counts as a failure. **No regression has been measured in any piece** (ratios 0.976–1.024). What limits certification is no longer machine noise but the twin: at 110 pairs the machine's own control resolved 2% on every piece, while the cross-crate floor reached 1.046, so a 2% budget is at the edge of what a paired benchmark of two crates can enforce, and the twin is what says so | `corpus-study/roundtrip/RUNTIME.md` |
| Provenance map export and debugger helpers (D29, §28.2) | the map is built and written per build (`target/uredo/provenance/`), and the helpers on top of it exist: `ubreak FILE.ure:LINE`, `uwhere` and `ulist`, in GDB and LLDB, with `launch.json` recipes for both VS Code adapters. **Both scripts are exercised against a real program under a real debugger** (`compiler/tests/debug.rs`, each skipping loudly when its debugger is absent): the breakpoint lands on the arm the Uredo line names, and the backtrace and listing read in `.ure` terms. The LLDB one was written blind and verified a day later against LLDB 18.1.3 — it ran unchanged, needing only the runtime-frame trimming LLDB's stack walk makes necessary and GDB's does not. Writing them found the map wrong on every multi-line construct: a `match` attributed all of its lines to its *last* arm, and an `if` and every loop attributed their header and closing brace to whatever statement the body ended on | `compiler/debug/`, `compiler/tests/debug.rs` |
| `uredo test` / `doc` integration (§28.1) | both delegate to Cargo; documentation examples are lowered into doctests (§26.4, D58), so `uredo test` runs them and `uredo doc` renders them | `examples/compat/docs`, `compiler/tests/doc.rs` |
| Compatibility suite (§35) | **assembled** on 2026-09-12, extended by the Phase 3 sweep on 2026-09-13 and closed to 24 entries, all exercised, **16 with nothing missing**, on 2026-09-14, in `examples/compat/` with the contract per entry in `docs/COMPATIBILITY.md`. The three structural gaps the recording pass found are closed: `tests/`, `examples/` and `benches/` are lowered as the Cargo targets they are, the mutation-rebuild sequence runs on a copy of a fixture, and the database entry carries a crate-level negative case whose diagnostic arrives through a proc macro. Six defects were found by building the fixtures, each fixed with a test: D55, D56, D57, a relative path dependency not surviving lowering, `uredo run` swallowing a line of program output that parsed as JSON, and the formatter spacing out the type arguments of a `use` path. What is still not proved is recorded per entry, and the list is now short. The bench and the bare-metal build were the last two "lowered by the same rule and never run", and both ran on 2026-09-14: `cargo bench` executes a bench target that reaches the library it measures, and `no_std` builds for `thumbv7em-none-eabihf` with the crate supplying its own `#[global_allocator]` and `#[panic_handler]`, linked as a staticlib so rustc has to resolve them. What each entry still records is narrower than what it recorded before: the allocator frees nothing, no hardware or emulator executes the archive, and Node is not a browser — the generated glue was bypassed until 2026-09-14 and is now exercised too. The wasm entry's gap was closed on 2026-09-13 — the module is run in Node, the constructor, the `inout` method and the `String`-returning function all called across the boundary and agreeing with the host test — leaving a narrower one in its place: the pre-bindgen module is called directly, so the test hand-writes wasm-bindgen's ABI rather than exercising its generated glue, and Node is an engine rather than a browser | `docs/COMPATIBILITY.md`, `docs/compat/record.py` |

### 0.2 Open items

The authoritative list is §38; this is a pointer to it. Held or under review: a `@value use` declaration for non-`Copy` foreign types (held: it would move with no sign at the parameter); bound aliases (E2); silencing a lint in source (§29.1, open with a countable trigger); postfix lifetime notation (`str @a`, held after the 2026-09-11 panel); `loop`/`unsafe`/`async` headers as trailing block arguments (held with D51; write them with `rust { }`); optional chaining (D26); turbofish-free calls (38.3); the contents of the prelude `Copy` table (38.1).

**What this document cites but does not publish.** Decisions here cite the record behind them by
path. Three sets of those live only in the working repository and are not in the published one:
`docs/oracle-reviews/` (the panel briefs and the unedited responses), `docs/compat/` with the
generated `docs/COMPATIBILITY.md`, and `docs/corpus-study/` apart from the two measurement scripts
the test suite reads. Everything else a decision cites is present, and `docs/spec_audit.py` checks
that — every other cited path must exist, and in the working repository the three sets above are
checked the other way round, so the exemption cannot outlive what it exempts.

### 0.3 Revision history

- **Revision 2026-09-10 (v0.2):** the panel review of v0.1 applied: D1–D29, and the sections v0.1 lacked — lexical structure (§6), expressions (§7), statements (§8), program structure (§11.1, §20.3), types in signatures and the coercion table (§9.7–§9.9), distribution model (§4.6), elaboration boundary (§5.2), failure-mode charter (§5.4), lowering contract (§25), versioning (§28.2), acceptance gate (§34), performance obligations (§36), ergonomic evaluation plan (§37), deferred features (§33.2), prior art (§41).  
- **Revision 2026-09-10c:** corpus-study amendments applied (`corpus-study/FINDINGS.md`): D30–D33  
- **Revision 2026-09-10d:** round-trip and compression amendments applied after the final-check panel (`oracle-reviews/decisions/01_amendments_v2.md`): D34–D46; bound aliases held, implicit test imports rejected  
- **Revision 2026-09-11e:** portfolio findings applied (`portfolio/README.md`): D47 (verbatim syntactic-reference arguments), D48 (`&'static` on unelidable `str` returns), D49 (same-line `else`); G16 annotated with corpus counts of `impl Trait` parameters; postfix lifetime notation held (§38)  
- **Revision 2026-09-11f:** D50 configuration-invariant lowering (§4.6), after the Phase 0 gate (§34) and one adverse review (`oracle-reviews/cfg/`)  
- **Revision 2026-09-11g:** D51 trailing block arguments (§6.3, §7.7, §17), resolving G9 after a four-seat panel (`oracle-reviews/g9/`)  
- **Revision 2026-09-11h:** D52 type parameters and `impl Trait` by value (P8), `?Sized` keeps the borrow; G15 withdrawn; `impl Trait` argument position no longer deferred (§33.2); after a four-seat panel (`oracle-reviews/generics/`)  
- **Revision 2026-09-12i:** D53 std-trait parameter modes (§12.3), resolving G11
- **Revision 2026-09-12 (v0.3, between 12i and 12j):** consolidation — status notes gathered into §0.1, open items into §0.2, §38 rewritten as decided/held, phase rows in §33.1 marked implemented. No rule changed. Two drifts between the document and the compiler were found and closed while verifying it: §31.2's generated Rust was hand-written and had diverged (it now *is* the compiler's output for §31.1, checked by re-running it), and `throws` closures (§17) were documented but unimplemented (now implemented, with the §15.3 wrapping and the D37 tail lowering). Every Uredo example in the document lexes, and the complete ones lower to Rust that parses — which is not the same as type-checking, a distinction the audit's own wording overstated until 2026-09-14. The manual's examples *are* type-checked, by rustc, in `compiler/tests/manual.rs`.
- **Revision 2026-09-12j (v0.3):** D54 the optional parameter follows its payload (§9.3, §10.1, §10.2), closing G10 and the owner's `T?` review; `@value use` held with its evidence restated over concrete parameters only; after a three-seat panel (`oracle-reviews/optional/`). The reviewers' claims were checked in the shell: the configuration-invariance hole two of them reported is already closed by D50 in both of its forms, `&mut T?` was verified useless (E0594) and is now rejected, and the byte-identical portfolio was found to mean *no coverage* rather than *no breakage*, so topic 09 and corpus program 06 now exercise the by-value path.
- **Revision 2026-09-12k (v0.3):** `uredo lint` implemented and specified (§29.1, §28.1, §0.1): the three checks §10.2 already promised — `redundant_take`, `borrowed_container`, `copy_candidate` — each decided from the declarations the lowerer reads, with `--deny` and `--allow`. No rule changed. Running it on the suites found one real leftover, a `take` on a type parameter in portfolio topic 21 that D52 had made redundant; topic 05's `take u64` is kept as the illustration it is and documented as such.
- **Revision 2026-09-12l (v0.3):** an `@allow`/`@expect` naming one of Uredo's own lints now warns and is still forwarded (§23); §29.1 records the three candidate shapes for source-level suppression, what is rejected and why, and the countable trigger to revisit; §38 gains the open item. After a four-seat panel (`oracle-reviews/lint-allow/`) whose agreement was verified rather than trusted: the recomputation seat corrected six published figures, the review seat found an option the record had never considered and a collision hazard applied selectively, and a fourth shape proposed in a second round was killed by building the exported package it would affect.
- **Revision 2026-09-12m (v0.3):** two §36 obligations discharged and recorded in §0.1 — the allocation fixtures over the twenty §43 programs (identical counts and bytes on both sides) and the `throws` tail fixture (the same-type tail `?` is the direct return; the converting tail is shorter than `Ok(e?)`). No rule changed; §36 gains a paragraph saying which obligations are now measured and which two are not.
- **Revision 2026-09-12n (v0.3):** the §36 size-and-memory row discharged: peak live bytes recorded per program (identical on nineteen of twenty; the twentieth is D12's borrowing `for` against a consuming one) and binary size gated against checked-in per-program limits rather than against the twin, for a reason that was measured rather than assumed. Only the runtime regression budget is left unmeasured.
- **Revision 2026-09-12o (v0.3):** `uredo new` and `uredo init` implemented (§28.1, §0.1): the §4.1 manifest, a binary or `--lib` template that the toolchain's own `fmt --check` and `lint` pass, and a refusal rather than an overwrite whenever a file is already there.
- **Revision 2026-09-12p (v0.3):** the §35 compatibility state recorded per entry (`docs/COMPATIBILITY.md`, generated) instead of claimed in a sentence; §0.1's row replaced with the counts and the three structural gaps. No rule changed. Narrowing the covered paths while writing it corrected four inflated raw-Rust shares: a round-trip piece's directory had been counted with the untouched original and the lowered output beside it, and the gate's with a hand-written comparison twin from a nested crate.
- **Revision 2026-09-12q (v0.3):** D55, an attribute on a tuple-struct or tuple-variant field, written inline (§23). Found by building the first §35 compatibility fixture: thiserror's `#[from]` had no spelling, so an error enum needed a `rust { }` block. The fixture also found that `uredo run` swallowed any line of program output that parsed as JSON, since cargo's messages share that pipe; only lines carrying cargo's `reason` field are consumed now.
- **Revision 2026-09-12r (v0.3):** D56, `@copy use` may name one instantiation of a generic foreign type (§10.1). Found by the §35 numeric fixture: the unparameterised form emitted a `Copy` assertion with a missing generic argument. Two toolchain defects were found with it and fixed: the formatter spaced out the type arguments of a `use` path, having never seen any there, and the fixture silently lost its by-value parameters as a result — the test caught it.
- **Revision 2026-09-12s (v0.3):** the §35 suite assembled — all 23 entries exercised (`examples/compat/`, `docs/COMPATIBILITY.md`). Language: D57, the name-value attribute (§23). Toolchain, all found by building the fixtures: `tests/`, `examples/` and `benches/` are lowered as Cargo targets (§28.1), a relative path dependency is rewritten so it resolves from the generated crate, and §27 follows a macro's expansion chain back to the call site so a diagnostic raised inside a dependency's macro is anchored in Uredo source.
- **Revision 2026-09-12t (v0.3):** `uredo doc` implemented and §26.4 written, with D58 deciding what a fenced block in a `##` comment means: Uredo source, lowered into a doctest, so examples are compiled and run rather than merely rendered.
- **Revision 2026-09-12u (v0.3):** the §36 runtime budget made measurable (§0.1, §36): paired in-process timings of four round-trip pieces with two controls, one of which — the original compiled twice, as two crates — revealed that code placement alone moves a paired benchmark by up to 4%, twice the budget being enforced. No regression is measured in any piece; whether a run certifies depends on what else the machine is doing.
- **Revision 2026-09-13 (v0.3):** no rule changed; the round-trip pipeline was restored so that §0.1's opcode-identity claim is reproducible from the archived sources rather than recorded. The pieces are rebuilt into crates by one shared step (`docs/corpus-study/roundtrip/tools/pieces.py`) that all three tools share.
- **Revision 2026-09-13b (v0.3):** the Phase 3 type-level sweep (§33.1): const generics already worked; generic associated types needed a `where` clause on an associated type declaration (§12.4, new) and pinning needed a written receiver type (D59, §12.3). All three are now exercised in native syntax by `examples/compat/typelevel`, and the phase row says so instead of promising raw Rust.
- **Revision 2026-09-13c (v0.4):** consolidation. Two orderings had drifted as rows and entries were added above their predecessors rather than below: the decision table ran D1–D52 upward and then D59–D53 downward, and the revision history ran forward to 12 September and backward from 13 September. Both are now in one order, oldest first, and `docs/spec_audit.py` fails if either drifts again. It also checks that every cross-reference resolves, that every decision row cites a section that exists, that the sections the *compiler* names in its diagnostics all exist, and that every example still lexes. No rule changed; §0.1 and §38 were brought up to date with D54–D59, the §35 suite and the §36 fixtures.
- **Revision 2026-09-13d (v0.4):** the v0.4 review applied (`oracle-reviews/v04/`, four seats). D60, supertraits, which §21 said used Rust syntax and which no spelling parsed — 57% of the corpus's trait declarations. §35's state paragraph was frozen at the recording pass, contradicting §0.1's "assembled": it now points at the generated record instead of repeating counts that go stale. Two figures corrected: the `@value use` categories are 59% of their total rather than "the mass", and an unenumerated "nine of the eleven defects" is now a countable claim. §12.2 says what a bare field name does when `Self` has an associated item of that name. The cold reader found eleven more, each verified before it was applied: §0.1's compatibility row carried two contradictory states spliced together; §9.3 stated the `&T?` precedence backwards, in the rule D54 exists to settle; §15.2's example did not compile; §26.4 sat above §26.3; §6.1's examples of keywords-as-identifiers named three words §6.2 reserves and the compiler rejects; §43's closing note was a day stale; §3.2's canonical list omitted its one carve-out; §8.1 and §10.3 called two different lists the same one; and four smaller things. Two defects were found by answering a reviewer's *question* rather than by any finding: a supertrait could not be written at all (D60), and neither could an inline `where` clause. `rust::` is recorded as emitting invalid Rust and is not fixed.
- **Revision 2026-09-13e (v0.4):** §38 records what would reverse P8 (D52), the closest call in the document, as two counts over a real project rather than as a sentiment: a mapped E0382 naming a P8 parameter above 1 per 1,000 lines, and `.clone()` written at P8 call sites above the number of `take` annotations the borrow default would have needed. The first is already a distinct branch of the mapper; the second needed a `uredo lint` check that did not exist, which 13f builds. Two defects were found while writing the repository's README, both by running the example rather than by reading it: `str?` and `[T]?` lowered to the unsized `Option<str>`/`Option<[T]>` in a parameter or return type, because §9.7's borrow was applied only when the bare form *was* the whole type (§9.3 now states the composition; it reaches rustc as an error on generated code otherwise), and `take name: T` — the mode written where the name goes — parsed as a pattern parameter and was reported as a lowering defect, blaming the compiler for a source error that now has its own diagnostic. Two published figures were corrected against their own generated records: the §35 suite is 24 entries with 15 complete, not 23 and 14, having grown by the Phase 3 sweep, and the runtime ratios span 0.976–1.024, not 0.974.
- **Revision 2026-09-13f (v0.4):** `uredo lint --measure` (§29.1, §28.1) collects the two counts §38 makes the condition for reversing D52, so the trigger published a day earlier is executable rather than aspirational: the parameters passing by value under P8 — the `take` annotations a borrow default would have cost, one per declaration — and the `.clone()` arguments handed to one of them, each printed with its site and the declaration it fed. It is deliberately not a fourth lint: a clone at a by-value call site may be exactly right, and §29.1's charter is that a finding restates a rule the document already carries. `--measure` therefore prints these at `note` level and in place of the findings, and `Level::Note` is added for the purpose. Over the corpus, the portfolio and the examples — 69 files — the counts are 11 and 0. The 11 was checked by hand against a second count written from the source, which first disagreed at 9 and was wrong twice: a type parameter of an `impl` block is in scope for its methods, and a `Copy`-bounded parameter is known-Copy (P1) rather than P8, so it needed no annotation under either rule and is not counted.
- **Revision 2026-09-13g (v0.4):** the first Uredo code written to work rather than to illustrate a rule: `tools/`, holding the repository's own scripts, beginning with `docs/spec_audit.py` ported to `tools/src/bin/spec-audit.ure`. It is diffed against the Python on the clean document and on a deliberately broken one, output and exit code identical. 165 lines of Uredo against the 287 generated from them — 1,920 tokens against 2,243, −14.4%, or **−11.1%** once D39's absolute paths (`::std::println!`, 21 occurrences, 84 tokens) are discounted, which is the honest figure and still clears §37's threshold; annotations 42 against 79. **No lifetime notation was needed anywhere in it**, which is the first evidence on §38's held question about the apostrophe. Three toolchain defects came out of building it, each with a test: `src/bin/` was lowered as a module directory, so §20.3's automatic declarations wrote `src/bin/mod.rs` and Cargo built a binary called `mod`; files beside the source were not copied into the generated crate, so an `include_str!` could not find its own neighbour and an exported package would have been incomplete; and `CARGO_MANIFEST_DIR` names the generated crate, which is right and cannot be otherwise, but §25 did not say so. §25 gains all three as clauses of the lowering contract, and the §35 target-kinds entry now exercises both `src/bin/` layouts. The six authoring errors are recorded in `tools/README.md`, since what a first user gets wrong is the measurement §37 cannot take from code written by the language's own author to demonstrate the language.
- **Revision 2026-09-13h (v0.4):** the other two tools ported — `docs/compat/record.py` and `corpus/run.py` — so `tools/` now holds 830 lines of Uredo doing the repository's own work. Each is diffed against its Python: `COMPATIBILITY.md` and `RESULTS.md` byte-identical, `results.json` equal field for field once the wall-clock timings are set aside. Tokens 8,299 against the 9,562 generated from them, −13.2%, or **−11.0%** discounting D39's absolute paths — within half a point of the single-file figure and of the corpus's −14.0%. **Annotations are 192 against 282, a ratio of 0.681 against the corpus's 0.552** (both figures as corrected in 13i), and the gap is the finding: this code lives on `Path`, `Command`, `Regex` and `fs`, and D2 makes every argument to a Rust item Rust's to spell, so Uredo saves on its own declarations and nothing else. §37 predicted that shape for framework-style code and it holds for tooling. **One lifetime was written in 830 lines** — a single `'static`, forced by D48 on a `str` return with nothing to elide from, and reported by the mapper as the D48 rule rather than as rustc's E0106. That is the first evidence on §38's held lifetime question, and it points at the narrow case: what a real Uredo program meets is `'static`, which is what all three seats of the 2026-09-11 panel called the one clean win. Four more authoring errors are recorded in `tools/README.md`; no toolchain defect was found by these two, the three from the first port having already been fixed.
- **Revision 2026-09-13i (v0.4):** three more tools ported, chosen for being a different shape from the first three: `metrics.py` (the tokenizer both suites measure with), `ircmp.py` and `irstat.py` (LLVM IR parsing). `tools/` now holds 1,093 lines of Uredo — tokens 10,750 against the 12,453 generated from them, −13.7%, **−11.4%** discounting D39's absolute paths. The tokenizer is verified the hardest of the six: byte-identical output to the Python over **every one of the 174 `.ure` and `.rs` files in the repository**, the compiler's own sources included; the IR tools agree on four real module pairs and on a module against itself, exit codes included.
  **What D2 costs is now counted.** `uredo lint --measure` reports the references written because a callee is a Rust item, which Uredo would have inserted for its own: **50 across `tools/`, against 35 across the corpus, the portfolio and the examples together** — 4.1 per 100 lines against 1.8. That is the whole of the annotation gap (247 vs 352, ratio 0.702, against the corpus's 0.552), and it is the number any future case for reopening D2 has to beat. §37's *gated* measure, signatures only, is untouched at 0.477.
  **Two defects in the measurement itself**, both found by measuring `tools/` rather than by reading the script. The annotation counter read string literals as code, so an English possessive counted as a lifetime and an `&` in a message as a borrow; blanking literals by pairing quotes with a pattern then got `r#"…"#` wrong, whose body may hold a quote of its own, and swallowed whole spans of code. The tokenizer — which already knows every literal form — now decides where a literal ends. The corpus's all-annotations ratio moves from 0.563 to **0.552** and §0.1 says so; its gated figures do not move, since no corpus program contains a raw string or a possessive.
  **One defect in the lexer**: `'\''` ended a character early, the escaped quote being taken for the closing one, and the stray quote then lexed as a lifetime. Every escape form is now tested.
  **Lifetimes written across 1,093 lines: one.** The D48 `'static` on a `str` return. A second, on a `const`, turned out to be unnecessary — §8.3 already gives a `const` of `str` type its `&'static str` — so the only apostrophe real Uredo code has needed is the one position the 2026-09-11 panel unanimously called the one clean win for a `@static` spelling. §38's held item keeps its trigger; the evidence so far says the presence of lifetimes is not what would stop a user, which is what that trigger asks about.
- **Revision 2026-09-13j (v0.4):** the owner's `@static` question put to a five-seat panel (`oracle-reviews/static/`) and **declined**; §38's held item keeps the apostrophe and gains a countable trigger. The owner's own objection — that `@static` beside `'a` would confuse — is what every seat independently concluded, and it argues against the change rather than for extending it to `@a`, which was priced and declined in September. Three findings decided it. A `@static` confined to reference position covers 46.6% of the corpus's `'static` occurrences and leaves 53.4% — bounds and generic arguments — spelled with an apostrophe, so it is a third notation rather than a smaller one. §23's `@` would acquire a positional meaning, and the consistency seat counts four sentences to amend for `@static` against none for the status quo. And no seat could name a language in which one member of a syntactic category takes a notation its siblings do not.
  **Inferring `'static` is rejected outright rather than held**, and that was settled in the shell rather than by argument: the predicate is undecidable from declarations. `fn rest(c: take std::str::Chars) -> str` compiles and runs today because rustc's elision counts the lifetime inside `Chars<'a>` — a foreign type whose parameters §5.2 forbids Uredo to resolve. The adverse seat supplied two further counterexamples, both verified with rustc: a literal can be returned under a shorter lifetime, and inferring `'static` on a generic slice return silently strengthens the API to `T: 'static`. What the exercise also established is that **Uredo does not decide this lifetime at all** — it emits `&str` and rustc's elision decides; D48 describes when the author will meet rustc's error, and is not a test the compiler runs.
  **An error in the panel's own prior record, corrected.** The 2026-09-11 synthesis claimed 65% of reference annotations are `'static` or `'_` rather than named. In reference position the counts give 38.4% (1,992 + 113 against 3,370); the recompute seat could find no denominator yielding 65%, the adverse seat reached 38.4% independently, and the review seat named that figure as the one thing in the dossier a reader would wrongly take on trust. `ANALYSE_lifetimes.md` says so now. The sample is also too small to decide anything: 1,093 lines cannot separate one lifetime per 1,000 lines from one per 300, and about 2,300 would be needed — which is what the reopen trigger asks for.
- **Revision 2026-09-13k (v0.4):** the port finished. `pieces.py`, `difftest.py`, `perf.py` and `runtime.py` join the six, so `tools/` is **1,786 lines of Uredo** — ten binaries and a shared library module — and **no Python remains in the repository's measurement path**. Each is diffed against the script it replaces: `PERF.md`, `perf.json` and `size_baseline.json` byte-identical, Python's one-space JSON indent and `\u00a7` escaping included; the reconstructed piece trees byte-identical; `difftest`'s generated manifest and output identical with 200,000 inputs per piece agreeing; `RUNTIME.md` identical on the deterministic `--report-only` path. Only `runtime`'s measuring path cannot be byte-compared, because its bound is a bootstrap and Python's Mersenne Twister is not worth reproducing; the point estimate it reports is a plain median with no randomness in it. Porting it found a function dead in the original — `t_quantile` and its table, unused since the bound became a bootstrap.
  Tokens 17,972 against the 20,743 generated from them, **−13.4%**, or −11.0% discounting D39's absolute paths. **D2's bill grows with the foreign-API surface**: 102 references written because the callee is a Rust item, against 35 across the corpus, the portfolio and the examples together — 5.7 per 100 lines against 1.8 — and that is the whole of the annotation gap (0.736 against the corpus's 0.571).
  **A published figure is corrected, and it is the headline one.** The tokenizer cut a `.ure` line at the `#` of a raw string, so corpus program 08's embedded JSON was counted as code on the Uredo side and as a single token on the Rust side. The corpus's token reduction is **−13.0%** over all twenty and **−13.4%** on programs 1–15, not −14.0% / −14.6%; the all-annotations ratio is 0.571, not 0.552. §37's gated thresholds are unaffected — all four still pass, and the signature measure never used that tokenizer. §0.1 carries the correction.
  **Four defects in the toolchain**, each with a test. A Rust keyword naming a *field* was escaped at the access but not at the declaration or the struct literal, so `gen` — reserved in Rust 2024 — reached rustc bare. `crate`, `self`, `Self` and `super` naming a binding produced uncompilable Rust with no Uredo diagnostic; §6.1 said they cannot be raw but not what happens when one is used. A diagnostic on an import named the wrong import, because rustfmt sorts `use` declarations and the provenance map aligns formatted to raw by walking tokens, which cannot follow a permutation. And the worst of the four: **`uredo fmt` turned a compiling program into one that does not parse**, by applying the over-wide-*statement* expansion to a continuation line inside a call — `if ok: "a" else: "b")` was broken after the header colon. A statement's brackets balance and a continuation's do not, which is the test now.
  **Lifetimes written: four in 1,786 lines, every one `'static`, every one D48.** 2.2 per 1,000 lines against the 3 per 1,000 the §38 trigger names — approaching it, still under it, and still short of the 2,300-line sample. No named lifetime has been needed yet.
  The §36 runtime budget was re-measured with the ported tool: **2 of 4 pieces pass, 2 inconclusive** (against 1 and 3 before), at a load average of 4.95 raised by this session's own builds. The twin control breached on both inconclusive pieces, as it did before — the floor is the method, not the machine.
- **Revision 2026-09-13l (v0.4):** two loops closed, and the closing of the first opened a third defect. **The formatter's property test only ever saw the portfolio** — 25 snippets written to be canonical and short, which never make it wrap a line, while the rest of the repository holds 182 lines over the limit. That is why `uredo fmt` could turn a program into text that does not parse and nothing noticed. The test now walks every `.ure` file in the repository and asserts the whole contract: a program that compiled must still compile, must generate the same Rust, and formatting twice must equal formatting once. Widening it alone was not enough, and the check was tested against its own known-failing case to find that out: with the fix reverted the widened test still passed, because the triggering shape had been edited out of the only file that had it. `compiler/tests/fixtures/fmt_hard/` now keeps those shapes on purpose, and with it the widened test does catch the original defect.
  **It caught a second one immediately.** A `match` arm is the one inline body whose lowering depends on how it was written — §13.1 gives `pat: expr` a bare expression and an indented body a block — so expanding an over-wide arm changed the generated Rust. The formatter now never expands an arm; §29 states both exclusions and the contract they serve.
  **The §38 lifetime trigger produces its own number.** `uredo lint --measure` counts written lifetimes by position — reference, bound, generic argument or binder, loop label — with `'static` and `'_` counted out of the total, in the corpus study's own buckets so the two can be set side by side. They are counted from the token stream, so an apostrophe in a comment, a string or a char literal is not one. Over `tools/`: **4 lifetimes, all `'static`, all in reference position, 1.9 per 1,000 lines over 2,137 non-blank lines** — under §38's threshold of 3, and 163 lines short of the 2,300-line sample the trigger asks for, which the command says rather than leaving to be worked out.
- **Revision 2026-09-13m (v0.4):** `uredo publish` implemented (§4.6, §28.1, §0.1), leaving `lsp` and `fix` as the only commands §28.1 names and does not have. It is `uredo package` and `cargo publish` in one step, and it earns its place over running the two by hand with the check Uredo owes before something irreversible: a published crate's public API is a promise (§4.4, D14), so an API that no longer matches `uredo-api.json` stops the command with the diff, and a package with no recorded manifest stops it sooner — publishing one would make the promise by accident. `--allow-api-change` overrides the first and says in the note what it is for. Everything else is Cargo's: flags forwarded unchanged, `--dry-run` included, credentials never reaching Uredo.
  Exercised to a `--dry-run` on a package built for the test, through the whole sequence — refused with no manifest, refused on an unreviewed addition with the item named, accepted once reviewed — and Cargo then packages the export, compiles it and stops at the upload. The test also checks what Cargo was handed: generated Rust, no `.ure`, no provenance directory. **Nothing has been published to crates.io**, and the upload path is therefore the one thing here that has never run.
- **Revision 2026-09-13n (v0.4):** `uredo fix` and `uredo lsp` implemented, so **every command §28.1 names now exists** (§0.1). Two carry a stated limit rather than a gap, and saying which is the point of the row.
  **`fix` applies only what it can prove changed nothing.** §28.2 promises the command for migrating source across a breaking syntax change; the surface is frozen and none has happened, so there are no migrations to run. What it does instead is apply the findings whose rewrite is invisible in the output, and the contract is *checked* rather than argued: the file is lowered before and after each edit, and an edit that moves a byte of generated Rust is put back and reported as refused. `redundant_take` is the only finding that qualifies — that lint's whole claim is that the annotation changes no signature — while `borrowed_container` and `copy_candidate` change what a function accepts and stay suggestions. Over the whole repository `fix` finds exactly one thing to do, and it is the *deliberate* illustration in portfolio topic 05: the command cannot tell a deliberate finding from an oversight, which is the gap §29.1 already holds open for source-level suppression, now with a second reason to close it.
  **`lsp` shares the compiler's front end rather than a second one**, which is the property §28.2 asks for: the diagnostics an editor shows are the ones `uredo check` prints, on the same Uredo lines, and hover is what `uredo explain` says about that line. It speaks `initialize`, `shutdown`/`exit`, full-text sync, `publishDiagnostics` for errors, warnings and lints, `textDocument/formatting` and `textDocument/hover`, and answers every request carrying an `id` even when the answer is nothing, since one left unanswered hangs the editor. Five tests drive it the way an editor does — `Content-Length` framing over stdio, not the functions behind it — including the two exit codes the protocol distinguishes. **What it does not have is the tolerant, lossless parse §28.2 also asks for**: on a file that does not parse it reports the parse errors and offers nothing else, where a tolerant parser would keep answering from the last good tree. That is the next thing an editor user would notice, and it is written down rather than discovered.
- **Revision 2026-09-13o (v0.4):** the debugger helpers (D29, §28.2) written and run, and the language server given the cheap half of §28.2's tolerance. Writing the helpers found the provenance map wrong on **every multi-line construct**, which is the finding rather than the helpers.
  A `match` is built as one string and emitted through one call, so every line of it was attributed to whatever `cur_line` had reached — its *last* arm. A debugger stopping on the header of a five-arm match was told it came from the bottom of the block. The same held for an `if` and for every loop: the header and the closing brace took the line of the last statement in the body. Each construct now marks its own lines, the header and its braces to the header's line, so `ubreak src/main.ure:12` lands on the arm that line names and `uwhere` reads back the line the frame is actually on. None of this changed a byte of generated Rust — the markers are stripped before output, and the golden tests say so — which is exactly why it survived this long.
  **What exists:** `ubreak FILE.ure:LINE`, `uwhere` and `ulist`, for GDB and LLDB, with `launch.json` recipes for both VS Code adapters and a README saying what they do not do. The GDB script is exercised against a real program under a real debugger, breakpoint, backtrace and listing asserted; **the LLDB script has never been run**, no LLDB being installed here, and both the script and §0.1 say so. The editor's source view still shows the generated Rust: changing that needs a debug adapter of Uredo's own, which §28.2 does not ask for.
  **The server keeps answering through a broken keystroke.** §28.2 wants a tolerant, lossless parse; the half that costs nothing is remembering the last text that compiled. Hover falls back to it — but only for a line whose text is unchanged since, because an answer about a line the author has rewritten is worse than none, and the answer says it is stale. Formatting still refuses a file that does not compile, as the formatter itself does. The tolerant parser remains owed.
- **Revision 2026-09-13p (v0.4):** §35's oldest unproved claim closed. The wasm entry had said, since it was written, that nothing called the module from JavaScript: it compiled, linked and passed a host test, which proves the attributes lower and the code works but not that an engine can reach it. It is now **run in Node** — `Counter::new(1)`, `bump(2)` and `bump(40)` returning 3 and 43, and `greet("wasm")` returning `"hello, wasm"` across the boundary — agreeing with the host test that was the only evidence before.
  The *pre-bindgen* module is called directly, so the test needs no `wasm-bindgen-cli`: the artefact `uredo build --target wasm32-unknown-unknown` already produces is instantiated with stubbed imports and its exports called by name. The cost is that `js/run.mjs` hand-writes wasm-bindgen's calling convention — the hash-suffixed export names, a `&str` as a (pointer, length) pair in the module's memory, a returned `String` through a caller-owned return area — rather than exercising the glue that would generate it. That is the narrower gap the entry now records, with the other one beside it: Node is an engine, not a browser, and the imported `console.log` is stubbed rather than observed.
- **Revision 2026-09-13q (v0.4):** the tolerant half of §28.2's parser, which turned out to be mostly a matter of not throwing away what the parser already recovers. It resynchronises at item and statement boundaries already; three things were losing that work.
  **An unclosed bracket swallowed the rest of the file.** Every line after it is a continuation, so one missing `)` cost every item below, and the error landed on the header of the next function rather than on the bracket. No valid program has an item at column 0 inside brackets, so a line that starts one now closes the run and the diagnostic points at the bracket where it opened. The same rule ends a trailing continuation — a line ending in `=` cannot be continued by `fn`.
  **A broken item header made every line of its body a module-level error.** The body is skipped with the header now, so one mistake reports once.
  Measured on four shapes — a typo mid-body, an unclosed bracket, a half-typed line and a broken header — across three items each: before, 2 to 3 items survived and one shape reported three errors for one mistake; after, **every recoverable item survives and each shape reports one error**, the broken item itself being the only thing lost.
  **The server answers from what parsed.** `compile_tolerant` lowers a partial module — never written anywhere, and unformatted by construction — so hover on a file with an error elsewhere is *current* rather than stale, and says the file is broken somewhere else. The last-good fallback stays for the case even that cannot reach: when the broken item is the one being asked about, and then only for a line unchanged since. Both paths are tested through the wire.
  What is still owed is the **lossless** half. The parser keeps no trivia and no error nodes, so it cannot reconstruct the source it was given; renames and code actions need that and are out until it exists. §28.2 says so rather than leaving `tolerant, lossless` to read as one thing.
- **Revision 2026-09-13r (v0.4):** three items closed, and a recomputation that found a fourth.
  **The §36 runtime budget is retired, on its own controls' evidence.** The twin control compiles the original's own source as a second crate, so its ratio is 1.0 by construction; its *median* was 1.0248 on `p5_sanitize`, a systematic 2.5% — larger than the 2% budget, and larger than that piece's own reading of 1.0189. A +1.9% measurement from an instrument that reads +2.5% on a null difference is not evidence of anything. That is bias, not noise: the machine's own control resolved 2% on 4 pieces of 4 and the cross-crate control on 2, and samples tighten the first while leaving the second, code placement not being random. The report showed only the bounds, which cannot tell bias from noise, and now shows the medians too. Instruction identity stands in its place and is stronger: 191 functions of 191 compile to the same sequence, which is what a wall-clock budget was a proxy for.
  **`no_std` reaches the allocating half.** `Vec` and `String` through `alloc`, written exactly as they are with `std`, and the `format` intrinsic lowering to `::alloc::format!` because the module said `@!no_std` and §11.3's table now reads that; `panic` lowers to `::core::panic!`, and `print` and `eprint` are refused in Uredo terms — a `no_std` target has no standard output — rather than emitted for rustc to reject on generated code. What the entry still records as missing is the target story rather than the language: the crate is built for the host, so the allocator a real bare-metal binary must supply is never written.
  **Every measured figure §0.1 publishes was recomputed from the artefacts**, 27 of them, and the derived arithmetic put to a seat that did not produce it (all 15 confirmed). One disagreement was real and it was not in §0.1: **`corpus/results.json` and `corpus/RESULTS.md`, the two halves of one run, had fallen a whole correction apart** — the JSON still said −14.0% where the Markdown said −13.0%. The cause was mine and mechanical: the JSON carried a wall-clock field that changed on every run, so regenerating it always showed a diff, so it kept being reverted, and the reverts took the corrected counts with them. The field is gone — nothing read it, §36 is where timing lives — which makes that drift unrepresentable rather than merely fixed. Comparing the *parsed* JSON had hidden a second divergence: the two implementations of the runner disagreed on non-ASCII escaping, so whichever ran last owned the committed bytes. Both now write Python's escaping and the records are byte-identical from either. `tools/src/bin/status-check.ure` recomputes the lot and fails when two records of one measurement disagree.
- **Revision 2026-09-14 (v0.4):** the last two Cargo target kinds that were *lowered by the same rule and never run*. Both were the same shape of gap — a code path that exists, that nothing had executed — which is the shape this project keeps finding defects in.
  **A bench.** No `benches/` directory existed anywhere in the repository, so §35's claim that benches are lowered like tests and examples had never been tested. `examples/compat/targets` now carries one: on stable there is no `#[bench]`, so a bench target is a plain `main` with the harness switched off, and `cargo bench` runs it against the library by name. That package is now every target kind at once — lib, bin, two under `src/bin/`, integration test, example, bench — and its entry has nothing left in its missing list.
  **A target with no operating system.** The `no_std` entry's language half closed yesterday; its *target* half said the crate is built for the host, so the allocator a bare-metal binary must supply is never written. It is written now: a bump allocator over an 8 KiB static arena and a `#[panic_handler]`, both in `rust { }` because a lang item and an unsafe trait over raw pointers are Rust's, both `#[cfg(not(test))]` so the host tests keep `std`'s. The crate builds for `thumbv7em-none-eabihf`, and because an rlib defers everything the test also links a **staticlib** of the generated crate, which forces rustc to find both — `__rust_alloc` is asserted present in the archive. What the entry still records is honest and smaller: the allocator frees nothing, and no hardware or emulator executes what was linked.
  §35 is now 24 entries, all exercised, **16 with nothing missing**.
- **Revision 2026-09-14b (v0.4):** the two remaining "never actually run" claims retired by getting hold of the tools they needed.
  **The LLDB helpers ran.** They had been written blind, with no LLDB on this machine, and §0.1 said so rather than implying otherwise. Ubuntu's packages extract without privileges — `apt-get download` then `dpkg-deb -x`, every dependency but `liblldb` already present — so LLDB 18.1.3 was available in a user directory in about a minute, and the script **worked unchanged on its first real session**: `ubreak src/main.ure:12` resolved to `main.rs:14`, the backtrace and the listing read in `.ure` terms. One difference from GDB showed up and is worth having found: LLDB's stack walk returns the whole stack, so `uwhere` printed seventeen Rust runtime frames that GDB's walk stops before. Both scripts now end at the outermost frame with a Uredo location and say how many they dropped, with `uwhere all` for the lot. Recorded in §0.1 with the skip condition, since a test that silently does nothing on a machine without the debugger is not a test.
  **wasm-bindgen's generated glue is exercised.** The entry called the pre-bindgen artefact directly, which needed no tool and hand-wrote wasm-bindgen's ABI — so it proved a JavaScript engine can reach the module and nothing about the glue a real caller uses. With a `wasm-bindgen-cli` matching the fixture's 0.2.128, `--target nodejs` generates that glue and Node calls it: `Counter` is a class, its `@wasm_bindgen(constructor)` its constructor, `bump` mutates the instance behind `self: inout`, and `greet` takes and returns ordinary JavaScript strings — including a non-ASCII one, which crosses the boundary intact through the glue's own encoding rather than ours. Both paths are kept: the raw one needs no tool, the glue one is what a user meets.
  What the wasm entry still records is one line: Node is an engine, not a browser.
- **Revision 2026-09-14c (v0.4):** a licence, and a manual — two of the three things the owner named as coming before crates.io.

- **Revision 2026-09-14d (v0.4):** the source extension, settled rather than left to taste. `.ur` was proposed for being two characters; the panel (`docs/oracle-reviews/extension/`) reached **keep `.ure`** unanimously, and overturned the orchestrator's reason for it on the way. The claim was "Linguist does not reassign an extension, so `.ur` cannot work"; the adverse seat produced Elixir and Euphoria, which *share* `.ex` under a content heuristic, and that was verified against the live `languages.yml` and `heuristics.yml` rather than taken on trust. What survived the correction is sharper and comes from the same file: Linguist requires 2,000 indexed public files before it will take a language at all, so Uredo cannot be recognised under **either** extension for years — during which `.ure` is merely unrecognised while `.ur` is actively rendered as UrWeb, a cost every *user* would inherit and each would have to mitigate in their own repository. Two of the theoretical failure modes were then tested and found not to exist: `file(1)` and shared-mime-info return `ASCII text` and `text/plain` for `.ure`, `.ur` and `.uredo` alike. D61 records the decision so it is not reopened on taste. Three numbers were wrong before they were published and the recompute rule caught all three: "242 two-letter extensions in Linguist" was a count of lines (it is 172 distinct, claimed 241 times); the blast radius was given as "~34 code sites" from a grep scoped to four directories (it is **164 lines across 26 files**, plus 21 lines of Uredo tooling); and the provenance map's `ure` field was counted as a cost when it holds a *path* and would not change — a correction both external seats made independently.

- **Revision 2026-09-14e (v0.4):** the manual, reformatted for the page rather than the terminal. Every code line now fits in 72 characters: trailing comments moved to the line above the code they annotate, multi-line struct and array literals opened out, a method chain broken on its leading dots — all of which the continuation rules already accepted, and all still compiled by `compiler/tests/manual.rs`. Eight passages that had been run-on sentences of parallel items are lists — the receiver rule, the three binding spellings, what a `throws` signature means, the three debugger commands, the layout rules, the three things visible in the first program, assignment against binding, and the six non-goals of §39. They were found by scanning every prose paragraph for the shape rather than by reading: two or more sentences opening with a code span, or three or more short parallel ones. What the scan returns now is prose. Keywords are bold where prose names one — an inline span every word of which is a keyword, so `pub(crate)` and `self: take` are bold while `take String` and `pub mod name` are not: those carry a type or a metavariable, and bolding them would bold something that is not a keyword. Tables, headings and spans already inside a bold run are left alone, and a real `SourceCodePro-Bold` is embedded rather than a synthesised one; the toolchain listing is a table, which also recovered a description that had lost its object (`uredo lint --measure` read "counts the specification's own triggers need"). In the PDF, no listing may now break across a page: the longest is 26 lines and fits with room to spare, and a two-line widow of code stranded on the next page reads as a separate example.

- **Revision 2026-09-15 (v0.4):** the elision question, asked openly — *what else could be made optional where it is inferable without ambiguity?* — and answered by a panel (`docs/oracle-reviews/elision/`) that overturned the framing it was given. The dossier conflated three claims: that information cannot be inferred, that the current grammar requires a token, and that removing a token would weaken a convention. Only the first follows from §5.2 and §4.4, and **both external seats found the conflation independently**. The specific instance is the sharper finding: `var` had been closed by citing "nothing body-dependent", but §4.4 defines that principle over **signatures**, and the mutability of a local binding appears in none — and the same sentence names *"the fixed tables the release ships"* as a legitimate input, so a table of std receiver modes is the mechanism this design already uses rather than something foreign to it. Re-measured on that basis, **77% of `var` bindings would be decidable, not 59%**, and the residual fails loudly as a rustc error rather than silently. `var` stays, on the reader's behalf rather than on an impossibility. D62 records what was declined and at what price. Two candidates that had never been examined came out of the adverse seat — omitting `->` while keeping the return type, and omitting `fn` in declaration position — and were priced with the project's own tokenizer at 0.72% and 0.97% of real Uredo's 30,158 tokens. A proposal of the orchestrator's own, a crate-level `@!default_derive`, was **withdrawn**: `@!default_error` completes an effect the function still declares with `throws`, whereas a default derive would make an unmarked type acquire capabilities, and `Clone` is written on only 30 of 99 declarations, so there is no default to compress. Two numbers were corrected before publication: 17 annotated bindings were tested by removing them and rebuilding, and **11 are required by rustc**, so annotations were closed more firmly than claimed; and the first lifetime count matched the char literal `'a'` as the lifetime `'a`. §38's lifetime trigger is re-measured over `tools/`, now 2,025 lines rather than the 1,093 cited: **4 written lifetimes, all `'static`, 1.98 per 1,000. It has not fired.** The standing limitation, from the review seat and recorded rather than answered: every measurement here is over code written by this language's author. Following that seat's counter-proposal — delete rather than make optional — was done the same day and is §7 of the analysis: **almost nothing would be deleted.** Shadowing is used **once in 3,161 lines** (the dossier's 36 counted `let` inside `rust { }` blocks, which is Rust's own), and the keyword cannot go regardless because `let PATTERN = … else:` needs it (D33). The turbofish cannot be replaced by an annotated binding: **17 of 22 are mid-chain and 1 of 22 is bindable**, the same argument §38.11 used for postfix `?` at 3.8%, here at 77%. Two reopen conditions were measured and neither fired: `@value use` wants `take` annotations clustered on a small set of *foreign* types, and they are spread over ~17 types with the top three at 32% and a *project* type at the head; and a non-`pub`-scoped `@!default_derive` has no default to compress, since the most common derive set on the 82 non-`pub` declarations is **the empty set (28%)** and only 47% carry `Debug`. The seat was right about the method on that last one, and the specification is where it was right: §38.5 already restricts `@!default_error` to non-`pub` items for the very reason the adverse seat gave against a default derive, and that precedent went unchecked before the proposal was withdrawn. It stays withdrawn on the measurement instead.

- **Revision 2026-09-15b (v0.4):** the first three release-gate items (`docs/oracle-reviews/release/`), all of them found by *being* a first user rather than by reading. **A missing `rustfmt` was reported as a compiler bug.** `uredo new hello && uredo run hello` — the two commands the tool itself suggests — ended in "lowering defect: the generated Rust did not parse (§5.4) … this is a compiler bug; report it", when the truth was that `rustfmt` was not on the PATH and nothing had been parsed. rustup ships rustfmt in its `default` profile but not in `minimal`, which rustup recommends *for CI*, so this was the first thing an unattended `cargo install` would have met. `rustfmt()` now returns a typed `FmtFailure`: `NotInstalled` is the user's toolchain and says how to fix it, `Rejected` keeps §5.4's wording for the case it was written for. `compiler/tests/toolchain.rs` strips every rustfmt-bearing directory from `PATH`, asserts the honest diagnostic and the absence of "compiler bug", and carries a positive control (§5.3) that the stripped `PATH` really has no rustfmt on it; reverting the fix makes it fail. A related blemish went with it: a whole-file diagnostic printed `file:0:0` and drew a caret under line 1, which pointed at source that had nothing to do with it — line 0 now means *no location* and prints the file alone. **The crate declared `MIT OR Apache-2.0` and shipped neither licence text**, which is exactly what `uredo publish` refuses a package for (§28.1). Both are now in the crate as symlinks to the repository's own copies, which `cargo package` dereferences into the archive, so the text ships without a second copy that could drift. The manifest gains `repository`, `readme`, `keywords`, `categories` and `rust-version = "1.85"` — the last verified with `cargo +1.85 check --all-targets` rather than asserted, and the three categories checked against crates.io's own `/api/v1/categories`. `compiler/tests/packaging.rs` asserts all of it, including that the crate's licence files agree byte for byte with the repository root's. 167 tests. **Nothing has been published.**

- **Revision 2026-09-15c (v0.4):** **§37's figures measured on a program ten times the corpus's size, and they do not hold.** `examples/restdemo` now has an idiomatic Rust twin — same modules, same structure, the same eight integration tests passing on both sides, written fresh rather than edited down from the generated Rust — and the comparison over library sources with the project's own tokenizer reads **tokens −10.4%** against the corpus's −13.0%, **lines −18.7%** against −35.3%, **non-whitespace characters −4.0%** against −8.1%, and **annotations 0.94 of Rust's** against the corpus's signature ratio of 0.48. The function count is 45 on both sides, which is the evidence that the twin is fair. The §37 gate is still cleared, by half a point. The reason is visible in the per-module split: a service is dominated by `use` lines, `serde` derives, generic bounds and arguments to foreign crates, all of which Uredo spells exactly as Rust does — D2 in particular passes foreign arguments verbatim — while the corpus's twenty programs are application-style, which is where the layout saves most. **The corpus figures do not generalise as stated, and the corpus is the smaller and easier sample.** This was the stop condition the demo's design note wrote down in advance: a result materially worse than the corpus's gets published rather than buried. §0.1's headline numbers are unchanged because they are the corpus's own and correctly described as such; what changes is that a second, larger measurement now stands beside them and disagrees. The first version of that table was measured with the Rust twin using a `macro_rules!` for its fourteen `impl Field for …` lines while the Uredo side wrote them out, which was recorded as a fairness note; both sides now use one, and it moved the token figure by 0.2 points — the **wrong** way for Uredo, since a brace-free one-line `impl` is already cheap and the macro declaration costs more than the lines it replaces. **`macro_rules!` has worked in Uredo since the lexer was written and the manual has never mentioned it**, which is why the demo's author reached for one in Rust and not in Uredo: the specification documents it only as lexer behaviour, three files in the repository use it, and none of them says it is a tool a reader may pick up. §14 of the manual now does, along with the limit that matters — a macro body is Rust (§22.5), so it generates Rust items and can never factor out repeated *Uredo* syntax. **Splitting the tokens says where the saving comes from and where it stops: 97% of the gap is punctuation, and identifiers are −0.8%.** The names in an Uredo program are Rust's — its types, methods and crate paths — so the language's whole lever on a program of this shape is braces, semicolons, `Result<…>` and the `Ok(…)` wrapper, and it has already been pulled. The corollary was tested rather than assumed: one `type Reply = Response<Full<Bytes>>` over ten call sites removed 50 tokens from the Uredo side and **51 from the Rust side**, moving the ratio by a tenth of a point. Identifier savings are language-neutral and shrink both sides equally. The one lever that is Uredo's alone is a bare `throws`, which names no error type where Rust writes `Result<T, E>` every time — and it is **net-negative in a multi-module crate**, because `@!default_error` does not reach submodules: nine declarations at about ten tokens each to save the eleven spellings not on `pub` items. Whether a crate-root declaration should propagate to child modules looked like an open design question and was not one: **§15.1's own example is annotated "crate root; a module may re-declare for its subtree", so the document had decided it and the implementation did not do it.** A conformance defect, not a question, and calling it open here was wrong. Fixed the same day — see the next entry.

- **Revision 2026-09-15d (v0.4):** **`@!default_error` never reached a child module, and §15.1 had always said it should.** Its example is annotated *"crate root; a module may re-declare for its subtree"*, and the implementation seeded each file only from its own inner attributes, so the specification's own example did not compile across a module boundary. Asked to put the question to a panel, the right answer was that it is not a question: the document decided it and the code disagreed. The previous entry's framing of it as open, and its claim that §15.1 and §4.4 disagree, are both withdrawn. The crate root's declaration now travels in `CrateIndex`, which is where a crate-wide fact belongs. Two consequences came out of fixing it, each with its own test and each a defect the fix would otherwise have introduced: the declared type must be **nameable from every module it reaches**, so a crate-root default is written `crate::AppError` rather than `AppError`; and a package's lib and bin are **two crates behind one index**, so the library's default leaked into the binary, where `crate::` names something else — a crate root now takes only its own. A third followed from neither: **`uredo lint` compiled every file with an empty index**, so it rejected a bare `throws` that `uredo check` accepts, and more generally could see nothing any other file in the crate declares. Per-file commands now locate the enclosing package and lint against its index. As a token lever the whole thing is worth 2 tokens on `examples/restdemo`, which is the honest measure of it: it was worth fixing because the code disagreed with the document, not because of what it saves. 175 tests.
  **Licensed MIT OR Apache-2.0**, the Rust ecosystem's convention, with both texts in the repository and the field added to every one of the 40 manifests. The reason for two rather than one is in the README: Apache-2.0 carries an express patent grant MIT is silent about, while its patent-retaliation terms make it incompatible with GPLv2, which MIT is not — offering both lets a user take whichever their situation needs. `uredo publish` now refuses a package with no licence, because crates.io requires one and `cargo publish --dry-run` only *warns*, so without the check the failure would arrive after the upload had begun. Its pre-flight reports every problem at once rather than one per run.
  **`docs/MANUAL.md` teaches the language**, which nothing did: this document says in §0 that it is a record rather than an introduction, and it meant it. Nineteen chapters — install, reading the generated Rust, the passing modes at length, optionals, errors, traits, iterators, interop, the toolchain, debugging — ending with the mistakes its author actually made, taken from the 24 recorded while writing `tools/`, not invented.
  **Its examples are type-checked, and that caught two of them.** The first check only lowered each block, which is what this document's own audit does; breaking an example on purpose showed that a `String` parameter returned by value passes such a check and fails rustc. With rustc in the loop, two manual examples turned out not to compile — both the same rule, a return borrowing from one of *two* borrowed parameters, which is precisely what the chapter around them teaches. The audit's wording is corrected too: **60 examples lex and 24 lower to Rust that parses**, which is not the same as compiling, and it had said "compiled as written" since it was written.

---

## 1. Vision

Uredo is a programming language designed to make Rust-level safety, performance, and ecosystem access available with substantially less day-to-day syntactic overhead.

Its central promise is:

> **Simple code should be simple to write. Expensive or dangerous operations should remain explicit. Rust should always remain available.**

Uredo is not intended to replace Rust's execution model, compiler backend, package ecosystem, or low-level capabilities. Uredo provides a simpler source language that compiles to ordinary Rust and uses Cargo and rustc as first-class parts of the toolchain.

The language follows four non-negotiable requirements:

1. **Remove notation, not semantics.** Uredo removes Rust punctuation and boilerplate; it does not remove ownership, borrowing, or explicit cost.
2. **Remain fully compatible with the Rust ecosystem at source/build level.**
3. **Never sacrifice execution speed for syntactic simplicity.**
4. **Allow direct Rust code whenever the programmer wants or needs it.**

### 1.1 Who Uredo is for

The **primary persona** is a developer or team that accepts Rust's ownership model and wants to write less notation: fewer `&`, `Ok(...)`, `Result<T, E>`, `let`, braces, semicolons and `impl` blocks. Uredo is designed, documented and diagnosed for that reader.

Uredo is **approachable, not ownership-free**. A programmer new to Rust meets ownership at the first borrow error; Uredo's contribution is that the diagnostic explains it in Uredo terms (§27) rather than that the concept disappears. Claims in this document are held to the primary persona.

A useful mental model is:

```text
Python / Go-like surface simplicity
            +
Rust ownership, type safety, ecosystem and performance
            +
progressive access to exact Rust
```

---

## 2. Name: Uredo

*Non-normative: this section, §41 and §42 record how the name was chosen and what the prior art was; they carry no rules.*

The name relates to Rust's own naming history. Graydon Hoare, asked about the name on IRC, said: "I think I named it after fungi. rusts are amazing creatures." (verbatim, see §42). *Uredo* is a genus of rust fungi (order Pucciniales), and in older botanical usage the name of the urediniospore stage of the rust life cycle.

```text
Rust  -> rust fungi
Uredo -> a rust-fungus genus / the uredinial stage
```

Two facts to weigh before a public launch, both from primary dictionary sources (§42): the Latin *ūrēdō* also means "blight, burning itch, nettle-rash", and the "life-cycle stage" reading invites the joke that Uredo is a phase Rust outgrows. The codename is not the launch decision.

Provisional conventions:

```text
Language:        Uredo
CLI:             uredo
Source files:    .ure
Formatter:       uredo fmt
Language server: uredo lsp
```

Name availability (checked 2026-09-10): `uredo` is unregistered on crates.io and PyPI. The Rust Foundation trademark policy expressly permits stating accurately that software "is compatible with the Rust programming language" (§42); Uredo's compatibility claim stays descriptive and secondary to its own name. Trademark, registry, repository and domain clearance still precede launch.

---

## 3. Design philosophy

### 3.1 Hide syntax, not semantics

Uredo may infer machinery that Rust requires the programmer to spell out, but everything inferred must be inspectable (§26).

```uredo
fn display(user: User):
    print(user.name)
```

generates:

```rust
fn display(user: &User) {
    println!("{}", user.name);
}
```

The Uredo programmer does not write `&`, and `uredo explain` reports:

```text
parameter: user
passing mode: shared borrow (rule P3 of the passing-mode table, §10.2: non-scalar type, no modifier)
Rust type: &User
inserted by Uredo: borrow
```

### 3.2 Zero-cost simplification — the canonical list

A Uredo convenience is acceptable only when it resolves at compile time without introducing a runtime cost the programmer did not write.

The compiler **never inserts** any of the following. If one is required, the source must make it visible or the compiler rejects the program:

- `.clone()` or any non-trivial copy
- heap allocation (`Box`, `Vec`, `String`, `format!`, …)
- reference counting (`Rc`, `Arc`)
- garbage collection or any runtime ownership mechanism
- locking, atomics, channels or any synchronization
- dynamic dispatch (`dyn`, vtables)
- runtime reflection
- serialization
- system calls, network or file I/O

This is the single canonical list. §26, §32 and §39 refer to it and do not restate it. It has one carve-out, stated where it applies: an array literal bound to a `Vec`-annotated name emits `Vec::from` (§9.4), which is the allocation the annotation asks for rather than one the compiler chose.

The promise covers what **Uredo introduces**. It does not claim knowledge of what a callee does: `process(&data)` may allocate inside `process`. `uredo explain` distinguishes the two (§26.2).

### 3.3 Progressive explicitness

| Level | Typical concepts |
|---|---|
| **Simple Uredo** | bindings, functions, structs, enums, `match`, loops, `T?`, `throws`/`?`, methods |
| **Explicit Uredo** | `take`, `inout`, `let` shadowing, explicit references and lifetimes, generics, traits, `Vec::from`, `format`, `move`, `@copy use`, macros |
| **Rust** | exact Rust syntax in `rust { }`, macros, `unsafe`, FFI, low-level optimization |

These levels describe the programmer's experience. They are not the implementation phases (§34).

### 3.4 Preserve Rust vocabulary; rename only on purpose

Uredo does not rename a Rust concept merely to look different. Unchanged:

```text
struct enum trait impl match if else for while loop
async await unsafe const static use pub mod as move
bool char str String u8..u128 usize i8..i128 isize f32 f64
Vec HashMap HashSet Option Some None Result Box
```

Renamed or replaced deliberately — this is the spelling comparison, not the list of every difference; the elaborations Uredo performs are §0's decisions and §10's modes:

| Rust | Uredo | Reason |
|---|---|---|
| `let` / `let mut` | bare binding / `var` (`let` kept, for shadowing) | the most frequent tokens in ordinary code |
| `Result<T, E>`, `Ok(..)`, `Err(..)` | `throws E`, plain `return`, `throw` (`?` and `.await` stay Rust's) | error plumbing is where Rust notation is densest |
| `println!` / `eprintln!` / `format!` / `panic!` | `print` / `eprint` / `format` / `panic` | the four intrinsics get ordinary call spellings; other macros are invoked directly (D23) |
| `#[attr]` | `@attr` | `#` is the comment character |
| `\|x\| expr` | `x => expr` | closure noise |
| `Option<T>` | `T?` (sugar; `Option<T>` still valid) | frequency |
| `&self` / `&mut self` / `self` | `self` / `self: inout` / `self: take` | same spellings as parameter modes; lowering fixed by §12.3, independent of `Copy`-ness |
| `{ }` blocks, `;` | indentation, newlines | the visible simplification |

Everything else that differs from Rust is a defect in this document.

---

## 4. Compatibility contract

"Rust compatible" is an architectural requirement with the following exact content.

### 4.1 Cargo compatibility

An Uredo project is also a Cargo project. Dependencies live in ordinary `Cargo.toml`:

```toml
[package]
name = "demo"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[dependencies]
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
```

Uredo does not create a separate package universe.

### 4.2 Rust crate consumption

Uredo code can call any Rust crate.

```uredo
use serde::Serialize
use std::collections::HashMap
```

Honest scope of "call":

- **Functions, types, methods, traits, derives**: usable in Uredo syntax. Arguments to Rust items follow Rust conventions verbatim (§10.3): the programmer writes `&text` where the Rust API takes a reference.
- **Derive and attribute macros on Uredo items**: forwarded (§23). Methods they generate are callable from Uredo; their signatures are unknown to Uredo, so arguments are passed verbatim.
- **Token-based macros** (`sqlx::query!`, RSX-style macros): invoked directly with Rust macro syntax (§22.5); their contents are opaque tokens and Uredo bindings are captured by name. Uredo does not parse macro grammars.

§35 splits the compatibility corpus into "native syntax" and "via raw Rust" accordingly.

### 4.3 Rust source coexistence

A project can contain both `.ure` and `.rs` files:

```text
src/main.ure
src/model.ure
src/optimized.rs
src/platform/linux.rs
```

After lowering, the `.ure` and `.rs` files of **one Cargo target** form one Rust crate. Each Cargo target (lib, bin, each integration test, each example, each bench) is lowered separately, exactly as Cargo compiles them separately.

### 4.4 Uredo callable from Rust: deterministic public signatures

Every `pub` item generates an ordinary public Rust item whose signature is **declaration-derived**: computed from the item's own declaration, the enclosing declaration context (an `impl`'s generics and its trait, a module's `@!default_error`), the crate's own declarations (`@copy use`, other `.ure` files' signatures), and the fixed tables the release ships (the prelude `Copy` table §10.1, the std-trait mode table §12.3) — and from nothing else. This is the meaning of "from the declaration alone" everywhere in this document (D1, D14). A body edit, a dependency update, or a change in compiler inference never changes a public signature. `uredo check --api` emits the public API as a machine-readable manifest and fails on unreviewed differences.

```uredo
pub fn distance(a: Point, b: Point) -> f64:
    ...
```

With `Point` declared in Uredo without `@derive(Copy)`, this is always:

```rust
pub fn distance(a: &Point, b: &Point) -> f64 { ... }
```

With `@derive(Copy)` on `Point`, always `fn distance(a: Point, b: Point)`. The rule is in the declaration, visible to the Rust caller.

### 4.5 No independent Rust ABI promise

Uredo's compatibility is source/build compatibility through Rust. C-compatible or other stable external ABIs remain available through Rust's normal `extern` facilities.

### 4.6 Distribution model

`cargo package` excludes the `target/` directory, so a crate whose Rust exists only under `target/uredo/` cannot be published or consumed. Uredo therefore distinguishes:

| Checkout state | What exists | `build` / `check` / `test` / `doc` | `publish` |
|---|---|---|---|
| **Authoring checkout** | `.ure` + `.rs` + `Cargo.toml` | `uredo <cmd>` (lowers, then delegates to Cargo). Plain `cargo <cmd>` works only after `uredo build` has run, and is not a supported workflow. | via `uredo publish` |
| **Exported package** (output of `uredo package`) | generated `.rs` + copied `.rs` + assets + build scripts + normalized `Cargo.toml` with `rust-version` | plain Cargo, no Uredo required | `cargo publish` |
| **Published crate** | the exported package | plain Cargo | — |

`uredo publish` is `uredo package` and `cargo publish` in one command, and it exists to run the one
check Uredo owes before a step that cannot be undone: a published crate's public API is a promise
(§4.4, D14), so an API that no longer matches `uredo-api.json` stops the command, and a package with
no recorded API stops it sooner. `--allow-api-change` overrides the first, which is the right flag
only when the change *is* the release. Everything else is Cargo's — flags are forwarded unchanged,
`--dry-run` included, and credentials never reach Uredo.

`uredo package` is the only path to crates.io in v0.x. **Uredo lowers each file once, for every target and feature set (D50).** `@cfg` and `@cfg_attr` are forwarded and rustc selects; the exported package carries one lowering with the forwarded attributes. This is sound because every elaboration is decided from declarations (§5.2), and D50 requires the declarations it reads to agree across configuration alternates: two declarations of the same item under different `@cfg` — in one module, or in same-named alternate modules — must agree in parameter passing modes, receiver form and the `str`/`[T]` status of the return type (modifiers and bounds may differ); `@derive(Copy)`, `@copy use` and `@!default_error` are unconditional, where conditionality includes an enclosing `@cfg`-guarded module and the `@rust(cfg…)` spelling. Violations are Uredo errors at the conflicting declaration. Field alternates under `@cfg` are allowed: the field names of all alternates count for the read-only field sugar (§12.2), every §12.2 exclusion stays in force regardless of the guard on the shadowing declaration, and an emitted `self.field` access is checked by rustc in each configuration — the diagnostic is rustc's on the generated access, not Uredo's. Uredo does not evaluate `cfg` predicates and rejects what it cannot show invariant from source. Generated source is never required to be committed in the authoring checkout.

---

## 5. Compilation model

```text
          .ure source
               |
     lexer / parser (tolerant, lossless)
               |
   Uredo name resolution (Uredo items only)
               |
   Uredo local type inference (Uredo signatures + literals + prelude)
               |
   elaboration (passing modes, receivers, try/throws, sugar)  --> provenance map
               |
        deterministic Rust generation
               |
   generated .rs  +  hand-written .rs  (per Cargo target)
               |
             Cargo  -->  rustc  -->  LLVM  -->  native / WASM
               |
   rustc JSON diagnostics --> provenance map --> Uredo diagnostics
```

### 5.1 Why compile to Rust first

This gives Uredo immediately: Rust's optimizer, borrow checking as a correctness barrier, monomorphization, Cargo, crates.io, build scripts, procedural macros, target support, WebAssembly, FFI, and mature debugger/profiler integration.

A future compiler may lower directly to a lower-level IR only if it preserves Rust compatibility and offers a compelling benefit.

### 5.2 The elaboration boundary

**Uredo owns its elaboration rules; rustc owns Rust resolution and type checking.** The two never overlap:

- Every elaboration Uredo performs (inserting `&`, `&mut`, `Ok(...)`, `impl` blocks, raw identifiers) is decided from Uredo source, Uredo-declared signatures, literals, and the fixed prelude. **No elaboration consults a Rust type fact** (whether a foreign type is `Copy`, which trait impl applies, what a macro expands to).
- Where Uredo cannot decide *what to elaborate* — the callee is a Rust item, a closure binding, or a method on a receiver whose type Uredo cannot resolve — it emits the arguments **verbatim** and rustc decides. An argument whose own type is opaque does not suppress a borrow that a resolved signature prescribes: §10.3 inserts `&` there unless the argument is syntactically a reference. `uredo explain` reports which branch applied.
- Successful compilation never selects an interpretation. The generator emits one lowering per construct; it never tries alternatives.

This boundary is what makes generated signatures deterministic (§4.4) and lowering independent of unstable compiler interfaces. A Rust semantic engine (rust-analyzer as a library, or a rustc driver) may be added later for **explain and IDE features only**; it is not a lowering dependency in v0.x.

### 5.3 Generated Rust requirements

- **Deterministic:** identical inputs, compiler version and configuration produce identical bytes. Stable item ordering, stable synthetic names, no timestamps, pinned rustfmt.
- **Readable:** generated code is the code a Rust reviewer reads; `uredo rust <file>` shows it.
- **Provenance-carrying:** every generated construct records its originating Uredo node, source range, elaboration rule, and synthetic role; the map is built against the exact bytes handed to rustc.
- **Absolute names:** every type, trait, constructor and macro the generator introduces is written by absolute path — `::core::result::Result`, `::core::result::Result::Ok`/`Err`, `::core::option::Option::Some`/`None`, `::core::convert::From`, `::core::default::Default`, `::std::format!`, `::std::println!` — so that a user `use anyhow::Result`, `use std::io::Result` or `use Status::*` (an enum with an `Ok` variant) cannot change the meaning of `throws`. Comments elsewhere in this document write `Result` and `Ok` for brevity.
- **Ejectable:** the exported package (§4.6) is maintainable Rust. Abandoning Uredo is a supported path.
- Generated files live under `target/uredo/<target>/` and are not committed.

### 5.4 Failure-mode charter

Two kinds of rustc diagnostic can arise on generated code, and the user is told which:

1. **Barrier diagnostics.** Ownership, borrowing, trait-bound and type-mismatch errors that the user's Uredo program caused. These are expected. They are translated into Uredo terms when the error family is mapped (§27), and otherwise forwarded with the nearest Uredo source anchor plus the generated-Rust excerpt. The user fixes their Uredo source.
2. **Lowering defects.** Any diagnostic the user's source could not have caused: a generated name collision, malformed generated syntax, a wrong auto-borrow, a raw-identifier omission. These are compiler bugs. The compiler says so and points at the generated crate, which is kept under `target/uredo/` for inspection and, in an emergency, direct patching. `uredo report` writes the bundle: the Uredo sources, the generated crate, the provenance between them, the command with its whole output, and the toolchain versions. It also says which of the two kinds it thinks this is, by where the errors land — every error mapping back to a Uredo line reads as a barrier diagnostic, an error in generated Rust that no Uredo line accounts for reads as a lowering defect, and an error in the reporter's own `.rs` module or in a dependency is neither. The bundle carries the reporter's source, so it is written and never sent, and it says so on its first line.

The generator is **total** over well-formed Uredo programs: it never refuses to produce Rust for a program that passes Uredo's own checks.

---

## 6. Lexical structure

### 6.1 Identifiers and raw identifiers

Identifiers follow Rust's rules (Unicode XID), except that the prefix `__uredo_` is reserved for the generator's synthetic names (§25): an identifier beginning with it is a Uredo error naming the reservation. A Rust keyword that §6.2 does *not* reserve, used as an Uredo identifier (`ref`, `box`, `dyn`, `try`, …), is emitted as a raw identifier `r#name`; a reserved word is rejected, not escaped. Uredo also accepts `r#name` in source. `self`, `Self`, `super`, `crate` cannot be raw.

### 6.2 Keywords

**Reserved** (never identifiers in Uredo source): `throw`, `throws`, `var`, and Rust's *control* keywords that Uredo also uses — `fn`, `let`, `if`, `else`, `match`, `for`, `while`, `loop`, `return`, `break`, `continue`, `struct`, `enum`, `impl`, `trait`, `mod`, `use`, `pub`, `const`, `static`, `type`, `where`, `as`, `in`, `async`, `unsafe`, `self`, `Self`, `super`, `crate`, `true`, `false`.

Rust's *other* keywords (`move`, `ref`, `box`, `dyn`, `try`, `mut`, `extern`, `macro`, …) are ordinary identifiers in Uredo where the grammar does not give them a role, and the generator emits them as raw identifiers (`r#move`, §6.1). A keyword's contextual role, listed next, wins over its identifier reading in that position only.

**Contextual** (keywords only in the listed position; identifiers elsewhere): `take` and `inout` immediately before a parameter's type, before a `for` header's source expression (`for s in take strings`), or after `self:`; `move` before a closure; `rust` immediately before `{` (§6.6). Hence `opt.take()` and `iter.take(3)` are ordinary method calls, and a field may be named `inout`.

### 6.3 Blocks, newlines, continuation

Uredo blocks are indentation-based. A colon — or, for a closure, the `=>` of its header (§17, D51) — ends a header line and is followed by a **body**: the following lines indented deeper (a block), or, for the headers listed below, exactly one inline body on the same physical line. Indentation is 4 spaces; tabs are an error; CRLF is normalized.

**Inline bodies.** The header of a function or method body, of an `if`/`else`/`while`/`for` body, or of a `match` arm may be followed on the same line by exactly one body that neither opens a block nor continues onto a further line: for arms one expression; for the others one expression or one non-block statement (`return`, `throw`, `break`, `continue`, a binding or an assignment):

```uredo
if c == '\t': return false
else: return value
fn type_id(self) -> AnyValueId: id
"K": value.checked_mul(1 << 10)
```

A body of more than one statement, or one that itself opens a block, uses the indented form. Type bodies (`struct`, `enum`), `impl` and `trait` bodies, modules and declaration groups have no inline form. A `rust { }` expression counts as a single expression.

**`else` anchor.** Every `if` has a layout anchor: the indentation of the logical statement that contains it. An `else:` line attaches to the nearest preceding unmatched `if` whose anchor equals the `else` line's indentation, and is read before statement termination. This covers `value = if cached: memo` followed by `else: compute()` (anchor: the `value =` statement) as well as one-line `if`s that begin their line.

**Same-line `else`.** An inline `if` body may be followed on the same line by `else:` and one inline body (D49): `n = if n % 2 == 0: n / 2 else: 3 * n + 1`, `if c: return a else: return b`. Expression parsing stops at the `else` keyword, so the form is unambiguous. Two restrictions keep it so: neither inline body may itself be an `if` (nest with the indented form instead — there is no dangling `else`), and an `else` belongs either on the `if`'s line or on its own anchored line, never both. `else if` has no inline form.

**Headers without a colon.** A `struct`, `enum`, `impl` or `trait` declaration that is complete without a colon has no body: `struct Marker` (unit struct, emits `struct Marker;`), `struct Meters(f64)` (tuple struct, fields as written), `enum Never`, `impl Eq for AnyValueId`, `trait Sealed`. A `fn` header without a colon is a required method and is legal only in a `trait` body (§19); `mod name` without a colon declares a file module (§20.3). An indented block after a colon-less header is a syntax error ("expected `:`").

**Items in blocks.** Item declarations (`fn`, `struct`, `enum`, `impl`, `trait`, `type`, `use`, `const`, `static`, `mod`, `macro_rules!`) are permitted inside function bodies and blocks, with Rust's block-item scoping, and are lowered in place. The generator never moves an item out of the block that contains it (the §12.2 nesting desugars within the same module), and a nested item cannot capture enclosing locals or `self`.

A newline terminates a statement **unless** one of the following holds:

- an opening `(`, `[`, `{` or `::<` is unclosed — except inside a trailing block argument (below), where the owning `(` is suspended and lines terminate statements as usual until the closing line;
- the line ends with a binary operator, `,`, `=` or `->` (a line ending in `=>` opens a closure block, §17, D51);
- the next non-blank line is indented deeper than the statement's first line and begins with `.` (method chain), `?`, or a binary operator.

```uredo
value = some_function(
    first,
    second,
)

result = users.iter()
    .filter(u => u.active)
    .count()
```

No semicolon exists in Uredo syntax.

**Trailing block arguments (D51).** Inside an unclosed `(` — never `[`, `{` or `::<` — a line whose last token is the `=>` of a closure header or the colon of a `match` or `if` header opens an indented block. The physical line containing that token is the block's **anchor**: the block's lines are indented deeper than it, and inside them the ordinary layout applies (statements terminate at newlines, blocks nest, further trailing blocks may open; delimiters opened inside the block suppress newlines as usual, and the block cannot end while one is open). The block ends at the first non-blank, non-comment line whose indentation is not deeper than the anchor, other than an `else:` at the anchor continuing a trailing `if` (below). That line must begin with `)` at exactly the anchor's indentation; the rest of the line resumes the enclosing statement in delimiter mode (`).collect()`, `))`, `)?`, or `):` completing an `if` header). The block therefore completes the last expression of its owning parenthesis pair: no comma or further argument may follow it there, though enclosing expressions continue (`apply((x =>` … `), 41)`). For a trailing `if` the layout anchor is the header line, overriding §6.3's statement anchor, and an `else:` at the anchor continues that `if` only; after a closure or `match` block an `else:` is an error. A closing line at another indentation, a closer on the block's last line, a comma before the closer, any other line at or above the anchor, or end of file inside the block are errors naming the anchor line. A call containing a trailing block continues onto further lines and is never an inline body (D34, D49). After the closing line, leading-continuation lines (`.collect()` on its own line) measure against the enclosing statement's first line, as always.

```uredo
summary = items.iter().fold(String::new(), (var acc, item) =>
    acc.push_str(&item.name)
    acc.push(' ')
    acc
)
workers: Vec<thread::JoinHandle<()>> = (0..8)
    .map(_ =>
        let counter = Arc::clone(&counter)
        thread::spawn(move () => *counter.lock().unwrap() += 1)
    )                                  # at the `.map` line's indentation
    .collect()
label = pick(match n:
    0: "none"
    _: "some"
)
if xs.iter().any(x =>
    y = *x * 2
    y > 4
):
    print("big")
```

### 6.4 Comments and doc comments

```uredo
# comment
## documentation for the following item     -> ///
##! documentation for the enclosing module   -> //!
```

`//` is not a comment in Uredo source. Inside `rust { }` blocks Rust lexing applies, so `//` and `#[...]` mean what they mean in Rust. A `#` that begins `#[`, `#!` or `r#` inside Rust blocks is never a comment.

### 6.5 Literals

Integer and float literals, suffixes (`42u64`, `3.14f32`), character, byte, string, raw string (`r"…"`, `r#"…"#`) and byte-string literals are Rust's. Unsuffixed integers default to `i32` and floats to `f64` unless context requires otherwise, as in Rust.

String literals are `&'static str` and never allocate. Interpolation is a property of certain **call positions**, not of literals (§11.3).

### 6.6 `rust { }` lexing

The `rust` keyword followed by `{` switches the lexer to a Rust-compatible token mode until the matching `}`; delimiters are balanced on tokens, not characters, so braces inside strings, raw strings and comments do not terminate the block. Indentation and newline rules do not apply inside. An unclosed Rust block is an error reported at the opening brace, and the parser recovers at the next top-level Uredo item. `rust { }` is the only Rust-block form; there is no indentation-delimited variant.

The same mode switch applies to a **macro invocation**: a path followed by `!` and an opening delimiter (`(`, `[`, `{`) is lexed as Rust tokens until the balanced close (§22.5). Uredo has no postfix `!`, so this is unambiguous.

### 6.7 Paths

Paths use `::` exactly as in Rust: `std::fs::read_to_string`, `LoadError::InvalidFormat`, `Vec::from`. `.` is field access, method call, and the postfix `.await` (§21); it is never a path separator.

---

## 7. Expressions and operators

### 7.1 Operators and precedence

Uredo uses Rust's operators with Rust's precedence and associativity (Rust Reference, "Expressions → Operator precedence"): unary `- ! & &mut *`, `as`, `* / %`, `+ -`, `<< >>`, `&`, `^`, `|`, comparisons, `&&`, `||`, ranges `.. ..=`, assignment and compound assignment. Chained comparisons are an error, as in Rust.

### 7.2 Integer semantics

Integer overflow follows Rust: panic in debug profiles, two's-complement wrap in release, with `wrapping_*`, `checked_*`, `saturating_*` methods available. Division by zero panics. There is no implicit numeric widening.

### 7.3 Casts

`expr as Type` is Rust's cast expression with Rust's semantics. `as` has no other meaning in expressions. (`use path as name` remains a rename.)

### 7.4 Ranges, tuples, indexing

`0..10`, `0..=10`, `..`, `a..` are range values. Tuples are written `(a, b)` with type `(A, B)`; `()` is unit. Indexing `xs[i]` is Rust indexing (bounds-checked). Slicing `xs[1..3]` yields a slice.

### 7.5 `if` and `match` as expressions

`if`/`else` and `match` are expressions when every branch produces a value; a block's value is its tail expression.

### 7.6 Generic arguments in expressions

Expression-position generic arguments use the turbofish: `.collect::<Vec<User>>()`, `serde_json::from_str::<User>(&text)`, `.sum::<f64>()`. Type positions use plain `<…>`. Dropping the turbofish is an open question (§38.3) because `f<T>(x)` competes with comparison syntax in a parser that has no type information.

### 7.7 Blocks as expressions

A bare indented block after `=` is not permitted. A `match` or `if` header may appear after `=`, after `return` and after `throw`; the indented block that follows belongs to that header, indented deeper than the first line of the statement containing it, and the whole is one expression (`bytes = match suffix:` …, `x = if c:` … with `else:` at the statement's indentation, §6.3). A `match` or `if` header, or a closure header, may also open its block as the trailing argument of a call: the block is anchored at the header's own line and closed by `)` on its own line (§6.3, D51). The D49 inline `if` is admitted in argument position as well: `f(if c: a else: b)`. A closure header ending in `=>` opens a block in the same way: after `=` it is anchored at the statement, and as a trailing argument at its own line (§17, D51). Other multi-statement expressions use a function or a `rust { }` expression block (§22.3).

---

## 8. Statements and bindings

### 8.1 Binding, assignment, shadowing

```uredo
x = 10            # `x` is unbound here and in every enclosing scope: immutable binding (let x = 10)
var total = 0     # mutable binding (let mut total = 0)
total = total + 1 # `total` already bound in scope: assignment; requires `var`
x = 11            # error: `x` is immutable; use `var x` to allow assignment
input = read_line()
let input = input.trim()   # new binding shadows the old one; type String -> &str
```

Rules:

- `name = expr` where `name` is not bound in the current or an enclosing scope introduces an immutable binding.
- `name = expr` where `name` is bound is an assignment. It is an error unless the binding was declared with `var`.
- `let name = expr` always introduces a new immutable binding, **shadowing** any existing `name` (Rust's `let x = x.trim()`); `let var name = expr` is not a form — write `var name = expr`. `var name = expr` always introduces a new mutable binding and may also shadow. Shadowing is only ever spelled with `let` or `var`; a bare `name = expr` never creates a second binding for a name that is already bound.
- **Pattern bindings.** `pat = expr` with a tuple, struct or tuple-struct pattern binds when **every** name in the pattern is unbound, and is a Rust destructuring assignment when **every** name is already bound (all must be `var`); a mix is an error. `let pat = expr` forces a binding. `var (a, b) = expr` binds both mutably. A pattern that binds no names (`_`, `(_, _)`, `S { .. }`) is always a binding: `_ = expr` lowers to `let _ = expr;`, which does not move, drop or consume an existing place (`_ = guard` keeps the guard alive) and only suppresses the unused-result warning. Refutable patterns use Rust's `let-else`:

```uredo
(low, high) = bounds()                    # binds two names
let Some(cfg) = load_config() else:       # refutable: the else block must diverge
    return
Point { x, y } = origin
```
- Field and index assignment (`user.active = true`, `xs[0] = 1`) require the root to be a mutable place: a `var` binding, an `inout` parameter, a `for … in inout` loop binding, or a `ref mut` pattern binding. §10.3's list for an `inout` *argument* is the first three of those and a field or index of one; a `ref mut` binding does not appear there because it is already a reference, so it is passed verbatim under D47 rather than borrowed. Where the root's type is not known to Uredo (a `match` or closure binding), rustc checks it and the error is mapped (§27).
- Compound assignment (`+=` etc.) is assignment.

Because the meaning of `x = …` depends only on whether `x` is bound in scope, a formatter or reader can determine it lexically.

### 8.2 Typed bindings

```uredo
count: u64 = 0
ratio: f64 = 0.5
name: String = String::from("Alice")
var buffer: Vec<u8> = Vec::new()
```

### 8.3 Constants and statics

```uredo
const MAX_USERS: usize = 1000
static GREETING: str = "hello"        # &'static str
```

**Declaration groups.** `[visibility] const:` and `[visibility] static:` followed by an indented block declare an ordered list of ordinary `const`/`static` items in the enclosing scope, all with that one visibility; each line is an item, not a binding, and there is no group item in the generated Rust. An attribute written on the group is applied to every member, and `##` doc comments on members forward to each. A member that needs its own visibility or attribute is declared on a single line.

```uredo
pub const:
    KILO: u64 = 1000
    MEGA: u64 = KILO * 1000
```

### 8.4 Tail expressions

The last expression of a function body, `if` branch, `match` arm or block is its value. `return expr` is available everywhere. Both are supported in v0.1; they map directly to Rust and have zero semantic cost. In a `throws` function both are lowered per §15.3.

---

## 9. Types

Uredo is statically typed with Rust's types. Uredo performs **local** type inference using Uredo-declared signatures, literals and the prelude; an expression whose type depends on a Rust item is "opaque to Uredo" and is typed by rustc. Opacity affects only which elaboration branch applies (§10.3); it never changes the meaning of a program.

### 9.1 Scalars

| Uredo | Rust | Note |
|---|---|---|
| `bool`, `char` | same | |
| `i8 … i128`, `isize`, `u8 … u128`, `usize` | same | `i32` default integer |
| `f32`, `f64` | same | `f64` default float |
| `()` | `()` | unit |

These, plus shared references, tuples and fixed arrays of them, are the **known-Copy** types (§10.1).

### 9.2 Strings

| Uredo | Rust | Position |
|---|---|---|
| `str` | `&str` | parameters, returns (§9.7), `const`/`static` (§8.3, where it is `&'static str`). A borrowed *field* is written in full, `&'a str`, with the lifetime on the type (§9.7) |
| `String` | `String` | owned; explicit |

```uredo
name = "Alice"                         # &'static str, no allocation
owned: String = String::from("Alice")  # allocation visible in the type and the call
```

### 9.3 Option

`T?` is sugar for `Option<T>`; `Option<T>` is also valid. Values are `Some(x)` and `None` (Rust spelling; there is no `none`). A leading `&` binds tighter than the suffix, in every type position, so the `?` wraps the reference: `&T?` is `Option<&T>`. A reference to the optional itself is written `&(T?)` or spelled `&Option<T>` (D54 ratifies this precedence, formerly G10; the parentheses are dropped from the generated Rust).

A parameter's passing mode follows its payload, not its optionality (D54): `x: u32?` is `Option<u32>` by value, because `u32` is known-Copy and so is `Option<u32>`; `x: String?` is `&Option<String>`; `x: &T?` is `Option<&T>` by value, since a shared reference is known-Copy. `take T?` passes the `Option` by value for any payload. §10.1 and §10.2 carry the rule.

Where `str` and `[T]` denote `&str` and `&[T]` — a parameter, a return type (§9.7) — they do so
under the suffix too: `x: str?` is `Option<&str>`, `-> [u8]?` is `Option<&[u8]>`. The literal
reading would be `Option<str>`, a type no value can have, so the two rules compose in the only
direction that means anything. `[u8; 4]?` is an array and stays `Option<[u8; 4]>`, and a holder for
which the unsized form is the point — `Box<str>`, `Rc<[T]>` — is untouched, as is a `str` written
as someone else's type argument.

### 9.4 Containers and literals

Each line below is an independent example, not a sequence (a second `numbers = …` would be an assignment, §8.1):

```uredo
bytes = [1, 2, 3, 4]                   # [i32; 4] — stack array, no allocation
numbers = Vec::from([1, 2, 3, 4])      # Vec<i32> — allocation visible
numbers = vec![1, 2, 3, 4]             # Rust's macro, invoked directly (§22.5); allocation visible in the name
numbers: Vec<i32> = [1, 2, 3, 4]       # allowed: the annotation makes the Vec visible, and Uredo
                                       # emits `Vec::from([1, 2, 3, 4])` — an elaboration the
                                       # annotation asks for, not the coercion §9.9 forbids
words: Vec<String> = Vec::new()
```

A bracket literal is always a fixed array. `HashMap`, `HashSet`, `Vec` keep their Rust names; Uredo does not introduce `List`/`Dict` aliases.

### 9.5 Slices

`[T]` in a parameter position lowers to `&[T]`; `inout [T]` to `&mut [T]`. Arrays and `Vec` coerce to slices at call sites (§9.9).

```uredo
fn total(values: [i32]) -> i32:
    values.iter().sum::<i32>()
```

### 9.6 Tuples, `Self`, trait objects

Tuple types `(A, B)`; `Self` inside `impl`/`struct` bodies; `dyn Trait` and `impl Trait` use Rust spelling. `Box<dyn Trait>` is written out: boxing and dynamic dispatch are never inserted (§3.2).

### 9.7 References and lifetimes in signatures

- **Parameters:** of ordinary functions and methods, `str`, `[T]` and any non-known-Copy **concrete** type are borrowed by default (§10.2); type parameters and `impl Trait` pass by value (P8), pattern parameters by value (P7), and an `impl` of a listed std trait takes the trait's modes (D53). Explicit `&T`, `&mut T`, `&'a T` are always allowed. Closure parameters are Rust's (§17).
- **Return types:** owned, except `str` and `[T]`, which denote `&str`/`&[T]` with Rust's elided lifetime (tied to `self` or the single borrowed parameter). If there is nothing to elide from — no borrowed input, counting `self` only when the receiver is `self` or `self: inout`, as in `fn sign(n: i32)` returning a literal — elision fails and the lifetime is written: `-> &'static str` (D48). Likewise when several borrowed parameters compete: `fn longer<'a>(a: &'a str, b: &'a str) -> &'a str`.
- **Struct and enum fields:** owned unless written with an explicit reference and an explicit lifetime parameter on the type. Uredo never invents a lifetime parameter for an item.

```uredo
struct Token<'a>:
    text: &'a str
```

- **Lifetimes** are written in Rust syntax when needed:

```uredo
fn choose<'a>(a: &'a str, b: &'a str) -> &'a str:
    ...
```

Uredo never introduces a runtime mechanism to avoid exposing a lifetime.

### 9.8 Literal inference

```uredo
a = 42        # i32 unless context requires another integer type
b = 42u64     # u64
c = 3.14      # f64 unless context requires f32
d = 3.14f32   # f32
```

### 9.9 Coercion table

Uredo permits exactly Rust's implicit coercions at Rust's coercion sites (function arguments, `let` with annotation, returns, struct fields), and no coercion that allocates or clones:

| From | To | Mechanism |
|---|---|---|
| `&String` | `&str` | deref coercion |
| `&Vec<T>`, `&[T; N]` | `&[T]` | deref / unsize |
| `&&T` | `&T` | deref coercion (one step; lets §10.3 wrap an argument whose reference-ness it cannot see) |
| `&&[T; N]` | `&[T]` | **never**: deref and unsize do not combine — hence §10.3's verbatim rule for syntactic references (D47) |
| `&mut T` | `&T` | reborrow |
| `&Box<T>` | `&T` | deref |
| `str` → `String` | — | **never** (write `.to_string()` / `String::from`) |
| `[T; N]` → `Vec<T>` | — | **never** as a coercion (write `Vec::from`, or annotate the binding `Vec<T>` and Uredo emits `Vec::from` for you, §9.4) |

---

## 10. Passing modes and ownership

This is Uredo's most important simplification, and it is decided **from the declaration alone**.

### 10.1 Known-Copy types

A type is *known-Copy* to Uredo if it is one of:

- a scalar (§9.1); a shared reference `&T`; a tuple or fixed array whose element types are known-Copy;
- `Option<T>` (written `T?`) and `Result<T, E>` whose payloads are known-Copy, applied recursively (D54) — the rule the bullet above already states for tuples and arrays, and what Rust's own `impl Copy for Option<T> where T: Copy` says;
- a type parameter in scope whose declared bounds include `Copy` (D54); the bound is in the crate's own text, so `T?` stays decidable from the declaration (D14). This does not change the mode of `x: T` itself, which is P8 by value either way;
- a Uredo-declared `struct`/`enum` carrying `@derive(Copy)`;
- a type in the **prelude Copy table**: a versioned list of std `Copy` types shipped with each Uredo release (`std::time::Duration`, `Instant`, `cmp::Ordering`, `net::SocketAddr`, `Ipv4Addr`, `Ipv6Addr`, `num::NonZero*`, `num::Wrapping<T>` and `ops::Range*<T>` for known-Copy `T`, …). The table is documented per release and grows only by addition;
- a foreign type declared in this crate with `@copy use`:

```uredo
@copy use uuid::Uuid          # Uuid is known-Copy for this crate

fn tag(id: Uuid):             # fn tag(id: Uuid)
    registry.insert(id)
```

A generic foreign type is `Copy` only for some arguments, so `@copy use` names **one instantiation**
(D56): `@copy use nalgebra::Vector3<f64>` makes `Vector3<f64>` known-Copy and leaves
`Vector3<String>` borrowed, and the emitted assertion is that instantiation. The `use` itself carries
no type arguments — Rust has nowhere to put them — so the generated import is `use nalgebra::Vector3;`.

For every table entry and `@copy use` the generator emits a compile-time assertion (`fn __uredo_assert_copy<T: Copy>() {}` instantiated with the type) so that a false declaration is a mapped diagnostic, never a silent move. Constructed known-Copy types — tuples, arrays, `Option`, `Result` — need no assertion of their own: rustc's impls for those constructors enforce the claim, and a wrong payload declaration is already caught by the payload's assertion. **No other type is treated as `Copy`, whatever rustc knows about it**, and no rule consults rustc: the table and the declarations are Uredo source, so signatures remain derivable from the crate's own text (D14).

Known-Copy-ness must not depend on the configuration, because it decides passing modes and Uredo lowers once for all configurations (D50): a conditional `@derive(Copy)`, a `Copy` type declared inside a `@cfg`-guarded module, a `@cfg`-guarded `@copy use`, and `@cfg` alternates of one type that disagree about `Copy` are each an error, reported in Uredo terms before rustc runs.

### 10.2 The passing-modes table

| Uredo parameter | Rust type | Rule |
|---|---|---|
| `x: S`, `S` known-Copy | `S` | P1: by value |
| `x: str` | `&str` | P2 |
| `x: [T]` | `&[T]` | P2 |
| `x: T`, any other **concrete** type, including foreign types | `&T` | P3: shared borrow |
| `x: T` with `T` a type parameter in scope (of the function, its `impl` or its trait); `x: impl Trait` | `T` / `impl Trait` | P8: by value (D52); the type form is the visible sign of the transfer, as the pattern is in P7. Moved unless the argument's type is `Copy` |
| `x: T` with a declared `?Sized` bound; `x: impl Trait + ?Sized` | `&T` / `&impl Trait` | P3: the `?Sized` bound is the visible sign of the borrow |
| `x: inout T` | `&mut T` | P4: mutable borrow (any `T`) |
| `x: take T` | `T` | P5: ownership transfer (copy for `Copy` types) |
| `x: &T`, `&mut T`, `&'a T`, `&impl Trait` | as written | P6: explicit |
| `x: T?` (equally `x: Option<T>`) | `Option<T>` when `T` is known-Copy (P1); `&Option<T>` otherwise (P3) | D54: optionality is not a passing mode — the payload decides, through §10.1's recursion into `Option`. A by-value optional here copies and moves nothing, so no transfer is hidden. With `T` a bare type parameter it is P3: the `?` marks optionality, not transfer, and a by-value `Option<T>` for an arbitrary `T` would move where the reader sees only an optional; a `T: Copy` bound makes it P1 like any other known-Copy payload. `take T?` is `Option<T>` for any payload, and spells the same mode as `x: T?` when `T` is known-Copy (`uredo lint`'s `redundant_take` reports it, as it does for `take T` on a type parameter) |
| `x: &T?` | `Option<&T>` | D54, formerly G10: the shape Rust reaches for when it borrows an optional payload (corpus: `Option<&X>` 340 against `&Option<X>` 10). A shared reference is known-Copy, so the optional passes by value. A borrow of the whole optional is `&(T?)` |
| `x: &mut T?` | rejected as a parameter | it would pass `&Option<&mut T>`, through which the payload cannot be mutated (rustc: E0594). The diagnostic names the two spellings that work: `inout T?` to mutate the optional itself, `take &mut T?` to take the mutable reference. In return, field and local-type position `&mut T?` is `Option<&mut T>` as written, which is how a method hands out a mutable optional payload (`fn slot_mut(self: inout) -> &mut u32?`) |
| `PATTERN: T` (e.g. `State(db): State<Db>`, `(a, b): (i32, i32)`) | `T` | P7: a pattern parameter is by value; the pattern is the visible sign that the value is consumed |
| `var x: T`, `var x: take T` | the mode's Rust type with a `mut` binding (`mut x: T`, `mut x: &T`, …) | `var` marks the binding, `take` the mode; the two are independent, so `var x: T` on a P3 type is `mut x: &T` — a rebindable reference, not a mutable value |

Receivers are not subject to this table: `self`, `self: inout`, `self: take` and `var self: take` are written and lowered as §12.3 says, whatever the type's `Copy`-ness.

Generic functions:

```uredo
fn first<T>(values: [T]) -> &T?:          # &[T] -> Option<&T>  (the `&T?` precedence, D54)
    values.first()

fn largest<T: Copy + PartialOrd>(values: [T]) -> T?:   # values is still &[T]
    ...
```

`x: T` for a type parameter `T`, and `x: impl Trait`, pass **by value** (P8, D52) whatever the other bounds; the one declared bound that changes the mode is `?Sized`, which keeps `&T` (P3). A `T: Copy` bound changes nothing there, since P8 is by value already; it does make `T` known-Copy as a *payload*, so `x: T?` under `T: Copy` is by value (§10.1, D54). `take T` on a type parameter is accepted and lowers identically (`uredo lint` flags it as redundant). To borrow a type parameter that is not `?Sized`, write `&T` (P6). This keeps every generated signature stable across dependency changes: the mode reads only the declaration. Evidence (corpus, `corpus-study/genparams_summary.txt`): idiomatic Rust passes 91% of generic-typed parameters by value; the former borrow default matched 9.2% of them; `?Sized` parameters are borrowed 255 to 12.

Idiom cost, stated plainly: a foreign `Copy` type that is neither in the prelude table nor declared with `@copy use` is passed as `&T`, and `fn f(v: Vec<User>)` is `&Vec<User>` (prefer `[User]`). `uredo lint` reports both (§29.1): `borrowed_container` flags `Vec<T>`/`String` parameters, written `&` or not, and suggests `[T]`/`str`; `copy_candidate` suggests `@copy use` (or `@derive(Copy)` for a type this crate declares) when every use of a borrowed parameter is a `*x` deref.

### 10.3 Call sites

At a call `f(a, b)`:

- If `f` resolves to an **Uredo-declared** function (or a method on a receiver whose type Uredo knows to be an Uredo type), each argument for a P2/P3 parameter is emitted as `&arg`, for a P4 parameter as `&mut arg`, and for a P1, P5, P6, P7 or P8 parameter verbatim (by value, as written, or already a reference). For P4 the argument is either an explicit `&mut e`, which is emitted as written, or a place from which Uredo inserts the borrow — and then it must be a *mutable* place: a `var` binding, an `inout` parameter (reborrowed), a `for … in inout` binding, or a field or index of one. Passing an immutable binding to `inout` is an error reported in Uredo terms. For P2/P3 the argument is any expression; a temporary is borrowed for the call as in Rust. **An argument that is syntactically already a reference is emitted verbatim** (D47): a `&e`/`&mut e` expression, a string or byte-string literal, or a binding whose *declared* type is a reference — a `str`, `[T]`, `&T`, `&mut T` or `&'a T` parameter, or an annotated `x: &T` binding. Only where the argument's reference-ness is not visible in the syntax (an unannotated binding, a call result) is `&arg` inserted; a resulting `&&T` coerces by one deref (§9.9). A `?Sized`-bounded type parameter is P3 and follows this same rule: a literal is verbatim and instantiates `T = str`, which `?Sized` permits, so no exception is needed (the former G15 insertion and its goal of spelling-independent instantiation are withdrawn by D52: for a P8 parameter the instantiation is the argument's spelled type, `tag("n")` → `U = &str`). The verbatim rule is load-bearing: `&&[u8; N]` — a wrapped byte-string literal — does **not** coerce to `&[u8]`, because a deref and an unsize cannot be combined.
- If `f` is a Rust item, or the receiver's type is opaque to Uredo, arguments are emitted **verbatim**. The programmer writes `&x` or `&mut x` as in Rust. The diagnostic for a resulting type mismatch says so.
- If `f` is a tuple-struct or tuple-variant constructor (`Meters(3.0)`, `Message::Text(name)`), arguments are emitted **verbatim**: a constructor's fields have their declared types and the value is moved in, as in Rust. Constructors are not subject to the passing-mode table.
- If `f` is a closure binding, arguments are emitted verbatim: closure parameters are Rust's (§17).
- `take` and P8 parameters receive the argument verbatim in all cases; the value is moved (copied for `Copy` types).

```uredo
var user = load_user()
activate(user)                    # Uredo fn activate(user: inout User) -> activate(&mut user)
text = std::fs::read_to_string(path)?
users = serde_json::from_str::<Vec<User>>(&text)?   # Rust item: `&` written by hand
greet(name)                       # Uredo fn greet(name: str), caller's `name: str` is declared as a reference -> verbatim
first_byte(b"hi")                 # byte-string literal is already &[u8; 2] -> verbatim (wrapping it would not coerce)
```

### 10.4 No implicit clone

```uredo
enqueue(job)        # fn enqueue(job: take Job)
print(job.id)       # error: `job` moved into `enqueue`
```

The compiler never inserts `.clone()`. The diagnostic (§27) suggests borrowing, an explicit `job.clone()`, or redesigning the boundary; for a P8 parameter the borrow suggestion `f(&x)` holds only where `&X` satisfies the bounds (the type parameter is then instantiated with `&X`), otherwise `x.clone()`.

### 10.5 Explicit references

Rust reference syntax is always available and is the escape when a default is wrong:

```uredo
fn parse<'a>(input: &'a str) -> Token<'a>:
    ...
```

---

## 11. Functions

```uredo
fn add(a: i32, b: i32) -> i32:
    a + b
```

### 11.1 Entry point

```uredo
fn main():                       # fn main()
fn main() throws AppError:       # fn main() -> Result<(), AppError>; AppError: Debug
@rust(tokio::main)
async fn main():                 # runtime attribute forwarded
```

`async fn main` without a runtime attribute is an error.

### 11.2 Named and default arguments

Not in v0.1: they have no Rust interop mapping. Builder patterns remain the Rust idiom.

### 11.3 `print`, `format` and interpolation

`print`, `eprint`, `format` and `panic` are Uredo intrinsics:

*The Rust column is schematic: generated code writes these macros by absolute path (`::std::println!`, §5.3).*

| Uredo | Rust | Allocation |
|---|---|---|
| `print(expr)` | `println!("{}", expr)` (`expr: Display`) | none |
| `print("literal with {a} and {b:?}")` | `println!("literal with {a} and {b:?}")` | none |
| `eprint(...)` | `eprintln!` | none |
| `format("…{a}…")` | `format!("…{a}…")` → `String` | **yes, visible in the name** |
| `panic("…{a}…")` | `panic!` | — |
| `format("… {}", path.display())` | `::std::format!("… {}", path.display())` | **yes** |

A string literal is a **format string only when it is the direct argument of a bare call to one of these intrinsics**. The literal is a Rust format string: `{name}` captures a binding in scope (Rust's captured-identifier rule; `{a + b}` is an error; field sugar does not apply inside a string — write `format("{}", self.balance)`) and accepts Rust format specs (`{x:?}`, `{v:>8.2}`). The literal may be followed by positional and named arguments exactly as Rust's `format_args!` accepts; those arguments are Uredo expressions, elaborated normally (§10.3 auto-borrow applies), and then placed in the generated macro call: `format("{}", length(text))` → `::std::format!("{}", length(&text))`. A literal containing `{…}` anywhere else is an ordinary string; to build a string, call `format`:

```uredo
url = format("https://example.com/users/{id}")     # String, allocation visible
response = reqwest::get(&url).await?
```

A **bare call** to `print`, `eprint`, `format` or `panic` is always the intrinsic; to call an item of the same name write it path-qualified (`self::format(2)`, `ParseSizeError::format(size)`), and such calls follow §10.3 like any other. Importing a same-named item and calling it bare is an error that names the intrinsic. Inside a macro invocation's delimiters intrinsics are unavailable (§22.5). Generated code writes the standard macros qualified (`::std::format!`, `::std::println!`).

---

## 12. Structs

```uredo
struct User:
    id: u64
    name: String
    active: bool
```

Fields are private to the module unless marked `pub`, as in Rust:

```uredo
pub struct User:
    pub id: u64
    pub name: String
    active: bool
```

Tuple structs use Rust syntax: `struct Meters(f64)`. Unit structs: `struct Marker` (colon-less declarations, §6.3).

### 12.1 Construction

Rust struct-literal syntax, and only that:

Independent examples, not a sequence:

```uredo
user = User { id: 42, name, active: true }      # `name` is punned: `name: name`
user = User {
    id: 42,
    name,
    active: true,
}
updated = User { active: false, ..user }        # struct update
m = Meters(3.0)                                 # tuple struct: positional
```

A struct literal is a path followed by `{`, which is unambiguous in Uredo: the other brace forms are introduced by a keyword (`rust {`), a macro's `!`, a `use` path's `::`, or a pattern's position (§13.1). Punning, struct update, foreign structs and enum struct variants (§13) therefore work by syntax alone. There is no parenthesised named-field form.

### 12.2 Methods

Methods may be nested in a `struct` or `enum` declaration, after its fields or variants; this is sugar for an inherent `impl` block. The body may also contain `impl Trait:` blocks implementing a trait for the enclosing type: each lowers to its own `impl<…> Trait for Type<…> where …`, carrying the enclosing type's generic parameters, bounds and `@cfg` attributes, emitted immediately after the type — the inherent impl first, then the trait impls in source order. A nested marker impl is written `impl Marker` (no colon, §6.3); the same two tokens at module level are an empty inherent impl of a type named `Marker`, so the trait reading applies only inside a type body. Impls whose self type is another type stay at module level. The next line at the type's own indentation ends a nested block.

```uredo
struct Account:
    balance: f64

    fn deposit(self: inout, amount: f64):
        self.balance += amount

    fn current_balance(self) -> f64:
        balance

    impl std::fmt::Display:
        fn fmt(self, f: inout std::fmt::Formatter<'_>) -> std::fmt::Result:
            write!(f, "{}", self.balance)      # inside a macro: Rust source, no field sugar (§22.5)
```

**Bare field names are read-only sugar.** Inside an instance method of a Uredo-declared type whose fields are visible, a bare identifier in an expression position that is not the target of `=` or a compound assignment denotes the place `self.field` when (a) it names a field of `Self`; (b) no parameter, local, pattern binding or Uredo-declared value item of that name is in scope — source-visible declarations only: explicit `use` items count, and a glob `use x::*` in the same module **disables** the sugar for that module, because Uredo cannot know what it imports; and (c) the identifier is not inside a nested item (a nested `fn` cannot capture `self`) or a `move` closure (which would move the field). An associated function or constant of `Self` never competes with the sugar, because it is reached as `Self::name`: a bare `name` is the field. Field sugar is the lowest-priority resolution. Every field write is spelled `self.field = …` / `self.field += …`; a bare `name = …` therefore always binds or assigns a local (§8.1). Diagnostics: `balance = 0.0` where `balance` is a field gets the note "this binds a new local `balance`; write `self.balance = 0.0` to assign the field"; `balance += amount` on an unbound name where `balance` is a field is the error "field writes are spelled `self.balance += amount`". The explicit `impl` form is equivalent:

```uredo
impl Account:
    fn deposit(self: inout, amount: f64):
        self.balance += amount
```

### 12.3 Receivers

| Uredo | Rust |
|---|---|
| `self` | `&self` |
| `self: inout` | `&mut self` |
| `self: take` | `self` |
| `var self: take` | `mut self` (builder pattern) |
| `self: T` for any other written type | `self: T` (D59) |
| no `self` | associated function |

**Std-trait parameter modes (D53).** In an `impl Trait for Type` whose trait is a std trait with fixed method signatures — `From`/`TryFrom`/`Into` (by value), `PartialEq`/`PartialOrd`/`Ord::cmp` (`&Rhs`), `Hash` (`&mut H`), `Display`/`Debug` (`&mut Formatter`), the `ops` operator traits (`rhs` by value), `Index`/`IndexMut`, `Extend`/`FromIterator`, `io::Read`/`Write`, and the rest of the table — which, like the prelude `Copy` table (§10.1), is versioned with the release, documented per release and grown only by addition, because it changes generated signatures (§4.4) — each parameter is lowered to the mode the trait declares, whatever §10.2 would have chosen: `fn from(e: ParseIntError)` is `from(e: ParseIntError)`, `fn eq(self, other: Self)` on a `Copy` type is `eq(&self, other: &Self)`, `fn fmt(self, f: Formatter<'_>)` is `fmt(&self, f: &mut Formatter<'_>)`. A written `take`/`inout` that agrees is accepted; the receiver is still written by the programmer. Traits outside the table follow §10.2 as before. This resolves G11.

**A written receiver type** (D59) covers the receivers Rust allows and Uredo does not model:
`self: Pin<&mut Self>`, `self: Rc<Self>`, `self: Arc<Self>`, `self: Box<Self>`. It is emitted as
written, and Uredo makes no claim about what may be done through it — a write through such a
receiver is passed on and rustc judges it, as §5.2 requires. Without this a hand-written `Future`
could not be spelled at all: the corpus has 862 such receivers, 849 of them on a `poll`.

**Every method writes its receiver explicitly**, in struct bodies, `impl` blocks and trait declarations alike. A method without `self` is an associated function, always. There is no receiver inference: a signature is read directly, a body edit never changes it, and the compiler needs no fixed-point analysis. A method that assigns to a field but declares `self` gets a mapped diagnostic ("this method mutates `balance`; declare `self: inout`").

```uredo
struct Account:
    balance: f64

    fn zero() -> Account:                 # associated function
        Account { balance: 0.0 }

    fn deposit(self: inout, amount: f64):
        self.balance += amount
```

### 12.4 Associated types

`type Name` declares one, `type Name = T` defines it, and both may carry bounds and a `where`
clause: `type View<'a>: Debug where Self: 'a`. The clause matters because rustc *requires* it for a
lifetime-parameterised associated type used as `Self::View<'_>`, which is the ordinary shape of a
generic associated type; the corpus writes 243 such declarations, 35 of them with the clause.
Bounds, parameters and the clause are kept as written (§5.2) and emitted unchanged.

### 12.5 Derives and attributes

```uredo
@derive(Debug, Clone, Serialize, Deserialize)
struct User:
    ...
```

lowers to `#[derive(...)]`. No trait is derived automatically. `@derive(Copy)` is the only way a Uredo-declared type becomes known-Copy (§10.1) and requires `Clone` as in Rust; foreign types use `@copy use`.

---

## 13. Enums and pattern matching

```uredo
enum Message:
    Quit
    Text(String)
    Move { x: i32, y: i32 }
```

Variant syntax is Rust's: `Text(String)` is a tuple variant, `Move { x: i32, y: i32 }` a struct variant. Construction follows §12.1: `Message::Move { x: 1, y: 2 }`.

### 13.1 Match

```uredo
match message:
    Quit: stop()
    Text(text):
        print(text)
    Move { x, y }:
        move_to(x, y)
```

- An arm is `pattern:` followed by one expression on the same line or by an indented block (§6.3). A single-expression arm lowers to `pat => expr,`; a block arm to `pat => { … }`. The generator never adds a block around a single expression and never removes a user-written `rust { }`.
- Within a `match`, a bare identifier that names a variant of the scrutinee's enum type is a variant pattern; other bare identifiers are bindings. When the scrutinee's type is opaque to Uredo, variants must be path-qualified (`Message::Quit`).
  *The hazard, stated plainly:* adding a variant to the enum can turn an existing catch-all binding arm of that name into a variant pattern. The compiler reports the change (a binding that becomes a pattern makes the arm's body reference an unbound name, or the match non-exhaustive), and path-qualifying (`Message::Quit`) or renaming the binding pins the reading. Uredo does not resolve this by inference: the scrutinee's variant set comes from the declaration, so the meaning is still decidable from source.
- Patterns are Rust's: tuple variants match positionally, struct variants by field name (`Move { x, y }`, `Move { x: a, .. }`). There is no positional matching of named fields.
- Matches are exhaustive unless `_` is provided. Guards use `pattern if cond:`.

```uredo
match value:
    x if x < 0:
        negative()
    0:
        zero()
    _:
        positive()
```

Matching a `Result` uses `Ok(v)` / `Err(e)` patterns (§15.5).

---

## 14. Optional values

```uredo
fn find_user(id: u64) -> User?:      # Option<User>
    ...

if let Some(u) = find_user(id):
    print(u.name)

if let Some(u) = &user:              # borrow instead of move
    print(u.name)

while let Some(job) = queue.pop():
    run(job)
```

`if let` follows Rust's move/borrow semantics exactly: matching a place expression by value moves out of it; write `&user` to borrow. Uredo defines no truthiness: integers, strings and objects are never conditions.

Optional chaining (`user?.address?.city`) is **not** in v0.x: no lowering exists that is both zero-cost and independent of the field types along the chain (§38). The idioms are Rust's:

```uredo
city = user.as_ref().and_then(u => u.address.as_ref()).map(a => &a.city)

if let Some(a) = user.as_ref().and_then(u => u.address.as_ref()):
    print(a.city)
```

---

## 15. Error handling

`Result<T, E>` remains the representation; `throws` is notation.

### 15.1 `throws`

```uredo
fn load_user(path: str) -> User throws LoadError:      # fn load_user(path: &str) -> Result<User, LoadError>
fn save(user: User) throws IoError:                    # fn save(user: &User) -> Result<(), IoError>
```

**Bare `throws`** names the crate's or module's declared default error type:

```uredo
@!default_error(AppError)                 # crate root; a module may re-declare for its subtree

fn helper(x: str) -> u32 throws:          # = throws AppError
    x.parse::<u32>()?                     # ParseIntError -> AppError via From (§15.4)

pub fn load(path: str) -> Config throws AppError:     # pub: the type is written
```

Rules: a bare `throws` with no default in scope is an error naming the attribute to add; `pub` functions and trait signatures always write the error type, so public APIs read without lookup; `throws E` overrides the default per function. The default is a line of this crate's source, so signatures stay derivable from declarations (D14) and nothing is inferred from bodies.

**Scope.** A crate root's declaration covers every module of that crate; a module's own re-declares for its subtree. Two consequences follow and both are enforced. The type must be **nameable from every module it reaches**, so a crate-root default is written as a path — `@!default_error(crate::AppError)`, not `AppError`, which resolves only at the root. And a package's library and binary are **two crates**: `src/lib.ure` and `src/main.ure` are each a root, `crate::` means a different crate in each, and neither inherits the other's default even though one index serves the package.

**Verbatim `Result` and `Option` return types are plain Rust.** A function may write `-> io::Result<Config>`, `-> anyhow::Result<()>` or `-> Option<User>` directly. It then behaves exactly as in Rust: no implicit `Ok` on `return` or on the tail expression, `?` is allowed, and `throw e` (`return Err(e)`) is allowed in the `Result` case. `throws` is sugar over this, not a different mechanism; the two forms may coexist in one crate.

### 15.2 Propagation with `?`

```uredo
text = std::fs::read_to_string(path)?
user = client.fetch(id).await?

fn timestamp(s: str) -> u64?:                       # `?` on Option needs an Option-returning fn
    secs: u64 = s.strip_prefix('@')?.parse().ok()?
    Some(secs)                                      # a verbatim `Option` return wraps nothing (§15.1)  # ? on Option, mid-chain
```

`?` is Rust's postfix operator, unchanged: it propagates `Err`/`None` and applies `From` conversion. It is permitted in any function or closure whose return type is `throws`, `T?`, or a verbatim `Result`/`Option` (§15.1), in a `throws` closure (§17), and in `main() throws E`. The v0.1/v0.2 prefix `try` form is withdrawn: the corpus study found 3.8% of `?` mid-chain and 17% of async `?` as `.await?`, where a prefix operator needs parentheses, and `?` on `Option` in `Option`-returning functions is ordinary Rust that a `throws`-only rule made illegal. In expression position `e?` cannot be confused with the type `T?`: the two never occur in the same grammatical position.

In a `throws` function or closure, the operand of a **tail or returned** `?` (only the outermost `?` of a tail expression counts) must be an owned `Result`; `Poll<Result<…>>` and other `Try` operands are used through `rust { }`, and the mapped diagnostic says so. This is the one position where `?` is not Rust's operator unchanged (D37); everywhere else it is.

### 15.3 Returning

Inside a `throws` function or closure (constructors are written unqualified here for brevity; generated code qualifies them, §5.3):

| Uredo | Rust |
|---|---|
| `return e` | `return Ok(e)` |
| `return` (no value, unit `throws` body) | `return Ok(())` |
| tail expression `e` | `Ok(e)` |
| tail expression `e?` | the `match` below |
| `return e?` | `return` followed by the same `match` |
| no tail expression, unit function | `Ok(())` appended |
| `throw e` | `return Err(e)` |

A tail or returned `e?` does **not** lower to `Ok(e?)`: on a `Result` with a large error payload that leaves a stack temporary and a copy of the payload (round-trip study, p1). It lowers to

```rust
match __uredo_owned(e) {
    ::core::result::Result::Ok(__uredo_v) => ::core::result::Result::Ok(__uredo_v),
    ::core::result::Result::Err(__uredo_x) => ::core::result::Result::Err(::core::convert::From::from(__uredo_x)),
}
```

with `#[inline(always)] fn __uredo_owned<T, E>(r: ::core::result::Result<T, E>) -> ::core::result::Result<T, E> { r }` emitted once per crate. `e` is evaluated once, in place, as the scrutinee; no temporary binding or block is added; the identity call makes rustc reject a borrowed or non-`Result` operand (§15.2). Verified: with the same error type the result is emitted as an alias of a direct return; with a converting error type it uses no more instructions than `Ok(e?)`.

```uredo
fn archive(user: take User) throws ArchiveError:
    storage::save(user)?             # lowered by the tail-? rule above
```

### 15.4 Multiple error types

`?` conversion uses Rust's `From`. Uredo never boxes or unifies errors. An application error type is written once:

```uredo
@derive(Debug)
enum AppError:
    Io(std::io::Error)
    Json(serde_json::Error)

impl From<std::io::Error> for AppError:
    fn from(e: std::io::Error) -> AppError:          # `From::from` takes by value (D53)
        AppError::Io(e)

impl From<serde_json::Error> for AppError:
    fn from(e: serde_json::Error) -> AppError:
        AppError::Json(e)
```

Note the modes: implementing a foreign trait requires the modes the trait declares. For the std traits in the D53 table (`From` among them) Uredo supplies them, so `fn from(e: ParseIntError)` is already by value and a written `take` is optional; for any other foreign trait Uredo cannot read the signature, so the programmer writes `take`/`inout` to match it. `thiserror`-style derives are usable via `@derive` and `@rust(...)` attributes (§23).

### 15.5 Inspecting a `Result` without propagating

```uredo
match load_user(path):
    Ok(u):
        greet(u.name)
    Err(e):
        eprint("failed: {e}")
```

`if let Ok(u) = load_user(path):` and `.is_ok()` etc. are Rust's.

---

## 16. Loops and iteration

```uredo
for user in users:            # for user in &users   (users is a place)
    print(user.name)

for user in inout users:      # for user in &mut users
    user.active = true

for s in take strings:        # for s in strings     (consumes)
    store.push(s)

for i in 0..10:               # ranges and iterator expressions are passed as written
for u in users.iter().filter(u => u.active):
```

Rule: if the `for` source is a **place expression** (a binding, field or index), Uredo borrows it (`&place`) unless `inout`/`take` is written. Any other expression (a range, a method chain, a call) is an iterator or temporary and is used as written. `for x in take strings` is the only way a loop consumes a collection **held in a place**; an iterator written inline is consumed as in Rust.

Two consequences follow, both diagnosed in Uredo terms (§27):

- `for x in inout place` requires a mutable place, exactly as an `inout` argument does (§10.3): a `var` binding, an `inout` parameter, or a field or index of one. An `inout` parameter used directly as the source is reborrowed (`&mut *p`).
- A binding that holds an **iterator or a range** is still a place, so the rule borrows it — and a reference to an iterator is not one. Write `for i in take r` to consume it, or build the iterator in the loop header. The diagnostic names the binding and gives that fix.

Iterator chains are Rust's and stay lazy; materializing is explicit:

```uredo
active = users.iter()
    .filter(u => u.active)
    .cloned()                  # requires User: Clone; the clone is visible
    .collect::<Vec<User>>()
```

`while cond:` and `loop:` are Rust's, with `break`, `continue`, labelled forms `'outer: loop:` and `break 'outer`.

---

## 17. Closures

```uredo
x => x * x
(a, b) => a + b
(x: f64) -> f64 => x * x
() => Vec::new()
(&b) => b.is_ascii_digit()      # parameters are Rust patterns
(var acc, x) => acc + x         # `var` marks a mutable parameter binding
move x => x + offset            # forces by-value capture
```

**Closure parameters are Rust's.** They take Rust's pattern syntax and `var` for a mutable binding, with optional type annotations; an annotation is the exact Rust type, not a passing-mode input, and `take`/`inout` do not apply (`(s: String) => s.len()` takes `String` by value; `(x: take T) => …` is an error). This is a deliberate divergence from §10.2 and an exemption from the transfer rules of §38, which govern `fn` and method parameter declarations only. Calls through a closure binding pass arguments verbatim (§10.3).

Closures lower to Rust closures; rustc infers `Fn`/`FnMut`/`FnOnce` and the capture mode of each variable from the body, as it does for Rust. `move` is written when by-value capture is required (spawning threads or tasks). A multi-line closure body is an indented block, opened by a `=>` that ends its line: after `=` (anchored at the statement, as a `match`/`if` header is, §7.7) or as the trailing argument of a call, closed by `)` on its own line (§6.3, D51). A block's value is its last expression, as in Rust; a unit closure whose last statement is a call with a value writes `_ = call(…)`. A `=>` inside `[` or `{` is a continuation of a one-expression body, not a block. Parameters, return types and the `?`/`throws` rule are unchanged inside a block:

```uredo
handler = (req: Request) -> Response throws HttpError =>      # `req: Request` is by value: closure parameters are Rust's
    body = read(req)?
    Response::ok(body)
```

A closure body may use `?` only if the closure's return type is a `Result` or `Option`, written `(req: Request) -> Response throws HttpError =>` (or bare `throws` under a default, §15.1), or a verbatim `Result`/`Option`. Closures are never boxed implicitly.

---

## 18. Generics

Generic **use** (`Vec<T>`, `HashMap<K, V>`, turbofish) is Core v0.1. Generic **definitions** are Phase 1 surface, implemented by the compiler and exercised by the portfolio (topics 13, 14, 21, 25).

```uredo
fn print_all<T: Display>(values: [T]):
    for v in values:
        print(v)

# a signature, shown without its body
fn merge<T, U, Output>(a: T, b: U) -> Output
where:
    T: Serialize
    U: Serialize
    Output: FromParts<T, U>
```

Passing modes for type parameters follow §10.2 (`a: T` is `T` by value, P8; `T: ?Sized` is `&T`; in the `merge` example above `a` and `b` are moved). A public generic function has a deterministic Rust signature (§4.4); bounds are never inferred into a public signature — the only declared bound the mode reads is `?Sized`.

---

## 19. Traits and implementations

```uredo
trait Named:
    fn name(self) -> str          # receiver explicit: trait signatures have no body to infer from

impl Named for User:
    fn name(self) -> str:
        &name                      # &self.name; return-position `str` is &str with elided lifetime
```

Uredo preserves Rust coherence and orphan rules because generated code obeys them. Associated types, generic associated types and supertraits use Rust's own syntax: `trait Ord: PartialOrd:` declares a supertrait, and the colon that opens the body is the one at the end of the line (D60). An associated type may carry bounds and a `where` clause (§12.4). Trait definitions are Phase 1 surface (§33).

---

## 20. Modules, imports, and visibility

### 20.1 Imports

```uredo
use std::collections::HashMap
use serde::{Serialize, Deserialize}
use std::io::Result as IoResult
use rayon::prelude::*
```

`use` declarations are also permitted inside function bodies and blocks, with Rust's scoping (§6.3).

### 20.2 Visibility

`pub`, `pub(crate)`, `pub(super)` as in Rust. Items, fields and modules are private by default.

### 20.3 File-to-module mapping

Uredo uses Rust 2018 path rules with `.ure` substituted, and declares modules automatically:

- `src/main.ure` (bin) and `src/lib.ure` (lib) are crate roots.
- `src/model.ure` is module `crate::model`; `src/net/client.ure` is `crate::net::client`, with `src/net.ure` (optional) as the `net` module body. `src/net/mod.ure` is also accepted; declaring both is an error.
- Hand-written `.rs` files follow the same mapping and are declared alongside; `src/optimized.rs` is `crate::optimized`.
- Automatically declared modules are **private**. To export a module from a library, write `pub mod name` in the parent, exactly as in Rust; that explicit declaration replaces the automatic one. `pub use` re-exports as in Rust.
- A file and a directory with the same name (`model.ure` and `model/`) form one module (the file is the body).
- `mod name:` followed by an indented block declares an **inline module**; `pub mod name:` exports it. `mod name` without a colon is the file-module declaration above, never an empty inline module. An inline module and a file module may not share a name in one parent.

Generated `mod` declarations carry `#[path]` attributes pointing at hand-written `.rs` files where copying is not used; module names are identical either way.

---

## 21. Async and concurrency

`async fn` and postfix `.await` are Rust's; `.await?` composes as in Rust. Uredo ships no runtime; Tokio, smol, Embassy and others are ordinary dependencies, entered through forwarded attributes:

```uredo
@rust(tokio::main)
async fn main() throws AppError:
    user = get_user(42).await?
    print("{user:?}")

async fn get_user(id: u64) -> User throws HttpError:
    url = format("https://example.com/users/{id}")
    response = reqwest::get(&url).await?
    response.json::<User>().await?
```

`Send`, `Sync`, pinning and lifetimes remain Rust semantics; Uredo neither hides nor bypasses them. Async surface syntax is Phase 2 (§33) and is implemented (§0.1): `async fn`, `.await` and runtime attributes are written directly, and corpus programs 16 and 17 exercise them.

---

## 22. Rust interop

### 22.1 Direct Rust item imports

Ordinary `use` resolves Rust items. The `rust::` prefix is accepted for documentation (`use rust::std::simd`) and is optional: it says "this is a Rust item" to the reader and is dropped when the import is lowered, so `use rust::std::simd` and `use std::simd` generate the same line. Only a leading prefix is dropped — `use crate::rust::helpers` is a module named `rust` and is left alone.

### 22.2 Item-level Rust blocks

```uredo
rust {
    pub fn popcnt(x: u64) -> u32 { x.count_ones() }
}
```

Items declared in a module-level Rust block belong to the enclosing module. Their signatures are **opaque to Uredo** (§9): Uredo can call them, passing arguments verbatim.

### 22.3 Statement and expression blocks

```uredo
rust { counter += 1; }                          # statement block: a Rust block expression; its `let`s are not visible outside
value: f32 = rust { unsafe { *ptr.add(index) } }   # expression block: the annotation is required
```

An expression block must carry a type annotation on its binding (or appear where the type is otherwise declared), because rustc types it after Uredo's pass — except in tail position (including as the operand of `return`), where no annotation is needed because no Uredo elaboration consults the type of a tail expression. In a `throws` body a tail `rust { }` supplies the *success* value and the `Ok` wrapping of §15.3 applies outside it. `?`, `.await` and `return` inside a Rust block refer to the enclosing generated Rust function, exactly as they do in Uredo code, and the enclosing function must admit them: `?` needs a `throws` function or one returning a verbatim `Result`/`Option` (§15.1), `.await` needs an `async` function, and `return` needs a matching return type. No Uredo elaboration is applied inside the block, so a `return` there is Rust's `return` on the generated signature.

### 22.4 Rust files

`.rs` files are members of the crate (§4.3, §20.3).

### 22.5 Macros

Rust macros are invoked **directly**, with Rust's own syntax, in expression, statement and item position:

```uredo
rows = sqlx::query!("SELECT id, name FROM users WHERE active = $1", active).fetch_all(&pool).await?
numbers = vec![1, 2, 3]
macro_rules! square { ($e:expr) => { $e * $e }; }      # item position
```

Lexing: a path followed by `!` and an opening delimiter switches to Rust token mode until the balanced close (§6.6). The macro's contents are opaque tokens — **Rust source**: no intrinsics, no call-site auto-borrow and no `throws` elaboration apply inside the delimiters; write `format!`, `&x`, `Ok(...)` there as in Rust. Uredo bindings in scope are Rust bindings of the same name in the generated function (raw identifiers written as `r#name`), so `active` above is captured with no further syntax. The lexer also switches to Rust token mode after `macro_rules! name` followed by a delimiter. The result of a macro expression is opaque to Uredo (§9): it is an ordinary argument expression, so a resolved callee's §10.3 rule applies to it (it is not syntactically a reference, so a P2/P3 parameter receives `&`), while the tokens inside the delimiters stay verbatim. A binding of it needs a type annotation only where a `rust { }` expression would (§22.3). `print`/`format` remain Uredo intrinsics (§11.3), so `println!` is legal but redundant. There is no `rust!` prefix form.

### 22.6 Procedural macros

Derive and attribute macros apply to generated items; Uredo forwards them (§23). Anything they generate is opaque to Uredo (§22.2).

---

## 23. Attributes

```uredo
@derive(Debug, Serialize)
@cfg(target_os = "linux")
@allow(dead_code)
@test
@rust(serde(rename_all = "camelCase"))       # exact forwarding of any attribute
```

`@name(args)` lowers to `#[name(args)]`; `@rust(tokens)` lowers to `#[tokens]` unchanged. A field attribute is written on its own line above the field it attaches to (`@serde(default)` above `name: String`); a struct-*literal* field takes its attribute inline, on the field itself (D44). A field of a **tuple struct or a tuple variant** has no line of its own, so its attribute is written inline before the type — `Parse(@from serde_json::Error)` — and lowers to `#[from]` in that position (D55). Rust's **name-value** attribute is written the same way it is read, `@name = value`, with the value the rest of the line: `@must_use = "the result is the new state"`, `@path = "platform/linux.rs"`, `@label = "user."` for a derive that reads one (D57; corpus: 797 occurrences, `doc` 327, `must_use` 140, `should_panic` 102, `path` 60). This is the spelling thiserror's `#[from]` needs, and without it an error enum has to be written in a `rust { }` block. An attribute may also precede a field in a struct literal (`@cfg(debug_assertions) type_name: …,`); it attaches to that field and rustc judges its validity. Attributes on block expressions are not supported; use `rust { }`. Crate-level attributes (`@!no_std`, `@!allow(...)`) are written at the top of the crate root and lower to `#![...]`.

An `@allow`/`@expect` naming one of `uredo lint`'s own lints (§29.1) is **still forwarded**, since Uredo never changes what reaches rustc, and a warning says what happened: the attribute silences nothing in Uredo and rustc will report `unknown lint` on the generated Rust. Forwarding rather than consuming keeps the attribute working the day rustc ships a lint of that name, which is why the diagnostic is a warning and never an error.

Two attributes are **Uredo's own** and are consumed, not forwarded:

| Attribute | Position | Meaning |
|---|---|---|
| `@copy use path::Type` | on a `use` item | declares the imported type known-Copy for this crate (§10.1); a `Copy` assertion is emitted |
| `@!default_error(Type)` | crate root or module top | the error type a bare `throws` denotes in that subtree (§15.1) |

---

## 24. Unsafe code

`unsafe` is never inferred.

```uredo
unsafe:
    ptr.write(value)
```

or exact Rust in a `rust { unsafe { … } }` block. Crossing Rust's safety boundary stays explicit and auditable.

---

## 25. Lowering contract

Generated Rust preserves the observable behaviour of the Uredo program under these rules:

- **Evaluation order and count:** operands are evaluated left to right, once, exactly where written. The generator never hoists an expression into a temporary binding, because doing so extends the temporary's drop scope (verified: a guard passed as `&make_guard()` drops at the end of the statement; a `let`-bound one drops at the end of the block).
- **Temporaries and drop scopes:** Rust's rules for the code as generated; the generator does not introduce blocks or closures around user expressions. In particular, `return`, `?`, `break` and `.await` are never wrapped in a generated closure.
- **Synthetic names** carry a reserved prefix (`__uredo_`) and are never visible to Uredo code.
- **`rust {}` blocks** contribute items to the enclosing module (item position) or are ordinary block expressions (statement/expression position). Source text and positions inside them are preserved byte-for-byte by lowering (the formatter may reformat a block in the *source*, §29, but generation never rewrites the block it is given) so that source-location-sensitive macros (`file!`, `line!`, `include_str!`) see what they would in Rust; line numbers are those of the generated file, and the spec does not promise otherwise.
- **Module structure** is preserved so that Rust's module-file lookup for hand-written `.rs` files is unchanged. Cargo's own target directories keep Cargo's meaning: a file directly under `src/bin/`, and a `main` in a directory under it, is a crate root rather than a module, and is never declared with `mod` — as `tests/`, `examples/` and `benches/` already are (§35).
- **Files beside the source travel with it.** Every file under `src/` that is not Uredo source is copied into the generated crate at the same relative path, so that `include_str!`, `include_bytes!` and `include!` resolve against the same neighbours they would in Rust, and so that the package `uredo package` exports is the package that was written.
- **`CARGO_MANIFEST_DIR` names the generated crate**, under `target/uredo`, because that is the crate rustc compiles and the one that is exported; it is not the authored package's directory. A path built from it therefore reaches the generated tree — the manifest, its copied extras and `src/` with everything beside the source — and not the checkout above it. Uredo does not rewrite the variable, since a literal path would make the exported package unbuildable anywhere else.

Any generated construct not covered by an audited elaboration rule is a lowering defect (§5.4).

---

## 26. Memory and performance transparency

### 26.1 What is always explicit

Everything on the canonical list in §3.2, in source or in the name of the Rust API called (`Vec::from`, `format`, `.cloned()`, `Box::new`, `Arc::new`, `Mutex::lock`).

### 26.2 `uredo explain`

```bash
uredo explain src/main.ure:42
```

```text
Expression: process(data)                      src/main.ure:42:5
Uredo elaboration:   process(&data)
    rule: P3 (default shared borrow; callee is Uredo fn `process`)
Inserted by Uredo:   borrow: shared · clone: none · allocation: none · dispatch: none · sync: none
Rust adjustments:    unknown (no semantic engine in v0.x)
Callee effects:      unknown (not analysed)
```

Evidence states are `proven`, `declared`, `observed`, `unknown`. `explain` never reports "no allocation" for anything it did not itself insert. For an opaque call it reports `arguments passed verbatim`.

### 26.3 Generated Rust view

`uredo rust src/main.ure` prints the generated file; the IDE command "Uredo: Show Generated Rust" is a core feature of the editor integration (planned with the LSP, §0.1).

---

### 26.4 Documentation and its examples

`uredo doc` lowers the package and runs `cargo doc`. A `##` comment becomes `///` and a `##!`
comment `//!`, as §6.4 says; what needed deciding is the **fenced code block** inside one, and the
answer is D58:

| Fence | Meaning |
|---|---|
| ```` ``` ```` or ```` ```uredo ```` | Uredo source. It is lowered and emitted as an ordinary doctest, so rustdoc compiles and runs it, and `uredo test` reports it beside the other tests |
| ```` ```no_run ````, ```` ```should_panic ````, ```` ```compile_fail ````, ```` ```edition2021 ```` … | the same, with rustdoc's attribute carried across |
| ```` ```rust ```` | Rust already; passed through untouched, for an example that has to be written in the target language |
| ```` ```text ````, ```` ```ignore ````, any other tag | rendered and never compiled, by rustdoc's own rule |

An example that does not lower is an error on the item it documents, naming the two escapes. The
example is lowered by wrapping it in a function, so it may declare items, use `?`, and write
anything a function body may.

**The rendered example is Rust, and that is deliberate.** The documentation belongs to the generated
crate, which is what a consumer of an exported package reads (§4.6): a Rust programmer, reading Rust
docs, who needs an example they can paste. The alternative — render the Uredo and leave it untested
— was rejected because the corpus writes 3,210 doc examples across 562 files and rustdoc compiles
almost all of them; an untested example is the one part of a document that rots silently. An Uredo
author still writes the example in Uredo, and still finds out when it breaks.

## 27. Compiler diagnostics

Diagnostics are translated by **provenance**, not by text substitution. Each generated construct knows its Uredo node and rule; rustc's JSON diagnostics (primary and secondary spans, suggestions, macro backtraces) are mapped through that record.

```uredo
enqueue(job)
print(job.id)
```

```text
error: `job` cannot be used here because `enqueue` takes ownership of it
  --> src/main.ure:12:5
   |
11 |     enqueue(job)
   |             ^^^ ownership transferred here
12 |     print(job.id)
   |           ^^^ used after transfer
   |
   = `enqueue` declares: fn enqueue(job: take Job)
   = help: borrow instead if `enqueue` does not need ownership, or write `enqueue(job.clone())`
   = note: Uredo did not insert a clone because cloning can allocate or copy substantial data
```

Rules:

- A mapped error family (use after move, missing `var`/`inout`, immutable argument to `inout`, missing `?`, wrong receiver) is rewritten in Uredo terms with Uredo spans.
- Any other rustc error is forwarded with its text, the nearest Uredo anchor, and the generated-Rust excerpt. An error raised **inside a macro** carries a span in the macro's own file, often in a dependency; the expansion chain is followed back to the call site, so a compile-time-checked query or a derive's complaint is anchored at the Uredo line that wrote it. Rust suggestions are applied to Uredo source only when a syntax-aware inverse transformation validates them.
- The compiler never invents a source location or an explanation it cannot justify from provenance.
- The raw rustc diagnostic is shown alongside the translated one (in the CLI, under the translated message; in an editor, in a secondary panel).
- Negative diagnostic fixtures are part of the Phase 0 gate (§34).

---

## 28. Toolchain, CLI and versioning

### 28.1 Commands

```bash
uredo new hello [--lib] [--name n]
uredo init [dir] [--lib] [--name n]
uredo check              # lower + cargo check
uredo check --api        # public API manifest diff (§4.4)
uredo build [--release]
uredo run
uredo test               # @test functions -> #[test]; delegates to cargo test
uredo doc [dir] [--open] # lower, then `cargo doc`; examples are lowered into doctests (§26.4)
uredo fmt
uredo lint [--deny] [--allow <lint>]
uredo lint --measure     # the D52 reversal counts (§38), which are not findings
uredo fix [--dry-run]    # apply the findings whose rewrite leaves the generated Rust unchanged
uredo explain <file>:<line>
uredo rust <file>        # show generated Rust
uredo package            # self-contained Cargo package (§4.6)
uredo publish [dir] [--dry-run] [--allow-api-change]   # export, check the API (§4.4), then `cargo publish`
uredo lsp                # a language server on stdio: diagnostics, formatting, hover
uredo report [dir] [--out path]   # bug bundle for a lowering defect (§5.4)
```

Every `.ure` file directly under `tests/`, `examples/` or `benches/` is lowered as the Cargo target it is: a separate crate, not a module of the library, compiled against the package by name (§35's target-kind field). No `mod` declaration is added for it, and a generated target whose source has been deleted is removed on the next build.

A relative `path` dependency is rewritten as the generated crate is written, since that crate does not sit where the authoring package does; a target's own `path` is left alone.

`uredoc` is not a separate binary. All build commands lower every Cargo target then delegate to Cargo; the generated crate is kept under `target/uredo/` with its provenance maps. Implemented in v0.3: `new`, `init`, `rust`, `check`, `check --api`, `build`, `run`, `test`, `doc`, `package`, `explain`, `fmt`, `lint`; and in v0.4 the last three, `publish`, `fix` and `lsp` (§0.1).

**`uredo fix` applies only what it can prove changed nothing.** §28.2 promises the command for migrating source across a breaking syntax change; none has happened, so there are no migrations, and what it does today is apply the findings whose rewrite is invisible in the output. The contract is checked rather than argued: the file is lowered before and after each edit, and an edit that moves a single byte of generated Rust is put back and reported as refused. `redundant_take` is the only finding that qualifies, because that lint's whole claim is that the annotation changes no signature; `borrowed_container` and `copy_candidate` change what a function accepts, so they stay suggestions. `fix` cannot know that a finding is *deliberate* — the one it finds across this repository is the illustration in portfolio topic 05 — which is the same gap §29.1 holds open for source-level suppression.

**`uredo lsp` shares the compiler's front end**, so the diagnostics an editor shows are the diagnostics `uredo check` prints, on the same Uredo lines, and hover is what `uredo explain` says about that line (§26.2). It speaks `initialize`, `shutdown`/`exit`, full-text sync, `publishDiagnostics` (errors, warnings and lints), `textDocument/formatting` and `textDocument/hover`; every request with an `id` is answered, including one it does not support, since a request left unanswered hangs the editor. On a file that does not parse it answers from what still did. The parser recovers at item and statement boundaries — a broken line costs its own statement, a broken header costs its own item and no more — so `compile_tolerant` lowers the rest and hover answers from it, saying that the file has an error elsewhere. Only when the broken item is the one being asked about does the server fall back to the last text that compiled, and then only for a line unchanged since. What is still owed is the *lossless* half: the parser keeps no trivia and no error nodes, so it cannot reconstruct the source it was given, which is what a rename or a code action would need.

`new` creates the directory and `init` fills one that exists; both write the §4.1 manifest, a
`.gitignore` holding `target`, and either `src/main.ure` — a `main` that prints an interpolated
string — or, with `--lib`, `src/lib.ure` with one documented function and one `@test`. The package
name comes from the directory, cleaned to what Cargo accepts, or from `--name`. Neither command
overwrites anything: an existing directory, manifest or entry file is an error naming the other
command.

### 28.2 Versioning and stability

- Each Uredo release declares a **supported rustc range** (tested lower and upper bound) and the **Rust edition it emits** (2024 for v0.x).
- Exported packages set `rust-version` to the generated code's true MSRV, which is independent of the Uredo compiler's own requirement.
- Uredo surface syntax follows semver: breaking syntax changes occur only in 0.x minor releases, each shipped with `uredo fix` migrations (the command exists, §28.1; no breaking change has yet needed one). An edition mechanism modelled on Rust's is reserved for post-1.0 breaks.
- Diagnostic translation keys on rustc error codes and provenance, never on message text, so rustc message changes do not break it.
- The LSP (implemented in v0.4, §28.1) is to run on the tolerant, lossless parser and work on files that do not compile — **tolerant since v0.4**, in the sense that the parser recovers at item and statement boundaries and the server answers from the items that survived; *lossless* it is not, the parser keeping no trivia and no error nodes; it shares syntax trees and provenance with the compiler and formatter. Rust semantic queries, when added, go through a version-pinned adapter.
- Debugging: Rust has no `#line` directive, so debug info names `target/uredo/*.rs`. Line-structure-preserving codegen (one Uredo statement → one Rust line where possible, never reordered) is a design constraint so that generated lines correlate with source in v0.1. In Phase 2 the provenance map (§5.3) is exported per build as a machine-readable file, and Uredo ships GDB and LLDB Python helpers plus a VS Code debug-adapter hook that translate `break main.ure:42` to the generated line and show `.ure` locations in frames and backtraces, using only public debugger extension APIs. **Implemented in v0.4** (`compiler/debug/`): `ubreak FILE.ure:LINE` sets the breakpoint, `uwhere` prints the backtrace in Uredo locations with the source line beside each frame, and `ulist` shows the Uredo source around the current frame; the VS Code hook is an `initCommands`/`setupCommands` entry in `launch.json` rather than an adapter of Uredo's own, so the editor's source view still shows the generated Rust and the debug console is where the Uredo view lives. Stepping is Rust's. Rewriting DWARF/PDB line tables is undertaken only if the corpus shows those helpers insufficient.

---

## 29. Formatting

A canonical formatter exists (`uredo fmt`, token-based, idempotent on the portfolio and the corpus):

- 4-space indentation; one blank line between items; trailing commas in multi-line literals and calls, except before the closing line of a trailing block argument, where D51 forbids one (§6.3)
- one canonical layout, minimal configuration
- Rust blocks are formatted by rustfmt (pinned version); comment style inside them is untouched
- generated Rust is formatted by the same rustfmt before provenance is computed
- an inline body (§6.3) is kept on one line only when it is a single expression or statement and the line fits rustfmt's default width (100 columns); otherwise the formatter expands it to a block
- a same-line `else` (§6.3, D49) is kept when the whole `if … else …` fits the width; otherwise both bodies are expanded to the indented form together — the formatter never leaves an inline `if` body with a block `else`
- **two lines are never expanded, however wide.** A *continuation* — a line inside an unclosed bracket, whose own brackets therefore do not balance — is not a statement, and breaking it at the colon of an `if` or `match` it happens to contain produces text that is not a program. A **`match` arm** is the one inline body whose lowering depends on how it was written (§13.1 gives `pat: expr` a bare expression and an indented body a block), so expanding one would change the generated Rust. Both were defects, both found by formatting real code rather than the portfolio, and `compiler/tests/fixtures/fmt_hard/` keeps the shapes so they cannot return
- **the formatter's contract is meaning-preservation, checked over every `.ure` file in the repository**: a program that compiled must still compile, generate the same Rust, and formatting twice must equal formatting once. The portfolio alone cannot check this — its snippets are written to be canonical and short, so they never make the formatter wrap a line

### 29.1 Lints

`uredo lint` reports what the passing-mode rules make pointless or costly, and nothing else: every
finding is decided from the declarations the lowerer already reads, so a lint never disagrees with
the signature the compiler generates. Findings are warnings — they never fail a build — and
`--deny` exits non-zero for a pipeline that wants them fixed, `--allow <lint>` silences one.

| Lint | Fires on | Says |
|---|---|---|
| `redundant_take` | `take` on a known-Copy type, a type parameter or `impl Trait` | the mode is by value already (P1, P8), so the annotation changes no signature; a known-Copy `take` copies rather than moves |
| `borrowed_container` | a `Vec<T>` or `String` parameter, whether the `&` is written or inferred | it passes as `&Vec<T>`/`&String`; `[T]`/`str` accepts slices, literals and the owned container at the same cost |
| `copy_candidate` | a P3 parameter whose every use in the body is `*x` | the value is being copied out of a borrow; if the type is `Copy`, `@copy use` (foreign) or `@derive(Copy)` (declared here) passes it by value (§10.1) |

`uredo lint --measure` is the one thing this command prints that is **not** a finding. §38 makes
the by-value rule for type parameters (D52) reversible on two counts, and one of them — a `.clone()`
written at a call site to feed a P8 parameter — is visible exactly where the passing modes are
resolved, so it is collected here rather than in a second tool. It is reported at `note` level, and
`--measure` prints it *instead of* the findings, because a clone at a by-value call site is not a
defect: it may be precisely what its author meant, and a lint that fires on correct code would earn
the suppression surface this section has so far refused. What the mode is costing is the ratio, not
any one site. The counterpart count, the `take` annotations a borrow default would have required, is
the number of parameters passing by value under P8 — one per declaration; a `Copy`-bounded type
parameter is not among them, since it was by value under either rule (§10.1). The other count in
§38's condition comes from a build log: the mapped E0382 for a P8 move ends its message with `(P8)`.

`--measure` reports two other things for the same reason. **What D2 costs**: the `&` and `&mut` an
author writes because the callee is a Rust item, which Uredo would have inserted had the callee been
its own (§10.3) — the measure that decides whether the surface's borrow-insertion is worth what it
does not reach. And **the lifetimes written, by position** — reference, bound, generic argument or
binder, loop label, with `'static` and `'_` counted out of the total — because §38 holds the
apostrophe notation open on a rate per 1,000 lines, and a trigger nobody can evaluate is not one.
The buckets are the corpus study's, so a project's rate can be set beside real Rust's. Lifetimes are
counted from the token stream: an apostrophe in a comment, a string or a char literal is not one.

A lint is added only when it restates a rule this document already carries, and the wording names
the rule. The portfolio keeps one deliberate `redundant_take` finding, in the topic that illustrates
`take` on a known-Copy type; the corpus and the examples are clean.

**Silencing one in source is not decided** (panel 2026-09-12, `oracle-reviews/lint-allow/`). `--allow`
is per-run, so a project with one deliberate finding loses that lint's coverage everywhere else, and
three shapes could fix it. None is adopted, and the shortlist stays open because the evidence that
would choose between them — whether a deliberate finding lands on one parameter or on a whole file —
does not exist until real Uredo projects do:

| Shape | Verified constraint |
|---|---|
| a consumed attribute, `@lint(allow(redundant_take))`, joining the two of §23 | it must be stripped completely: an unknown attribute is a rustc **error** (`cannot find attribute`), unlike an unknown lint name, which is a warning. A tool namespace is not available — `#[allow(uredo::x)]` is E0710 and `#![register_tool(uredo)]` is E0658 on stable |
| a comment directive, `# uredo: allow(redundant_take)` | needs no lexer change: findings carry the parameter's own line, including in a signature written over several lines, and the lint pass holds the source. It would make a `#` comment semantically significant for the first time, and the formatter would have to stop treating it as free text |
| per-path configuration in the package manifest | no source surface, and it matches a deliberate *file*; it cannot mark one parameter, which is the shape 86.5% of real Rust suppressions take |

Overloading `@allow` is rejected: §23 lists `@allow(dead_code)` as forwarded, and the meaning of a
name would change the day rustc ships a lint that shares it. Forwarding the attribute and hiding
rustc's `unknown lint` warning is rejected on measurement: the warning then reaches everyone who
builds the package `uredo package` exports.

**Revisit when either is countable**: a deliberate finding on more than 5% of the files of a real
Uredo project outside these suites, or a project that must pass `--allow` for a lint it otherwise
wants enforced. Until then `--allow` is the only suppression, and §23's warning says so at the point
where a user reaches for the attribute.

---

## 30. Standard library policy

Uredo does not fork the Rust standard library. `std::fs`, `std::io`, `std::collections`, `std::sync`, `std::thread` are used directly. The prelude is exactly Rust's prelude. `print`, `eprint`, `format` and `panic` are intrinsic bare-call names (§11.3), not shadowable prelude items. No alias hides a cost or a storage model.

---

## 31. Example program

### 31.1 Uredo

```uredo
@!default_error(AppError)                            # crate root: the error a bare `throws` means

use serde::Deserialize

@derive(Debug, Deserialize)
struct User:
    id: u64
    name: String
    active: bool
    balance: f64

@derive(Debug)
enum AppError:
    Io(std::io::Error)
    Json(serde_json::Error)

impl From<std::io::Error> for AppError:
    fn from(e: std::io::Error) -> AppError:          # `From::from` takes by value (D53)
        AppError::Io(e)

impl From<serde_json::Error> for AppError:
    fn from(e: serde_json::Error) -> AppError:
        AppError::Json(e)

fn active_balance(path: str) -> f64 throws:          # bare throws = AppError (§15.1)
    text = std::fs::read_to_string(path)?
    users = serde_json::from_str::<Vec<User>>(&text)?
    var total = 0.0
    for user in users:
        if user.active:
            total += user.balance
    total

fn print_report(path: str) throws:
    balance = active_balance(path)?
    print("Active balance: {balance}")

fn main() throws AppError:                           # entry point: written explicitly for readers
    print_report("users.json")?
```

### 31.2 Generated Rust

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct User {
    id: u64,
    name: String,
    active: bool,
    balance: f64,
}

#[derive(Debug)]
enum AppError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> AppError {
        AppError::Io(e)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> AppError {
        AppError::Json(e)
    }
}

fn active_balance(path: &str) -> ::core::result::Result<f64, AppError> {
    let text = std::fs::read_to_string(path)?;
    let users = serde_json::from_str::<Vec<User>>(&text)?;
    let mut total = 0.0;
    for user in &users {
        if user.active {
            total += user.balance;
        }
    }
    ::core::result::Result::Ok(total)
}

fn print_report(path: &str) -> ::core::result::Result<(), AppError> {
    let balance = active_balance(path)?;
    ::std::println!("Active balance: {balance}");
    ::core::result::Result::Ok(())
}

fn main() -> ::core::result::Result<(), AppError> {
    print_report("users.json")?;
    ::core::result::Result::Ok(())
}
```

This Rust is closed (no undeclared names) and compiles with serde 1 and serde_json 1 (verified 2026-09-10). Function **bodies** may be generated differently as long as §25 holds; **signatures** may not (§4.4).

---

## 32. What Uredo deliberately does not simplify

Everything on the canonical list in §3.2, plus: `unsafe`; explicit lifetimes where an API genuinely communicates them; FFI; the choice between `[T]`, `Vec<T>` and `[T; N]`; the `From` impls of an error type.

---

## 33. The core surface and deferred features

**"Core surface"** names the frozen v0.1 *language* surface throughout this document; it is not the version of this document, which is v0.4 (§0.3). Where an older sentence says "in v0.1" it means the core surface.

### 33.1 Construct → phase → corpus program

As of v0.3 every row up to and including the Phase 2 rows is implemented by the reference compiler and exercised by the programs named; the Phase 3 rows are reachable through Rust syntax and `rust { }` and are exercised by portfolio topics 15, 23 and 25 and corpus program 20.

| Construct | Phase | Corpus programs (§43) | Notes |
|---|---|---|---|
| `use`, modules, `pub` | 0 | all | file mapping §20.3 |
| bindings, `var`, typed bindings, `const`/`static`, `const:`/`static:` groups | 0 | 1–4 | §8 |
| scalars, `str`/`String`, arrays, slices, tuples | 0 | 2, 3 | `Vec` via `Vec::from` |
| functions, tail return, `main` | 0 | all | |
| `print`/`format`/interpolation | 0 | 1, 3 | §11.3 |
| `struct`, construction, field visibility | 0 | 4 | |
| `if`/`else`, `for`, `while`, `loop`, ranges | 0 | 2, 10 | |
| passing modes P1–P8, call-site rule | 0 | 13, 14, 15 | the bet; P8 (type parameters by value) is D52 |
| inline bodies, one-line arms, `else` anchor, colon-less item headers | 0 | all | §6.3 |
| items in blocks, inline `mod name:` | 0 | 19 | §6.3, §20.3 |
| item-level `rust {}` | 0 | 19, 20 | |
| `enum`, `match`, guards | 1 | 5 | |
| `T?`, `if let`, `while let` | 1 | 6 | |
| `throws`/`?`/`throw`, `From` impls, verbatim `Result` returns | 1 | 7, 8, 9 | |
| methods, explicit receivers, read-only field sugar, nested trait impls | 1 | 4, 12 | §12.2 |
| `@derive`, attributes, `@test` | 1 | 8, 12 | |
| closures, iterator chains | 1 | 10 | |
| generic definitions, `where` | 1 | 11 | generic *use* is Phase 0 |
| trait definitions and impls | 1 | 12 | |
| statement/expression `rust {}`, `unsafe:` blocks | 2 | 20 | portfolio 23 |
| direct macro invocation `path!(…)` | 1 | 8, 20 | lexer support in Phase 0 |
| `let` shadowing, pattern bindings, `let-else`, `var` by-value bindings, `@copy use`, `@!default_error` | 0 / 1 | 3, 9, 13 | |
| `async`/`await`, runtime attributes, trailing block arguments (D51) | 2 | 16, 17, 10 | |
| `uredo package`, clean-consumer test | 0 (gate) | — | §4.6 |
| diagnostics provenance, negative fixtures | 0 (gate) | — | §27 |
| a Uredo spelling for lifetimes (Rust's `'a` syntax is the rule, §9.7; a postfix notation is held, §38) | 3 | 15 | |
| const generics, generic associated types, pinning | 2 | — | implemented and exercised in native syntax (`examples/compat/typelevel`); const generics needed nothing, the other two needed a `where` clause on an associated type and a written receiver type (D59) |
| FFI, `no_std` | 3 | — | via raw Rust, and exercised that way (§35, `examples/compat/{wasm,nostd}`) |

### 33.2 Not in the core surface

Three different states are listed together below, and they are not the same: a **deferred** proposal may still be adopted; a **decided exclusion** was considered and settled the other way (receiver inference, D27; an indentation-delimited raw block, D28); a **rejected** proposal will not return (implicit `use super::*` in test modules). Everything a deferred item would have spelled more briefly is still expressible — in Rust syntax, in a `rust { }` block, or by writing the longer form; that is what "reachable via raw Rust" means, and it does not apply to the rejected item, which has no spelling at all.

Named arguments, default arguments, optional chaining, pattern `ref`/`&` binders beyond Rust's, `.vec()`-style aliases, turbofish-free generic calls, indentation-delimited Rust blocks, receiver inference, DWARF line-table rewriting, associated-type sugar, operator overloading sugar (use `impl std::ops::Add`), doc-tests, bound aliases (`trait Name = …`, held until defined over the bound AST with hygiene and cycle checks), implicit `use super::*` in test modules (rejected), attributes on block expressions.

Every item here is a syntax decision not yet made wrong.

---

## 34. Implementation phases

### Phase 0: architectural acceptance gate

Goal: prove the elaboration boundary (§5.2), the passing-mode rules (§10) and the distribution model (§4.6) before parser, IR and generator acquire assumptions.

Implement narrowly (the list as the gate was run, before D52 added P8): lexer/parser, bindings, scalars, functions, `struct`, `if`/`for`, passing modes P1–P7 with the call-site rule, item-level `rust {}`, module mapping, deterministic generation with provenance, Cargo invocation, `uredo package`.

**The gate.** All of the following must pass, with thresholds set before running (passed on 2026-09-11, §0.1):

1. An Uredo call into a **generic Rust callee** with distinct owned and borrowed interpretations (the `route<T: Mode>` case): arguments pass verbatim; the program means what the source says; `explain` says "verbatim".
2. A method **generated by a derive macro** on an Uredo struct, called from Uredo.
3. A closure crossing an ownership-sensitive external API (`thread::spawn` with `move`).
4. A deliberately **rejected** ownership program (use after `take`) with a diagnostic mapped to Uredo spans, and an immutable argument to `inout` rejected in Uredo terms.
5. A mixed `.ure`/`.rs` package **exported with `uredo package` and consumed from a clean plain-Rust project** with no Uredo installed.
6. One feature-flag-dependent build and one cross-target `check`.
7. A paired code-generation comparison (§36) for two small kernels, showing identical optimized IR.

Record: explicit-annotation count per program, raw-Rust share by operation, diagnostic mapping rate, cold/warm build time. Keep failures in the corpus. **Do not freeze §10 or §12.3 until the gate passes.**

### Phase 1: application language

Add enums, `match`, `T?`, `throws`/`?`/`throw` with `@!default_error`, methods, derives/attributes/`@test`, generic and trait definitions, closures, iterator ergonomics, diagnostic mapping for the listed error families.

Success: the §43 corpus programs 1–15 — the application-style set, being those that solve a task rather than demonstrate an integration — meet the §37 thresholds. *Met on 2026-09-12 (§0.1).*

### Phase 2: serious Rust interoperability

Add statement/expression Rust blocks, async surface syntax and runtime attributes, procedural-macro integration tests, the exported provenance map and the map-aware debugger helpers (§28.2), `uredo test`/`doc` integration, the full §35 corpus.

Success: Tokio, Serde, Axum, Rayon, SQLx (via raw Rust), clap and nalgebra usable without Uredo-specific wrappers, with the native/raw split published.

### Phase 3: systems capability

FFI, pinning, GATs, const generics, SIMD via Rust, `no_std` (crate-root attribute forwarding designed in Phase 0, exercised here). Explicit lifetimes are written in Rust's syntax and work today (§9.7); an Uredo spelling for them is held (§38). `unsafe:` blocks are Phase 2 (§33.1) and implemented.

Success: an expert stays in one Uredo project from application code down to specialized Rust systems code.

---

## 35. Compatibility test suite

Each corpus entry is a contract, not a crate name:

```text
crate + version + features
Cargo target kind (lib / bin / integration test / example)
required native-Uredo operations          (must work in Uredo syntax)
permitted raw-Rust escapes                 (by operation, with the reason)
expected behaviour on fixture inputs
expected diagnostics for the paired negative case
clean-archive consumption                  (uredo package -> plain cargo build)
mutation rebuilds                          (edit .ure / .rs / Cargo.toml / feature; stale output removed)
```

Corpus (native-syntax vs via-raw-Rust recorded per entry): serde derive, serde_json, Tokio main/test, reqwest, Axum, Rayon, SQLx macros (raw), regex, clap derive (field attributes via `@`), tracing, anyhow/thiserror, nalgebra, ndarray, wasm-bindgen, C FFI, `build.rs`, proc macros, feature flags, `cfg`, `no_std` (Phase 3).

**State of this suite: `docs/COMPATIBILITY.md`**, generated by `docs/compat/record.py` from the
per-entry contracts in `docs/compat/entries.json` and from the sources themselves, so the record
cannot drift from what it describes. It states for each entry what is exercised, in which target
kind, which escapes are permitted and why, the record that shows the entry behaving correctly, and
what is missing. **The counts live in that file and are not repeated here**, because it is generated
from the entries and the sources and this paragraph is not: §0.1 carries the state, and the file
carries the detail.

The three gaps that belonged to no single entry are closed, and each was closed by building the
fixture rather than by argument: `tests/`, `examples/` and `benches/` are lowered as the Cargo
targets they are, so a package can have all four kinds; the mutation-rebuild sequence runs on a copy
of a fixture, covering an edit, a feature switch, two deletions and a manifest change; and the
database entry carries a crate-level negative case whose diagnostic arrives through a proc macro,
which is what forced §27 to follow a macro's expansion back to the call site.

Locked-dependency CI is reproducible; a scheduled job tests dependency updates separately.

---

## 36. Performance validation

"Equivalent optimized code within normal compiler variance" is not a test. These are:

| Obligation | CI gate |
|---|---|
| Semantic agreement | Paired Uredo and Rust programs agree on outputs, errors and instrumented side effects for fixture inputs |
| Lowering discipline | Every synthesized construct belongs to an audited rule (§25); prohibited runtime helpers fail compilation tests |
| Exact codegen fixtures | Selected kernels produce identical normalized optimized LLVM IR / assembly under identical settings (pinned toolchain, target-cpu, profile, panic strategy, LTO, codegen-units) |
| Allocation fixtures | Allocation count and bytes (global-allocator counter) do not exceed the paired baseline |
| Runtime regression budget | **Retired 2026-09-13, on its own controls' evidence.** It asked that the upper one-sided 99% bound of the paired runtime ratio be ≤ 1.02, with two controls deciding whether a run could resolve 2%: the original timed against itself, and against its own source compiled as a second crate. The second control settled the obligation by failing it — its ratio is 1.0 by construction and its *median* was 1.0248 on one piece, so the crate boundary alone costs more than the budget, and no sample count removes a systematic cost. Instruction identity replaces it (the row below), being what the budget was a proxy for. A timing budget returns only with a method that has no crate boundary in it |
| Size and memory | Checked-in limits per fixture; increases need a reviewed baseline change |
| `throws` tail fixture | For an owned `Result<T, E>` with a 32-byte `E` and the same error type, the tail-`?` lowering (§15.3) is emitted as an alias of the direct return; a converting tail uses no more instructions than `Ok(e?)` |

Discharged so far (§0.1 carries the numbers): semantic agreement, on the twenty §43 programs and their
twins; exact codegen, on 191 round-trip functions and the two gate kernels; **allocation fixtures**, on
all twenty programs, where the two sides prove not merely bounded but identical; **size and memory**,
where peak live bytes is recorded per program and binary size gates against checked-in per-program
limits (`corpus/size_baseline.json`, rewritten only deliberately); and the **`throws` tail fixture**,
where the same-type tail `?` is the direct return instruction for instruction and the converting tail
is two instructions shorter than the form it replaced. Binary size is deliberately **not** compared
against the twin: the two crates differ in name, so every mangled symbol differs in length, and one
measured comparison moved by 384 bytes — one padding step — when the names were made equal, so at that
granularity the paired figure measures the build layout. The **runtime budget** is retired (`corpus-study/roundtrip/RUNTIME.md`): no piece ever showed a
regression, but the control that was supposed to say whether a run could resolve 2% showed instead
that the method cannot — the crate boundary is worth up to 2.5% on a comparison whose true answer is
1.0. Instruction identity is the standing obligation in its place, and it is a stronger one: the
generated code does not need to be *timed* against the original when it compiles to the same
instructions.
What is not settled is the runner — an idle machine certifies, a machine doing other work does not,
and §36's word "designated" is exactly that choice. Lowering discipline is enforced by the golden
tests rather than by a separate audit.

Two Rust baselines are kept and reported separately: Rust that mirrors Uredo's chosen signatures (measures lowering overhead) and independently idiomatic Rust, which measures Uredo's defaults against what a Rust programmer would have written — not the same signature, since `x: Duration` is by value under §10.1's prelude table where the baseline may write `&Duration`.

---

## 37. Ergonomic evaluation plan

The claim "materially less source complexity" is measured, not asserted. The corpus study (`corpus-study/FINDINGS.md`, 1.09 M lines of real Rust across ten repositories) found that the v0.2 rules remove **9–14% of tokens**, 68% of it braces and semicolons, and that the passing-mode rules change parameter annotations far more than they change token counts. The spec's own v0.1 example (28.7%) was a punctuation-dense best case and is not a target. For each §43 program, record:

- token count and non-whitespace characters, Uredo vs idiomatic Rust
- count of Uredo-specific annotations required (`take`, `inout`, `&`, lifetimes, `@copy use`, turbofish)
- **inference hit rate**: fraction of parameters whose Uredo-chosen mode equals the idiomatic Rust signature
- barrier diagnostics per 100 lines during authoring, and fraction mapped to Uredo terms
- raw-Rust share by operation

Phase 1 thresholds (set from the corpus study, revisable only upward):

- token reduction versus idiomatic Rust ≥ 10% on application-style programs (§43's programs 1–15; 16–20 exercise an integration or a raw-Rust escape and are measured but not gated) (corpus range 8.8–13.9%);
- parameter annotations ≤ ½ of the idiomatic Rust's, counted **in signatures only** — parameter lists and return types — as `take`, `inout`, `&`, `var` and lifetime ticks against Rust's `&`, `mut` and lifetime ticks; bodies are excluded, because Uredo keeps Rust's `&` and `*` for calls into Rust items, so counting them measures the Rust API rather than the surface (corpus: 2–5× fewer on CLI code; break-even on framework code whose types are by-value wrappers);
- inference hit rate ≥ 90% on non-generic parameters;
- ≥ 80% of barrier diagnostics mapped;
- `rust {}` share reported by operation.

If the corpus cannot meet these, §1's claims shrink; the yardstick does not. Measured on 2026-09-12: all four met (§0.1).

---

## 38. Design questions: decided, held and open

The ten questions left open by the panel review were decided interactively on 2026-09-10 (38.1–38.10), two more after the corpus study (38.11–38.12). Each row records the options considered, the choice, and the reason; the rule itself lives in the section cited. The paragraphs after the table record what was decided, held or rejected since.

| # | Question | Decision | Reason | Section |
|---|---|---|---|---|
| 38.1 | Foreign `Copy` types | Prelude table of std `Copy` types plus `@copy use` per crate, with a generated `Copy` assertion (over: write `take`; `@copy use` only) | removes the most visible cost of P3 while keeping signatures derivable from the crate's own source | §10.1 |
| 38.2 | Struct construction | Rust braces only (over: parens with named fields; parens without punning) | the only form under which punning, `..base`, foreign structs, struct variants and patterns are decided by syntax alone | §12.1, §13 |
| 38.3 | Turbofish in expressions | Keep (over: `f<T>(x)` with a C#-style lookahead rule) | reversible direction; the lookahead rule silently reparses `f(a < b, c > (d))` | §7.6 |
| 38.4 | Macro passthrough | Direct `path!(…)` invocation (over: `rust!` prefix; raw block only) | smallest rule, matches Rust, keeps the surrounding line in Uredo; `vec![…]` comes free | §22.5 |
| 38.5 | Bare `throws` | `@!default_error(E)` default; explicit on `pub` (over: always explicit; body inference) | removes repetition with no inference; public signatures stay self-describing | §15.1 |
| 38.6 | Shadowing | `let name = expr` creates a new binding (over: forbidden; bare assignment shadows) | Rust vocabulary in its Rust meaning; bare `x = …` stays lexically decidable | §8.1 |
| 38.7 | Optional chaining | Deferred; idioms documented (over: `?.` limited to Uredo-declared types) | no zero-cost lowering independent of field types; a type-limited version has an invisible boundary | §14 |
| 38.8 | Receiver inference on private methods | None; receivers explicit everywhere (over: fixed-point inference for private methods) | one uniform rule, no associated-function ambiguity, nothing body-dependent | §12.3 |
| 38.9 | Rust-block form | `rust { }` only (over: `rust:` indented block; both) | one form for all three positions; no re-indentation of rustfmt output | §6.6 |
| 38.10 | Debugging | Line-preserving codegen now; map-aware GDB/LLDB/VS Code helpers in Phase 2; DWARF rewriting only on measured need (over: nothing further; DWARF rewriting) | uses the provenance map already built; defers the fragile toolchain-specific work until evidence | §28.2 |
| 38.11 | Error propagation spelling (corpus study) | Rust's postfix `?` and `.await`; prefix `try`/`await` withdrawn | 3.8% of `?` are mid-chain and 17% of async `?` are `.await?`, where prefix forms need parentheses; `?` on `Option` was illegal under the `throws`-only rule | §15.2 |
| 38.12 | Mutable by-value bindings, pattern parameters, pattern bindings, `let-else`, verbatim `Result` returns (corpus study) | added | untranslatable or unspecified shapes at 1.6 (`mut`), 0.8 (pattern params), 5.2 (pattern `let`) per 1,000 lines | §8.1, §10.2, §12.3, §15.1 |

**Decided since the panel (2026-09-10 to 2026-09-13).** The corpus study's four amendments (D30–D33); the final-check amendments (D34–D46); the portfolio findings (D47–D49); the Phase 0 gate and the configuration rule (D50); the G9 layout rule for trailing block arguments (D51, panel `oracle-reviews/g9/`); the passing mode of type parameters and `impl Trait` (D52, panel `oracle-reviews/generics/`: idiomatic Rust passes 91% of generic-typed parameters by value, the borrow default matched 9.2%; the visible signs of a non-`Copy` transfer are now the `take` annotation, a pattern parameter and the type-parameter form); the std-trait mode table (D53, closing G11); `impl Trait` in argument position (G16, closed by D52); the passing mode of an optional parameter and the `&T?` precedence (D54, closing G10, panel `oracle-reviews/optional/`). Five more were found by building things rather than by argument, each with its own row: an attribute on a tuple field (D55) and the name-value attribute form (D57), both needed before a derive-macro crate could be used at all; `@copy use` on one instantiation of a generic foreign type (D56); what a fenced block in a `##` comment means (D58); and a written receiver type (D59), without which a hand-written `Future` could not be spelled. The §35 compatibility suite and the §36 fixtures are what produced them: every one of those five came out of building a fixture rather than out of reading the document, as did four toolchain defects recorded in §0.3 for the same days.

**Owner's condition, restated.** `take` remains the only *annotation* of a non-`Copy` ownership transfer. The other source-visible signs of a transfer are the pattern of a P7 parameter, the type form of a P8 parameter, and the trait's own signature in an `impl` of a listed std trait (D53). Outside those, a concrete parameter that takes ownership writes `take`. The corpus-study numbers that motivated the review stand on the record (`corpus-study/FINDINGS.md`; borrow-by-default needed fewer parameter annotations than Rust on CLI and library code — ripgrep 497 vs 597, clap 389 vs 840 — and more on framework code whose types are by-value wrappers).

**Decided 2026-09-12: the optional parameter (D54, panel `oracle-reviews/optional/`).** Three candidates were measured against every `Option`-outer parameter in the corpus (898, excluding `&mut Option`, which is `inout` under all of them), and two were prototyped and run:

| Rule | `x: T?` lowers to | Matches idiomatic Rust |
|---|---|---|
| the former rule | `&Option<T>` always | 1.1% (10 of 898) |
| by value always | `Option<T>` always | 98.9% |
| **D54, the payload decides** | `Option<T>` when `T` is known-Copy, else `&Option<T>` | 65.1%, and 44.5% of the 546 where the container's mode is the question |

By-value-always was rejected although it scores highest, because it breaks readers of borrowed optionals: `describe(a.email)` where `a: &Account` is E0507, "cannot move out of `a.email` which is behind a shared reference". Both programs in the suites that read an optional field stopped compiling under the prototype. Repairing it at the call site would require the argument's type, which §10.3 deliberately does not consult; repairing it with a diagnostic leaves the user writing `.as_ref()` at every read. A fourth candidate — lower `x: T?` to `Option<&T>`, the shape idiomatic Rust uses most — was rejected as undecidable in the same way: inserting `.as_ref()` without the argument's type produces `Option<&&T>` where the caller already holds an optional reference (E0308), and it changes what the callee's body sees.

D54's own rate is not the yardstick it optimises. It takes every free copy — `&Option<u32>` was absurd — while never moving a non-`Copy` payload without `take`, and its misses are parameters idiomatic Rust passes by value *as moves*: one framework's ECS wrappers (`Res` 75, `ResMut` 23, `Single` 12), `StyledStr` (16), `String` (9), `Vec` (6). Where the payload is a project type an Uredo project would write `@derive(Copy)` or `@copy use`, the rule follows it automatically. The `Result` half of the §10.1 extension rests on soundness and symmetry with `Option`, not on measurement: it would be indefensible for `Result<u32, u8>` to be borrowed beside a by-value `Option<u32>`.

**Held: `@value use`.** A declaration that a foreign non-`Copy` type passes by value, the way `@copy use` declares one known-Copy. Concrete (non-generic) parameters in the corpus are by value 22,027 to 12,365, and 13,162 of the by-value ones — 60% — would need `take` or `@value use` today (`corpus-study/value_use_concrete.txt`, a syn pass; the earlier textual count mixed in `impl Into`, `F` and `Self`, whose mode D52 already settles). What can be named is not `@value use` material: framework parameters whose mode the framework's own traits fix (`Res` 2,463, `Query` 1,856, `ResMut` 1,385, `Commands` 1,246), types that are `Copy` in their own crate and would be declared with `@copy use` (`Vec2` 222, `Vec3` 156), and genuinely owned values where `take` is the honest spelling (`Vec` 155, `TokenStream` 125, `String` 121, `Request` 84). Those ten types are 7,813 of the 13,162, and the twenty most common are 9,624 — so a long tail of project types remains unexamined, and the case for holding rests on the head of the distribution rather than on all of it. The asymmetry with `@copy use` is deliberate and is the reason: a wrong or distant `@copy use` never hides a *move*, since both modes it chooses between are non-consuming, while `@value use` would hide one — with no sign at the parameter at all. Reopen on evidence from real Uredo code that `take` annotations cluster on a small set of foreign types, measured over concrete parameters only.

**Held.** Bound aliases (`trait Name = …`) until defined over the bound AST with hygiene, definition-site resolution and cycle checks (E2); the postfix lifetime notation (`str @a` for `&'a str`, panel 2026-09-11, `oracle-reviews/lifetimes/`) — as proposed it covered only the borrow position (14% of named-lifetime occurrences in the corpus), broke orthogonality with the passing modes in return and field position and repealed P6/D47/D48 silently; reopen only as a full substitution over the whole lifetime grammar with its own decision row, or on evidence that the spelling rather than the presence of lifetimes is what stops users. **`@static` alone was put to a second panel on 2026-09-13** (`oracle-reviews/static/`), after `tools/` measured one written lifetime — a `'static` — in 1,093 lines of real Uredo, and was **declined**: a `@static` confined to reference position covers 46.6% of the corpus's `'static` occurrences and leaves bounds (34.7%) and generic arguments (17.1%) spelled with an apostrophe, so it is a third notation rather than a smaller one; §23's `@` would acquire a positional meaning; and no seat could name a language that gives one member of a syntactic category a notation its siblings do not have. **Reopen at more than 3 written lifetimes per 1,000 lines** of real Uredo outside these suites, measured over at least 2,300 lines — the sample the recompute seat says is needed to tell that rate from the one observed — or on a design covering bound and generic-argument position with the §23 collision resolved in the grammar. *Inferring* `'static` where nothing appears to be borrowed was considered in the same panel and is **rejected outright, not held**: the predicate is undecidable from declarations, since a foreign type may carry a lifetime that Uredo may not resolve (§5.2) and rustc's elision counts it — `fn rest(c: take std::str::Chars) -> str` compiles and runs today, and the inference would break it. `loop`/`unsafe`/`async` headers as trailing block arguments (corpus: 143, written with `rust { }`).

**Open, with a countable trigger.** Silencing a `uredo lint` finding in source (§29.1, panel
`oracle-reviews/lint-allow/`): three shapes remain on the shortlist and none is adopted, because the
granularity of deliberate findings in real Uredo code is what chooses between them and no such code
exists yet. Revisit at a deliberate finding on more than 5% of a real project's files, or at a
project forced to `--allow` a lint it wants enforced. Whatever is adopted ships with an `expect`
form or a written reason not to: the corpus writes `expect` 409 times against `allow` 751, because a
suppression that outlives its cause is this feature's known failure.

**Rejected.** Implicit `use super::*` in test modules (E6). Overloading `@allow` for Uredo's own
lints, and forwarding such an attribute while hiding rustc's `unknown lint` warning — the first
contradicts §23's own example and would change meaning if rustc shipped a colliding name, the second
puts a permanent warning into every downstream build of an exported package (both measured,
`oracle-reviews/lint-allow/`).

**What would reverse P8 (D52).** The by-value rule for type parameters and `impl Trait` was the
closest call in this document — 91% of the corpus's generic-typed parameters pass by value, but that
corpus is Rust written under Rust's rules, and the cost of being wrong falls on the call site rather
than on the signature that chose the mode. Two counts would reopen it, both taken over a real Uredo
project rather than over these suites.

- **Moves the author did not want.** A mapped E0382 naming a P8 parameter, at more than **1 per
  1,000 lines** — the unit 38.3's turbofish trigger already uses. The mapper (§27) has a branch for
  exactly this case: it names the callee and ends the message with `(P8)`, so the count is a grep
  over a build log and needs no new tool.
- **Clones written to pay for the mode.** A `.clone()` at a call site whose only purpose is to
  satisfy a P8 parameter, counted against the `take` annotations the borrow default would have
  required instead — which is the number of generic-typed parameters that pass by value, countable
  from the source the same way §37 counts annotations. If the clones exceed that number, P8 costs
  more than it saved and type parameters with no `Copy` bound go back to P3, leaving `take T` as the
  spelling for a transfer.

Both counts are collected by `uredo lint --measure` (§29.1), which prints them together with the
site of every clone; the E0382 rate is a grep over a build log. Neither has been measured on
Uredo code, because no project of the size needed exists — the §43 programs and the compatibility
fixtures are too small to be evidence either way, and saying so is the point of recording the
trigger rather than the verdict.

**Still open, with re-evaluation triggers.** Turbofish-free calls (38.3) if the corpus shows turbofish frequent in application code (measured: 2.5 per 1,000 lines outside bevy, 10 inside); optional chaining (38.7) if a lowering independent of field types is found; the exact contents of the prelude `Copy` table (38.1), fixed per release from corpus usage.

**Recorded exemption (D42).** Closure parameters are Rust's; the `take` rule governs `fn`/method declarations only.

---

## 39. Non-goals

Uredo v0.x does not: replace Cargo, crates.io, rustc or LLVM; provide a garbage collector or any runtime ownership fallback; invent an async runtime; hide anything on the §3.2 list; add class hierarchies or Python's dynamic semantics; stabilize Rust's ABI; wrap crates in Uredo-specific APIs; embed a Rust semantic engine in the lowering path (§5.2).

The escape hatch is Rust itself.

---

## 40. Language identity in one example

```uredo
struct User:
    id: u64
    name: String
    active: bool

fn greet(name: str):
    print("Hello {name}")

fn activate(user: inout User):
    user.active = true

fn archive(user: take User) throws ArchiveError:
    storage::save(user)?

fn find_user(id: u64) -> User?:
    ...
```

A Rust programmer understands all of it immediately. A programmer who accepts ownership writes it without `&`, `&mut`, `Result<T, E>`, `Ok(...)`, `Err(...)`, receiver punctuation or `impl` boilerplate, and meets several of those concepts first in a diagnostic, in Uredo terms. Progression to `fn slice<'a>(buffer: &'a Buffer) -> &'a [u8]:` or to `rust { unsafe fn kernel(...) { ... } }` is always available.

> **Complexity should be available, not mandatory. Performance costs should be visible, not magical. Rust should be underneath, not outside.**

---

## 41. Prior art and the lessons adopted

| Project | What it tried | What happened | Rule Uredo adopts |
|---|---|---|---|
| CoffeeScript | simpler syntax over JavaScript | declined when the host absorbed its features; debugging through generated code stayed painful | §28.2 debugging is a design constraint; §3.4 minimizes inventions Rust could plausibly adopt |
| TypeScript | gradual explicitness over JavaScript, tooling first | succeeded; host semantics remained the spec | §5.2 never deviate from Rust semantics, only notation; LSP before syntax freeze |
| Mojo | "superset of Python" | compatibility claim narrowed later (verify wording) | §4.2 states compatibility at the strength the macro story can keep |
| Carbon | interop-first C++ successor | still experimental after years | §34 phases interop early and prices it |
| Swift | `inout`, later exclusivity enforcement (SE-0176) | needed a formal model to make `inout` sound | Uredo inherits Rust's aliasing rules through rustc and credits Swift for `inout`; `inout` is not a storable reference |
| Hylo (Val) | parameter-passing conventions as the core model (verify status) | research language | §10 passing modes are defined formally before syntax freeze |
| Reason / ReasonML | OCaml reskinned for JS developers | largely abandoned | §37 measures the payoff instead of assuming notation drives adoption |

*One rule adopted here is not yet met: "LSP before syntax freeze". The core surface was frozen on the strength of the gate and the corpus (§0.1) with the LSP still unbuilt; the risk that buys — syntax decisions taken without editor feedback — is accepted knowingly and recorded here rather than quietly dropped.*

---

## 42. References (verified 2026-09-10 unless marked)

- Graydon Hoare, IRC log on the origin of the name Rust — https://notes.zachmanson.com/on-the-origins-of-rust/ (quote verified verbatim)
- NCBI Taxonomy, Pucciniales (rust) — https://www.ncbi.nlm.nih.gov/Taxonomy/Browser/wwwtax.cgi?id=5258
- Wiktionary, *uredo* (archaic botany: urediniospore; Latin ūrēdō: blight, burning itch) — https://en.wiktionary.org/wiki/uredo ; Wikipedia, *Uredo* (genus of rust fungi) — https://en.wikipedia.org/wiki/Uredo ; Merriam-Webster entry not verifiable by non-browser fetch
- Rust Foundation, Rust Language Trademark Policy — https://rustfoundation.org/policy/rust-trademark-policy/ (permits accurate compatibility statements; no date on page)
- Rust Reference: keywords, paths, closure expressions and capture modes, destructors and drop scopes, type coercions, procedural macros, preludes/`no_std`
- Cargo Book: build scripts, manifest `include`/`exclude`, `cargo package`, publishing, external tools / JSON messages, profiles, `rust-version`, cargo targets
- rustdoc JSON output: unstable, tracking issue #76578 — https://doc.rust-lang.org/rustdoc/unstable-features.html
- Swift Ownership Manifesto — https://github.com/apple/swift/blob/main/docs/OwnershipManifesto.md
- Panel review record: `docs/oracle-reviews/` (dossiers, unedited seat reviews, verification log, synthesis)

---

## 43. The representative corpus (design milestone, completed 2026-09-12)

Before freezing the core surface, write the twenty representative programs and run the Phase 0 gate (§34). Both are done (§0.1); what remains outstanding from this section's own checklist is named at its end. Corpus:

1. hello world · 2. numeric computation · 3. string processing · 4. struct CRUD · 5. enum + match · 6. optional handling · 7. file reading · 8. JSON with Serde · 9. error propagation with a `From`-based error type · 10. iterator pipeline · 11. generic function · 12. trait implementation · 13. ownership transfer · 14. mutable borrowing · 15. shared borrowing · 16. async HTTP request · 17. Tokio program · 18. Rayon parallel loop · 19. call into a `.rs` module · 20. inline raw Rust optimization

For each, record: Uredo source; generated Rust; equivalent idiomatic Rust; the §37 metrics; compiler diagnostics and their mapping; allocation counts; assembly comparison where relevant. §33.1 maps each construct to the programs that exercise it; a construct no program exercises is not frozen.

*Done for all twenty: source, generated Rust, idiomatic twin, the §37 metrics, and output equality against the twin (`corpus/RESULTS.md`). **Owed when this was written and since delivered:** the allocation, memory and size gates now run on all twenty programs (`corpus/PERF.md`), and the runtime gate runs on the round-trip pieces, which are the only paired code here that does enough work to time (§0.1, `corpus-study/roundtrip/RUNTIME.md`). What these twenty do not have is an assembly comparison of their own; the opcode-level evidence remains the 191 round-trip functions and the gate kernels. The freeze rests on the gate and the ergonomic thresholds; the performance obligations of §36 are met for the kernels measured and open for the rest.*

