# What is decided, and why

Short answers to the things people ask for first. Each one was decided, most of them against a
measurement, and the full argument is in `docs/UREDO_LANGUAGE_SPEC_v0.4.md` at the section named.

**This is not a wishlist of things not yet built.** Everything below was considered and settled. If
you want one of them reopened, §38 says what would reopen it — usually a number, taken over real
code. Bring the number and the case is already half made; bring taste and it is not.

---

### "Why do I write `take`? Can't the compiler tell?"

It can. The point is that *you* can too. `take` is the only annotation that marks a non-`Copy`
ownership transfer, so a move is never invisible at the call site. Everything else — the borrow,
the `&` insertion, the `Ok(…)` wrapping — is inferred. This one is deliberate. §10.2, D52.

### "Why `var`? Rust has `mut` and you could infer it."

Mostly it could: 77% of `var` bindings in real Uredo are decidable, 48% lexically and the rest with
a table of std receiver modes. It stays because it tells the *reader* the binding changes, and
because the remaining 13% would need a large table versioned with every Rust release. The reasoning
was wrong the first time it was written down and was corrected by a review. §8.1, D6.

### "Why must I write `self`, `self: inout`, `self: take`?"

Receiver inference was considered for private methods and rejected: it would make a signature depend
on a body, and one uniform rule beats two. §12.3, §38.8.

### "Why no `?.` optional chaining?"

Deferred, not forgotten. There is no lowering for it that is independent of field types, and Uredo
has no type engine. A version limited to Uredo-declared types would have an invisible boundary,
which is worse than not having it. §14, §38.7.

### "Why is the turbofish still here? It's ugly."

Because 17 of the 22 turbofishes in real Uredo are mid-chain — `xs.iter().sum::<f64>() / n` — where
there is no binding to annotate instead. An annotated binding replaces 1 of the 22. §38.3.

### "Why do I still write `&` when calling Rust functions?"

Because resolving a foreign signature needs a type engine, and Uredo deliberately has none.
Arguments to Rust items are Rust's, verbatim. It is the single largest source of `&` in real Uredo
code and the thing new users trip over most. §5.2, D2.

### "Why lifetimes and apostrophes at all?"

They are Rust's, kept because they mean what Rust means. Two attempts at a smaller notation were
put to review panels and both were declined — one covered 14% of occurrences, the other 46.6%, so
each would have been a *third* notation rather than a smaller one. Inferring `'static` is rejected
outright: it is undecidable from declarations. Real Uredo writes 1.98 lifetimes per 1,000 lines.
§9.7, §38.

### "Why not make the block colon optional? The indentation is right there."

Because a colon-less declaration already means something else — `struct Marker` is a unit struct —
and real code uses the colon to end a multi-line header. Every comparable language keeps a
header/body marker: Python's `:`, F#'s `then`/`do`, Haskell's `then`/`=`, Scala 3's `then`/`do`/`:`.
§6, D62.

### "Why not drop `fn` and `->` too?"

They were examined and priced: 0.97% and 0.72% of all tokens. Both were declined — that is a small
saving for dropping the two tokens a Rust programmer recognises a declaration by. §6, §11, D62.

### "Why `.ure` and not `.ur`?"

GitHub Linguist assigns `.ur` to UrWeb, and will not take a new language until it has 2,000 indexed
public files. So under `.ur` every Uredo file would render as another language for years, a cost
every *user* would inherit; under `.ure` it is merely unrecognised. §20.3, D61.

### "Why braces for struct literals, when everything else is Python-shaped?"

It is the only form under which field punning, `..base`, foreign structs, struct variants and
patterns are all decided by syntax alone. §12.1, §38.2.

### "Why is there no type engine? Half of this would be easier."

Yes, and that is the trade. Without one, a public signature is computable from its own declaration,
so a body edit or a dependency update can never change it — which is what makes the generated Rust
reviewable and the API stable. Nearly every "why can't it just infer…" answer above traces back
here. §5.2, §4.4, D1, D14.

---

## How to argue with any of this

§38 records, for the decisions most likely to be wrong, **what would reverse them and how it would
be counted** — a rate per 1,000 lines of real Uredo, a diagnostic frequency, a clone count. Three
of those triggers have been measured and none has fired. The measurements are more welcome than the
opinions, and a decision that loses to one gets a new row rather than an argument.
