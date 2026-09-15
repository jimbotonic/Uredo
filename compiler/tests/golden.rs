//! Golden tests: each portfolio topic's `snippet.ure` must compile to exactly its `lowered.rs`.
//!
//! Set `UREDO_GOLDEN_UPDATE=1` to rewrite the expected files from the compiler's output
//! (review the diff before committing).

use std::fs;
use std::path::PathBuf;

fn portfolio_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("docs").join("portfolio")
}

/// Topics whose constructs are implemented so far.
fn phase_topics() -> Vec<String> {
    let want: Vec<String> = std::env::var("UREDO_GOLDEN_TOPICS").ok().map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_default();
    let mut dirs: Vec<String> = fs::read_dir(portfolio_dir())
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
        .collect();
    dirs.sort();
    if want.is_empty() {
        dirs
    } else {
        dirs.into_iter().filter(|d| want.iter().any(|w| d.starts_with(w.as_str()))).collect()
    }
}

fn check_topic(topic: &str) -> Result<(), String> {
    let dir = portfolio_dir().join(topic);
    let src = fs::read_to_string(dir.join("snippet.ure")).map_err(|e| format!("{}: {}", topic, e))?;
    let out = uredo::compile(&src, true);
    if out.has_errors() {
        let mut msg = format!("{}: compile errors:\n", topic);
        for d in &out.diags {
            msg.push_str(&d.render("snippet.ure", &src));
        }
        msg.push_str("--- raw output ---\n");
        msg.push_str(&out.raw);
        return Err(msg);
    }
    let expected_path = dir.join("lowered.rs");
    let expected = fs::read_to_string(&expected_path).unwrap_or_default();
    if out.rust == expected {
        return Ok(());
    }
    if std::env::var("UREDO_GOLDEN_IGNORE_DOCS").is_ok() && strip_docs(&out.rust) == strip_docs(&expected) {
        return Ok(());
    }
    if std::env::var("UREDO_GOLDEN_UPDATE").is_ok() {
        fs::write(&expected_path, &out.rust).unwrap();
        return Ok(());
    }
    Err(format!("{}: generated Rust differs from lowered.rs\n{}", topic, diff(&expected, &out.rust)))
}

fn strip_docs(s: &str) -> String {
    s.lines().filter(|l| !l.trim_start().starts_with("///") && !l.trim_start().starts_with("//!")).collect::<Vec<_>>().join("\n")
}

fn diff(expected: &str, actual: &str) -> String {
    let ignore = std::env::var("UREDO_GOLDEN_IGNORE_DOCS").is_ok();
    let (expected, actual) = if ignore { (strip_docs(expected), strip_docs(actual)) } else { (expected.to_string(), actual.to_string()) };
    let e: Vec<&str> = expected.lines().collect();
    let a: Vec<&str> = actual.lines().collect();
    let mut out = String::new();
    let n = e.len().max(a.len());
    let mut shown = 0;
    for i in 0..n {
        let el = e.get(i).copied().unwrap_or("<eof>");
        let al = a.get(i).copied().unwrap_or("<eof>");
        if el != al {
            out.push_str(&format!("line {}:\n  expected: {}\n  actual:   {}\n", i + 1, el, al));
            shown += 1;
            if shown > 12 {
                out.push_str("  …\n");
                break;
            }
        }
    }
    out
}

#[test]
fn portfolio_golden() {
    let mut failures = Vec::new();
    let topics = phase_topics();
    for t in &topics {
        if let Err(e) = check_topic(t) {
            failures.push(e);
        }
    }
    if !failures.is_empty() {
        panic!("{} of {} topics failed:\n\n{}", failures.len(), topics.len(), failures.join("\n\n"));
    }
}
