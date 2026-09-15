//! What Uredo says when the toolchain around it is incomplete.
//!
//! These exist because the first command a new user runs — `uredo new hello && uredo run hello`,
//! which the tool itself suggests — reported a missing `rustfmt` as "lowering defect … this is a
//! compiler bug; report it". §5.4 reserves that wording for Rust that Uredo generated and rustfmt
//! refused. A rustfmt that is not installed is a fact about the user's machine, and `rustup`
//! ships it in the `default` profile but not in `minimal`, which rustup recommends for CI — so an
//! unattended `cargo install uredo` meets this before it meets anything else.

use std::path::PathBuf;
use std::process::Command;

/// A PATH with every directory that holds a `rustfmt` removed, and nothing else changed.
fn path_without_rustfmt() -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    let kept: Vec<&str> = std::env::split_paths(&path)
        .filter(|d| !d.join("rustfmt").exists() && !d.join("rustfmt.exe").exists())
        .filter_map(|d| d.to_str().map(|s| Box::leak(s.to_string().into_boxed_str()) as &str))
        .collect();
    Some(kept.join(":"))
}

fn write_program(dir: &PathBuf) -> PathBuf {
    std::fs::create_dir_all(dir).expect("scratch dir");
    let file = dir.join("main.ure");
    std::fs::write(&file, "fn main():\n    print(\"hello\")\n").expect("write source");
    file
}

#[test]
fn a_missing_rustfmt_is_not_reported_as_a_compiler_bug() {
    let Some(path) = path_without_rustfmt() else {
        eprintln!("skipping: no PATH to strip");
        return;
    };
    // A positive control (§5.3): the check is worthless unless rustfmt is really gone from it.
    assert!(
        !std::env::split_paths(&path).any(|d| d.join("rustfmt").exists()),
        "the stripped PATH still has a rustfmt on it, so this test proves nothing"
    );

    let dir = std::env::temp_dir().join("uredo_toolchain_test");
    let file = write_program(&dir);
    let out = Command::new(env!("CARGO_BIN_EXE_uredo"))
        .args(["rust", file.to_str().unwrap()])
        .env("PATH", &path)
        .output()
        .expect("run uredo");
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        err.contains("rustfmt is not installed"),
        "a missing rustfmt should say so plainly; got:\n{}",
        err
    );
    assert!(
        err.contains("rustup component add rustfmt"),
        "the diagnostic should say how to fix it; got:\n{}",
        err
    );
    assert!(
        !err.contains("compiler bug"),
        "a missing rustfmt is the user's toolchain, not a bug in Uredo; got:\n{}",
        err
    );
    assert!(
        !err.contains("did not parse"),
        "nothing was parsed, so the diagnostic must not claim a parse failure; got:\n{}",
        err
    );
}

#[test]
fn the_same_program_lowers_when_rustfmt_is_present() {
    // The other half of the control: with a normal PATH this program is fine, so the failure
    // above is the missing tool and nothing else.
    let dir = std::env::temp_dir().join("uredo_toolchain_test_ok");
    let file = write_program(&dir);
    let out = Command::new(env!("CARGO_BIN_EXE_uredo"))
        .args(["rust", file.to_str().unwrap()])
        .output()
        .expect("run uredo");
    let _ = std::fs::remove_dir_all(&dir);
    if which_rustfmt() {
        assert!(out.status.success(), "stderr:\n{}", String::from_utf8_lossy(&out.stderr));
        assert!(String::from_utf8_lossy(&out.stdout).contains("fn main()"));
    }
}

fn which_rustfmt() -> bool {
    std::env::var("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join("rustfmt").exists()))
        .unwrap_or(false)
}
