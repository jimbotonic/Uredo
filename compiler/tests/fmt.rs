//! `uredo fmt` (§29): canonical spacing, indentation and layout; idempotence; the portfolio and
//! the examples are canonical and formatting never changes what is generated.

use std::fs;
use std::path::PathBuf;

fn fmt(src: &str) -> String {
    uredo::fmt::format(src).unwrap_or_else(|d| panic!("{:?}", d))
}

fn same(src: &str) {
    assert_eq!(fmt(src), src, "expected canonical input to be unchanged");
}

/// Every `.ure` file in the repository. The portfolio alone is not enough to hold the formatter
/// to its contract: those 25 snippets are written to be canonical and short, so they never make it
/// wrap a line, while the rest of the repository holds 182 lines over the 100-column limit. The
/// defect this guards against — a wrapped continuation line expanded as though it were a statement,
/// turning a program into text that does not parse — lived entirely in the part not covered.
fn every_source() -> Vec<PathBuf> {
    fn walk(dir: &PathBuf, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().map(|n| n == "target").unwrap_or(false) {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().map(|x| x == "ure").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut v = Vec::new();
    for top in ["corpus", "docs", "examples", "tools"] {
        walk(&root.join(top), &mut v);
    }
    // and the fixtures, which hold the wrapping cases on purpose: production code contains them
    // only sometimes, and a property test whose hard cases come and go is not a guard
    walk(&root.join("compiler/tests/fixtures"), &mut v);
    v.sort();
    assert!(v.len() > 80, "expected the whole repository, found {} files", v.len());
    v
}

fn portfolio() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/portfolio");
    let mut v: Vec<PathBuf> = fs::read_dir(dir).unwrap().flatten().map(|e| e.path().join("snippet.ure")).filter(|p| p.exists()).collect();
    v.sort();
    v
}

#[test]
fn portfolio_is_canonical_and_formatting_is_idempotent() {
    for p in portfolio() {
        let src = fs::read_to_string(&p).unwrap();
        let once = fmt(&src);
        assert_eq!(once, src, "{} is not canonical", p.display());
        assert_eq!(fmt(&once), once, "{} not idempotent", p.display());
    }
}

#[test]
fn formatting_never_changes_the_generated_rust() {
    for p in portfolio() {
        let src = fs::read_to_string(&p).unwrap();
        // perturb outside literals: trailing spaces on every line and doubled blank lines
        let messy: String = src.lines().map(|l| if l.trim().is_empty() { "\n\n".to_string() } else { format!("{}   \n", l) }).collect();
        let formatted = fmt(&messy);
        let a = uredo::compile(&src, true);
        let b = uredo::compile(&formatted, true);
        assert!(!a.has_errors() && !b.has_errors(), "{}", p.display());
        let strip = |s: &str| s.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>().join("\n");
        assert_eq!(strip(&a.rust), strip(&b.rust), "{}", p.display());
    }
}

#[test]
fn spacing_rules() {
    same("use std::sync::{Arc, Mutex}\n");
    same("fn f<T: AsRef<[u8]> + ?Sized>(x: T) -> Vec<&str>:\n    x.as_ref().len()\n");
    same("fn main():\n    a = [1, 2, 3]\n    b = a[..2]\n    c = 0..10\n    d = &a\n    e = -x * !y\n    f = w as Box<dyn Shape>\n    g = pub_fn(a, b)?\n");
    same("fn main():\n    match toks:\n        [first, .., last]: 1\n        [x, rest @ ..]: 2\n        Config { ref mut n, .. }: 3\n");
    same("macro_rules! square {\n    ($e:expr) => { $e * $e };\n}\n");
    same("fn main():\n    v = rust { unsafe { 1 } }\n    tokio::spawn(async move { work().await })\n    println!(\"{}\", v)\n");
    same("enum List:\n    Cons(i32, Box<List>)\n    Nil\n");
    same("pub(crate) fn scale(p: Point, k: f64) -> Point:\n    p\n");
    same("fn main():\n    xs = fib().take(10).collect::<Vec<u64>>()\n    if !seen.insert(n): print(\"dup\")\n");
}

#[test]
fn reindents_and_normalises_blank_lines_and_comments() {
    let messy = "fn main():\n    x  =  1   \n\n\n\n# a comment\n    y = 2 # trailing\n";
    assert_eq!(fmt(messy), "fn main():\n    x = 1\n\n    # a comment\n    y = 2  # trailing\n");
}

#[test]
fn continuation_and_closing_lines() {
    let messy = "fn main():\n    v = f(\n      1,\n      2\n    )\n    w = items.iter()\n            .map(x => x)\n            .collect::<Vec<i32>>()\n";
    let expect = "fn main():\n    v = f(\n        1,\n        2,\n    )\n    w = items.iter()\n        .map(x => x)\n        .collect::<Vec<i32>>()\n";
    assert_eq!(fmt(messy), expect);
}

#[test]
fn trailing_block_argument_layout() {
    let messy = "fn main():\n    s = xs.iter().fold(0, (acc, x) =>\n        y  = x*2\n        acc + y\n    )\n    w = (0..3)\n            .map(i =>\n                i + 1\n            )\n            .collect::<Vec<i32>>()\n";
    let expect = "fn main():\n    s = xs.iter().fold(0, (acc, x) =>\n        y = x * 2\n        acc + y\n    )\n    w = (0..3)\n        .map(i =>\n            i + 1\n        )\n        .collect::<Vec<i32>>()\n";
    assert_eq!(fmt(messy), expect);
}

#[test]
fn items_get_one_blank_line_and_docs_stay_attached() {
    let messy = "use a::B\nuse c::D\n## doc\nfn f():\n    1\nfn g():\n    2\nstruct P(i32)\nstruct Q\n";
    let expect = "use a::B\nuse c::D\n\n## doc\nfn f():\n    1\n\nfn g():\n    2\n\nstruct P(i32)\nstruct Q\n";
    assert_eq!(fmt(messy), expect);
}

#[test]
fn over_long_inline_bodies_are_expanded() {
    let long = format!("fn main():\n    if condition_number_one && condition_number_two: {}\n", "call_a_very_long_function_name(argument_one, argument_two, argument_three)");
    let out = fmt(&long);
    assert!(out.contains("condition_number_two:\n        call_a_very_long"), "{}", out);
    let long_else = "fn main():\n    n = if some_long_condition_name(value_one): compute_the_first_branch(value_one) else: compute_the_second_branch(value_two)\n";
    let out = fmt(long_else);
    assert!(out.contains("value_one):\n        compute_the_first_branch(value_one)\n    else:\n        compute_the_second_branch(value_two)\n"), "{}", out);
}

#[test]
fn block_header_continuations_clear_the_body() {
    // a `for` whose source is a multi-line chain: continuations at +8, body at +4 (else the
    // two would share a column)
    let src = "fn main():\n    for item in read(dir)\n            .filter(x => x.ok())\n            .flatten():\n        print(\"{item}\")\n";
    same(src);
    let messy = "fn main():\n    for item in read(dir)\n        .filter(x => x.ok())\n        .flatten():\n        print(\"{item}\")\n";
    assert_eq!(fmt(messy), src);
}

#[test]
fn macro_brace_bodies_keep_their_space() {
    same("fn main():\n    gen_tests! {\n        a => 1,\n    }\n    v = vec![1, 2]\n    print(\"{v:?}\")\n");
}

#[test]
fn a_long_signature_is_not_split_at_a_generic_bound() {
    same("struct S:\n    pub(crate) fn downcast_ref<T: std::any::Any + Clone + Send + Sync + 'static>(self) -> T?:\n        None\n");
}

/// The formatter must never turn a program into something that is not one. A long call whose last
/// argument is a one-line `if a: x else: y` was broken after the `if` header's colon, because the
/// expansion that handles an over-wide *statement* was applied to a continuation line inside the
/// call. A statement's brackets balance; a continuation's do not.
#[test]
fn a_wrapped_continuation_line_is_not_expanded_as_a_statement() {
    let src = "fn main():\n    ok = true\n    conclusive = false\n    for i in 0..2:\n        print(\"{:<14} ratio {:.4}  upper {:.4}   controls: self {:.4} twin {:.4}   {}\",\n            i, 1.0, 1.0, 1.0, 1.0,\n            if ok: \"pass\" else if !conclusive: \"INCONCLUSIVE (the machine is too noisy)\" else: \"FAIL\")\n";
    let formatted = fmt(src);
    assert_eq!(formatted, src, "the formatter changed a line it cannot safely expand");
    assert!(!uredo::compile(&formatted, false).has_errors(), "the formatted program no longer compiles");
    // the same expression as a statement of its own is still expanded when it is too wide
    let wide = "fn main():\n    ok = true\n    for i in 0..2:\n        for j in 0..2:\n            for k in 0..2:\n                x = if ok: \"a rather long string that pushes this line over the hundred column limit\" else: \"b\"\n                print(\"{x}{i}{j}{k}\")\n";
    let out = fmt(wide);
    assert!(out.contains("x = if ok:\n"), "an over-wide statement should still expand:\n{}", out);
    assert!(!uredo::compile(&out, false).has_errors(), "{}", out);
}

/// The formatter's contract on every file in the repository: it may change the text, but a program
/// that compiled must still compile, must generate the same Rust, and formatting twice must be the
/// same as formatting once. Nothing here asserts the input was canonical — several archived pieces
/// are kept as they were recorded — only that formatting is meaning-preserving.
#[test]
fn formatting_preserves_meaning_on_every_source_in_the_repository() {
    let strip = |s: &str| s.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>().join("\n");
    for p in every_source() {
        let src = fs::read_to_string(&p).unwrap();
        let formatted = fmt(&src);
        assert_eq!(fmt(&formatted), formatted, "formatting is not idempotent on {}", p.display());
        let before = uredo::compile(&src, true);
        let after = uredo::compile(&formatted, true);
        assert_eq!(
            before.has_errors(),
            after.has_errors(),
            "formatting changed whether {} compiles:\n{:?}",
            p.display(),
            after.diags
        );
        assert_eq!(strip(&before.rust), strip(&after.rust), "formatting changed what {} generates", p.display());
    }
}
