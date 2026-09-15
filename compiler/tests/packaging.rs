//! What the published crate must contain.
//!
//! The crate declared `license = "MIT OR Apache-2.0"` and shipped neither licence text, which is
//! the very thing Uredo's own `uredo publish` refuses a package for (§28.1). Publishing to a
//! registry is permanent — `cargo yank` "does not delete any data" — so the first archive has to
//! be right, and these assert it rather than a memory of having checked.

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn manifest() -> String {
    std::fs::read_to_string(crate_root().join("Cargo.toml")).expect("read Cargo.toml")
}

/// The value of a top-level `key = ...` line, as written.
fn field(key: &str) -> Option<String> {
    manifest()
        .lines()
        .find(|l| l.trim_start().starts_with(&format!("{} =", key)))
        .map(|l| l.split_once('=').unwrap().1.trim().to_string())
}

#[test]
fn every_declared_licence_ships_its_text() {
    let declared = field("license").expect("the manifest declares a licence");
    assert!(declared.contains("MIT") && declared.contains("Apache-2.0"), "unexpected licence: {}", declared);

    for (file, marker) in [("LICENSE-MIT", "MIT License"), ("LICENSE-APACHE", "Apache License")] {
        let path = crate_root().join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is declared in `license` but is not in the crate: {}", file, e));
        assert!(text.len() > 500, "{} is present but looks empty ({} bytes)", file, text.len());
        assert!(text.contains(marker), "{} does not look like the licence it names", file);

        // It may be a symlink to the repository root — cargo dereferences those when it packages,
        // so the archive gets the text. Whichever it is, it must agree with the root copy, or the
        // project has two licences that can drift apart.
        let root = crate_root().parent().expect("repository root").join(file);
        if root.exists() {
            let root_text = std::fs::read_to_string(&root).expect("read the root copy");
            assert_eq!(text, root_text, "{} in the crate differs from the one at the repository root", file);
        }
    }
}

#[test]
fn the_manifest_carries_what_a_registry_reader_needs() {
    for key in ["description", "repository", "readme", "keywords", "categories", "rust-version"] {
        assert!(field(key).is_some(), "the manifest has no `{}`", key);
    }
    let readme = field("readme").unwrap().trim_matches('"').to_string();
    assert!(
        Path::new(&crate_root().join(&readme)).exists(),
        "`readme = {}` names a file that is not in the crate",
        readme
    );
    // crates.io rejects a category that is not in its fixed list; these three were checked
    // against its `/api/v1/categories` endpoint.
    let cats = field("categories").unwrap();
    for c in ["compilers", "development-tools", "command-line-utilities"] {
        assert!(cats.contains(c), "category `{}` went missing from the manifest", c);
    }
    // At most five keywords, each at most twenty characters: crates.io's limits.
    let kw = field("keywords").unwrap();
    let words: Vec<&str> = kw.trim_matches(['[', ']'].as_slice()).split(',').map(|w| w.trim().trim_matches('"')).filter(|w| !w.is_empty()).collect();
    assert!(words.len() <= 5, "crates.io allows at most 5 keywords, found {}", words.len());
    for w in &words {
        assert!(w.len() <= 20, "keyword `{}` is longer than crates.io's 20-character limit", w);
        assert!(
            w.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
                && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "keyword `{}` is not in crates.io's accepted form",
            w
        );
    }
}
