# uredo — the Uredo compiler

Source-to-source compiler from Uredo (`.ure`) to readable Rust, implementing
`docs/UREDO_LANGUAGE_SPEC_v0.4.md`. The Phase 0 gate of §34 passes and the Phase 1 surface is implemented; §0.1 of the spec lists what is not.

```bash
cargo build
target/debug/uredo new hello [--lib] [--name n]   # create a package (§4.1)
target/debug/uredo init [dir] [--lib]             # the same, in a directory that exists
target/debug/uredo rust path/to/file.ure          # print the generated Rust
target/debug/uredo rust --raw path/to/file.ure    # before rustfmt
target/debug/uredo rust --map path/to/file.ure    # with the Uredo line each Rust line came from
target/debug/uredo run  path/to/package           # lower src/**/*.ure, then `cargo run`
target/debug/uredo build path/to/package [--release]
target/debug/uredo check path/to/package
target/debug/uredo check --api path/to/package [--update-api]   # public API manifest diff (§4.4)
target/debug/uredo test  path/to/package
target/debug/uredo package path/to/package [--out dir]   # self-contained Cargo package (§4.6)
target/debug/uredo publish path/to/package [--dry-run]   # export, check the API (§4.4), cargo publish
target/debug/uredo explain path/to/file.ure:42           # what Uredo elaborated on that line (§26.2)
target/debug/uredo doc  path/to/package [--open]         # rustdoc, with examples lowered (§26.4)
target/debug/uredo report path/to/package [--out dir]    # a bug bundle for a lowering defect (§5.4)
target/debug/uredo fmt [--check] path/to/dir             # canonical formatting (§29)
target/debug/uredo lint [--deny] [--allow <lint>] path   # idiom findings (§29.1)
target/debug/uredo fix [--dry-run] path                  # apply the fixes that change no output
target/debug/uredo lsp                                   # a language server on stdio (§28.1)

`debug/` holds the GDB and LLDB helpers (D29, §28.2): `ubreak FILE.ure:LINE`, `uwhere`, `ulist`.
See `debug/README.md`.
target/debug/uredo lint --measure path                   # the D52 reversal counts (§38)
```

`check`/`build`/`run`/`test` lower every `.ure` under `src/` into `target/uredo/src/`, copy `.rs`
files and `Cargo.toml` alongside, add the automatic `mod` declarations of §20.3, write the
provenance maps to `target/uredo/provenance/`, and delegate to Cargo with the package's own
`target/` as the artefact directory. `rustfmt` (edition 2024) must be on the path: generated
code is formatted before it is compared or compiled (§29).

**Diagnostics (§27).** Cargo is run with JSON messages. Every rustc span inside a generated
file is mapped through the provenance map to the Uredo line, and to a column by finding the
span's text in that line. Mapped families are rewritten in Uredo terms: E0382 (use after a
`take` parameter took ownership: the callee, its Uredo declaration, the clone help and the
§10.4 note), E0596 (a `var` binding is needed for a `self: inout` method), E0384, E0308 (with
the elaboration Uredo performed on that line) and E0106 (D48). Everything else is forwarded on
its Uredo anchor. rustc's own rendering of the generated Rust follows in a secondary panel.

**Provenance (§5.3).** The lowerer tags each emitted line with its Uredo line. rustfmt only
moves tokens and adds or removes trailing commas, so the map is carried to the formatted bytes
by aligning the two token streams (`src/provenance.rs`). Elaboration records — rule, Uredo
text, generated text, callee declaration, moved arguments — are kept with the map and feed
both `explain` and the translator.

**API manifest (§4.4).** Every `pub` item's generated Rust signature — functions, methods,
structs with their public fields and API-relevant attributes, enums with their variants, traits,
trait impls, type aliases, constants, modules and re-exports — is collected during lowering,
keyed by module path, and written to `target/uredo/api.json`. `uredo check --api` compares it
with the reviewed baseline `uredo-api.json` in the package root: the first run creates the
baseline, a body edit leaves it unchanged, a declaration change fails until
`--update-api` accepts it. Binary roots are excluded (a `main.ure` has no consumers).

**Trailing block arguments (§6.3, D51).** Inside an unclosed `(`, a line ending in a closure `=>`
or a `match`/`if` colon opens an indented block anchored at that line; the lexer suspends the
parenthesis (a block-argument frame), statement layout resumes, and the block ends at the `)`
line at the anchor's indentation. A `=>` ending a line after `=` opens a block anchored at the
statement. The boundary cases raised by the panel are in `tests/diagnostics.rs`.

**Configuration (§4.6, D50).** Each file is lowered once for every target and feature set;
`@cfg`/`@cfg_attr` are forwarded and rustc selects. The lowerer checks that what it elaborates
from is configuration-invariant: same-named declarations (in one module or in same-named
alternate modules) must agree in passing modes, receiver and `str`/`[T]` return status, and
`@derive(Copy)`, `@copy use` and `@!default_error` cannot be conditional, including inside a
`@cfg`-guarded module. Violations are Uredo errors at the conflicting declaration.

**Package (§4.6).** `uredo package` writes generated and copied Rust with a normalised manifest
(`rust-version` set, no workspace table, no compiler artefacts). `examples/geometry` is exported
this way and consumed by the plain-Rust project `tests/fixtures/consumer`.

## Layout

| File | Role |
|---|---|
| `src/lexer.rs` | Tokens with `Indent`/`Dedent`/`Newline`; the §6.3 continuation rules; `#`/`##`/`##!` comments; `rust { }`, macro bodies and `async { }` captured as raw Rust text (§6.6); `@` attributes |
| `src/parser.rs` | Recursive descent to `ast.rs`. Types, patterns, macro bodies and Rust blocks stay raw text (§5.2) |
| `src/lower.rs` | Elaboration and printing: bind-vs-assign (§8.1), the passing-mode table (§10.2) with P8 by-value generics (D52), the call-site rule (§10.3, D47), receivers (§12.3), field sugar (§12.2), nested impls (D45), `throws` lowering with the D37 tail match (§15.3), `T?` (§9.3), `@copy use` assertions (§10.1), intrinsics (§11.3) |
| `src/diag.rs` | Uredo-side diagnostics with Uredo spans |
| `src/lint.rs` | `uredo lint` (§29.1): `redundant_take`, `borrowed_container`, `copy_candidate`, decided from the facts the lowerer already has and collected into `Output::lints` on every compile; also the §23 warning when an `@allow`/`@expect` names one of these, which is still forwarded to rustc unchanged |
| `src/fmt.rs` | `uredo fmt` (§29): token-based canonical spacing, indentation from the lexer's block structure, D51 closer placement, blank-line and item separation, trailing commas, over-long inline bodies expanded; comments, `rust { }` blocks and macro bodies kept verbatim |
| `src/provenance.rs` | Line map and elaboration records; token alignment through rustfmt |
| `src/api.rs` | Public API manifest entries and the manifest diff (§4.4) |
| `src/main.rs` | CLI, package lowering (§20.3, §28.1), `package`, `explain`, `check --api`, rustc diagnostic translation (§27) |
| `tests/gate.rs` | The Phase 0 gate of §34 (items 1, 2, 3, 5, 6, 7) on `examples/gate`; record in `examples/gate/GATE.md` |
| `tests/fmt.rs` | Formatter: spacing rules, layout cases, idempotence, the portfolio is canonical, formatting never changes the generated Rust |
| `tests/cli.rs` | End-to-end: translated E0382/E0596/E0308 on `tests/fixtures/moved` and `mismatch`, `explain`, `rust --map`, `package` consumed by `tests/fixtures/consumer` |
| `tests/golden.rs` | Every `docs/portfolio/*/snippet.ure` must produce its `lowered.rs` byte for byte. `UREDO_GOLDEN_TOPICS=01,02` restricts; `UREDO_GOLDEN_IGNORE_DOCS=1` compares without doc comments; `UREDO_GOLDEN_UPDATE=1` rewrites the expected files (review the diff) |
| `tests/diagnostics.rs` | Negative fixtures: programs Uredo itself rejects, and the line they are reported on |
| `tests/doc.rs` | Documentation examples (§26.4): each kind of fence, the error for one that does not compile, and `examples/compat/docs` whose five examples are compiled and run by `uredo test` and rendered by `uredo doc` |
| `tests/compat.rs` | The §35 suite: eight packages under `examples/compat/` run against the real crates — forwarded attributes, extractors and handlers, a builder chain, foreign operators, a slicing macro, a build script, a proc macro, a wasm target, a compile-time-checked query and a `no_std` library — plus all four Cargo target kinds, the mutation-rebuild sequence and the crate-level negative case |
| `tests/perf.rs` | §36 fixtures: the `throws` tail comparison against `examples/perf/tail_rust` (same-type tail `?` identical to the direct return, converting tail no worse than `Ok(e?)`) and one program of the allocation, memory and size sweep, so `corpus/perf.py` cannot rot |
| `tests/new.rs` | `new` and `init`: the manifest and template they write, that the result runs and its test passes, that the template satisfies `fmt --check` and `lint`, and that both commands refuse rather than overwrite |
| `tests/lint.rs` | Each lint's positive and negative cases, and the command: `--deny`, `--allow`, an uncompilable file, and a clean corpus and examples tree |

## What the elaboration never does

No rule consults a Rust type fact (§5.2). A parameter typed by a type parameter or `impl Trait`
passes by value (P8, D52) unless its bound includes `?Sized`; the E0382 translation names the
generic parameter when a moved argument is reused. Auto-borrow applies only to callees Uredo declared
(module functions, nested and `impl` methods of Uredo types, in this file or in any other `.ure`
file of the crate: every file's declarations are indexed under its module path before lowering,
and `use` items resolve against that index); calls to Rust items and to closures receive their
arguments verbatim. Method-call auto-borrow needs the receiver's type
to be known from Uredo source: an annotation, a struct literal, or an associated function
declared to return `Self`. Otherwise the arguments are emitted as written and rustc decides.

## Not yet implemented

- `uredo lsp`, `publish`, `fix` (§28.1).
- Debugger helpers over the provenance map (§28.2, Phase 2).
- Bound aliases (E2, held), optional chaining (D26, deferred).
- Column mapping in translated diagnostics is by text search in the Uredo line; a span whose text was rewritten (an inserted `&`) falls back to its first identifier.
