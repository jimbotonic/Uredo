//! `uredo lint` (§28.1): the three idiom lints, the cases they must not report, and the CLI.

use std::path::PathBuf;
use std::process::Command;

fn lints(src: &str) -> Vec<String> {
    let out = uredo::compile(src, false);
    assert!(!out.has_errors(), "unexpected errors: {:?}", out.diags);
    out.lints
        .iter()
        .map(|d| format!("{}: {} {}", uredo::lint::name_of(d).unwrap_or("?"), d.msg, d.notes.join(" ")))
        .collect()
}

fn only(src: &str) -> String {
    let l = lints(src);
    assert_eq!(l.len(), 1, "expected one finding, got {:?}", l);
    l.into_iter().next().unwrap()
}

fn none(src: &str) {
    assert!(lints(src).is_empty(), "expected no findings in {:?}, got {:?}", src, lints(src));
}

// ----- redundant_take -----

#[test]
fn take_on_a_known_copy_type_is_redundant() {
    let f = only("fn e(x: take u32) -> u32:\n    x\nfn main():\n    print(\"{}\", e(1))\n");
    assert!(f.starts_with("redundant_take:"), "{}", f);
    assert!(f.contains("copies rather than moves"), "{}", f);
}

#[test]
fn take_on_a_type_parameter_is_redundant() {
    let f = only("fn f<T: Display>(x: take T) -> String:\n    x.to_string()\nfn main():\n    print(f(1))\n");
    assert!(f.starts_with("redundant_take:"), "{}", f);
    assert!(f.contains("P8"), "{}", f);
}

#[test]
fn take_on_an_optional_with_a_known_copy_payload_is_redundant() {
    // D54: `take u32?` and `u32?` are the same mode
    let f = only("fn g(x: take u32?) -> bool:\n    x.is_some()\nfn main():\n    print(\"{}\", g(None))\n");
    assert!(f.starts_with("redundant_take:"), "{}", f);
}

#[test]
fn take_that_changes_the_mode_is_not_reported() {
    none("fn h(s: take String) -> usize:\n    s.len()\nfn main():\n    print(\"{}\", h(String::from(\"x\")))\n");
    none("fn i(o: take String?) -> bool:\n    o.is_some()\nfn main():\n    print(\"{}\", i(None))\n");
}

// ----- borrowed_container -----

#[test]
fn an_owned_container_parameter_suggests_the_slice_type() {
    let f = only("fn a(names: Vec<String>) -> usize:\n    names.len()\nfn main():\n    print(\"{}\", a(Vec::new()))\n");
    assert!(f.starts_with("borrowed_container:"), "{}", f);
    assert!(f.contains("[String]"), "{}", f);
    let f = only("fn b(s: String) -> usize:\n    s.len()\nfn main():\n    print(\"{}\", b(String::from(\"x\")))\n");
    assert!(f.contains("`str`"), "{}", f);
}

#[test]
fn a_written_reference_to_a_container_is_reported_too() {
    let f = only("fn c(v: &Vec<u8>) -> usize:\n    v.len()\nfn main():\n    print(\"{}\", c(&Vec::new()))\n");
    assert!(f.starts_with("borrowed_container:") && f.contains("[u8]"), "{}", f);
}

#[test]
fn the_slice_types_and_owning_modes_are_not_reported() {
    none("fn d(names: [String]) -> usize:\n    names.len()\nfn main():\n    print(\"{}\", d(&Vec::new()))\n");
    none("fn e(s: str) -> usize:\n    s.len()\nfn main():\n    print(\"{}\", e(\"x\"))\n");
    // `take` and `inout` say something the slice type cannot
    none("fn f(v: take Vec<u8>) -> usize:\n    v.len()\nfn main():\n    print(\"{}\", f(Vec::new()))\n");
    none("fn g(v: inout Vec<u8>):\n    v.push(1)\nfn main():\n    var w = Vec::new()\n    g(w)\n");
}

// ----- copy_candidate -----

#[test]
fn a_parameter_used_only_through_a_deref_suggests_copy() {
    let f = only("use foreign::Tag\nfn k(t: Tag) -> Tag:\n    *t\nfn main():\n    print(\"x\")\n");
    assert!(f.starts_with("copy_candidate:"), "{}", f);
    assert!(f.contains("@copy use"), "{}", f);
}

#[test]
fn a_crate_declared_type_is_told_to_derive_copy() {
    let src = "@derive(Debug)\nstruct Local:\n    n: u32\nfn l(x: Local) -> Local:\n    *x\nfn main():\n    print(\"x\")\n";
    let f = only(src);
    assert!(f.starts_with("copy_candidate:") && f.contains("@derive(Copy)"), "{}", f);
}

#[test]
fn a_parameter_used_as_a_value_is_not_reported() {
    let src = "@derive(Debug)\nstruct Local:\n    n: u32\nfn m(y: Local) -> u32:\n    y.n\nfn n(z: Local) -> Local:\n    if z.n > 0: *z else: *z\nfn main():\n    print(\"x\")\n";
    none(src);
}

#[test]
fn a_parameter_used_only_inside_a_macro_is_not_reported() {
    // the macro body is opaque tokens (§22.5); the use counts, so the deref rule cannot fire
    none("use foreign::Tag\nfn o(t: Tag) -> String:\n    format!(\"{:?}\", t)\nfn main():\n    print(\"x\")\n");
}

// ----- the command -----

fn uredo(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_uredo"))
        .args(args)
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        .output()
        .expect("run uredo");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
}

#[test]
fn the_command_reports_deny_allow_and_a_clean_tree() {
    let (code, out, _) = uredo(&["lint", "tests/fixtures/lint"]);
    assert_eq!(code, 0, "findings alone must not fail: {}", out);
    assert!(out.contains("lint: borrowed_container") && out.contains("lint: redundant_take") && out.contains("lint: copy_candidate"), "{}", out);
    assert!(out.contains("3 finding(s) in 1 file(s)"), "{}", out);

    let (code, out, _) = uredo(&["lint", "--deny", "tests/fixtures/lint"]);
    assert_eq!(code, 1, "{}", out);

    let (code, out, _) = uredo(&["lint", "--allow", "borrowed_container", "--allow", "redundant_take", "--allow", "copy_candidate", "tests/fixtures/lint"]);
    assert_eq!(code, 0);
    assert!(out.contains("no findings"), "{}", out);

    let (code, _, err) = uredo(&["lint", "--allow", "nosuch", "tests/fixtures/lint"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown lint `nosuch`"), "{}", err);

    // the corpus and the examples are clean; the portfolio keeps one deliberate illustration
    let (code, out, _) = uredo(&["lint", "../corpus", "../examples"]);
    assert_eq!(code, 0, "{}", out);
    assert!(out.contains("no findings"), "{}", out);
}

#[test]
fn a_file_that_does_not_compile_is_reported_not_linted() {
    let (code, _, err) = uredo(&["lint", "tests/fixtures/lint_broken/src/main.ure"]);
    assert_eq!(code, 1, "{}", err);
    assert!(err.contains("did not compile"), "{}", err);
}

// ----- the misdirected `@allow` warning (§23) -----

fn warnings(src: &str) -> Vec<String> {
    let out = uredo::compile(src, false);
    assert!(!out.has_errors(), "unexpected errors: {:?}", out.diags);
    out.diags
        .iter()
        .filter(|d| d.level == uredo::diag::Level::Warning)
        .map(|d| format!("{} {}", d.msg, d.notes.join(" ")))
        .collect()
}

#[test]
fn an_allow_naming_a_uredo_lint_warns_on_every_spelling() {
    for src in [
        "@allow(redundant_take)\nfn e(x: take u32) -> u32:\n    x\nfn main():\n    print(\"{}\", e(1))\n",
        "@rust(allow(redundant_take))\nfn e(x: take u32) -> u32:\n    x\nfn main():\n    print(\"{}\", e(1))\n",
        "@expect(redundant_take)\nfn e(x: take u32) -> u32:\n    x\nfn main():\n    print(\"{}\", e(1))\n",
        "@!allow(redundant_take)\nfn main():\n    print(\"x\")\n",
    ] {
        let w = warnings(src);
        assert_eq!(w.len(), 1, "expected one warning for {:?}, got {:?}", src, w);
        assert!(w[0].contains("`redundant_take` names a Uredo lint"), "{}", w[0]);
        assert!(w[0].contains("uredo lint --allow redundant_take"), "{}", w[0]);
    }
}

#[test]
fn the_attribute_is_still_forwarded_unchanged() {
    // Uredo never changes what reaches rustc: if rustc ever ships a lint of the same name, the
    // user's suppression must still work. The warning is advisory only (§23).
    let out = uredo::compile("@allow(redundant_take)\nfn e(x: take u32) -> u32:\n    x\nfn main():\n    print(\"{}\", e(1))\n", false);
    assert!(out.rust.contains("#[allow(redundant_take)]"), "{}", out.rust);
    assert!(!out.has_errors(), "the warning must not fail the build: {:?}", out.diags);
}

#[test]
fn an_ordinary_allow_is_silent() {
    assert!(warnings("@allow(dead_code)\nfn g() -> u32:\n    1\nfn main():\n    print(\"{}\", g())\n").is_empty());
    assert!(warnings("@expect(unused_variables)\nfn h(n: u32) -> u32:\n    0\nfn main():\n    print(\"{}\", h(2))\n").is_empty());
    assert!(warnings("@derive(Debug)\nstruct A:\n    n: u32\nfn main():\n    print(\"{:?}\", A { n: 1 })\n").is_empty());
}

#[test]
fn one_attribute_naming_several_lints_warns_once() {
    let w = warnings("@allow(dead_code, redundant_take, borrowed_container)\nfn e(x: take u32) -> u32:\n    x\nfn main():\n    print(\"{}\", e(1))\n");
    assert_eq!(w.len(), 1, "{:?}", w);
    assert!(w[0].contains("redundant_take`, `borrowed_container"), "{}", w[0]);
}

// ----- the D52 reversal counts (§38), which are measurements rather than findings -----

fn measures(src: &str) -> (usize, Vec<String>) {
    let out = uredo::compile(src, false);
    assert!(!out.has_errors(), "unexpected errors: {:?}", out.diags);
    assert!(out.lints.iter().all(|d| uredo::lint::name_of(d).is_some()), "a measurement leaked into the findings");
    (out.measures.p8_params, out.measures.p8_clones.iter().map(|d| d.msg.clone()).collect())
}

#[test]
fn by_value_generic_parameters_are_counted_where_a_take_would_have_been_written() {
    // one per declaration: the `take` a borrow default would have cost. A `Copy` bound makes the
    // parameter known-Copy (P1), which needed no annotation either way, so it is not counted; a
    // slice of `T` is borrowed; and an impl block's own parameter is in scope for its methods.
    let src = "struct Pair<T>:\n    a: T\n    b: T\n\
               \nimpl<T: Clone> Pair<T>:\n    fn new(a: T, b: T) -> Self:\n        Self { a, b }\n\
               \nfn boxed<T>(v: T) -> Box<T>:\n    Box::new(v)\n\
               fn tagged<U: std::fmt::Display + Copy>(u: U) -> String:\n    format(\"{u}\")\n\
               fn first<T>(xs: [T]) -> &T?:\n    xs.first()\n\
               fn shown(v: impl std::fmt::Debug) -> String:\n    format(\"{v:?}\")\n\
               fn main():\n    print(\"{}\", boxed(1))\n";
    let (params, clones) = measures(src);
    assert_eq!(params, 4, "expected `a`, `b`, `v` and the `impl Trait`; clones {:?}", clones);
    assert!(clones.is_empty(), "{:?}", clones);
}

#[test]
fn a_clone_handed_to_a_by_value_parameter_is_counted_and_named() {
    let src = "fn tag<T: Into<String>>(t: T) -> String:\n    t.into()\n\
               fn main():\n    n = String::from(\"ada\")\n    a = tag(n.clone())\n    b = tag((n.clone()))\n    print(\"{a}{b}\")\n";
    let (params, clones) = measures(src);
    assert_eq!(params, 1);
    assert_eq!(clones.len(), 2, "{:?}", clones);
    // the message names what was cloned, through the parentheses, and the callee it fed
    assert!(clones.iter().all(|m| m.starts_with("`n` is cloned into `tag`")), "{:?}", clones);
    // a clone at a borrowing call site is not the mode's cost, and is not counted
    let (_, clones) = measures("fn len(s: String) -> usize:\n    s.len()\nfn main():\n    n = String::from(\"a\")\n    print(\"{}\", len(n.clone()))\n");
    assert!(clones.is_empty(), "{:?}", clones);
}

#[test]
fn the_measure_mode_prints_both_counts_and_no_findings() {
    let (code, out, err) = uredo(&["lint", "--measure", "tests/fixtures/lint"]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(out.contains("D52 reversal counts"), "{}", out);
    assert!(out.contains("by-value generic parameters (P8)"), "{}", out);
    assert!(out.contains("`.clone()` at a P8 call site"), "{}", out);
    assert!(!out.contains("finding(s)"), "{}", out);
}

// ----- the §38 lifetime rate, which the held notation question is conditioned on -----

#[test]
fn written_lifetimes_are_counted_by_position() {
    let src = "struct Token<'a>:\n    text: &'a str\n\nfn longest<'a>(a: &'a str, b: &'a str) -> &'a str:\n    if a.len() > b.len(): a else: b\n\nfn owned<T: Clone + 'static>(t: take T) -> T:\n    t\n\nfn label() -> &'static str:\n    var n = 0\n    'outer: loop:\n        n += 1\n        if n > 2: break 'outer\n    \"done\"\n\nfn main():\n    print(\"{} {} {}\", longest(\"aa\", \"b\"), owned(1u8), label())\n";
    let out = uredo::compile(src, false);
    assert!(!out.has_errors(), "{:?}", out.diags);
    let l = &out.measures.lifetimes;
    // `&'a str` x3 in `longest`, `&'a str` in the field, `&'static str` in the return
    assert_eq!(l.in_reference, 5, "{:?}", l);
    assert_eq!(l.in_bound, 1, "{:?}", l);          // `+ 'static`
    assert_eq!(l.in_generics, 2, "{:?}", l);       // `<'a>` on the struct and on `longest`
    assert_eq!(l.label, 2, "{:?}", l);             // `'outer:` and `break 'outer`
    assert_eq!(l.total(), 10, "{:?}", l);
    assert_eq!(l.static_named, 2, "{:?}", l);
    assert_eq!(l.anonymous, 0, "{:?}", l);

    // an apostrophe in a comment, a string or a char literal is not a lifetime
    let quiet = uredo::compile("fn main():\n    # the author's own note\n    q = '\\''\n    print(\"it's fine {q}\")\n", false);
    assert!(!quiet.has_errors(), "{:?}", quiet.diags);
    assert_eq!(quiet.measures.lifetimes.total(), 0, "{:?}", quiet.measures.lifetimes);
}

#[test]
fn the_measure_mode_reports_the_lifetime_rate() {
    let (code, out, err) = uredo("lint --measure tests/fixtures/lint".split(' ').map(|s| s.to_string()).collect::<Vec<_>>().iter().map(|s| s.as_str()).collect::<Vec<_>>().as_slice());
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(out.contains("Lifetimes written over the same files"), "{}", out);
    assert!(out.contains("per 1,000 lines"), "{}", out);
    assert!(out.contains("§38 asks for 2,300 lines"), "{}", out);
}

// ----- `uredo fix` (§28.1): the fixes that provably change nothing in the output -----

#[test]
fn fix_applies_only_what_leaves_the_generated_rust_identical() {
    let work = std::env::temp_dir().join(format!("uredo-fix-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&work);
    let file = work.join("fixme.ure");
    let src = "fn scalar(x: take u32) -> u32:\n    x + 1\n\nfn generic<T: std::fmt::Debug>(t: take T) -> String:\n    format(\"{t:?}\")\n\nfn two(a: take u8, b: take u8) -> u8:\n    a + b\n\nfn kept(s: take String) -> usize:\n    s.len()\n\nfn main():\n    print(\"{} {} {} {}\", scalar(1), generic(2u8), two(1, 2), kept(String::from(\"x\")))\n";
    std::fs::write(&file, src).unwrap();
    let before = uredo::compile(src, true);
    assert!(!before.has_errors(), "{:?}", before.diags);

    let path = file.to_string_lossy().to_string();
    let (code, out, err) = uredo(&["fix", "--dry-run", &path]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(out.contains("would apply 4 fix(es)"), "{}", out);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), src, "a dry run must not write");

    let (code, out, err) = uredo(&["fix", &path]);
    assert_eq!(code, 0, "{}{}", out, err);
    assert!(out.contains("applied 4 fix(es)"), "{}", out);

    let fixed = std::fs::read_to_string(&file).unwrap();
    // both parameters of a two-parameter line are fixed, and the one that changes the mode is not
    assert!(fixed.contains("fn two(a: u8, b: u8)"), "{}", fixed);
    assert!(fixed.contains("fn scalar(x: u32)"), "{}", fixed);
    assert!(fixed.contains("fn generic<T: std::fmt::Debug>(t: T)"), "{}", fixed);
    assert!(fixed.contains("fn kept(s: take String)"), "`take String` changes the mode and is not a fix:\n{}", fixed);

    // the contract: the output is byte-identical, and nothing is left to fix
    let after = uredo::compile(&fixed, true);
    assert!(!after.has_errors(), "{:?}", after.diags);
    assert_eq!(before.rust, after.rust, "fix changed the generated Rust");
    let (code, out, _) = uredo(&["fix", "--dry-run", &path]);
    assert_eq!(code, 0);
    assert!(out.contains("would apply 0 fix(es)"), "{}", out);

    let _ = std::fs::remove_dir_all(&work);
}
