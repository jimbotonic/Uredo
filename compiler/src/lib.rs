//! The Uredo compiler library: lex -> parse -> lower -> rustfmt, with provenance (§5.3).

pub mod api;
pub mod ast;
pub mod diag;
pub mod fmt;
pub mod lexer;
pub mod lint;
pub mod lsp;
pub mod lower;
pub mod parser;
pub mod provenance;

use diag::{Diag, Level};
use std::io::Write;
use std::process::{Command, Stdio};

pub struct Output {
    /// Generated Rust (formatted when rustfmt succeeded).
    pub rust: String,
    /// Generated Rust before formatting (for lowering-defect reports, §5.4).
    pub raw: String,
    pub diags: Vec<Diag>,
    /// Provenance for `rust` (line map against the formatted bytes, elaboration records).
    pub map: provenance::Map,
    /// Public API entries of this module (§4.4).
    pub api: Vec<api::ApiEntry>,
    /// Idiom findings for `uredo lint` (§28.1); never errors, and printed by that command only.
    pub lints: Vec<Diag>,
    /// The D52 reversal counts (§38), printed by `uredo lint --measure`.
    pub measures: lint::Measures,
}

impl Output {
    pub fn has_errors(&self) -> bool {
        self.diags.iter().any(|d| d.level == Level::Error)
    }
}

/// Parses a module and indexes its declarations for cross-file resolution (§10.3).
pub fn index_source(src: &str, module_path: &str) -> lower::CrateIndex {
    let (toks, _) = lexer::lex(src);
    let (module, _) = parser::parse(src, toks);
    lower::index_module(&module, module_path)
}

/// Compiles one Uredo module to Rust source text.
pub fn compile(src: &str, format: bool) -> Output {
    compile_in_crate(src, format, &lower::CrateIndex::default(), "", "")
}

/// Lowers whatever parsed, errors and all — for the language server, and nothing else.
///
/// `compile` stops at the first batch of errors, which is right for a build: half a lowering is not
/// a program. An editor is the other case. The parser recovers at item and statement boundaries, so
/// a file with one broken line still yields every other item, and answering about those is the
/// whole of §28.2's tolerance. The result is never written anywhere: it carries the diagnostics and
/// the elaborations, and its `rust` is unformatted and incomplete by construction.
pub fn compile_tolerant(src: &str) -> Output {
    let (toks, mut diags) = lexer::lex(src);
    let lifetimes = lint::count_lifetimes(&toks, src);
    let (module, pdiags) = parser::parse(src, toks);
    diags.extend(pdiags);
    let lowered = lower::lower(&module, src);
    diags.extend(lowered.diags);
    let map = provenance::Map { ure: String::new(), rs: String::new(), lines: lowered.raw_map.clone(), elabs: lowered.elabs };
    Output {
        rust: lowered.raw.clone(),
        raw: lowered.raw,
        diags,
        map,
        api: lowered.api,
        lints: lowered.lints,
        measures: lint::Measures { lifetimes, ..lowered.measures },
    }
}

/// Compiles one module with the declarations of the rest of its crate available.
pub fn compile_in_crate(src: &str, format: bool, index: &lower::CrateIndex, crate_name: &str, module_path: &str) -> Output {
    let (toks, mut diags) = lexer::lex(src);
    let lifetimes = lint::count_lifetimes(&toks, src);
    let (module, pdiags) = parser::parse(src, toks);
    diags.extend(pdiags);
    let empty = || Output { rust: String::new(), raw: String::new(), diags: Vec::new(), map: provenance::Map::default(), api: Vec::new(), lints: Vec::new(), measures: lint::Measures { lifetimes: lifetimes.clone(), ..Default::default() } };
    if diags.iter().any(|d| d.level == Level::Error) {
        let mut o = empty();
        o.diags = diags;
        return o;
    }
    let lowered = lower::lower_in_crate(&module, src, index, crate_name, module_path);
    diags.extend(lowered.diags);
    let mut map = provenance::Map { ure: String::new(), rs: String::new(), lines: lowered.raw_map.clone(), elabs: lowered.elabs };
    if diags.iter().any(|d| d.level == Level::Error) {
        return Output { rust: lowered.raw.clone(), raw: lowered.raw, diags, map, api: lowered.api, lints: lowered.lints, measures: lint::Measures { lifetimes: lifetimes.clone(), ..lowered.measures } };
    }
    let rust = if format {
        match rustfmt(&lowered.raw) {
            Ok(s) => {
                map.lines = provenance::remap(&lowered.raw, &lowered.raw_map, &s);
                s
            }
            Err(FmtFailure::NotInstalled) => {
                diags.push(
                    Diag::error(0, 0, "rustfmt is not installed, and Uredo formats every file it generates (§29)")
                        .note("install it with `rustup component add rustfmt`")
                        .note("rustup's `minimal` profile omits rustfmt; its `default` profile has it")
                        .note("this is your toolchain, not a bug in Uredo"),
                );
                lowered.raw.clone()
            }
            Err(FmtFailure::Rejected(e)) => {
                diags.push(Diag::error(0, 0, "lowering defect: the generated Rust did not parse (§5.4)").note(e).note("this is a compiler bug; run with `--keep-rust` and report it"));
                lowered.raw.clone()
            }
        }
    } else {
        lowered.raw.clone()
    };
    Output { rust, raw: lowered.raw, diags, map, api: lowered.api, lints: lowered.lints, measures: lint::Measures { lifetimes: lifetimes.clone(), ..lowered.measures } }
}

/// Why `rustfmt` produced no formatted text.
///
/// The distinction is load-bearing and was not always made. A rustfmt that is **absent** is a
/// fact about the user's toolchain; a rustfmt that ran and **refused** means Uredo generated Rust
/// that does not parse, which is the compiler bug §5.4 describes. Reporting the first as the
/// second tells a new user their first program broke the compiler: `rustup` ships rustfmt in its
/// `default` profile but not in `minimal`, which rustup itself recommends for CI, so this is the
/// first thing an unattended `cargo install uredo` meets.
#[derive(Debug)]
pub enum FmtFailure {
    /// `rustfmt` is not on the PATH.
    NotInstalled,
    /// `rustfmt` ran and rejected the text, or could not be spoken to.
    Rejected(String),
}

/// Runs the pinned rustfmt over generated text (§29).
pub fn rustfmt(src: &str) -> Result<String, FmtFailure> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FmtFailure::NotInstalled,
            _ => FmtFailure::Rejected(format!("cannot run rustfmt: {}", e)),
        })?;
    child.stdin.take().unwrap().write_all(src.as_bytes()).map_err(|e| FmtFailure::Rejected(e.to_string()))?;
    let out = child.wait_with_output().map_err(|e| FmtFailure::Rejected(e.to_string()))?;
    if !out.status.success() {
        return Err(FmtFailure::Rejected(String::from_utf8_lossy(&out.stderr).to_string()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
