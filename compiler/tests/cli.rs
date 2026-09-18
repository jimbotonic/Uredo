//! End-to-end tests of the `uredo` binary: diagnostic translation (§27), `explain` (§26.2),
//! provenance (`rust --map`) and `package` consumed from plain Rust (§4.6, gate item 5).

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn uredo(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_uredo")).args(args).current_dir(root()).output().expect("run uredo");
    let strip = |b: &[u8]| {
        let s = String::from_utf8_lossy(b).to_string();
        // drop ANSI colour codes from rustc's rendered panel
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                while let Some(d) = chars.next() {
                    if d == 'm' {
                        break;
                    }
                }
                continue;
            }
            out.push(c);
        }
        out
    };
    (out.status.code().unwrap_or(-1), strip(&out.stdout), strip(&out.stderr))
}

/// `fixtures/moved` is driven by three tests, and every `uredo` invocation rewrites that package's
/// `target/uredo/` in place. Run in parallel they clobber each other's lowering, which surfaces as
/// cargo reporting `couldn't read src/main.rs` from a directory another test is mid-way through
/// rewriting. The three share the fixture on purpose — they are about one program — so the lock is
/// the fix rather than three copies of it. It was always a race; it only started losing once there
/// were enough other tests to make the window matter.
static MOVED: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Take the lock, and do not care whether a failing test poisoned it: the poison would turn one
/// real failure into three confusing ones.
fn moved_fixture() -> std::sync::MutexGuard<'static, ()> {
    MOVED.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn use_after_take_is_translated() {
    let _guard = moved_fixture();
    let (code, _, err) = uredo(&["check", "tests/fixtures/moved"]);
    assert_ne!(code, 0);
    assert!(err.contains("error[E0382]: `job` cannot be used here because `enqueue` takes ownership of it"), "{}", err);
    assert!(err.contains("--> src/main.ure:20:11"), "{}", err);
    assert!(err.contains("ownership transferred here"), "{}", err);
    assert!(err.contains("used after transfer"), "{}", err);
    assert!(err.contains("`enqueue` declares: fn enqueue(job: take Job) -> Vec<Job>"), "{}", err);
    assert!(err.contains("write `enqueue(job.clone())`"), "{}", err);
    // the raw rustc diagnostic stays available in the secondary panel
    assert!(err.contains("--- rustc, on generated src/main.rs:"), "{}", err);
}

#[test]
fn immutable_receiver_is_translated() {
    let _guard = moved_fixture();
    let (_, _, err) = uredo(&["check", "tests/fixtures/moved"]);
    assert!(err.contains("error[E0596]: `other` must be a `var` binding: `bump` takes `self: inout`"), "{}", err);
    assert!(err.contains("--> src/main.ure:22:5"), "{}", err);
    assert!(err.contains("help: declare it with `var other = …`"), "{}", err);
}

#[test]
fn type_mismatch_keeps_rustc_text_and_explains_the_borrow() {
    let (code, _, err) = uredo(&["check", "tests/fixtures/mismatch"]);
    assert_ne!(code, 0);
    assert!(err.contains("error[E0308]: mismatched types"), "{}", err);
    assert!(err.contains("--> src/main.ure:11:14"), "{}", err);
    assert!(err.contains("Uredo elaborated `describe(job)` to `describe(&job)`"), "{}", err);
}

#[test]
fn provenance_map_follows_rustfmt() {
    let (code, out, _) = uredo(&["rust", "--map", "../docs/portfolio/06_borrowing/snippet.ure"]);
    assert_eq!(code, 0);
    // `deposit(&mut alice, 5.0);` comes from Uredo line 54, whatever rustfmt did to the file
    let line = out.lines().find(|l| l.contains("deposit(&mut alice, 5.0);")).expect("generated call");
    let cols: Vec<&str> = line.split('|').next().unwrap().split_whitespace().collect();
    assert_eq!(cols[1], "54", "{}", line);
    // doc comment lines carry no mapping
    assert!(out.lines().any(|l| l.contains("   - | /// ")), "{}", out);
}

#[test]
fn explain_reports_the_rule() {
    let (code, out, _) = uredo(&["explain", "../docs/portfolio/06_borrowing/snippet.ure:54"]);
    assert_eq!(code, 0, "{}", out);
    assert!(out.contains("Expression:          deposit(alice, 5.0)"), "{}", out);
    assert!(out.contains("Uredo elaboration:   deposit(&mut alice, 5.0)"), "{}", out);
    assert!(out.contains("P4 `inout` mutable borrow inserted"), "{}", out);
    assert!(out.contains("callee declares: fn deposit(acct: inout Account, amount: f64)"), "{}", out);
    assert!(out.contains("Inserted by Uredo:   borrow: mutable ×1 · clone: none · allocation: none"), "{}", out);
    assert!(out.contains("Rust adjustments:    unknown (no semantic engine in v0.x)"), "{}", out);
    assert!(out.contains("Generated Rust ("), "{}", out);
}

#[test]
fn explain_on_a_plain_line() {
    // line 49 of topic 06: `p = Point { x: 3.0, y: 4.0 }` — nothing elaborated
    let (code, out, _) = uredo(&["explain", "../docs/portfolio/06_borrowing/snippet.ure:49"]);
    assert_eq!(code, 0, "{}", out);
    assert!(out.contains("Uredo elaboration:   none (emitted as written)"), "{}", out);
}

#[test]
fn package_is_consumed_from_plain_rust() {
    let (code, out, err) = uredo(&["package", "../examples/geometry"]);
    assert_eq!(code, 0, "{}{}", out, err);
    let pkg = root().join("../examples/geometry/target/package/geometry");
    assert!(pkg.join("src/shapes.rs").exists());
    let manifest = std::fs::read_to_string(pkg.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("rust-version"), "{}", manifest);
    assert!(!manifest.contains("[workspace]"), "{}", manifest);
    assert!(!pkg.join("provenance").exists(), "an exported package carries no compiler artefacts");
    // a plain Cargo project depends on it by path; no `uredo` involved
    let run = Command::new("cargo").args(["run", "-q"]).current_dir(root().join("tests/fixtures/consumer")).output().expect("cargo");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(stdout.trim(), "24 25 unit: area 3.14");
}

#[test]
fn api_manifest_fails_on_unreviewed_signature_change() {
    // work on a private copy of the geometry example
    let tmp = std::env::temp_dir().join(format!("uredo-api-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    let src = root().join("../examples/geometry");
    for f in ["Cargo.toml", "src/lib.ure", "src/shapes.ure"] {
        std::fs::copy(src.join(f), tmp.join(f)).unwrap();
    }
    let dir = tmp.to_string_lossy().to_string();
    // first run creates the baseline
    let (code, out, _) = uredo(&["check", "--api", &dir]);
    assert_eq!(code, 0, "{}", out);
    assert!(out.contains("API manifest created"), "{}", out);
    assert!(tmp.join("uredo-api.json").exists());
    // a body edit changes nothing in the public API (§4.4)
    let shapes = tmp.join("src/shapes.ure");
    let text = std::fs::read_to_string(&shapes).unwrap();
    std::fs::write(&shapes, text.replace("std::f64::consts::PI * r * r", "3.0 * r * r")).unwrap();
    let (code, out, _) = uredo(&["check", "--api", &dir]);
    assert_eq!(code, 0, "{}", out);
    assert!(out.contains("public API unchanged"), "{}", out);
    // a declaration edit is a signature change: fails until reviewed
    let text = std::fs::read_to_string(&shapes).unwrap();
    std::fs::write(&shapes, text.replace("pub fn describe(c: Circle) -> String:", "pub fn describe(c: take Circle) -> String:")).unwrap();
    let (code, out, err) = uredo(&["check", "--api", &dir]);
    assert_ne!(code, 0, "{}{}", out, err);
    assert!(err.contains("~ changed  shapes::describe (fn)"), "{}", err);
    assert!(err.contains("was: fn describe(c: &Circle) -> String"), "{}", err);
    assert!(err.contains("now: fn describe(c: Circle) -> String"), "{}", err);
    assert!(err.contains("unreviewed public API change"), "{}", err);
    // reviewed: accept the new manifest, then it is quiet again
    let (code, out, _) = uredo(&["check", "--api", "--update-api", &dir]);
    assert_eq!(code, 0, "{}", out);
    assert!(out.contains("API manifest updated"), "{}", out);
    let (code, out, _) = uredo(&["check", "--api", &dir]);
    assert_eq!(code, 0, "{}", out);
    // removing a public item is reported too
    let text = std::fs::read_to_string(&shapes).unwrap();
    std::fs::write(&shapes, text.replace("    pub fn scale(self: inout, k: f64):\n        self.w *= k\n        self.h *= k\n", "")).unwrap();
    let (code, _, err) = uredo(&["check", "--api", &dir]);
    assert_ne!(code, 0);
    assert!(err.contains("- removed  shapes::Rect::scale (method)"), "{}", err);
    let _ = std::fs::remove_dir_all(&tmp);
}


#[test]
fn move_through_a_generic_parameter_is_translated() {
    let (code, _, err) = uredo(&["check", "tests/fixtures/moved_generic"]);
    assert_ne!(code, 0);
    assert!(err.contains("error[E0382]: `name` cannot be used here because `add_user` takes it by value: its parameter is a type parameter or `impl Trait` (P8)"), "{}", err);
    assert!(err.contains("moved here (by-value generic parameter)"), "{}", err);
    assert!(err.contains("`add_user` declares: fn add_user<N: Into<String>>(name: N) -> String"), "{}", err);
    assert!(err.contains("help: write `add_user(&name)` if a reference satisfies"), "{}", err);
}

#[test]
fn borrowed_iterator_in_a_for_loop_is_translated() {
    let (code, _, err) = uredo(&["check", "tests/fixtures/for_iter"]);
    assert_ne!(code, 0);
    assert!(err.contains("`r` is an iterator, not a collection"), "{}", err);
    assert!(err.contains("borrowed here by the `for` rule (§16)"), "{}", err);
    assert!(err.contains("help: write `for … in take r`"), "{}", err);
}

// ----- the rules that speak Uredo where rustc can only speak Rust (§27) -----
//
// Each of these was a mistake made while writing `examples/restdemo`, and each used to reach the
// programmer as a rustc type error about generated code. The assertions are on the wording,
// because the wording is the feature: a translated diagnostic that does not name the rule is the
// same diagnostic.

#[test]
fn an_optional_tail_is_not_wrapped_in_some() {
    let (code, _, err) = uredo(&["check", "tests/fixtures/tail_some"]);
    assert_ne!(code, 0);
    assert!(err.contains("returns `i32?`, which is `Option<i32>`"), "{}", err);
    assert!(err.contains("does not wrap a tail in `Some`"), "{}", err);
    assert!(err.contains("help: write `Some(…)` around it, or return `None`"), "{}", err);
    assert!(err.contains("wraps it in `Ok` (§15.3)"), "{}", err);
}

#[test]
fn a_throws_tail_that_is_already_a_result_needs_a_question_mark() {
    let (code, _, err) = uredo(&["check", "tests/fixtures/tail_result"]);
    assert_ne!(code, 0);
    assert!(err.contains("this tail is already a `Result`"), "{}", err);
    assert!(err.contains("wraps its tail in `Ok` (§15.3)"), "{}", err);
    assert!(err.contains("help: add `?`"), "{}", err);
}

#[test]
fn returning_a_borrowed_parameter_names_the_passing_mode() {
    let (code, _, err) = uredo(&["check", "tests/fixtures/return_param"]);
    assert_ne!(code, 0);
    assert!(err.contains("`s` is a parameter declared `s: String`, which is a shared borrow (§10.1)"), "{}", err);
    assert!(err.contains("help: declare it `s: take String`"), "{}", err);
}

#[test]
fn a_for_binding_that_is_a_borrow_says_so() {
    let (code, _, err) = uredo(&["check", "tests/fixtures/for_borrow"]);
    assert_ne!(code, 0);
    assert!(err.contains("`for w in words` iterates by shared reference (D12)"), "{}", err);
    assert!(err.contains("help: write `for w in take words`"), "{}", err);
}

#[test]
fn an_optional_of_a_borrow_is_not_an_optional() {
    let (code, _, err) = uredo(&["check", "tests/fixtures/option_borrow"]);
    assert_ne!(code, 0);
    assert!(err.contains("`String?` means `Option<String>`, and this is an `Option<&String>`"), "{}", err);
    assert!(err.contains("the shorthand is for those two types only (§9.3)"), "{}", err);
}

#[test]
fn a_two_parameter_closure_without_parentheses_is_not_an_unknown_name() {
    let (code, _, err) = uredo(&["check", "tests/fixtures/closure_parens"]);
    assert_ne!(code, 0);
    assert!(err.contains("looks like the first parameter of a closure"), "{}", err);
    assert!(err.contains("needs parentheses"), "{}", err);
}

/// The guard, and the reason the two tail rules carry one.
///
/// A mismatch that merely *happens* inside a function returning `T?`, or inside a `throws`
/// function, is not the tail rule. Explaining it as one would be confidently wrong, and would send
/// the reader to a line that is not the problem — worse than rustc's honest "mismatched types".
#[test]
fn the_tail_rules_do_not_fire_away_from_the_tail() {
    let dir = std::env::temp_dir().join(format!("uredo-nontail-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"nontail\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n").unwrap();
    // the mismatch is an argument in the middle of the body; the tail below it is correct
    std::fs::write(
        dir.join("src/main.ure"),
        "fn want(o: i32?) -> bool:\n    o.is_some()\n\nfn pick(x: i32) -> i32?:\n    _b = want(x)\n    Some(x)\n\nfn main():\n    print(\"{:?}\", pick(1))\n",
    )
    .unwrap();
    let (code, _, err) = uredo(&["check", dir.to_str().unwrap()]);
    assert_ne!(code, 0);
    assert!(err.contains("error[E0308]: mismatched types"), "rustc's own wording should survive: {}", err);
    assert!(!err.contains("does not wrap a tail in `Some`"), "the tail rule fired away from the tail: {}", err);
    let _ = std::fs::remove_dir_all(&dir);
}

// ----- `uredo report` (§5.4) -----

/// The bundle has to say which of §5.4's two kinds of failure this is, and be wrong in neither
/// direction: the program's own ownership error is not a compiler bug, and neither is a mistake in
/// the reporter's hand-written Rust.
#[test]
fn a_bug_bundle_says_which_kind_of_failure_it_found() {
    let _guard = moved_fixture();
    let out_dir = std::env::temp_dir().join(format!("uredo-report-{}", std::process::id()));

    // a barrier diagnostic: use after `take`, which the program itself caused
    let _ = std::fs::remove_dir_all(&out_dir);
    let (code, out, err) = uredo(&["report", "tests/fixtures/moved", "--out", out_dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{}{}", out, err);
    let report = std::fs::read_to_string(out_dir.join("REPORT.md")).expect("REPORT.md");
    assert!(report.contains("probably a barrier diagnostic"), "{}", report);
    assert!(report.contains("**This bundle contains your source code**"), "the warning must be first: {}", report);
    for f in ["src/main.ure", "generated/main.rs", "output.txt", "Cargo.toml"] {
        assert!(out_dir.join(f).exists(), "{} missing from the bundle", f);
    }
    assert!(std::fs::read_dir(out_dir.join("provenance")).unwrap().count() > 0, "no provenance in the bundle");
    // the versions someone would ask for
    assert!(report.contains("| rustc |") && report.contains("| cargo |") && report.contains("| target |"), "{}", report);

    // an error in the reporter's own Rust module: not a lowering defect
    let _ = std::fs::remove_dir_all(&out_dir);
    let (code, out, err) = uredo(&["report", "tests/fixtures/report_handwritten", "--out", out_dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{}{}", out, err);
    let report = std::fs::read_to_string(out_dir.join("REPORT.md")).expect("REPORT.md");
    assert!(report.contains("not a lowering defect"), "{}", report);
    assert!(report.contains("hand-written Rust"), "{}", report);

    // a package that compiles: nothing to report
    let _ = std::fs::remove_dir_all(&out_dir);
    let (code, _, err) = uredo(&["report", "../examples/hello", "--out", out_dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{}", err);
    let report = std::fs::read_to_string(out_dir.join("REPORT.md")).expect("REPORT.md");
    assert!(report.contains("not reproduced"), "{}", report);
    let _ = std::fs::remove_dir_all(&out_dir);
}

/// `uredo publish` (§4.6): the export and `cargo publish` in one command, with the check Uredo owes
/// before an irreversible step. A published crate's public API is a promise (§4.4, D14), so an
/// unreviewed change stops the command and an unrecorded API stops it sooner.
#[test]
fn publish_refuses_an_unreviewed_public_api_and_then_verifies_the_export() {
    let work = std::env::temp_dir().join(format!("uredo-publish-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let dir = work.to_string_lossy().to_string();

    let (code, out, err) = uredo(&["new", &dir, "--lib", "--name", "publishprobe"]);
    assert_eq!(code, 0, "{}{}", out, err);

    // nothing recorded yet: both pre-flight problems are reported together, since publishing is not
    // a step anyone wants to attempt twice to learn two things
    let (code, _, err) = uredo(&["publish", &dir, "--dry-run"]);
    assert_eq!(code, 1, "{}", err);
    assert!(err.contains("no `uredo-api.json`"), "{}", err);
    assert!(err.contains("no `license`"), "{}", err);
    assert!(err.contains("2 thing(s) to settle"), "{}", err);

    // `uredo new` does not choose a licence — that is the author's — so the fixture supplies one
    let manifest = work.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap().replace("[package]\n", "[package]\nlicense = \"MIT OR Apache-2.0\"\n");
    std::fs::write(&manifest, text).unwrap();

    let (code, out, err) = uredo(&["check", "--api", &dir]);
    assert_eq!(code, 0, "{}{}", out, err);

    // an API change is refused, and named
    std::fs::write(work.join("src/lib.ure"), std::fs::read_to_string(work.join("src/lib.ure")).unwrap() + "\npub fn added() -> u8:\n    7\n").unwrap();
    let (code, _, err) = uredo(&["publish", &dir, "--dry-run"]);
    assert_eq!(code, 1, "{}", err);
    assert!(err.contains("added"), "the diff should name the item:\n{}", err);
    assert!(err.contains("unreviewed public API change"), "{}", err);

    // reviewed, it goes through, and Cargo verifies the export by building it
    let (code, out, err) = uredo(&["check", "--api", "--update-api", &dir]);
    assert_eq!(code, 0, "{}{}", out, err);
    let (code, out, err) = uredo(&["publish", &dir, "--dry-run"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(out.contains("public API unchanged"), "{}", out);
    assert!(err.contains("aborting upload due to dry run"), "{}{}", out, err);

    // what Cargo was handed is plain Rust: no `.ure`, no provenance, and a note saying so
    let pkg = work.join("target/package/publishprobe");
    assert!(pkg.join("src/lib.rs").exists(), "the export has generated Rust");
    assert!(!pkg.join("src/lib.ure").exists(), "the export carries no Uredo source");
    assert!(!pkg.join("provenance").exists(), "the export carries no compiler artefacts");
    assert!(std::fs::read_to_string(pkg.join("UREDO-GENERATED.txt")).unwrap().contains("no Uredo is required"));

    let _ = std::fs::remove_dir_all(&work);
}

/// A crate with no licence is rejected by crates.io, and `cargo publish --dry-run` only warns — so
/// the failure would arrive after the upload had begun. `uredo publish` catches it first.
#[test]
fn publish_refuses_a_package_with_no_licence() {
    let work = std::env::temp_dir().join(format!("uredo-licence-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let dir = work.to_string_lossy().to_string();
    let (code, out, err) = uredo(&["new", &dir, "--lib", "--name", "licenceprobe"]);
    assert_eq!(code, 0, "{}{}", out, err);

    let (code, _, err) = uredo(&["publish", &dir, "--dry-run"]);
    assert_eq!(code, 1, "{}", err);
    assert!(err.contains("no `license` or `license-file`"), "{}", err);
    assert!(err.contains("MIT OR Apache-2.0"), "the note should name the convention:\n{}", err);

    // with one, the licence is settled and only the API remains
    let manifest = work.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap().replace("[package]\n", "[package]\nlicense = \"MIT OR Apache-2.0\"\n");
    std::fs::write(&manifest, text).unwrap();
    let (code, _, err) = uredo(&["publish", &dir, "--dry-run"]);
    assert_eq!(code, 1, "the API is still unrecorded:\n{}", err);
    assert!(!err.contains("no `license`"), "the licence is settled and should not be reported again:\n{}", err);
    assert!(err.contains("1 thing(s) to settle"), "{}", err);
    let (code, out, err) = uredo(&["check", "--api", &dir]);
    assert_eq!(code, 0, "{}{}", out, err);
    let (code, out, err) = uredo(&["publish", &dir, "--dry-run"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(err.contains("aborting upload due to dry run"), "{}{}", out, err);

    let _ = std::fs::remove_dir_all(&work);
}
