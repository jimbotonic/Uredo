//! Rules that only show themselves across a module boundary.
//!
//! A single-file lowering cannot see the crate root, so a defect in how a crate-wide declaration
//! reaches a child module is invisible to every check that lowers one file at a time. This is
//! why `@!default_error` was rejected in every module but the root for as long as it existed,
//! while §15.1's own example says the opposite: "crate root; a module may re-declare for its
//! subtree". Found by writing `examples/restdemo`, 2026-09-15.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

/// The two fixture checks share `tests/fixtures/default_error/target`; they must not lower or
/// build it concurrently. `gate.rs` carries the same guard for the same reason.
static LOCK: Mutex<()> = Mutex::new(());
fn serial() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/default_error")
}

fn uredo(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_uredo")).args(args).output().expect("run uredo");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

#[test]
fn the_crate_roots_default_error_reaches_a_child_module() {
    let _guard = serial();
    let dir = fixture();
    let (ok, text) = uredo(&["check", dir.to_str().unwrap()]);
    assert!(ok, "the fixture must build:\n{}", text);
    assert!(
        !text.contains("bare `throws` needs"),
        "a child module was refused the crate root's default:\n{}",
        text
    );

    // and the generated Rust says which error each module ended up with
    let generated = dir.join("target/uredo/src/child.rs");
    let child = std::fs::read_to_string(&generated).expect("the child module was generated");
    assert!(
        child.contains("crate::RootError"),
        "the child should inherit the root's error type, got:\n{}",
        child
    );
}

#[test]
fn a_module_may_re_declare_for_its_own_subtree() {
    let _guard = serial();
    let dir = fixture();
    let (ok, text) = uredo(&["check", dir.to_str().unwrap()]);
    assert!(ok, "the fixture must build:\n{}", text);
    let generated = dir.join("target/uredo/src/grandchild.rs");
    let own = std::fs::read_to_string(&generated).expect("the module was generated");
    assert!(
        own.contains("crate::grandchild::OwnError"),
        "a module's own declaration must override the root's, got:\n{}",
        own
    );
    assert!(
        !own.contains("RootError"),
        "the root's error leaked past a re-declaration:\n{}",
        own
    );
}

#[test]
fn a_binary_does_not_inherit_the_librarys_default() {
    // A package's lib and bin share one index but are two crates. `crate::` in the binary names
    // the binary, so the library's default must not reach it — found when the demo's `main.ure`
    // suddenly resolved `crate::error::Error` against a crate that has no `error` module.
    let dir = std::env::temp_dir().join("uredo_two_roots_fixture");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("scratch");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"two_roots\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/lib.ure"),
        "@!default_error(crate::LibError)\n\n@derive(Debug)\npub struct LibError\n",
    )
    .unwrap();
    // the binary declares none, so a bare `throws` here must be refused rather than inherited
    std::fs::write(dir.join("src/main.ure"), "fn helper() throws:\n    ()\n\nfn main():\n    ()\n").unwrap();
    let (ok, text) = uredo(&["check", dir.to_str().unwrap()]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!ok, "the binary inherited the library's default:\n{}", text);
    assert!(text.contains("bare `throws` needs"), "wrong diagnostic:\n{}", text);
}

#[test]
fn a_bare_throws_with_no_default_anywhere_is_still_an_error() {
    // The positive control (§5.3): the check above is worthless if a bare `throws` were simply
    // accepted everywhere now.
    let dir = std::env::temp_dir().join("uredo_no_default_fixture");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("scratch");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"no_default\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/lib.ure"), "pub mod child\n").unwrap();
    std::fs::write(dir.join("src/child.ure"), "fn f(x: str) -> u32 throws:\n    x.parse::<u32>()?\n").unwrap();
    let (ok, text) = uredo(&["check", dir.to_str().unwrap()]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!ok, "a bare `throws` with no default declared anywhere must fail");
    assert!(text.contains("bare `throws` needs"), "wrong diagnostic:\n{}", text);
}
