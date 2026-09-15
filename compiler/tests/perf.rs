//! §36 performance obligations that are measurable as fixtures: the tail-`?` codegen row and the
//! allocation row. Both run the real toolchain; neither can pass by being skipped.
//!
//! The full sweep over the twenty §43 programs is `corpus/perf.py`, whose committed result is
//! `corpus/PERF.md`; this file runs one program of it, so the harness cannot rot.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());
fn serial() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(cmd: &mut Command) -> (bool, String, String) {
    let out = cmd.output().expect("spawn");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
}

/// `(instructions, alloca bytes, opcode digest)` for each function of an IR file.
fn ir_stats(ll: &Path) -> HashMap<String, (usize, usize, String)> {
    let tool = root().join("../docs/corpus-study/roundtrip/tools/irstat.py");
    let (ok, out, err) = run(Command::new("python3").arg(&tool).arg(ll));
    assert!(ok, "irstat failed: {}{}", out, err);
    let mut m = HashMap::new();
    for line in out.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() == 4 {
            m.insert(f[0].to_string(), (f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].to_string()));
        }
    }
    assert!(!m.is_empty(), "no functions in {}:\n{}", ll.display(), out);
    m
}

fn ir_of(manifest: &Path, target_dir: &Path) -> PathBuf {
    let (ok, out, err) = run(Command::new("cargo")
        .args(["rustc", "-q", "--release", "--lib", "--manifest-path"])
        .arg(manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .args(["--", "--emit=llvm-ir", "-C", "codegen-units=1", "-C", "debuginfo=0"]));
    assert!(ok, "{}{}", out, err);
    std::fs::read_dir(target_dir.join("release/deps"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name().unwrap().to_string_lossy().starts_with("tailfix-")
                && p.extension().map(|e| e == "ll").unwrap_or(false)
        })
        .expect("IR file")
}

/// §36, the `throws` tail fixture: with the same error type the tail `?` (§15.3, D37) must be the
/// hand-written direct return; with a converting error type it must not exceed the `Ok(e?)` form.
#[test]
fn tail_question_mark_costs_nothing_over_the_direct_return() {
    let _guard = serial();
    let perf = root().join("../examples/perf");
    let (ok, out, err) = run(Command::new(env!("CARGO_BIN_EXE_uredo")).args(["check", "../examples/perf/tail_ure"]).current_dir(root()));
    assert!(ok, "{}{}", out, err);
    assert!(!out.contains("warning") && !err.contains("warning"), "the fixture must build clean:\n{}{}", out, err);

    let ure = ir_stats(&ir_of(&perf.join("tail_ure/target/uredo/Cargo.toml"), &perf.join("tail_ure/target")));
    let rs = ir_stats(&ir_of(&perf.join("tail_rust/Cargo.toml"), &perf.join("tail_rust/target")));

    for f in ["inner", "archive", "convert"] {
        assert!(ure.contains_key(f) && rs.contains_key(f), "`{}` missing: {:?} / {:?}", f, ure.keys(), rs.keys());
    }
    // the opaque callee must be identical, or the comparison is measuring something else
    assert_eq!(ure["inner"].2, rs["inner"].2, "`inner` differs: {:?} vs {:?}", ure["inner"], rs["inner"]);

    // same error type: identical code (an alias would also satisfy §15.3; this is the stronger form)
    assert_eq!(
        ure["archive"].2, rs["archive"].2,
        "the same-type tail `?` is not the direct return: {:?} vs {:?}",
        ure["archive"], rs["archive"]
    );
    assert_eq!(ure["archive"].1, 0, "the same-type tail `?` must leave no stack temporary: {:?}", ure["archive"]);

    // converting error type: no more instructions than `Ok(e?)`, the form D37 replaced
    assert!(
        ure["convert"].0 <= rs["convert"].0,
        "the converting tail `?` is worse than `Ok(e?)`: {} vs {} instructions",
        ure["convert"].0,
        rs["convert"].0
    );
    assert!(ure["convert"].1 <= rs["convert"].1, "alloca bytes: {} vs {}", ure["convert"].1, rs["convert"].1);
}

/// §36, the allocation, memory and size rows on one program; `corpus/perf.py` runs all twenty.
#[test]
fn generated_rust_does_not_allocate_more_or_exceed_its_size_baseline() {
    let _guard = serial();
    let (ok, out, err) = run(Command::new("python3")
        .arg(root().join("../corpus/perf.py"))
        .arg("01_hello")
        .current_dir(root().join("..")));
    assert!(ok, "the §36 comparison failed:\n{}{}", out, err);
    assert!(out.contains("1/1 pass"), "{}", out);
}
