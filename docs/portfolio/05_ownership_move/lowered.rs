//! 05 — Ownership transfer: `take`, moves, and why nothing is cloned for you.

#[derive(Debug, Clone)]
struct Job {
    id: u32,
    payload: String,
}

/// `take` is the only spelling of a non-`Copy` ownership transfer (§38 rule).
/// The parameter owns the value; the caller's binding is dead afterwards.
fn enqueue(job: Job) -> Vec<Job> {
    Vec::from([job])
}

/// Without `take`, a non-Copy parameter is a shared borrow (rule P3): `&Job`.
fn describe(job: &Job) -> String {
    ::std::format!("job {} carrying {} bytes", job.id, job.payload.len())
}

fn main() {
    let job = Job {
        id: 1,
        payload: String::from("hello"),
    };
    ::std::println!("{}", describe(&job));
    let queue = enqueue(job);
    ::std::println!("{}", queue.len());

    let twin = Job {
        id: 2,
        payload: String::from("again"),
    };
    let copy_of_twin = twin.clone();
    let _ = enqueue(copy_of_twin);
    ::std::println!("{twin:?}");

    let n: u64 = 7;
    consume_number(n);
    ::std::println!("{n} still here");
}

/// `take` on a known-Copy type is the one `take` that copies rather than moves, which is why
/// `uredo lint` reports it as redundant: the signature is the same as `n: u64` (P1).
fn consume_number(n: u64) {
    ::std::println!("got {n}");
}
