//! The Phase 0 architectural acceptance gate (§34), as executable checks. Items 4 (mapped
//! ownership diagnostics) live in `cli.rs` and `diagnostics.rs`; the rest are here. Every
//! check runs the real toolchain: nothing is skipped silently.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

/// The checks share `examples/gate/target`; they must not lower or build it concurrently.
static LOCK: Mutex<()> = Mutex::new(());
fn serial() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn gate_dir() -> PathBuf {
    root().join("../examples/gate")
}

fn run(cmd: &mut Command) -> (bool, String, String) {
    let out = cmd.output().expect("spawn");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
}

fn uredo(args: &[&str]) -> (bool, String, String) {
    run(Command::new(env!("CARGO_BIN_EXE_uredo")).args(args).current_dir(root()))
}

/// Items 1, 2, 3 and 6 (feature-dependent build): the gate binary prints one line per item.
#[test]
fn items_1_2_3_run_and_mean_what_the_source_says() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/gate"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("item1 owned borrowed"), "item 1: verbatim arguments to a generic Rust callee\n{}", out);
    assert!(out.contains("item2 true Job { id: 1, payload: \"payload\" } 0"), "item 2: derive-generated methods\n{}", out);
    assert!(out.contains("item3 7"), "item 3: move closure into thread::spawn\n{}", out);
    assert!(out.contains("item6 hello\n"), "item 6: default feature set\n{}", out);
}

#[test]
fn item_1_explain_says_verbatim() {
    let _guard = serial();
    let lib = gate_dir().join("src/lib.ure");
    let src = std::fs::read_to_string(&lib).unwrap();
    let line = src.lines().position(|l| l.contains("(route::route(n), route::route(&n))")).unwrap() + 1;
    let (ok, out, err) = uredo(&["explain", &format!("../examples/gate/src/lib.ure:{}", line)]);
    assert!(ok, "{}{}", out, err);
    let verbatim = out.matches("arguments passed verbatim (callee is a Rust item").count();
    assert_eq!(verbatim, 2, "both route calls must be reported verbatim\n{}", out);
    assert!(!out.contains("borrow inserted"), "{}", out);
}

#[test]
fn item_6_feature_flag_and_cross_target() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/gate", "--features", "fancy"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("item6 hello (fancy)"), "{}", out);
    // cross-target check: only the target's std rlibs are needed, not a linker
    let target = "x86_64-unknown-linux-musl";
    let installed = run(Command::new("rustup").args(["target", "list", "--installed"])).1;
    assert!(installed.contains(target), "gate item 6 needs the {} target installed (rustup target add {})", target, target);
    let (ok, out, err) = uredo(&["check", "../examples/gate", "--target", target]);
    assert!(ok, "{}{}", out, err);
}

/// Item 5: the mixed `.ure`/`.rs` package exported and consumed from a clean plain-Rust project.
#[test]
fn item_5_mixed_package_consumed_from_plain_rust() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["package", "../examples/gate"]);
    assert!(ok, "{}{}", out, err);
    let pkg = gate_dir().join("target/package/gate");
    assert!(pkg.join("src/route.rs").exists(), "hand-written module copied");
    assert!(pkg.join("src/lib.rs").exists() && pkg.join("src/main.rs").exists());
    let lib = std::fs::read_to_string(pkg.join("src/lib.rs")).unwrap();
    assert!(lib.contains("pub mod route;"), "explicit declaration kept, no duplicate\n{}", lib);
    assert_eq!(lib.matches("mod route;").count(), 1, "{}", lib);
    let (ok, out, err) = run(Command::new("cargo").args(["run", "-q"]).current_dir(root().join("tests/fixtures/gate_consumer")));
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.trim(), "owned borrowed 6");
}

/// Item 7: identical optimised IR for two kernels, Uredo-generated versus hand-written Rust.
#[test]
fn item_7_paired_ir_is_identical() {
    let _guard = serial();
    let kernels = gate_dir().join("kernels");
    // lower the Uredo kernels (target/uredo holds the generated crate)
    let (ok, out, err) = uredo(&["check", "../examples/gate/kernels/uredo"]);
    assert!(ok, "{}{}", out, err);
    let ir_of = |manifest: &Path, target_dir: &Path| -> PathBuf {
        let (ok, out, err) = run(Command::new("cargo").args(["rustc", "-q", "--release", "--lib", "--manifest-path"]).arg(manifest).arg("--target-dir").arg(target_dir).args(["--", "--emit=llvm-ir", "-C", "codegen-units=1", "-C", "debuginfo=0"]));
        assert!(ok, "{}{}", out, err);
        let deps = target_dir.join("release/deps");
        std::fs::read_dir(&deps).unwrap().flatten().map(|e| e.path()).find(|p| p.file_name().unwrap().to_string_lossy().starts_with("kern-") && p.extension().map(|e| e == "ll").unwrap_or(false)).expect("IR file")
    };
    let ure_ll = ir_of(&kernels.join("uredo/target/uredo/Cargo.toml"), &kernels.join("uredo/target"));
    let rs_ll = ir_of(&kernels.join("rust/Cargo.toml"), &kernels.join("rust/target"));
    let ircmp = root().join("../docs/corpus-study/roundtrip/tools/ircmp.py");
    let (ok, out, err) = run(Command::new("python3").arg(&ircmp).arg(&ure_ll).arg(&rs_ll));
    assert!(ok, "IR differs:\n{}{}", out, err);
    let summary = out.lines().last().unwrap_or("");
    assert!(summary.contains("functions=3 same_opcodes=3") || summary.contains("same_opcodes=3"), "{}", out);
    assert!(summary.contains("only_one_side=0") && summary.contains("diff=0"), "{}", out);
}
