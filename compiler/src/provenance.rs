//! Provenance (§5.3, §28.2): which Uredo line produced each generated Rust line, and which
//! elaboration rules fired. Built against the exact bytes handed to rustc: the raw map is
//! carried through rustfmt by aligning the two token streams.

use serde::{Deserialize, Serialize};

/// One elaboration Uredo performed (§5.2): the record `explain` and the diagnostic
/// translator read.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Elab {
    /// Uredo position of the construct.
    pub line: usize,
    pub col: usize,
    /// `call`, `method`, `field-sugar`, `throws-tail`, `intrinsic`, `for-borrow`, `receiver`,
    /// `binding`.
    pub kind: String,
    /// The rule that applied, in the words of the spec (`P3 (default shared borrow; …)`).
    pub rule: String,
    /// Uredo source text of the construct.
    pub before: String,
    /// Generated Rust text.
    pub after: String,
    /// `borrow: shared · clone: none · allocation: none · dispatch: none · sync: none`.
    pub inserted: String,
    pub callee: Option<String>,
    /// The callee's Uredo declaration, when known (`fn enqueue(job: take Job)`).
    pub declared: Option<String>,
    /// Arguments whose ownership moved into the callee (`take` parameters).
    pub takes: Vec<String>,
    /// Arguments passed by value through a type parameter or `impl Trait` (P8, D52): moved
    /// unless their type is `Copy`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub by_value: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Map {
    /// The Uredo file, relative to the package root.
    pub ure: String,
    /// The generated file, relative to the generated package root.
    pub rs: String,
    /// `lines[i]` is the Uredo line (1-based) that produced generated line `i + 1`; 0 = none.
    pub lines: Vec<usize>,
    pub elabs: Vec<Elab>,
}

impl Map {
    pub fn ure_line(&self, rs_line: usize) -> Option<usize> {
        // walk back to the nearest mapped line
        let mut i = rs_line;
        while i >= 1 {
            match self.lines.get(i - 1) {
                Some(0) => i -= 1,
                Some(l) => return Some(*l),
                None => return None,
            }
        }
        None
    }
    /// Generated lines produced by a Uredo line.
    pub fn rs_lines(&self, ure_line: usize) -> Vec<usize> {
        self.lines.iter().enumerate().filter(|(_, l)| **l == ure_line).map(|(i, _)| i + 1).collect()
    }
}

/// A minimal Rust token stream for alignment: (line, text). Whitespace and comments dropped.
pub fn tokens(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut line = 1;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // comments
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
            continue;
        }
        // raw strings r"…", r#"…"#, br"…"
        let mut j = i;
        if chars[j] == 'b' || chars[j] == 'c' {
            j += 1;
        }
        if chars.get(j) == Some(&'r') {
            let mut k = j + 1;
            let mut hashes = 0;
            while chars.get(k) == Some(&'#') {
                hashes += 1;
                k += 1;
            }
            if chars.get(k) == Some(&'"') {
                let start = i;
                let start_line = line;
                k += 1;
                loop {
                    if k >= chars.len() {
                        break;
                    }
                    if chars[k] == '\n' {
                        line += 1;
                    }
                    if chars[k] == '"' {
                        let mut n = 0;
                        while chars.get(k + 1 + n) == Some(&'#') && n < hashes {
                            n += 1;
                        }
                        if n == hashes {
                            k += 1 + n;
                            break;
                        }
                    }
                    k += 1;
                }
                out.push((start_line, chars[start..k].iter().collect()));
                i = k;
                continue;
            }
        }
        // strings
        if c == '"' || ((c == 'b' || c == 'c') && chars.get(i + 1) == Some(&'"')) {
            let start = i;
            let start_line = line;
            let mut k = if c == '"' { i + 1 } else { i + 2 };
            while k < chars.len() {
                if chars[k] == '\\' {
                    k += 2;
                    continue;
                }
                if chars[k] == '\n' {
                    line += 1;
                }
                if chars[k] == '"' {
                    k += 1;
                    break;
                }
                k += 1;
            }
            out.push((start_line, chars[start..k.min(chars.len())].iter().collect()));
            i = k;
            continue;
        }
        // char literal or lifetime
        if c == '\'' {
            let start = i;
            if chars.get(i + 1) == Some(&'\\') {
                let mut k = i + 2;
                while k < chars.len() && chars[k] != '\'' {
                    k += 1;
                }
                out.push((line, chars[start..=k.min(chars.len() - 1)].iter().collect()));
                i = k + 1;
                continue;
            }
            if chars.get(i + 2) == Some(&'\'') {
                out.push((line, chars[start..i + 3].iter().collect()));
                i += 3;
                continue;
            }
            let mut k = i + 1;
            while k < chars.len() && (chars[k].is_alphanumeric() || chars[k] == '_') {
                k += 1;
            }
            out.push((line, chars[start..k].iter().collect()));
            i = k;
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push((line, chars[start..i].iter().collect()));
            continue;
        }
        out.push((line, c.to_string()));
        i += 1;
    }
    out
}

/// Maps the raw line map onto the formatted text by aligning token streams. rustfmt moves tokens
/// and adds or removes trailing commas, so a two-pointer walk that tolerates a comma on either
/// side carries most of it; on any other mismatch the nearest mapping is kept.
///
/// It also **reorders `use` declarations**, which a two-pointer walk cannot follow: the tokens of
/// the reordered run diverge, the walk keeps the last mapping through it, and every import in the
/// run then points at the wrong line of Uredo source. Those lines are repaired afterwards by
/// matching each formatted `use` to the raw `use` with the same text, which is exact because the
/// generator writes each import once.
pub fn remap(raw: &str, raw_map: &[usize], formatted: &str) -> Vec<usize> {
    let rt = tokens(raw);
    let ft = tokens(formatted);
    let n_lines = formatted.lines().count();
    let mut out = vec![0usize; n_lines];
    let mut i = 0;
    let mut j = 0;
    let mut last = 0usize;
    while j < ft.len() {
        let (fl, ftok) = &ft[j];
        let ure = if i < rt.len() {
            let (rl, rtok) = &rt[i];
            if rtok == ftok {
                i += 1;
                raw_map.get(rl - 1).copied().unwrap_or(0)
            } else if ftok == "," {
                // rustfmt added a trailing comma
                last
            } else if rtok == "," {
                // rustfmt removed a comma: skip it on the raw side and retry
                i += 1;
                continue;
            } else {
                // unexpected divergence: keep the last mapping and resync on the next token
                i += 1;
                last
            }
        } else {
            last
        };
        if ure != 0 {
            last = ure;
        }
        if *fl >= 1 && *fl <= n_lines && out[fl - 1] == 0 {
            out[fl - 1] = ure;
        }
        j += 1;
    }
    repair_reordered_imports(raw, raw_map, formatted, &mut out);
    out
}

/// rustfmt sorts `use` declarations within a group; the token walk cannot follow a permutation, so
/// each formatted import is matched to the raw import with the same text and takes its mapping.
/// A repeated import consumes matches in order, so two identical lines cannot both claim the first.
fn repair_reordered_imports(raw: &str, raw_map: &[usize], formatted: &str, out: &mut [usize]) {
    let mut raw_uses: Vec<(String, usize)> = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        let t = line.trim();
        if t.starts_with("use ") && t.ends_with(';') {
            raw_uses.push((t.to_string(), raw_map.get(i).copied().unwrap_or(0)));
        }
    }
    if raw_uses.is_empty() {
        return;
    }
    let mut taken = vec![false; raw_uses.len()];
    for (i, line) in formatted.lines().enumerate() {
        let t = line.trim();
        if !(t.starts_with("use ") && t.ends_with(';')) {
            continue;
        }
        if let Some(k) = raw_uses.iter().enumerate().position(|(k, (text, _))| !taken[k] && text == t) {
            taken[k] = true;
            if i < out.len() && raw_uses[k].1 != 0 {
                out[i] = raw_uses[k].1;
            }
        }
    }
}
