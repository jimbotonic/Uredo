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
| **tokens** | 4,855 | 5,388 | **−9.9%** | −13.0% |
| lines | 518 | 637 | **−18.7%** | −35.3% |
| non-whitespace characters | 14,839 | 15,429 | **−3.8%** | −8.1% |
| — of which identifiers | 2,235 | 2,251 | **−0.7%** | |
| — of which punctuation | 2,504 | 3,022 | **−17.1%** | |
| functions | 57 | 57 | 0.0% | |

Both sides carry the content negotiation, so the comparison is whole-library again. **The
equivalence is byte-level, not merely behavioural**: driven with the same requests the two
servers return the same lengths *and the same ETag*, which is a hash of the bytes they wrote —

```
uredo      keyed 110  compact 49  listing 334  listing compact+br 151   etag "1195307f2dd081e0"
rust twin  keyed 110  compact 49  listing 334  listing compact+br 151   etag "1195307f2dd081e0"
```
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

### Two ways to send less

Both opt-in, both driven by request headers, and they matter at opposite ends.

**Dropping the field names.** `Accept: application/vnd.restdemo.compact+json` returns the same
values positionally. A service with a fixed type set already knows the field order, so repeating
the names in every element of every list is paying for a schema the client also has.

**Brotli.** `Accept-Encoding: br`, above a 256-byte threshold — below that a brotli frame costs
more than it saves, which is measured rather than assumed.

Response bodies from the running server, in bytes:

| | one item | listing of 50 |
|---|---|---|
| keyed JSON | 117 | 5,983 |
| compact | **56** (−52%) | 2,933 (−51%) |
| keyed + br | 117 (unchanged, under the threshold) | **258** (−96%) |
| compact + br | **56** | **186** (−97%) |

So: **dropping names is the whole win for a small response, compression is the whole win for a
large one**, and neither is a default. A single item is too small for brotli to touch; a listing
compresses to 4% of itself because the repeated field names are exactly what a compressor eats.

Each representation carries **its own ETag**, computed over the bytes actually sent, and every
response carries `Vary: accept, accept-encoding`. Without that a cache keyed on the URL alone
would hand one client's shape to another — which is the part of content negotiation that is easy
to ship broken, so there is a test for it.

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

#### How fast it is, and what that can be compared to

`bench/target/release/floor` is a hyper server that returns a fixed 15-byte body and does nothing
else. It exists because without it the service's numbers are uninterpretable: at 32 connections
`/health` measured 166,332 rps and the floor measured 165,567 — **the service was indistinguishable
from a server that does nothing**, which means the harness was the bottleneck and the number was
the generator's, not ours.

At 128 connections, where there is headroom:

| | rps | p50 | share of the floor |
|---|---|---|---|
| floor (fixed body, no work) | 221,638 | — | 100% |
| `/health` | 204,751 | 535 µs | **92%** |
| `/items/1` (lock, lookup, clone, serialise, ETag) | 187,923 | 599 µs | **84%** |
| `/items` (50 items serialised) | 89,252 | 1,350 µs | 40% |

So the routing, store and ETag work costs about **16%** of what this machine and this HTTP stack
can do at all; the other 84% is hyper, loopback and the generator. That is the useful reading, and
it is the only one this setup supports.

#### Against a TechEmpower front-runner, built and run here

The comparison that *does* transfer is one run on this machine. `may-minihttp` — a Rust framework
in the top tier of TechEmpower's final round — was fetched, built and run locally against a real
Postgres with the benchmark's own schema, so its code path is upstream. Only two things differ
from the harness: the database host, because `tfb-database` is a name that exists only inside it,
and the listen address.

Same generator, 128 connections, 8 seconds, server pinned to cpus 0–7:

| | rps | what it does |
|---|---|---|
| `may-minihttp` `/plaintext` | 353,986 | a fixed 13-byte body |
| `may-minihttp` `/json` | 257,625 | serialise `{"message":"Hello, World!"}` |
| **uredo `/health`** | **216,904** | serialise a two-field struct |
| floor: hyper, fixed body | 196,390 | nothing at all |
| **uredo `/items/1`** | **194,919** | lock, lookup, clone, serialise 8 fields, ETag |

Over three repetitions with the order reversed each time, `uredo /health` measured **75%, 76% and
99%** of `may-minihttp /json` — median about **76%**, on comparable work.

Two readings, and the second matters more:

- **The gap is mostly hyper, not Uredo.** `may-minihttp` beats a *bare hyper server returning a
  fixed body* by 31%, because it does not use hyper — it has its own HTTP implementation on
  coroutines. Anything built on hyper, in any language, is bounded near that floor.
- **`uredo /items/1` matches bare hyper while doing real work** — a read lock, a map lookup, a
  clone, an eight-field serialisation and an FNV hash for the ETag.

Caveats that are not decoration: one desktop with client and server sharing it, no pipelining,
run-to-run spread of 25–30%, and a single workload shape.

`bench/against-tfb.sh` rebuilds that setup — database, schema, framework source and the two
repointings — so the comparison can be re-run rather than taken on trust.

#### Why the published numbers are not quoted beside these

Those numbers do not transfer, and putting them beside these would be worse than saying nothing:

- **Different machines.** Published figures come from server-class hardware with the client on a
  *separate* box over 10GbE. Here client and server share one 16-core desktop and compete for it.
- **Different workload.** The very large published figures are almost always the *plaintext* test:
  a fixed ~13-byte body, **pipelined**, with many requests in flight per connection. This
  generator does not pipeline, and `/items/1` takes a read lock, looks up a `BTreeMap`, clones,
  serialises and hashes the body for an ETag.
- **Different client and concurrency.** Tuned `wrk` thread and connection counts per framework
  against a fixed 128 connections here.

Any of those alone moves throughput by more than the differences anyone would be comparing. The
honest way to get a cross-framework number is to run this service *in* the other harness, on its
hardware, under its rules — not to quote two numbers taken under different conditions.

What **is** available and not yet run is the comparison the design note permits: a
framework-shaped equivalent on identical routes, on this machine, under this generator. A win
there belongs to fixed routing and borrowed parsing rather than to Uredo.

#### What the single store lock costs, measured

The store is one `RwLock` over one `BTreeMap`, so every write excludes every read. At 48 read
connections against 16 concurrent writers, median of four repetitions:

| | reads/s | p99 |
|---|---|---|
| reads alone | 144,411 | 971 µs |
| reads with 64,334 writes/s alongside | 110,802 (**−23.3%**) | 1,121 µs (**+15.4%**) |

The per-repetition drops were −22.7%, −24.8%, −23.5% and −23.1%, so this one is repeatable in a
way most numbers on this page are not. Part of it is the writers taking CPU rather than taking
the lock, so 23% is an upper bound on the lock's own cost, and the control is that the writer
alone sustains 116,196/s — it is genuinely able to compete.

The first attempt measured nothing: a shell `curl` loop as the writer runs at a few hundred
requests per second against 130,000 reads, and reporting that as "no contention" would have been
reporting the loop's speed. The generator takes a method and a body now.

**These numbers replace the ones published here on 2026-09-17, which were wrong** — −14% and
+37%, against −23.3% and +15.4%. Both were understated or overstated for the same reason, and it
is the next section.

#### The benchmark harness was leaking servers, and had been for days

`measure()` started each server with `pin … &` and recorded `$!` as its pid. `pin` is a shell
*function*, and `$!` on a background function is the subshell bash forks to run it — not the
server that subshell then starts. Every `kill` in the script killed the wrapper.

Servers accumulated: across a run, across a session, across days. Seventy were found alive, some
twenty-six hours old, each holding a tokio runtime's worth of parked threads. They were idle, so
the damage was not what it first looked like — but the noise floor climbed run by run, and in the
middle of the framework comparison below it reached **40.5%**, which is larger than anything this
page tries to measure.

The proof that it mattered: the axum comparison was run twice before the fix and twice after, and
**the sign changed**. Before: −2.7% and −4.6%, with axum losing six paired runs out of six. After:
+4.5% and +1.7%. Had the comparison stopped at the second run, this page would carry a confident,
carefully-controlled, six-for-six finding that axum costs 4.6% — and it would have been an
artefact of the harness.

Fixed by starting the server as a simple command so `$!` names it, by a `stop()` that escalates
to `SIGKILL` and aborts the whole run if a server survives, and by a pre-flight check that
refuses to measure anything while a server from an earlier run is alive.

This is the third time on this page that a control caught something the measurement itself was
happy to report. The pattern is not that the benchmarks were careless; it is that **a benchmark
tells you what it measured, and only a control tells you whether that was the thing you meant.**

#### The framework-shaped comparison

`examples/restdemo/axum/` and `examples/restdemo/axum-ure/` are the same service with axum
supplying the HTTP edge, in Rust and in Uredo. Both depend on the hand-written twin as a library,
so the store, the model, the query grammar, the wire shapes, the ETag and the error surface are
imported rather than rewritten: what differs between the four binaries is the edge, and nothing
else. All four pass the same eleven tests, the axum ones running the twin's `tests/api.rs` and
`tests/api.ure` with one name changed.

Counting only the edge — the accept loop, the router, the dispatch, the extractors and the
binary — with one tokenizer over both languages:

| HTTP edge | lines | tokens |
|---|---|---|
| Rust, no framework | 170 | 1,612 |
| Uredo, no framework | 142 | 1,505 |
| Rust + axum | 102 | 1,181 |
| Uredo + axum | **85** | **1,115** |

Which separates cleanly, because the two effects barely interact:

| | tokens | lines |
|---|---|---|
| the language alone, without a framework | −6.6% | −16.5% |
| the language alone, with axum | −5.6% | −16.7% |
| the framework alone, in Rust | −26.7% | −40.0% |
| the framework alone, in Uredo | −25.9% | −40.1% |
| both together | −30.8% | −50.0% |

**The framework is worth four times what the language is worth here**, and saying so is the point
of running it. Anyone choosing between "write it in Uredo" and "write it in Rust with axum" on
token count alone should choose axum. The honest pitch is that they compose: axum in Uredo is the
smallest of the four, and it is smaller than axum in Rust by about what Uredo is worth anywhere.

Two things temper the language column. The −6.6% here is **half** the −13.4% this project
publishes for its corpus, because an HTTP edge is unusually hostile to Uredo's savings: it is
dense in type-heavy signatures and calls into foreign crates, where there are no braces, no
`let` and no `//` to drop. And the Uredo edge carries 25 annotations against axum-in-Rust's 18 —
the extractors arrive by value, so `take` appears on every `HeaderMap` and every `Bytes`, which
is the mode being visible rather than the mode being free.

On throughput there is nothing to report, which is the expected result twice over:

| | median of per-pair differences | paired runs won | noise floor |
|---|---|---|---|
| axum vs hand-rolled hyper | +4.5%, then +1.7% | 5 of 6, then 3 of 6 | 18.1%, 40.5% |
| Uredo + axum vs Rust + axum | +3.2% | 4 of 6 | 16.3% |

Neither difference is resolvable by this harness on this machine. For the second row that is the
§36 result again — generated Rust performs like written Rust, and a paired cross-crate benchmark
cannot resolve a few percent. For the first, it means axum's router, extractors and tower stack
cost nothing this workload can detect at ~110,000 requests per second, and `axum/src/bin/http1.rs`
exists to check that the auto http1/http2 detection in `axum::serve` was not hiding a cost the
rest of the framework was paying back.

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

A fourth thing came out of it later, and it is the leaking harness two sections above: `identify()`
proved the right server was answering, and nothing proved the wrong ones had stopped.

The framework-shaped comparison the design permits — against an equivalent on identical routes —
has now been run, and is two sections above.

## What it cost, and what that bought

Four defects in the compiler, every one found by writing this rather than by reading anything:

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

Thirteen authoring errors, which are the other deliverable:

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
