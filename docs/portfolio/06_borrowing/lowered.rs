//! 06 — Borrowing by default: shared borrows, `inout`, call-site rules, `Copy`.
//!
//! The passing-modes table (§10.2) is decided from the declaration alone:
//!   scalar / known-Copy   -> by value   (P1)
//!   str / [T]             -> &str / &[T] (P2)
//!   any other type        -> &T          (P3, shared borrow)
//!   inout T               -> &mut T      (P4)
//!   take T                -> T           (P5)
//!   &T / &mut T / &'a T   -> as written  (P6)

#[derive(Debug, Default)]
struct Account {
    owner: String,
    balance: f64,
}

/// Shared borrow (P3): read-only access, no `&` written by the caller.
fn report(acct: &Account) -> String {
    ::std::format!("{} has {:.2}", acct.owner, acct.balance)
}

/// Mutable borrow (P4): the caller's binding must be `var`; the call site gets `&mut`.
fn deposit(acct: &mut Account, amount: f64) {
    acct.balance += amount;
}

/// Two borrows in one signature: both shared, both zero-cost.
fn richer(a: &Account, b: &Account) -> String {
    if a.balance >= b.balance {
        a.owner.clone()
    } else {
        b.owner.clone()
    }
}

/// A struct that is known-Copy because it says so (§10.1): passed by value (P1).
#[derive(Clone, Copy, Debug)]
struct Point {
    x: f64,
    y: f64,
}

fn length(p: Point) -> f64 {
    (p.x * p.x + p.y * p.y).sqrt()
}

/// Foreign Copy types from std are in the prelude Copy table (D20): by value too.
fn plus_one_second(d: std::time::Duration) -> std::time::Duration {
    d + std::time::Duration::from_secs(1)
}

/// A Copy type from another crate is declared once per crate with `@copy use` (D20). The generator
/// emits a `Copy` assertion, so a wrong declaration is a diagnostic, never a silent move. Nothing
/// else — not even a type rustc knows is Copy — is passed by value without such a declaration.
use tokio::time::Instant;
fn __uredo_assert_copy<T: Copy>() {}
const _: fn() = __uredo_assert_copy::<Instant>;

fn elapsed_ms(start: Instant) -> u128 {
    start.elapsed().as_millis()
}

fn main() {
    let mut alice = Account {
        owner: String::from("alice"),
        balance: 10.0,
    };
    let bob = Account {
        owner: String::from("bob"),
        ..Account::default()
    };

    ::std::println!("{}", report(&alice));
    deposit(&mut alice, 5.0);
    ::std::println!("{}", richer(&alice, &bob));

    let p = Point { x: 3.0, y: 4.0 };
    ::std::println!("{}", length(p));
    ::std::println!("{p:?}");

    let d = plus_one_second(std::time::Duration::from_secs(1));
    let started = Instant::now();
    ::std::println!("elapsed {} ms", elapsed_ms(started));
    ::std::println!(
        "{:?}",
        started.elapsed() < std::time::Duration::from_secs(1)
    );
    ::std::println!("{d:?}");

    let joined = ["a", "b"].join(&String::from("-"));
    ::std::println!("{}", joined);
}
