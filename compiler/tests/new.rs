//! `uredo new` and `uredo init` (§28.1): what a package starts as, and what the two commands refuse.

use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("uredo-new-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn uredo_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_uredo")).args(args).current_dir(dir).output().expect("run uredo");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
}

#[test]
fn new_creates_a_binary_package_that_runs() {
    let dir = scratch("bin");
    let (code, out, err) = uredo_in(&dir, &["new", "hello"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(out.contains("created binary package `hello`"), "{}", out);

    let manifest = std::fs::read_to_string(dir.join("hello/Cargo.toml")).expect("Cargo.toml");
    for line in ["name = \"hello\"", "edition = \"2024\"", "rust-version = \"1.85\"", "[dependencies]"] {
        assert!(manifest.contains(line), "manifest missing {:?}:\n{}", line, manifest);
    }
    assert!(dir.join("hello/src/main.ure").exists());
    assert_eq!(std::fs::read_to_string(dir.join("hello/.gitignore")).unwrap().trim(), "target");

    let (code, out, err) = uredo_in(&dir.join("hello"), &["run"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert_eq!(out.trim(), "hello, world");

    // the template must satisfy the toolchain's own checks
    let (code, out, _) = uredo_in(&dir.join("hello"), &["fmt", "--check", "src"]);
    assert_eq!(code, 0, "the template is not canonically formatted: {}", out);
    let (code, out, _) = uredo_in(&dir.join("hello"), &["lint", "src"]);
    assert_eq!(code, 0);
    assert!(out.contains("no findings"), "the template trips a lint: {}", out);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_lib_creates_a_library_whose_test_passes() {
    let dir = scratch("lib");
    let (code, out, err) = uredo_in(&dir, &["new", "mathlib", "--lib"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(out.contains("created library package `mathlib`"), "{}", out);
    assert!(dir.join("mathlib/src/lib.ure").exists());
    let (code, out, err) = uredo_in(&dir.join("mathlib"), &["test"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(out.contains("test add_works ... ok"), "{}{}", out, err);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn init_uses_the_directory_it_is_in() {
    let dir = scratch("init");
    let here = dir.join("my-app");
    std::fs::create_dir_all(&here).unwrap();
    let (code, out, err) = uredo_in(&here, &["init"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(std::fs::read_to_string(here.join("Cargo.toml")).unwrap().contains("name = \"my-app\""), "{}", out);
    let (code, out, err) = uredo_in(&here, &["run"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert_eq!(out.trim(), "hello, world");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn both_commands_refuse_rather_than_overwrite() {
    let dir = scratch("refuse");
    assert_eq!(uredo_in(&dir, &["new", "hello"]).0, 0);

    let (code, _, err) = uredo_in(&dir, &["new", "hello"]);
    assert_eq!(code, 1);
    assert!(err.contains("already exists") && err.contains("uredo init"), "{}", err);

    let (code, _, err) = uredo_in(&dir.join("hello"), &["init"]);
    assert_eq!(code, 1);
    assert!(err.contains("already has a Cargo.toml"), "{}", err);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_directory_name_that_cannot_be_a_package_name_asks_for_one() {
    let dir = scratch("name");
    let (code, _, err) = uredo_in(&dir, &["new", "9lives"]);
    assert_eq!(code, 2, "{}", err);
    assert!(err.contains("--name"), "{}", err);

    let (code, out, err) = uredo_in(&dir, &["new", "9lives", "--name", "nine_lives"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(std::fs::read_to_string(dir.join("9lives/Cargo.toml")).unwrap().contains("name = \"nine_lives\""));
    let _ = std::fs::remove_dir_all(&dir);
}
