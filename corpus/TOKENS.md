# How much Uredo saves, and on what that depends

A single headline number invites one question — *which code?* — and cannot answer it. This is the
answer: every pair this repository can measure, scored with one tokenizer, and the rule that puts a
given piece of code at one end of the range or the other.

Reproduce with `python3 corpus/tokens.py`.

## The range

**−3.8% to −18.5% tokens, median −13.0%, token-weighted −10.6%, over 29 measured pairs.**

The corpus's gated figure — −13.4% on the application-style programs — is not wrong, and it is not
typical either: it sits near the top of this distribution, because the twenty programs were written
to exercise language features and are therefore unusually dense in the statements and blocks that
Uredo's layout removes. The distribution is what to quote.

It also agrees with the one prediction made before any of this was built. The corpus study modelled
the reduction for ten real repositories — ripgrep, bat, fd, mini-redis, mdBook, axum, json, clap,
tokio, bevy — and got **8.8% to 13.9%** (`docs/corpus-study/FINDINGS.md`, R9). The measured
token-weighted −10.6% and median −13.0% land inside that, which is the more reassuring result: the
model was built from token counts on code nobody had translated, and the translations did not beat
it.

## The rule

**Three quarters of everything Uredo removes is braces and semicolons.**

| token | net removed | share of the saving |
|---|---:|---:|
| `;` | +682 | +30.6% |
| `{` | +485 | +21.7% |
| `}` | +484 | +21.7% |
| `,` | +270 | +12.1% |
| `let` | +223 | +10.0% |
| `&` | +170 | +7.6% |
| `\|` (closure pipes) | +154 | +6.9% |
| `:` (block headers, annotations) | **−600** | **−26.9%** |

Braces and semicolons alone are 1,651 tokens, **74%** of the entire saving. Everything else very
nearly cancels: `let`, call-site `&`, closure pipes, `mut` and attribute brackets come off; the
colons that end every block header and carry every type annotation go back on, and they are the
single largest cost.

Which gives the rule, and it is mechanical rather than aesthetic:

> Uredo's saving is proportional to how many **statements and blocks** a piece of code has per
> token. It is not proportional to anything else.

The evidence is that the twin's `{`/`}`/`;` share is the only predictor with any signal
(r = **+0.52**); the shares of `::`, `<`/`>`, `&` and identifiers all correlate at |r| < 0.25.

### So code lands at the bottom of the range when

1. **It is raw Rust.** `rust { }` blocks and hand-written `.rs` modules save exactly zero by
   construction. `corpus/20_raw_rust` (−6.0%) and `corpus/19_rs_module` (−3.8%) are there on
   purpose, and they are the floor.
2. **It is dense in paths and generics.** `restdemo`'s HTTP edge (−6.6%) and its axum edge (−5.6%)
   spend 5.8–6.5% of their tokens on `::` alone — the highest in the table — and those tokens are
   Rust's, unchanged. `roundtrip/p2_size` (−6.2%) has the lowest brace share of any pair, 4.5%.
3. **It is long call chains.** `roundtrip/p6_fs` (−8.5%) is dominated by `with_context` chains:
   one statement, many tokens, one semicolon.

### And at the top when

It is ordinary application code — struct and impl bodies, `match`, short statements.
`corpus/07_file_reading` and `corpus/14_mutable_borrow` both reach −18.5%.

## What this means for the claim

The defensible statement is **"10–13% fewer tokens on ordinary application code, less on code that
is mostly paths, generics or raw Rust, and nothing at all inside `rust { }`"** — not a single
figure. The last clause is not a caveat bolted on; it is the escape hatch working as designed, and
a number that hid it would be measuring the wrong thing.

## Method

One tokenizer for both languages (`docs/corpus-study/roundtrip/tools/metrics.py`), comments and
blank lines excluded on both sides.

A pair is counted **whole**. If the Uredo side ships a hand-written `.rs` module beside the `.ure`,
that module is counted, because the question here is what the program cost rather than what its
Uredo-written part cost. `corpus/run.py` asks the narrower question and excludes the shared module
from both sides, which is why `19_rs_module` reads −11.6% there and −3.8% here. Neither is wrong:
the gap between them *is* rule 1, measured.

The round-trip and corpus tokenizers agree where they overlap — both give −13.4% on corpus
programs 1–15 — so the two studies' figures can be read together.

## Every pair

| pair | Rust tokens | Uredo tokens | saving | `{` `}` `;` share of the Rust twin |
|---|---:|---:|---:|---:|
| `corpus/07_file_reading` | 378 | 308 | **−18.5%** | 10.3% |
| `corpus/14_mutable_borrow` | 341 | 278 | **−18.5%** | 13.8% |
| `corpus/01_hello` | 39 | 32 | **−17.9%** | 15.4% |
| `roundtrip/p4_any_value` | 855 | 716 | **−16.3%** | 9.5% |
| `corpus/16_async_http` | 426 | 357 | **−16.2%** | 9.9% |
| `roundtrip/p3_rand` | 377 | 316 | **−16.2%** | 13.5% |
| `roundtrip/p1_human` | 300 | 253 | **−15.7%** | 12.0% |
| `roundtrip/p5_sanitize` | 677 | 571 | **−15.7%** | 13.4% |
| `corpus/04_struct_crud` | 487 | 413 | **−15.2%** | 10.5% |
| `corpus/08_json_serde` | 318 | 270 | **−15.1%** | 8.5% |
| `corpus/09_error_from` | 410 | 350 | **−14.6%** | 10.0% |
| `corpus/12_trait_impl` | 392 | 337 | **−14.0%** | 12.2% |
| `corpus/05_enum_match` | 410 | 353 | **−13.9%** | 8.3% |
| `corpus/11_generic` | 438 | 378 | **−13.7%** | 9.8% |
| `corpus/02_numeric` | 346 | 301 | **−13.0%** | 11.0% |
| `corpus/06_optional` | 541 | 476 | **−12.0%** | 8.1% |
| `corpus/13_ownership` | 381 | 336 | **−11.8%** | 11.5% |
| `corpus/18_rayon` | 259 | 229 | **−11.6%** | 10.4% |
| `corpus/15_shared_borrow` | 365 | 327 | **−10.4%** | 8.2% |
| `corpus/03_strings` | 400 | 359 | **−10.2%** | 9.8% |
| `restdemo (whole service)` | 5388 | 4858 | **−9.8%** | 8.9% |
| `corpus/10_iterators` | 544 | 494 | **−9.2%** | 7.7% |
| `roundtrip/p6_fs` | 1597 | 1462 | **−8.5%** | 9.0% |
| `corpus/17_tokio` | 310 | 286 | **−7.7%** | 11.0% |
| `restdemo (HTTP edge)` | 1612 | 1505 | **−6.6%** | 9.2% |
| `roundtrip/p2_size` | 1767 | 1657 | **−6.2%** | 4.5% |
| `corpus/20_raw_rust` | 233 | 219 | **−6.0%** | 10.7% |
| `restdemo (axum edge)` | 1181 | 1115 | **−5.6%** | 7.7% |
| `corpus/19_rs_module` | 364 | 350 | **−3.8%** | 11.3% |
