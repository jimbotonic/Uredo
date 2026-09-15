//! Diagnostics: Uredo-side errors with Uredo spans (§27).

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Level {
    Error,
    Warning,
    /// Neither wrong nor suspect: something recorded because a measurement counts it (§38).
    Note,
}

/// A rewrite `uredo fix` may apply: on `line`, the first occurrence of `find` becomes `replace`.
///
/// Deliberately a text edit rather than a span. A fix is only ever offered for a finding whose
/// rewrite leaves the *generated Rust* byte-identical, and `fix` verifies that after applying it,
/// so the edit does not need to be clever — it needs to be checkable.
#[derive(Debug, Clone)]
pub struct Fix {
    pub line: usize,
    pub find: String,
    pub replace: String,
    /// what to call it in the report
    pub describe: String,
}

#[derive(Debug, Clone)]
pub struct Diag {
    pub level: Level,
    pub line: usize,
    pub col: usize,
    pub msg: String,
    pub notes: Vec<String>,
    pub fix: Option<Fix>,
}

impl Diag {
    pub fn error(line: usize, col: usize, msg: impl Into<String>) -> Diag {
        Diag { level: Level::Error, line, col, msg: msg.into(), notes: Vec::new(), fix: None }
    }
    pub fn warning(line: usize, col: usize, msg: impl Into<String>) -> Diag {
        Diag { level: Level::Warning, line, col, msg: msg.into(), notes: Vec::new(), fix: None }
    }
    pub fn note_at(line: usize, col: usize, msg: impl Into<String>) -> Diag {
        Diag { level: Level::Note, line, col, msg: msg.into(), notes: Vec::new(), fix: None }
    }
    /// Attaches the rewrite that would resolve this finding.
    pub fn fixed_by(mut self, line: usize, find: impl Into<String>, replace: impl Into<String>, describe: impl Into<String>) -> Diag {
        self.fix = Some(Fix { line, find: find.into(), replace: replace.into(), describe: describe.into() });
        self
    }
    pub fn note(mut self, n: impl Into<String>) -> Diag {
        self.notes.push(n.into());
        self
    }
    /// Renders the diagnostic in rustc's style with the offending source line.
    pub fn render(&self, file: &str, src: &str) -> String {
        let level = match self.level {
            Level::Error => "error",
            Level::Warning => "warning",
            Level::Note => "note",
        };
        // Line 0 means the diagnostic is about the file as a whole rather than a place in it —
        // a missing rustfmt, a lowering defect. Printing `file:0:0` and a caret under line 1
        // pointed at source that had nothing to do with it.
        if self.line == 0 {
            let mut out = format!("{}: {}\n  --> {}\n", level, self.msg, file);
            for n in &self.notes {
                out.push_str(&format!("   = {}\n", n));
            }
            return out;
        }
        let mut out = format!("{}: {}\n  --> {}:{}:{}\n", level, self.msg, file, self.line, self.col);
        if let Some(line) = src.lines().nth(self.line.saturating_sub(1)) {
            let w = self.line.to_string().len();
            out.push_str(&format!("{:w$} |\n{} | {}\n{:w$} | {}^\n", "", self.line, line, "", " ".repeat(self.col.saturating_sub(1)), w = w));
        }
        for n in &self.notes {
            out.push_str(&format!("   = {}\n", n));
        }
        out
    }
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.msg)
    }
}
