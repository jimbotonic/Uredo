# Design: a REST service in Uredo

Draft, 2026-09-15. Nothing is built yet. This says what to build, why that shape, and what would
make it a failure.

## 1. The two claims, kept apart

The goal is an opinionated JSON REST service that is **simple and extremely efficient**. That
target stands. What follows separates it into two claims, because they are earned in different
places and only one of them is about Uredo.

**The speed claim belongs to the design, not the language.** Uredo is a surface syntax with no
runtime; §36 retired the runtime budget because generated Rust performs like hand-written Rust. So
the service written in Uredo *is* the Rust service, and cannot be faster than the same design
written in Rust by hand. Rapidoid's advantage came from JVM headroom — avoiding reflection, a
hand-rolled HTTP parser — against a slow baseline. Against hyper and tokio that headroom is gone.

**But the design still has real room, and it is the same room Rapidoid found.** A framework-shaped
service pays for generality: a dynamic router, a tower middleware stack, boxed futures, an owned
`String` per path parameter, a fresh allocation per body. A service that fixes its routes and its
type set at compile time pays for none of that. That is where the speed comes from, and it is
measurable against a framework-shaped equivalent.

**Uredo's claim is then narrower and sharper: that it does not get in the way.** A language that
could not express borrowed, zero-copy data would make this design unwritable. That was the largest
risk in this draft and it was tested before the draft was finished — a lifetime-parameterised struct
over borrowed bytes lowers to exactly the Rust one would write by hand, compiles and runs:

```uredo
struct Request<'a>:
    method: &'a str
    path: &'a str
    body: &'a [u8]

impl<'a> Request<'a>:
    fn parse(raw: &'a [u8]) -> Request<'a>?:
        text = std::str::from_utf8(raw).ok()?
        ...
        Some(Request { method, path, body: raw })
```

*(An authoring error was made writing even that: the tail was first written as a bare
`Request { … }`. A `-> T?` return is a verbatim `Option` and only `throws` bodies get the `Ok`
wrapping of §15.3. Recorded, because errors of exactly this kind are one of the deliverables.)*

## 2. What the measurement says is missing

Across all 62 files of real Uredo — `tools/`, `corpus/`, `examples/`:

| construct | uses today | files |
|---|---|---|
| **`Arc` / shared ownership** | **0** | **0** |
| **`Mutex` / `RwLock`** | **0** | **0** |
| trait declaration | 2 | 2 |
| generic `fn` with a bound | 3 | 3 |
| lifetime in a signature | 2 | 2 |
| `impl Trait for Type` | 5 | 4 |
| multi-file `mod` | 7 | 7 |
| `async fn` | 20 | 6 |

**Shared ownership across tasks has never been written in this language.** It is the defining shape
of a concurrent service and the place the ownership vocabulary is most likely to hurt — which is
precisely where the last review located the real friction.

And the specification says the rest itself, about what would reverse D52:

> *Neither has been measured on Uredo code, because no project of the size needed exists — the §43
> programs and the compatibility fixtures are too small to be evidence either way, and saying so is
> the point of recording the trigger rather than the verdict.*

A service built around `Arc<AppState>` is where a `.clone()` written to satisfy a by-value P8
parameter would actually appear. **This demo is the first thing that could move §38's D52 trigger
off "not measured".** That is the strongest reason to build it, and it is the project's own.

## 3. Scale

| | lines |
|---|---|
| corpus programs | 25–55 each |
| largest Uredo program today (`tools/src/bin/corpus-run.ure`) | 515 |
| the axum compatibility entry | 78 (recorded "partial") |
| **target for this demo** | **800–1,200 across several modules** |

800 is the floor because §38's triggers are rates per 1,000 lines; below that the demo cannot
be evidence either way, which is the trap the fixtures already fell into.

## 4. Shape

**`hyper` + `tokio` directly, not a framework.** The point of the design is to not pay for
generality, and axum is already recorded as a compatibility entry — using hyper directly adds a new
one rather than repeating it.

```
src/
  main.ure        the listener and the accept loop
  router.ure      a fixed match over method and path segments — no dynamic dispatch
  handler.ure     the handlers: borrowed request in, pre-sized response out
  store.ure       the state: Arc<RwLock<…>>, the shared-ownership surface
  model.ure       the fixed type set, serde with borrowed deserialisation
  error.ure       one error type, `throws`, the status mapping
  wire.ure        request parsing and response writing over borrowed bytes
tests/
  api.ure         integration tests against a live listener on an OS-chosen port
```

Each module earns its place by exercising something §2 shows is thin:

| module | what it is there to exercise |
|---|---|
| `store.ure` | **`Arc`, `RwLock`** — zero uses today — `inout`, and every `.clone()` the P8 rule provokes |
| `wire.ure` | **lifetimes in signatures** — 2 uses today — borrowed `&'a str` / `&'a [u8]`, no allocation per request |
| `error.ure` | a declared error type, `throws`, `@!default_error`, a **trait impl** for the status mapping |
| `model.ure` | serde derive with borrowed fields, `T?` optionals, validation returning `throws` |
| `router.ure` | a `match` over segments; **generic helpers with bounds** — 3 uses today |
| `handler.ure` | `async fn` at length, and the passing modes under real call pressure |
| `tests/api.ure` | the `tests/` target kind, async tests, a second crate root |

**The opinionated part, and what it actually buys.** A fixed, small field type set (`&'a str`,
`i64`, `bool`, `T?`, `Vec<T>`), fixed routes, one error shape, one store. In Rust this does not buy
what it buys on the JVM — monomorphisation already specialises and there is no reflection to
dodge. It buys **no dynamic router, no middleware stack, no boxed futures, no per-request
allocation**, a small API and a fast build. Those are the mechanisms; the benchmark in §5 is what
decides whether they matter.

## 5. What it must produce

The program is not the deliverable. These are:

1. **§37 metrics at 10× the corpus scale** — tokens, lines, characters, signature annotations —
   against an idiomatic Rust twin of the same service. Every published figure today comes from
   25–55 line exercises; this is the first test of whether they hold.
2. **`uredo lint --measure` on it** — the two D52 counts, and the E0382 grep over the build log.
   Today they read 11 and 0 over 69 files that are all too small to matter.
3. **The written-lifetime rate**, which §38 wants at ≥2,300 lines and above 3 per 1,000 to reopen
   the lifetime notation. A borrowed-data service is the one program likely to produce it; `tools/`
   sits at 1.98 per 1,000 over 2,025 lines.
4. **The authoring-error list.** The 24 errors from `tools/` produced the manual's best chapter.
   One was made writing the six-line sketch in §1.
5. **Two benchmarks, with different expectations.**
   - *Against the hand-written Rust twin of the same design:* **expect parity.** A null result
     confirms §36 and is the honest outcome; a gap either way is a finding about the compiler.
     **Run, 2026-09-15: +2.9% median per-pair difference over a range of −33% to +10%, against a
     same-binary noise floor of 31%. Parity, to the resolution the machine allows.** An earlier
     run reported 39,495 against 39,219 and was of neither program — see the demo's README.
     `bench/run.sh`.
   - *Against a framework-shaped equivalent (axum, same routes, same responses):* **expect a win**,
     and report it as a result about the *design* — fixed routing and borrowed parsing against a
     dynamic router and an owned-`String` path — never as a result about Uredo.

   Both measured with the same load generator, same machine, same release profile, reporting
   latency distribution rather than a single throughput number, and with the measurement script
   committed so the figure can be reproduced.

## 6. How it could fail, and the stop condition

- **It becomes a product.** The scope is fixed at §4 and does not grow. No auth, no database, no
  middleware stack, no configuration system. If a feature is not exercising a row in §2's table, it
  is not in the demo.
- **It becomes a benchmark fight.** §5 permits exactly two comparisons — the hand-written Rust
  twin of the same design, and one framework-shaped equivalent on identical routes — and both are
  run by a committed script on one machine. Anything else is out: no actix, no Rapidoid, no
  cross-language table, no tuned configuration on one side and a default on the other. The
  framework comparison is a statement about *dynamic routing and per-request allocation*, and if it
  cannot be stated that narrowly it should not be published at all.
- **It is still the author writing Uredo.** This is evidence about the *language*, not about
  adoption. It does not substitute for the five-person trial in the release gate.
- **Maintenance.** It is a second artefact that must keep building. If it cannot be kept green, it
  should be deleted rather than left broken — a stale demo is worse than none.

**Stop condition:** if the §37 deltas at this scale come out materially worse than the corpus's
(−13.0% tokens, −35.3% lines), that is a finding and it gets published, not buried. The demo is an
experiment with a result, not a showcase with a conclusion.
