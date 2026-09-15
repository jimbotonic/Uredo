# restdemo — an opinionated JSON REST service in Uredo

Fixed routes, a fixed field type set, no framework: `hyper` and `tokio` directly. Design and
rationale in `examples/restdemo/DESIGN.md`.

```bash
uredo test examples/restdemo     # 3 integration tests over real sockets
uredo run  examples/restdemo     # listens on 127.0.0.1:8080
```

```
GET    /health        {"status":"ok","items":N}
GET    /items?done=&label=&contains=&limit=&offset=      filtered, paged, ETagged
POST   /items         201, or 409 on a duplicate name
GET    /items/:id     404 when absent
PATCH  /items/:id     absent field means unchanged
DELETE /items/:id     204
```

## The accepted types

`u8 u16 u32 u64 usize · i8 i16 i32 i64 isize · f32 f64 · String · bool`, plus `T?`, a list
`Vec<T>` and a dictionary `Map<T>` (`BTreeMap<String, T>`) over any of them.

That set is **enforced, not described**: `src/field.ure` declares it as a trait `Field`, and
`model.ure` carries a function that asks the compiler to prove every field of every model type
implements it. A field outside the set stops the build and the error names the type.

## Adding an endpoint

One arm and one closure. The adapters in `handler.ure` are generic, so each monomorphises at its
call site — the dispatch stays a `match` over a compile-time route and nothing is boxed.

```uredo
Route::ListItems: value_out(200, () => Ok(state.list()))?
Route::GetItem(id): value_out(200, () => state.get(id).ok_or(Error::NotFound))?
Route::CreateItem: body_in(req, 201, (new: NewItem) => create(&state, new)).await?
```

An endpoint never sees a request, a status or a byte: it is a function over typed values.

## Threads

`@rust(tokio::main(flavor = "multi_thread"))`, a task per connection, and an `RwLock` so
concurrent reads do not queue behind each other. `concurrent_writers_all_land_and_ids_are_unique`
runs fifty creates in flight on four workers and checks all fifty land with distinct ids.

## Status, against the design's own bar

| | design target | actual |
|---|---|---|
| source lines | 800–1,200 | **911** (592 by the project tokenizer) — in range |
| `Arc` / `RwLock` in real Uredo | the gap to close | **closed** — `store.ure` |
| D52 reversal counts | first real measurement | **4 by-value generic parameters, 0 clones** — a ratio of 0.00 against the trigger's 1.0 |
| written lifetimes | evidence toward §38's trigger | **9 in 781 lines — 11.5 per 1,000**, nearly four times §38's threshold of 3, but the sample is 1,519 lines short of the 2,300 the rate needs |
| borrowed struct fields across all real Uredo | the zero-copy premise | **0 → 2** |
| idiomatic Rust twin, benchmarks | both | **not written** |

**Now within the design's size range, with two readings worth having and one still out of reach.**

The D52 counts: four parameters pass by value under P8 and **no call site wrote a `.clone()` to
pay for it** — 0.00 against a threshold of 1.0, a small reading in D52's favour.

The lifetime rate is the interesting one. At **11.5 per 1,000 lines** this program writes lifetimes
nearly four times as often as §38's reopen threshold of 3 — because a borrowed query parser is
what a lifetime is *for*. But §38 asks for 2,300 lines before a rate counts, and this is 781. So
**the rate clears the bar and the sample does not**, and the trigger has not fired. Recording it
that way round is the point of having written the trigger down.

Before `query.ure` there were **no borrowed struct fields anywhere in real Uredo** — the zero-copy
premise the design rests on had never been written. There are now two.

The idiomatic Rust twin and the benchmarks are still not written.

## The idiomatic Rust twin, and what it says

`idiomatic/` is the same service written in Rust by hand — same modules, same structure, and the
**same eight integration tests**, so "they do the same thing" is a test result rather than a claim.
It was written fresh rather than edited down from the generated Rust, which would have rigged the
comparison.

Measured with the project's own tokenizer over library sources only, as the corpus method does:

| | Uredo | Rust | delta | the corpus, for comparison |
|---|---|---|---|---|
| **tokens** | 3,981 | 4,444 | **−10.4%** | −13.0% |
| lines | 425 | 523 | **−18.7%** | −35.3% |
| non-whitespace characters | 12,227 | 12,749 | **−4.1%** | −8.1% |
| — of which identifiers | 1,846 | 1,863 | **−0.9%** | |
| — of which punctuation | 2,040 | 2,486 | **−17.9%** | |
| annotations | 79 | 84 | **−6.0%** | signature ratio 0.48 |
| functions | 45 | 45 | 0.0% | — |

**Every measure is materially worse at this scale than the corpus predicted**, and the design note
said that if it came out that way it gets published rather than buried. The identical function
count on both sides is the evidence that the twin is a fair one.

Why the gap. The corpus programs are application-style — control flow, iteration, struct literals
— which is where Uredo's layout saves most. A service of this shape is dominated by things Uredo
spells exactly as Rust does: `use` lines, `serde` derives, generic bounds, and arguments to foreign
crates, which D2 passes through verbatim. The annotation figure is the sharpest version of it —
0.94 of Rust's here against the corpus's 0.48 — because these signatures are mostly generic bounds
and foreign types rather than the ownership annotations §37 counts.

It still clears §37's gate of a 10% token reduction, and it clears it by a third of a point on a
program that is not application-style. The honest summary is that **the corpus figures do not
generalise as stated**, and the corpus is the smaller, easier sample.

*An earlier version of this table read −10.5% / −16.8% / −3.4%, measured when the Rust twin used a
`macro_rules!` for its fourteen `impl Field for …` lines and the Uredo side wrote them out. Both
sides now use one. It moved the token figure by 0.2 points — and moved it the **wrong** way for
Uredo, because a brace-free one-line `impl` is already cheap and the macro declaration costs more
than the fourteen lines it replaced. Lines improved by 1.9 points, since the formatter had been
putting a blank line between each. The asymmetry was immaterial, which is only knowable by having
removed it.*

### Where the saving is, and what is left

Splitting the tokens answers the question people ask next — *can it be made leaner still?*

**97% of the gap is punctuation.** Identifiers are −0.8%, which is to say identical: the names in
an Uredo program are Rust's names — its types, its methods, its crate paths — and Uredo does not
touch them. Its entire lever on this program is removing braces, semicolons, `Result<…>` and the
`Ok(…)` wrapper, and it has already pulled that lever.

So the remaining headroom is in identifiers, and **identifier savings are language-neutral**. That
was tested rather than assumed: introducing one `type Reply = Response<Full<Bytes>>` at ten call
sites removed **50 tokens from the Uredo side and 51 from the Rust side**, and moved the ratio by
0.1 of a point. Both programs got about 1.1% leaner; the gap did not move. Shorter names, more
aliases and tighter `use` lists all behave the same way, because Rust can have them too.

The one lever that is Uredo's alone is a bare `throws`, which spells no error type where Rust must
write `Result<T, Error>` every time. Finding out why it did not work here turned up a compiler
defect rather than a design question: **`@!default_error` never reached a child module**, though
§15.1 had always said a crate root's declaration covers its subtree. That is fixed, the demo now
uses it, and it is worth **2 tokens** — which is the honest measure of this lever. It was worth
fixing because the code disagreed with the document, not because of what it saves.

### The benchmark against the twin

`bench/run.sh`, committed. `GET /items/1`, 32 keep-alive connections, 8-second runs, eight
repetitions, server pinned to eight cores and the generator to the other eight, order reversed
every repetition, session warm-up discarded.

| | median rps | p50 | spread across runs |
|---|---|---|---|
| **Uredo** | **140,404** | ~180 µs | 31.5% |
| **Rust twin** | **136,722** | ~185 µs | 17.1% |

**Median of the per-pair differences: +2.9% for Uredo, over a per-pair range of −33% to +10%, and
6 of 8 paired runs.** The control is what that has to be read against: the *same binary* run twice
varies by **31%**. Nothing here resolves a difference smaller than about ±10%.

**So: parity, to the resolution this machine allows** — which is what §36 predicts, since Uredo has
no runtime and the generated Rust is what runs. The two binaries differ by 152 bytes and carry
identical release profiles, which is the other reason to disbelieve any gap this measurement
might have shown.

#### A retraction

An earlier version of this section reported **39,495 against 39,219 rps and called it parity. That
measurement was of neither program.** Port 8080 was held by an unrelated admin service; both
servers failed to bind with `Address already in use`, exited, and the load generator measured that
service twice — serving 404s to a path it did not have. The script never checked that the thing
answering was the thing under test.

Three things came out of fixing it, and they are why the numbers above are worth more than the
numbers they replace:

- Both servers now take `RESTDEMO_ADDR`, and the benchmark picks a port nothing is using.
- `identify()` refuses to time anything until `/health` answers `{"status":"ok"}` **and** the path
  under test returns 200. A benchmark that cannot tell what it is measuring is not a benchmark.
- The ordering was `ABAB`, and the machine ramps: across a session both sides climb monotonically
  as frequency and caches warm, so the second binary always got the later, faster slot. That
  showed up as a confident **−12.3%** which was entirely the ordering. It is `ABBA` now, and the
  statistic is the median of per-pair differences rather than a difference of pooled medians.

What this is **not**: a claim about any framework. The design permits exactly one other
comparison — against a framework-shaped equivalent on identical routes — and it has not been run.

## What it cost, and what that bought

Two defects in the compiler, both found by writing this rather than by reading anything:

- **The formatter deleted a closing `)`.** `tokio::spawn(rust { … })` — a multi-line `rust { }`
  block whose closing line also closes the call around it. The covered lines of a raw token are
  emitted as placeholders because the token prints its own text, so a token starting on the
  token's *last* line was dropped: `})` became `}` and a program that compiled stopped parsing.
  Fixed, with `compiler/tests/fixtures/fmt_hard/src/trailing_close.ure` to keep the shape, and the
  fixture was checked against the unfixed compiler.
- **A macro could not be reached through an absolute path.** `::std::format!(…)` did not parse,
  though `format!` and `std::format!` did: the parser's leading-`::` branch read the path and
  never looked for the `!`, unlike the relative-path branch beside it. D39 spells macros exactly
  that way in *generated* code, so the form the compiler emits could not be written in source.
- **`T?` was not lowered in an `impl` target.** `impl<T: Field> Field for T?` emitted the `?`
  verbatim and the generated Rust did not parse. The impl header was the one type position that
  never went through `lower_type`; the trait path had the same gap. `compiler/tests/types.rs`
  covers both, and checks the generated Rust *compiles* rather than merely parses, since a
  lowering that parses is what let this through.
- **`Ok(())` after a diverging loop.** A `throws` body ending in `loop { … }` with no `break`
  cannot fall through, so §15.3's wrapping was unreachable and rustc said so. Hand-written Rust
  would let the loop be the tail. Fixed conservatively: any `break` inside, even a nested one,
  keeps the old behaviour.

Seven authoring errors, which are the other deliverable:

1. `@tokio::main` — an attribute whose path has `::` is forwarded as `@rust(tokio::main)`.
2. The tail of a `-> T?` function is **not** wrapped in `Some`. Only `throws` bodies get §15.3's
   `Ok`. Made while writing a six-line sketch in the design note.
3. A fallible function returning nothing is `throws`, not `-> ()`.
4. A bare `throws` is refused on a `pub` item (§15.1) even with `@!default_error` declared — a
   published signature says what it returns.
5. `throws` with no `->` is a **unit** return. Writing `fn f(…) throws E:` when the body yields a
   value discarded it, and the error surfaced far away at a hyper trait bound rather than at the
   function.
6. Writing `Ok(…)` yourself in a `throws` body double-wraps. Seven sites in one function.
7. `-> !` does not compile: the never type is still unstable, so a diverging function cannot say so.

8. An empty function body still needs a statement — `()`. A colon-less declaration means *no*
   body, which is a different thing.
9. `format(…)` inside `assert!(…)` is Rust's `format!`. Inside a macro's delimiters the tokens are
   Rust's and no intrinsic applies (§22.5) — a rule the manual states and its author still missed.
10. `for t in tasks:` borrows the place (D12), so the handles were never owned and could not be
    awaited. `for t in take tasks:`.

Numbers 4, 5 and 6 are the same misunderstanding of one rule seen from three sides, which is worth
more than three separate notes: **`throws` describes the error half of the signature and says
nothing about the value half.**

11. An assignment cannot be an inline `match` arm — `"done": q.done = Some(v)` reads as a
    binding. Each assigning arm takes an indented block.
12. The tail of a `throws` body that is *already* a `Result` needs `?`, or §15.3 wraps it into
    `Ok(Result<…>)`. The fourth sighting of the same rule.
13. Uredo's inline `if … else …` is not available inside a macro's delimiters, because those
    tokens are Rust's (§22.5). The second sighting.

A formatting observation, not a defect: `uredo fmt` puts a blank line between consecutive one-line
items, so `field.ure`'s fourteen `impl Field for …` lines become twenty-eight. Canonical is
canonical, but a run of one-line impls reads worse for it, and it is why the line counts above are
quoted from the tokenizer, which counts code lines.

## What is deliberately not here

No auth, no database, no middleware, no configuration, no TLS, no HTTP/2. The field types are
`String`, `i64`, `bool`, `T?` and `Vec<T>` and that is the whole set. A feature that does not
exercise a language shape is not in the demo.

**No performance claim.** Nothing here has been benchmarked. When it is, the comparison is against
an idiomatic Rust twin of the same design — where the expected result is parity, because Uredo has
no runtime (§36) — and against one framework-shaped equivalent, where any win belongs to fixed
routing and borrowed parsing rather than to Uredo.
