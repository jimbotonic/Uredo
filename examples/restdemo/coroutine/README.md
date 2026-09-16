# A spike: the same shape on stackful coroutines

**This is one endpoint, not the service.** It exists to answer one question — can Uredo write an
HTTP server on the stack the TechEmpower front-runner uses, and what does that cost or save? —
and the answer is worth more than the sixteen lines it took.

`may_minihttp` is `may-minihttp`'s own crate: stackful coroutines rather than futures. The
interesting part for Uredo is what it does **not** need. No `async fn`, no `.await`, no futures. A
coroutine handler is straight-line blocking code, which is the shape Uredo spells most directly —
and `async` blocks are the one construct that still needs `rust { }` (§38, held with D51), so the
tokio version of this service uses one and this version uses none.

```uredo
impl HttpService for Service:
    fn call(self: inout, req: take Request, rsp: inout Response) -> io::Result<()>:
        match req.path():
            "/health":
                rsp.header("Content-Type: application/json")
                rsp.body("{\"status\":\"ok\",\"items\":0}")
            _:
                rsp.status_code(404, "Not Found")
        Ok(())
```

`req: take Request` is the one thing worth pointing at: a foreign trait fixes its own parameter
modes, and §10.2's default would have borrowed it. D53's table covers std traits only, so for
anyone else's trait the mode is written.

## What it measures

Three alternating repetitions, 128 connections, 8 seconds, server pinned to cpus 0–7, same
generator throughout, and `may-minihttp` built from the benchmark's own source against a real
Postgres:

| | rep 1 | rep 2 | rep 3 | median |
|---|---|---|---|---|
| uredo + hyper, `/health` | 297,656 | 287,611 | 283,502 | 287,611 |
| **uredo + coroutines, `/health`** | 368,557 | 352,627 | 370,852 | **368,557** |
| `may-minihttp` `/json` | 366,740 | 361,867 | 359,752 | 361,867 |

**Uredo on coroutines lands at parity with the front-runner** — within 2%, winning two of three,
which at that margin is a coin flip. And it is about **28% faster than the same service on
hyper**.

That is the answer to "is the language the bottleneck": it is not, and it never was. The
generated Rust is the same work, so swapping the HTTP stack moves the number and swapping the
surface syntax does not. It is the same finding as the twin benchmark from the other direction.

## The full read path, and a prediction that was wrong

`/items/1` is now served by this edge too — the same lock, lookup, clone, eight-field
serialisation and ETag as the tokio one, because **it is the same code**: the store, model, query
parser, representations, ETag and router come from the service's own library as a dependency.
Only the HTTP edge is written twice.

Before measuring, the prediction was that the 28% seen on `/health` would **shrink**, since
`/items/1` spends a larger share of its time in our code and none of that changes with the stack.

| | rep 1 | rep 2 | rep 3 |
|---|---|---|---|
| hyper `/items/1` | 152,644 | 159,823 | 149,969 |
| **coroutines `/items/1`** | **226,732** | **235,414** | **214,513** |
| | +48% | +47% | +43% |

**It grew, to about +47%.** The prediction was wrong and it is recorded because being wrong in a
stated direction is worth more than not having said anything.

The likely reason, offered as a hypothesis rather than a conclusion: the hyper edge builds a
`Response` through a builder with a `HeaderMap`, then copies the body into `Bytes`; the coroutine
edge pushes headers into a fixed array and moves the `Vec`. Both costs scale with the number of
headers and the size of the body, so the *more* a response carries, the more the hyper edge pays.

One asymmetry in our own code is part of it and should not be hidden: on the hyper edge
`/health` goes through `value_out`, which sets one header, while `/items/1` goes through
`value_out_cached`, which sets four. On the coroutine edge both set four. So some of the widening
is our handler, not the stack.

## What this cost, which is the part nobody advertises

`may_minihttp` **0.1.11 — the version the TechEmpower entry itself depends on — cannot set a
header computed at runtime.** Its `header()` takes `&'static str`. An ETag is a hash of the body,
so it cannot be expressed at all; this port depends on the git master, where
`IntoResponseHeader` accepts a `String`.

That is worth stating plainly next to the throughput: part of what the faster stack buys is
bought by doing less, and the released version of it does not support a feature this service
has. The published number and the feature set are not independent.

## What it does not say

- **One endpoint.** `/health` against `/json`, which is comparable work. The real service's
  `/items/1` takes a lock, looks up a map, clones, serialises eight fields and hashes an ETag.
  Porting all of it is a different exercise and has not been done.
- **`may` is stackful coroutines**, which is not a free lunch: `unsafe` underneath, its own
  stacks, and documented care needed around blocking syscalls and thread-local storage. The
  30% is bought, not found.
- Numbers are higher here than elsewhere in this repository because the machine was warm. Only
  the within-session comparison is meaningful, which is why all three were run alternating.
