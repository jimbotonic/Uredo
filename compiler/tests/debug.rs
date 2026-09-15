//! The GDB helpers (D29, §28.2), run against a real program under a real debugger.
//!
//! Skipped when `gdb` is not installed, and the skip is loud: a debugger helper nobody ran is the
//! thing this test exists to prevent.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(program: &str) -> bool {
    Command::new(program).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Builds a corpus program with the compiler under test and runs a scripted GDB session on it.
fn gdb_session(program: &str, commands: &[&str]) -> String {
    let package = root().join("..").join("corpus").join(program);
    let built = Command::new(env!("CARGO_BIN_EXE_uredo"))
        .args(["build"])
        .arg(&package)
        .output()
        .expect("uredo build");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));

    let binary = package.join("target/debug").join(format!("c{}", program));
    assert!(binary.exists(), "no binary at {}", binary.display());

    let mut gdb = Command::new("gdb");
    gdb.arg("-batch").arg("-q").arg("-ex").arg(format!("source {}", root().join("debug/uredo_gdb.py").display()));
    for c in commands {
        gdb.arg("-ex").arg(c);
    }
    let out = gdb.arg(&binary).current_dir(&package).output().expect("gdb");
    String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr)
}

#[test]
fn a_breakpoint_set_in_uredo_terms_lands_on_the_right_line() {
    if !have("gdb") {
        eprintln!("SKIPPED: gdb is not installed, so the debugger helpers were not exercised");
        return;
    }
    // corpus 05: `match self:` at line 11, its five arms at 12..16
    let out = gdb_session("05_enum_match", &["ubreak src/main.ure:12", "run", "uwhere", "ulist 1"]);

    // the command says which generated line it chose, and GDB agrees it is a breakpoint
    assert!(out.contains("src/main.ure:12 is main.rs:"), "{}", out);
    assert!(out.contains("Breakpoint 1"), "{}", out);
    // it stopped on the arm that line names, not on the header or the last arm
    assert!(out.contains("Command::Move { dx, dy } if *dx == 0 && *dy == 0 =>"), "{}", out);

    // the backtrace reads in Uredo locations, with the source beside each frame
    assert!(out.contains("src/main.ure:12    Move { dx, dy } if *dx == 0 && *dy == 0"), "{}", out);
    assert!(out.contains("src/main.ure:34    print(\"{}\", cmd.describe())"), "{}", out);

    // and the listing marks the line the frame is on
    assert!(out.contains("=>  12"), "{}", out);
    assert!(out.contains("   11          match self:"), "{}", out);
}

/// The LLDB helpers, which say the same three things through a different debugger's API. Written
/// before any LLDB was available and verified afterwards: it worked unchanged, and the only thing it
/// needed was trimming the Rust runtime frames LLDB walks and GDB stops before.
#[test]
fn the_lldb_helpers_say_the_same_things() {
    if !have("lldb") {
        eprintln!("SKIPPED: lldb is not installed, so its helpers were not exercised");
        return;
    }
    let package = root().join("..").join("corpus").join("05_enum_match");
    let built = Command::new(env!("CARGO_BIN_EXE_uredo")).arg("build").arg(&package).output().expect("uredo build");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));

    let script = root().join("debug/uredo_lldb.py");
    let out = Command::new("lldb")
        .arg("--batch")
        .args(["-o", &format!("command script import {}", script.display())])
        .args(["-o", "ubreak src/main.ure:12"])
        .args(["-o", "run", "-o", "uwhere", "-o", "ulist 1", "-o", "quit"])
        .arg(package.join("target/debug/c05_enum_match"))
        .current_dir(&package)
        .output()
        .expect("lldb");
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);

    assert!(text.contains("src/main.ure:12 is main.rs:"), "{}", text);
    assert!(text.contains("src/main.ure:12    Move { dx, dy } if *dx == 0 && *dy == 0"), "{}", text);
    assert!(text.contains("src/main.ure:34    print(\"{}\", cmd.describe())"), "{}", text);
    assert!(text.contains("more frame(s) with no Uredo source"), "the runtime frames should be trimmed and counted:\n{}", text);
    assert!(text.contains("=>  12"), "{}", text);
}

#[test]
fn the_map_attributes_a_block_to_its_own_header() {
    // Not a debugger test but the reason the one above works: a multi-line construct is built as
    // one string, and before D29's helpers were written every line of a `match` was attributed to
    // its *last* arm and every closing brace to the last statement inside it.
    let src = "fn describe(n: u8) -> &'static str:\n    match n:\n        0: \"zero\"\n        1: \"one\"\n        _: \"many\"\n\nfn main():\n    for i in 0..2:\n        if i > 0:\n            print(\"{}\", describe(i))\n";
    let out = uredo::compile(src, true);
    assert!(!out.has_errors(), "{:?}", out.diags);
    let at = |needle: &str| -> usize {
        let i = out.rust.lines().position(|l| l.trim().starts_with(needle)).unwrap_or_else(|| panic!("{} not in\n{}", needle, out.rust));
        out.map.lines.get(i).copied().unwrap_or(0)
    };
    assert_eq!(at("match n"), 2, "the match header is its own line");
    assert_eq!(at("0 =>"), 3);
    assert_eq!(at("1 =>"), 4);
    assert_eq!(at("_ =>"), 5);
    assert_eq!(at("for i in"), 8);
    assert_eq!(at("if i > 0"), 9);
}
