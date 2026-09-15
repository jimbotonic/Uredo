//! `uredo doc` and documentation examples (§26.4, D58): a fenced block in a `##` comment is Uredo
//! source, lowered into the generated crate as an ordinary doctest.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());
fn serial() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn uredo(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_uredo")).args(args).current_dir(root()).output().expect("run uredo");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
}

#[test]
fn each_kind_of_fence_is_treated_as_its_tag_says() {
    let src = "## Joins.\n##\n## ```\n## joined = join(\"a\", \"b\")\n## ```\n##\n## ```rust\n## let x: u32 = 1;\n## ```\n##\n## ```text\n## a-b\n## ```\n##\n## ```no_run\n## join(\"a\", \"b\")\n## ```\npub fn join(a: str, b: str) -> String:\n    format(\"{a}-{b}\")\n";
    let out = uredo::compile(src, true);
    assert!(!out.has_errors(), "{:?}", out.diags);
    // an untagged block is Uredo, lowered, and left untagged so rustdoc tests it
    assert!(out.rust.contains("/// let joined = join(\"a\", \"b\");"), "{}", out.rust);
    // a `rust` block is passed through as written
    assert!(out.rust.contains("/// ```rust") && out.rust.contains("/// let x: u32 = 1;"), "{}", out.rust);
    // `text` is prose
    assert!(out.rust.contains("/// ```text") && out.rust.contains("/// a-b"), "{}", out.rust);
    // an attribute is carried across, and its block is still lowered
    assert!(out.rust.contains("/// ```no_run"), "{}", out.rust);
    assert!(out.rust.contains("/// join(\"a\", \"b\");"), "{}", out.rust);
}

#[test]
fn an_example_that_does_not_compile_is_an_error() {
    let src = "## Broken.\n##\n## ```\n## x = = 1\n## ```\npub fn f() -> u32:\n    1\n";
    let out = uredo::compile(src, true);
    assert!(out.has_errors(), "a broken example must not pass silently:\n{}", out.rust);
    let d = out.diags.iter().find(|d| d.level == uredo::diag::Level::Error).unwrap();
    assert!(d.msg.contains("a documentation example does not compile"), "{}", d.msg);
    assert!(d.notes.iter().any(|n| n.contains("```rust")), "the note should name the escape: {:?}", d.notes);
}

#[test]
fn a_documented_package_runs_its_examples_and_renders_them() {
    let _guard = serial();
    // every example in the fixture is compiled and run by `cargo test`, including the module's own
    let (ok, out, err) = uredo(&["test", "../examples/compat/docs"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("test src/lib.rs - join (line 15) ... ok"), "{}", out);
    assert!(out.contains("test src/lib.rs - (line 8) ... ok"), "the module-level example\n{}", out);
    assert!(out.contains("- compile ... ok"), "the `no_run` example is compiled, not run\n{}", out);
    assert!(out.contains("5 passed"), "{}", out);

    let (ok, out, err) = uredo(&["doc", "../examples/compat/docs"]);
    assert!(ok, "{}{}", out, err);
    let page = root().join("../examples/compat/docs/target/doc/compat_docs/fn.join.html");
    assert!(page.exists(), "no rendered page at {}", page.display());
    let html = std::fs::read_to_string(&page).unwrap();
    assert!(html.contains("rust-example-rendered"), "the example is not rendered as code");
    // rustdoc syntax-highlights the block, so compare the page with its tags removed
    let text: String = {
        let mut out = String::new();
        let mut chars = html.chars();
        while let Some(c) = chars.next() {
            if c == '<' {
                for d in chars.by_ref() {
                    if d == '>' {
                        break;
                    }
                }
                continue;
            }
            out.push(c);
        }
        out.replace("&quot;", "\"").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
    };
    assert!(text.contains("let joined = join(\"a\", \"b\");"), "the example's text is missing from the page");
    assert!(text.contains("use compat_docs::join;"), "{}", &text[..text.len().min(400)]);
}
