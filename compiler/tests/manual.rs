//! `docs/MANUAL.md` teaches the language, and a manual whose examples have rotted is worse than
//! none. Every block fenced `uredo` must compile as written; a fragment or an error message is
//! fenced `text` and is not compiled. The manual says so in its own preamble, which is the claim
//! this file keeps true.

use std::fs;
use std::path::PathBuf;

fn manual() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/MANUAL.md");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {}", path.display(), e))
}

/// Every fenced block, with its info string and the line it starts on.
fn blocks(text: &str) -> Vec<(String, String, usize)> {
    let mut out = Vec::new();
    let mut fence: Option<(String, usize, Vec<&str>)> = None;
    for (i, line) in text.lines().enumerate() {
        match (line.strip_prefix("```"), &mut fence) {
            (Some(_), Some((tag, at, body))) => {
                out.push((tag.clone(), body.join("\n"), *at));
                fence = None;
            }
            (Some(tag), None) => fence = Some((tag.trim().to_string(), i + 2, Vec::new())),
            (None, Some((_, _, body))) => body.push(line),
            (None, None) => {}
        }
    }
    assert!(fence.is_none(), "an unterminated fence in MANUAL.md");
    out
}

#[test]
fn every_uredo_example_in_the_manual_compiles() {
    let text = manual();
    let all = blocks(&text);
    let examples: Vec<&(String, String, usize)> = all.iter().filter(|(tag, _, _)| tag == "uredo").collect();
    assert!(examples.len() >= 20, "only {} `uredo` examples; the manual has lost its teeth", examples.len());

    // Lowering is not compiling. An example can lower cleanly and still be rejected by rustc — a
    // `String` parameter returned by value does exactly that — so each one is handed to rustc as
    // well. Checking only the lowering is the mistake this test was written with and caught by
    // breaking an example on purpose.
    let work = std::env::temp_dir().join(format!("uredo-manual-{}", std::process::id()));
    let _ = fs::create_dir_all(&work);
    for (_, body, at) in &examples {
        let out = uredo::compile(body, true);
        assert!(
            !out.has_errors(),
            "MANUAL.md:{} does not lower:\n{}\n---\n{}",
            at,
            body,
            out.diags
                .iter()
                .filter(|d| d.level == uredo::diag::Level::Error)
                .map(|d| d.render("MANUAL.md", body))
                .collect::<String>()
        );

        let kind = if out.rust.contains("fn main(") { "bin" } else { "lib" };
        let source = work.join(format!("example_{}.rs", at));
        fs::write(&source, &out.rust).expect("write the example");
        let rustc = std::process::Command::new("rustc")
            .args(["--edition", "2024", "--crate-type", kind, "--emit", "metadata", "--out-dir"])
            .arg(&work)
            .arg(&source)
            .output()
            .expect("rustc");
        assert!(
            rustc.status.success(),
            "MANUAL.md:{} lowers but does not compile:\n{}\n--- generated ---\n{}\n--- rustc ---\n{}",
            at,
            body,
            out.rust,
            String::from_utf8_lossy(&rustc.stderr)
        );
    }
    let _ = fs::remove_dir_all(&work);
    eprintln!("{} `uredo` examples lower and compile; {} blocks in total", examples.len(), all.len());
}

#[test]
fn the_manual_cites_sections_that_exist() {
    let text = manual();
    let spec = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/UREDO_LANGUAGE_SPEC_v0.4.md")).expect("the specification");

    // the sections the specification defines, by their headings
    let mut sections: Vec<String> = Vec::new();
    for line in spec.lines() {
        let trimmed = line.trim_start_matches('#').trim_start();
        if line.starts_with("##") {
            let number: String = trimmed.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            let number = number.trim_end_matches('.').to_string();
            if !number.is_empty() {
                sections.push(number);
            }
        }
    }
    assert!(sections.len() > 40, "only found {} sections in the specification", sections.len());

    // every `§n` the manual cites, outside a fenced block
    let mut inside = false;
    let mut missing: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with("```") {
            inside = !inside;
            continue;
        }
        if inside {
            continue;
        }
        let mut rest = line;
        while let Some(at) = rest.find('§') {
            rest = &rest[at + '§'.len_utf8()..];
            let number: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            let number = number.trim_end_matches('.').to_string();
            if !number.is_empty() && !sections.contains(&number) {
                missing.push(number);
            }
        }
    }
    assert!(missing.is_empty(), "the manual cites sections the specification does not have: {:?}", missing);
}
