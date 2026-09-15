# Uredo

Uredo is a Python-like surface syntax for Rust. It compiles source-to-source to readable,
deterministic Rust, and then hands that Rust to Cargo and rustc. There is no runtime, no garbage
collector, no ownership fallback and no Uredo-specific reimplementation of anything: the generated
crate is an ordinary Cargo package that a Rust programmer can read, review and ship.

The promise it is built around:

> Simple code should be simple to write. Expensive or dangerous operations should stay explicit.
> Rust should always remain available.

```uredo
##! A tiny address book.

@derive(Debug)
struct Contact:
    name: String
    email: String?

impl Contact:
    fn new(name: take String, email: take String?) -> Contact:
        Contact { name, email }

    fn domain(self) -> str?:
        email.as_deref()?.split('@').nth(1)

fn main():
    book = Vec::from([
        Contact::new(String::from("ada"), Some(String::from("ada@example.org"))),
        Contact::new(String::from("bob"), None),
    ])
    for c in book:
        print("{}: {}", c.name, c.domain().unwrap_or("-"))
```

`uredo run` prints `ada: example.org` / `bob: -`. This is what it generated to do it:

```rust
//! A tiny address book.

#[derive(Debug)]
struct Contact {
    name: String,
    email: ::core::option::Option<String>,
}

impl Contact {
    fn new(name: String, email: ::core::option::Option<String>) -> Contact {
        Contact { name, email }
    }

    fn domain(&self) -> ::core::option::Option<&str> {
        self.email.as_deref()?.split('@').nth(1)
    }
}

fn main() {
    let book = Vec::from([
        Contact::new(String::from("ada"), Some(String::from("ada@example.org"))),
        Contact::new(String::from("bob"), None),
    ]);
    for c in &book {
        ::std::println!("{}: {}", c.name, c.domain().unwrap_or("-"));
    }
}
```

Six things in that pair are the whole design:

- **Indentation replaces braces**, `#` replaces `//`, `@` replaces `#[…]`, `T?` replaces
  `Option<T>`, and `=` binds a new name where Rust writes `let`.
- **A parameter is a shared borrow unless its declaration says otherwise.** `name: take String`
  transfers ownership and is the only *annotation* that does; `self` is `&self`, `self: inout` is
  `&mut self`, `self: take` is `self`.
- **`str` and `[T]` mean `&str` and `&[T]`** in a parameter or return type, including under `?`.
- **Inside a method, a bare field name reads `self.field`** — `email` above — under conditions
  narrow enough to stay decidable from the source alone (§12.2).
- **Every decision is made from the declaration**, never from a type Uredo would have to infer or
  look up in a dependency. That is why the generated signature of a `pub` item cannot change when a
  dependency does, and why the whole lowering is one pass with no semantic engine behind it.
- **`rust { … }` is the escape hatch**, and it is expected to be used. Uredo is a surface, not a
  wall.

## Try it

```bash
cd compiler && cargo build --release          # rustfmt (edition 2024) must be on the path
export PATH="$PWD/target/release:$PATH"

uredo new hello && cd hello
uredo run
uredo rust src/main.ure                       # see the generated Rust
uredo explain src/main.ure:5                  # see what Uredo elaborated on a line, and why
```

`uredo check`, `build`, `run`, `test` and `doc` lower every `.ure` under `src/` into
`target/uredo/`, write a provenance map beside it, and delegate to Cargo. Diagnostics come back
translated: a rustc error on generated code is re-anchored to the Uredo line that produced it and
re-worded in Uredo's terms where Uredo made the choice that caused it. `uredo package` exports a
self-contained Cargo package, which is how Uredo code reaches crates.io — a consumer needs Cargo,
not Uredo.

## What is in this repository

| Path | What it is |
|---|---|
| `DECISIONS.md` | **Why it is the way it is.** Short answers to what people ask for first — `take`, `var`, the turbofish, the apostrophes, why there is no type engine — each pointing at the section that owns the argument. Read it before proposing a change. |
| `docs/MANUAL.md` | **The manual — start here.** Teaches the language: install, the passing modes, optionals and errors, traits, interop, the toolchain, and the mistakes its author actually made. Every example in it is compiled by rustc in the test suite. |
| `docs/MANUAL.pdf` | The same manual, typeset: title page, table of contents, running heads and highlighted listings. `docs/pdf/build.sh` regenerates it from the markdown, so the two cannot drift. |
| `docs/UREDO_LANGUAGE_SPEC_v0.4.md` | The specification. §0 records every decision and its evidence; §1–§33 are the rules; §34 onward hold the acceptance criteria, the questions still open, and the record of how each claim was measured. It is a record, not an introduction — read the manual first. |
| `compiler/` | The reference compiler and CLI, in Rust. `cargo test` runs 167 tests. |
| `corpus/` | Twenty paired programs — Uredo and idiomatic Rust — that must print the same thing. `run.py` measures them; `perf.py` measures allocation, memory and binary size. |
| `docs/portfolio/` | 25 topics, one per language area, generated byte for byte by the compiler and checked by a golden test. |
| `examples/compat/` | The ecosystem fixtures: serde, tokio, axum, sqlx, clap, wasm-bindgen, C FFI, proc macros, `build.rs`, `no_std` and the rest, one Cargo package each. |

## Status

**The v0.1 surface is frozen** (2026-09-12): no rule may change what existing source means without a
decision row and a migration. Every language rule in the spec is implemented and exercised. What is
built and what is not is recorded in one place — §0.1 of the spec — and that table governs wherever
another section sounds more finished than it is.

Every command the spec names is built. What is *not* finished is stated rather than implied.

- The language server's parser is **tolerant but not lossless**: it answers a broken file from the
  items that still parsed, but keeps no trivia, which rules out renames and code actions.
- `uredo fix` has **no migrations to run**, the surface being frozen; what it does today is apply the
  findings whose rewrite provably changes no generated Rust.
- **Nothing has been published to crates.io.** The licence is settled (MIT OR Apache-2.0) and the
  manual is written; a public repository is what remains.
- The `no_std` fixture builds for a bare-metal target and links its own allocator, but **no hardware
  or emulator executes it**, and that allocator frees nothing.
- The wasm module runs in Node through both the raw artefact and wasm-bindgen's generated glue, but
  **never in a browser**.
- The §36 **runtime budget is retired rather than met**: its own control showed a cross-crate paired
  benchmark cannot resolve 2%, and instruction identity stands in its place.

What has been measured, on the twenty-program corpus and its idiomatic Rust twins:

- **−13.4% tokens** on the application-style programs, −13.0% over all twenty. The v0.1 draft's
  28.7% was a punctuation-dense best case and is not the target.
- **52 signature annotations against Rust's 109**, counting `take`, `inout`, `&`, `var` and lifetime
  ticks in parameter lists and return types.
- **64 of 64 non-generic parameters** get the same passing mode the idiomatic Rust chose.
- **Identical allocation counts and bytes** on all twenty, and identical peak live bytes on
  nineteen. The twentieth holds 15 bytes more, because Uredo's `for` iterates a place by shared
  reference where the Rust twin's loop consumes it.
- **No runtime regression measured** on the round-trip pieces (ratios 0.976–1.024), against a 2%
  budget whose floor is set by the benchmark method rather than by the machine.
- **191 of 191 functions opcode-identical** to their originals in the round-trip study, with equal
  instruction totals and alloca bytes, and 200,000 random inputs per piece agreeing.

## What Uredo is not

It does not replace Cargo, crates.io, rustc or LLVM; it adds no garbage collector and no runtime
ownership fallback; it invents no async runtime; it hides neither ownership, lifetimes, `unsafe`,
traits nor the error model; it adds no class hierarchy and none of Python's dynamic semantics; and
it wraps no crate in an Uredo-specific API.

The escape hatch is Rust itself.

## How this was built

Every rule that was a judgement call was decided the same way: measure it on real Rust first,
prototype it behind a switch, verify the claim in a shell rather than in prose, put it to a panel of
models with differentiated briefs, verify what the panel claims, then apply it with a test and a
decision row, keeping the losing arguments alongside the winning one. The reviews repeatedly
overturned the document, and the fixtures repeatedly overturned the reviews: most of the defects
found in the last week came from *building* something rather than from reading anything.

Where a decision could be wrong, the spec records what would reverse it and how that would be
counted — §38 — rather than only why it was taken.

## Licence

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

**Why both**, which is the Rust ecosystem's convention and not an accident. Apache-2.0 carries an
express patent grant that MIT is silent about, so a user who needs that protection can take it; but
Apache-2.0's patent-retaliation terms count as further restrictions under GPLv2, so code under it
cannot be linked into a GPLv2-only project, and MIT can. Offering both lets each user take whichever
fits, and matches what Rust itself and most of the crates Uredo generates code against already use —
which means every licence scanner, corporate policy and contributor agreement in the ecosystem
already knows what to do with it. Apache-2.0 also declines to grant trademark rights explicitly
(§6 of that licence), which suits a project whose name is its own (§42).

Unless you state otherwise, any contribution you intentionally submit for inclusion shall be dual
licensed as above, without additional terms or conditions.
