//! Type-position lowering, where it had been skipped.
//!
//! `T?` is `Option<T>` in every type position (§9.3). The impl target was emitted verbatim, so
//! `impl<T: Field> Field for T?` generated Rust that did not parse — found by writing
//! `examples/restdemo`. The check is that the shape lowers *and* that the generated Rust
//! compiles, since a lowering that merely parses is what let this through.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The tests run in parallel, so each call needs its own directory — sharing one keyed on the
/// process id had them reading each other's source.
static NONCE: AtomicUsize = AtomicUsize::new(0);

fn scratch(tag: &str) -> std::path::PathBuf {
    let n = NONCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("uredo_types_{}_{}_{}", tag, std::process::id(), n))
}

fn lower(src: &str) -> (bool, String, String) {
    let dir = scratch("ure");
    std::fs::create_dir_all(&dir).expect("scratch");
    let file = dir.join("t.ure");
    std::fs::write(&file, src).expect("write");
    let out = Command::new(env!("CARGO_BIN_EXE_uredo"))
        .args(["rust", file.to_str().unwrap()])
        .output()
        .expect("run uredo");
    let _ = std::fs::remove_dir_all(&dir);
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn compiles(rust: &str) -> Result<(), String> {
    let dir = scratch("rs");
    std::fs::create_dir_all(&dir).expect("scratch");
    let file = dir.join("t.rs");
    std::fs::write(&file, rust).expect("write");
    let out = Command::new("rustc")
        .args(["--edition", "2024", "--crate-type", "lib", "--emit", "metadata", "--out-dir"])
        .arg(&dir)
        .arg(&file)
        .output()
        .expect("rustc");
    let _ = std::fs::remove_dir_all(&dir);
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).to_string()) }
}

#[test]
fn the_optional_suffix_lowers_in_an_impl_target() {
    let (ok, rust, err) = lower(
        "trait Marker\n\nimpl Marker for String\n\nimpl<T: Marker> Marker for T?\n",
    );
    assert!(ok, "did not lower:\n{}", err);
    assert!(
        rust.contains("impl<T: Marker> Marker for ::core::option::Option<T> {}"),
        "the impl target kept its `?`:\n{}",
        rust
    );
    compiles(&rust).expect("the generated Rust must compile");
}

#[test]
fn the_optional_suffix_lowers_in_a_trait_argument_of_an_impl() {
    let (ok, rust, err) = lower(
        "trait Sink<T>\n\nstruct Bin\n\nimpl Sink<String?> for Bin\n",
    );
    assert!(ok, "did not lower:\n{}", err);
    assert!(
        rust.contains("Sink<::core::option::Option<String>> for Bin"),
        "the trait argument kept its `?`:\n{}",
        rust
    );
    compiles(&rust).expect("the generated Rust must compile");
}

#[test]
fn a_macro_can_be_reached_through_an_absolute_path() {
    // D39 spells macros absolutely in generated code (`::std::println!`), but source could not:
    // the leading-`::` path branch parsed the path and never looked for the `!`.
    let (ok, rust, err) = lower(
        "fn main():\n    text = ::std::format!(\"{}-{}\", 1, 2)\n    len = ::std::format!(\"x\").len()\n    ::std::println!(\"{text} {len}\")\n",
    );
    assert!(ok, "an absolute-path macro did not lower:\n{}", err);
    assert!(rust.contains("::std::format!"), "{}", rust);
    assert!(rust.contains("::std::println!"), "{}", rust);
    compiles(&format!("#[allow(dead_code)] fn wrapper() {{ {} }}", rust.replace("fn main()", "fn inner()")))
        .or_else(|_| compiles(&rust.replace("fn main()", "pub fn run()")))
        .expect("the generated Rust must compile");
}

#[test]
fn an_inherent_impl_on_an_optional_alias_still_lowers() {
    // The plain `impl Type:` form goes through the same header, so it is checked too.
    let (ok, rust, err) = lower("struct Holder:\n    inner: String?\n\nimpl Holder:\n    fn get(self) -> String?:\n        self.inner.clone()\n");
    assert!(ok, "did not lower:\n{}", err);
    assert!(rust.contains("impl Holder {"), "{}", rust);
    compiles(&rust).expect("the generated Rust must compile");
}

// ---- the value half of a signature -------------------------------------------
//
// `throws` says what a function returns when it fails and nothing about what it returns when it
// succeeds. Four of the thirteen authoring errors in `examples/restdemo` were that one split,
// one of them at seven sites in a single function, and Uredo diagnosed none of them: it lowered
// `Ok(1)` to `Result::Ok(Ok(1))` and left rustc to report a type error against the signature
// line. These check the two shapes that are decidable from the declaration alone.

#[test]
fn a_hand_written_ok_in_a_throws_body_is_diagnosed() {
    let (ok, _rust, err) = lower("@derive(Debug)\nstruct E\n\nfn f() -> u32 throws E:\n    Ok(1)\n");
    assert!(!ok, "this should not lower");
    assert!(err.contains("wraps its tail in `Ok` for you"), "{}", err);
    assert!(err.contains("§15.3"), "the diagnostic must name the rule:\n{}", err);
}

#[test]
fn an_err_tail_is_diagnosed_the_same_way() {
    let (ok, _rust, err) = lower("@derive(Debug)\nstruct E\n\nfn f() -> u32 throws E:\n    Err(E)\n");
    assert!(!ok, "this should not lower");
    assert!(err.contains("Ok(Err(…))"), "{}", err);
}

#[test]
fn an_optional_return_whose_tail_builds_the_payload_is_diagnosed() {
    for tail in ["P", "P { }"] {
        let src = format!("struct P:\n    x: u8\n\nfn f() -> P?:\n    {}\n", tail.replace("{ }", "{ x: 1 }"));
        let (ok, _rust, err) = lower(&src);
        assert!(!ok, "`{}` should not lower", tail);
        assert!(err.contains("which is `Option<P>`"), "{}", err);
        assert!(err.contains("write `Some(…)` or `None`"), "{}", err);
    }
}

#[test]
fn the_shapes_that_are_correct_are_left_alone() {
    // A value type that is itself a `Result` really does want `Ok(…)` as its tail.
    let (ok, _r, err) = lower(
        "@derive(Debug)\nstruct E\n\nfn f() -> Result<u32, u8> throws E:\n    Ok(1)\n",
    );
    assert!(ok, "a Result-valued `throws` may write `Ok`:\n{}", err);

    // An optional return whose tail is an expression that yields one.
    let (ok, _r, err) = lower(
        "struct P:\n    x: u8\n\nfn f(v: Vec<P>) -> P?:\n    v.into_iter().next()\n",
    );
    assert!(ok, "an Option-yielding tail must not be flagged:\n{}", err);

    // `None` is a path, and must not be mistaken for the payload.
    let (ok, _r, err) = lower("struct P:\n    x: u8\n\nfn f() -> P?:\n    None\n");
    assert!(ok, "`None` is a correct tail:\n{}", err);

    // A correct `throws` body, which is the commonest shape in the repository.
    let (ok, _r, err) = lower("@derive(Debug)\nstruct E\n\nfn f() -> u32 throws E:\n    1\n");
    assert!(ok, "a plain value tail must not be flagged:\n{}", err);
}
