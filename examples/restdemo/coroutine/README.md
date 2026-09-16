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

## What it does not say

- **One endpoint.** `/health` against `/json`, which is comparable work. The real service's
  `/items/1` takes a lock, looks up a map, clones, serialises eight fields and hashes an ETag.
  Porting all of it is a different exercise and has not been done.
- **`may` is stackful coroutines**, which is not a free lunch: `unsafe` underneath, its own
  stacks, and documented care needed around blocking syscalls and thread-local storage. The
  30% is bought, not found.
- Numbers are higher here than elsewhere in this repository because the machine was warm. Only
  the within-session comparison is meaningful, which is why all three were run alternating.
