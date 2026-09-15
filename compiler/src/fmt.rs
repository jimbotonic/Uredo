//! `uredo fmt` (§29): the canonical formatter.
//!
//! Token-based: every physical line is rebuilt from its tokens with canonical spacing, comments
//! are kept where they were (trailing comments keep an alignment of two or more spaces), and
//! indentation is recomputed from the lexer's block structure — statement lines from the
//! `Indent`/`Dedent` tokens, continuation lines from the line that opened the innermost
//! unclosed delimiter (closers return to that line's indentation, which is also the D51
//! anchor rule). Blank lines collapse to one; items at module level are separated by one blank
//! line; multi-line calls and literals get a trailing comma; an inline body or a same-line
//! `else` that no longer fits 100 columns is expanded to the indented form. `rust { }` blocks
//! and macro bodies are left exactly as written.

use crate::diag::Diag;
use crate::lexer::{self, Tok, Token};

const WIDTH: usize = 100;


const ITEM_KEYWORDS: &[&str] = &["fn", "pub", "struct", "enum", "impl", "trait", "mod", "macro_rules", "const", "static", "type", "async", "unsafe", "use"];

const UNARY_AFTER_KEYWORDS: &[&str] = &[
    "return", "in", "if", "else", "match", "while", "throw", "break", "move", "take", "inout", "as", "let", "var", "for", "loop", "throws", "where", "mut",
];

/// One physical source line with its tokens (excluding layout tokens) and its comment.
struct Line {
    number: usize,
    toks: Vec<Token>,
    /// A `#` comment on this line: (spaces before it in the source, text from `#`).
    comment: Option<(usize, String)>,
    /// Original indentation (spaces).
    orig_indent: usize,
    /// Layout tokens that preceded the first token of this line.
    indents: i32,
    /// True when the line is part of a multi-line raw token (rust block, macro body, string).
    verbatim: bool,
    /// Whether this line is a doc comment (`##`/`##!`) line.
    doc: Option<String>,
    blank: bool,
}

pub fn format(src: &str) -> Result<String, Vec<Diag>> {
    let (toks, diags) = lexer::lex(src);
    if diags.iter().any(|d| d.level == crate::diag::Level::Error) {
        return Err(diags);
    }
    let src_lines: Vec<&str> = src.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let lines = collect_lines(&src_lines, &toks);
    let out = layout(&lines, &src_lines);
    Ok(out)
}

fn collect_lines<'a>(src_lines: &[&'a str], toks: &[Token]) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    // which physical lines are covered by a multi-line raw token
    let mut covered = vec![false; src_lines.len() + 2];
    for t in toks {
        if let Tok::RustBlock(text) | Tok::MacroBody(text) | Tok::Str(text) = &t.tok {
            let n = text.matches('\n').count();
            for l in t.line + 1..=t.line + n {
                if l < covered.len() {
                    covered[l] = true;
                }
            }
        }
    }
    let mut pending_indents: i32 = 0;
    let mut by_line: std::collections::BTreeMap<usize, Vec<Token>> = Default::default();
    let mut indents_at: std::collections::BTreeMap<usize, i32> = Default::default();
    for t in toks {
        match &t.tok {
            Tok::Indent => pending_indents += 1,
            Tok::Dedent => pending_indents -= 1,
            Tok::Newline | Tok::Blank | Tok::Eof => {}
            _ => {
                if !by_line.contains_key(&t.line) {
                    indents_at.insert(t.line, pending_indents);
                    pending_indents = 0;
                }
                by_line.entry(t.line).or_default().push(t.clone());
            }
        }
    }
    for (i, l) in src_lines.iter().enumerate() {
        let number = i + 1;
        let orig_indent = l.len() - l.trim_start().len();
        let trimmed = l.trim();
        if covered[number] {
            // continuation of a multi-line raw token: its text is printed with the token
            lines.push(Line { number, toks: vec![], comment: None, orig_indent, indents: 0, verbatim: true, doc: None, blank: true });
            continue;
        }
        if trimmed.is_empty() {
            lines.push(Line { number, toks: vec![], comment: None, orig_indent, indents: 0, verbatim: false, doc: None, blank: true });
            continue;
        }
        if trimmed.starts_with("##") {
            lines.push(Line { number, toks: vec![], comment: None, orig_indent, indents: indents_at.get(&number).copied().unwrap_or(0), verbatim: false, doc: Some(trimmed.to_string()), blank: false });
            continue;
        }
        if trimmed.starts_with('#') {
            lines.push(Line { number, toks: vec![], comment: Some((0, trimmed.to_string())), orig_indent, indents: 0, verbatim: false, doc: None, blank: false });
            continue;
        }
        let ltoks = by_line.remove(&number).unwrap_or_default();
        // trailing comment: after the last token's end, a `#` outside strings
        let code_end = ltoks.last().map(|t| t.end).unwrap_or(0);
        let mut comment = None;
        if let Some(rest) = l.get(code_end..) {
            if let Some(p) = rest.find('#') {
                let before = &rest[..p];
                if before.trim().is_empty() {
                    comment = Some((before.len(), rest[p..].trim_end().to_string()));
                }
            }
        }
        lines.push(Line { number, toks: ltoks, comment, orig_indent, indents: indents_at.get(&number).copied().unwrap_or(0), verbatim: false, doc: None, blank: false });
    }
    lines
}

struct Opener {
    kind: char,
    /// indentation assigned to the line that opened it
    line_indent: usize,
    opened_on: usize,
    /// number of formatted lines emitted since it opened whose last token was not `,`
    last_line_ends_with_comma: bool,
    /// the opener started a trailing block argument (D51): its body lines are statements
    is_block_arg: bool,
}

fn layout(lines: &[Line], _src_lines: &[&str]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut stack: Vec<usize> = vec![0]; // statement indentation levels
    let mut openers: Vec<Opener> = Vec::new();
    let mut last_code_line = 0usize;
    // the statement being continued starts a block (`for`/`if`/`while`/`match`), so its
    // continuation lines must clear the body's indentation
    let mut stmt_opens_block = false;
    let mut stmt_indent = 0usize;
    let mut prev_blank = true; // start of file counts as blank
    let mut prev_item_kind: Option<&'static str> = None;
    let mut prev_was_body = false;
    // comment and doc lines are emitted at the indentation of the next code line, because the
    // lexer's Indent token arrives with that line
    let mut pending: Vec<(bool, String)> = Vec::new(); // (is_doc, text)
    // the indentation at which a `match`'s arms sit, innermost last
    let mut match_arms: Vec<usize> = Vec::new();
    let mut enum_body_indent: Option<usize> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        i += 1;
        if line.verbatim {
            // already emitted as part of the raw token on its first line
            continue;
        }
        if line.blank {
            if !pending.is_empty() {
                // a blank after comments: flush them at the current statement level
                let ind = if openers.is_empty() { *stack.last().unwrap() } else { openers.last().unwrap().line_indent + 4 };
                for (_, t) in pending.drain(..) {
                    out.push(format!("{}{}", " ".repeat(ind), t));
                }
                prev_blank = false;
            }
            if !prev_blank {
                out.push(String::new());
            }
            prev_blank = true;
            continue;
        }
        // indentation bookkeeping from layout tokens
        for _ in 0..line.indents.max(0) {
            // a block belongs to the line that opened it: the statement's first line, or —
            // for a trailing block argument (D51) — the header line inside the call
            let base = match openers.last() {
                // only the block the header line itself opened is anchored to that line;
                // blocks nested inside it are ordinary statements
                Some(o) if o.is_block_arg && o.opened_on == last_code_line => o.line_indent,
                _ => stmt_indent,
            };
            stack.push(base + 4);
        }
        for _ in 0..(-line.indents).max(0) {
            if stack.len() > 1 {
                stack.pop();
            }
        }
        if line.indents != 0 {
            stmt_indent = *stack.last().unwrap();
        }
        if let Some(doc) = &line.doc {
            pending.push((true, doc.clone()));
            continue;
        }
        if line.toks.is_empty() {
            pending.push((false, line.comment.clone().unwrap().1));
            continue;
        }
        let first = &line.toks[0];
        let starts_with_closer = matches!(&first.tok, Tok::Punct(")") | Tok::Punct("]") | Tok::Punct("}"));
        let continuation = !openers.is_empty() && !starts_with_closer && line.indents == 0 && !is_statement_start(line, &openers);
        let leading_cont = openers.is_empty() && line.indents == 0 && starts_with_leading_continuer(first) && !prev_blank;
        let indent = if starts_with_closer && !openers.is_empty() {
            // closing line: the opener's line indentation (D51 anchor, rustfmt style)
            openers.last().unwrap().line_indent
        } else if continuation {
            openers.last().unwrap().line_indent + 4
        } else if leading_cont {
            // a continuation of a block header goes two levels deep, so that it cannot be
            // confused with the body that follows at one level
            stmt_indent + if stmt_opens_block { 8 } else { 4 }
        } else {
            let s = *stack.last().unwrap();
            stmt_indent = s;
            stmt_opens_block = matches!(&line.toks[0].tok, Tok::Ident(w) if ["for", "if", "while", "match", "else", "loop"].contains(&w.as_str()))
                || matches!(&line.toks[0].tok, Tok::Lifetime(_));
            s
        };
        // trailing comma before a closing line of a multi-line call/literal
        if starts_with_closer {
            if let Some(o) = openers.last() {
                if o.opened_on != line.number && !o.is_block_arg && !o.last_line_ends_with_comma && o.kind != '{' || (o.kind == '{' && o.opened_on != line.number && !o.is_block_arg && !o.last_line_ends_with_comma) {
                    if let Some(prev) = out.last_mut() {
                        let t = prev.trim_end();
                        if !t.ends_with(',') && !t.is_empty() && !t.contains("..") && !t.ends_with('(') && !t.ends_with('[') && !t.ends_with('{') {
                            // don't add after a struct-update base `..x` or after a comment
                            if !t.contains('#') {
                                *prev = format!("{},", t);
                            }
                        }
                    }
                }
            }
        }
        // item separation at module level (a doc/attr block belongs to the item after it)
        let kind = item_kind(line);
        if indent == 0 && openers.is_empty() {
            let k = if pending.iter().any(|(d, _)| *d) { kind.unwrap_or("fn") } else { kind.unwrap_or("stmt") };
            maybe_item_blank(&mut out, &mut prev_blank, prev_was_body, &mut prev_item_kind, k, indent);
        }
        for (_, t) in pending.drain(..) {
            out.push(format!("{}{}", " ".repeat(indent), t));
        }
        // enum variant lines are type lines (`Cons(i32, Box<List>)`), and so is a `use` item: a path
        // there can carry type arguments (`@copy use nalgebra::Vector3<f64>`, D56) and never a
        // comparison, so `<` must stay tight
        let is_use = line.toks.iter().take(2).any(|t| matches!(&t.tok, Tok::Ident(w) if w == "use"));
        let is_variant = is_use
            || (enum_body_indent == Some(indent)
                && !matches!(&line.toks[0].tok, Tok::Ident(w) if ["fn", "pub", "impl", "async", "unsafe"].contains(&w.as_str())));
        if matches!(&line.toks[0].tok, Tok::Ident(w) if w == "enum") || (matches!(&line.toks[0].tok, Tok::Ident(w) if w == "pub") && matches!(line.toks.get(1).map(|t| &t.tok), Some(Tok::Ident(w)) if w == "enum")) {
            enum_body_indent = Some(indent + 4);
        } else if enum_body_indent.map(|e| indent < e).unwrap_or(false) {
            enum_body_indent = None;
        }
        let (text, ends_with_comma) = render(&line.toks, &mut openers, indent, line.number, is_variant);
        let mut full = format!("{}{}", " ".repeat(indent), text);
        if let Some((spaces, c)) = &line.comment {
            let gap = if *spaces >= 2 { *spaces } else { 2 };
            full = format!("{}{}{}", full, " ".repeat(gap), c);
        }
        // A `match` arm is the one inline body whose *lowering* depends on how it was written:
        // §13.1 gives `pat: expr` a bare expression and `pat:` with an indented body a block, so
        // expanding an over-wide arm would change the generated Rust. Every other inline body —
        // an `if`, an `else` — lowers the same either way and may be expanded.
        while match_arms.last().map(|a| indent < *a).unwrap_or(false) {
            match_arms.pop();
        }
        let is_match_arm = match_arms.last() == Some(&indent);
        if line.toks.last().map(|t| matches!(&t.tok, Tok::Punct(":"))).unwrap_or(false)
            && line.toks.iter().any(|t| matches!(&t.tok, Tok::Ident(w) if w == "match"))
        {
            match_arms.push(indent + 4);
        }
        // expand an inline body that no longer fits (§29)
        let expanded = if full.chars().count() > WIDTH && line.comment.is_none() && !is_match_arm {
            expand_inline(&line.toks, indent)
        } else {
            None
        };
        match expanded {
            Some(ls) => out.extend(ls),
            None => out.push(full),
        }
        if let Some(o) = openers.last_mut() {
            o.last_line_ends_with_comma = ends_with_comma;
        }
        last_code_line = line.number;
        prev_blank = false;
        prev_was_body = indent > 0 || !openers.is_empty();
    }
    for (_, t) in pending.drain(..) {
        out.push(format!("{}{}", " ".repeat(*stack.last().unwrap()), t));
    }
    while out.last().map(|l| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

fn maybe_item_blank(out: &mut Vec<String>, prev_blank: &mut bool, prev_was_body: bool, prev_item_kind: &mut Option<&'static str>, kind: &'static str, indent: usize) {
    if indent != 0 || out.is_empty() {
        *prev_item_kind = Some(kind);
        return;
    }
    let starts_item = kind != "stmt";
    let same_group = matches!((*prev_item_kind, kind), (Some("use"), "use") | (Some("const"), "const") | (Some("type"), "type") | (Some("doc"), _) | (Some("attr"), _)) || (!prev_was_body && *prev_item_kind == Some(kind) && kind == "struct");
    if starts_item && !*prev_blank && !same_group && (prev_was_body || *prev_item_kind != Some(kind) || kind == "fn" || kind == "struct" || kind == "impl" || kind == "enum" || kind == "trait") {
        out.push(String::new());
        *prev_blank = true;
    }
    *prev_item_kind = Some(kind);
}

fn item_kind(line: &Line) -> Option<&'static str> {
    match &line.toks[0].tok {
        Tok::Attr { .. } => Some("attr"),
        Tok::Ident(w) => {
            let w = w.as_str();
            if w == "pub" {
                if let Some(t) = line.toks.get(1) {
                    if let Tok::Ident(w2) = &t.tok {
                        return Some(kind_of(w2));
                    }
                }
                return Some("fn");
            }
            if ITEM_KEYWORDS.contains(&w) { Some(kind_of(w)) } else { None }
        }
        Tok::RustBlock(_) => Some("rust"),
        _ => None,
    }
}

fn kind_of(w: &str) -> &'static str {
    match w {
        "use" => "use",
        "const" | "static" => "const",
        "struct" => "struct",
        "enum" => "enum",
        "impl" => "impl",
        "trait" => "trait",
        "mod" => "mod",
        "type" => "type",
        _ => "fn",
    }
}

fn is_statement_start(line: &Line, openers: &[Opener]) -> bool {
    // inside a trailing block argument the lexer emits Indent tokens for the first statement;
    // later statements arrive with indents == 0 but are not continuations: they are at block level
    openers.last().map(|o| o.is_block_arg).unwrap_or(false) && line.indents == 0 && line.orig_indent > openers.last().unwrap().line_indent
}

fn starts_with_leading_continuer(t: &Token) -> bool {
    matches!(&t.tok, Tok::Punct(".") | Tok::Punct("?") | Tok::Punct("+") | Tok::Punct("/") | Tok::Punct("%") | Tok::Punct("&&") | Tok::Punct("||") | Tok::Punct("|") | Tok::Punct("^") | Tok::Punct("==") | Tok::Punct("!=") | Tok::Punct("<=") | Tok::Punct(">=") | Tok::Punct("<") | Tok::Punct(">") | Tok::Punct("<<") | Tok::Punct(">>"))
}

/// Renders one line's tokens with canonical spacing; updates the opener stack.
/// Returns the text and whether it ends with a comma.
fn render(toks: &[Token], openers: &mut Vec<Opener>, line_indent: usize, line_no: usize, type_line: bool) -> (String, bool) {
    let mut out = String::new();
    let mut prev: Option<&Tok> = None;
    let mut type_ctx = type_line; // after `:` / `->` in a signature or annotation, until `=`, `,`, `)`
    let mut generic_depth = 0i32;
    // is this a `fn`/`struct`/`impl`/`trait`/`type` header line? (types on the right of `:` and `->`)
    let header = matches!(&toks[0].tok, Tok::Ident(w) if ["fn", "pub", "struct", "impl", "trait", "type", "enum", "async", "unsafe", "const", "static"].contains(&w.as_str()));
    let mut use_brace = false;
    for (idx, t) in toks.iter().enumerate() {
        let next = toks.get(idx + 1).map(|t| &t.tok);
        let text = tok_text(&t.tok);
        let space_before = match &t.tok {
            Tok::Punct(p) => {
                let p = *p;
                match p {
                    "(" | "[" => {
                        // call/index: tight after an ident/closer/`>`/`?`; a space after keywords and operators
                        match prev {
                            None => false,
                            Some(Tok::Ident(w)) => is_keyword_before_paren(w) && !(idx >= 2 && matches!(&toks[idx - 2].tok, Tok::Punct(".") | Tok::Punct("::"))),
                            Some(Tok::Punct("<")) | Some(Tok::Punct(">>")) | Some(Tok::Punct("..")) | Some(Tok::Punct("..=")) => false,
                            Some(Tok::Punct(")")) | Some(Tok::Punct("]")) | Some(Tok::Punct("?")) | Some(Tok::Punct(">")) | Some(Tok::Punct("!")) | Some(Tok::Punct(".")) | Some(Tok::Punct("::")) | Some(Tok::Punct("&")) | Some(Tok::Punct("*")) | Some(Tok::Punct("(")) | Some(Tok::Punct("[")) | Some(Tok::Punct("-")) => {
                                // `(` after a closer is a call `f()(…)`; after `!` it's a macro; after `&`/`*`/`-` unary
                                match prev {
                                    Some(Tok::Punct("-")) => !is_unary_prev(toks.get(idx.wrapping_sub(2)).map(|t| &t.tok)),
                                    _ => false,
                                }
                            }
                            Some(Tok::Str(_)) | Some(Tok::Int(_)) | Some(Tok::Float(_)) | Some(Tok::Char(_)) => true,
                            Some(Tok::Attr { .. }) => true,
                            Some(Tok::MacroBody(_)) | Some(Tok::RustBlock(_)) => true,
                            Some(Tok::Lifetime(_)) => true,
                            _ => true,
                        }
                    }
                    "{" => !matches!(prev, Some(Tok::Punct("::"))),
                    ")" | "]" => false,
                    "}" => !matches!(prev, Some(Tok::Punct("{"))) && !use_brace,
                    ".." | "..=" => matches!(prev, Some(Tok::Punct(",")) | Some(Tok::Punct("{")) | Some(Tok::Punct("@")) | Some(Tok::Punct("=")) | Some(Tok::Punct("=>"))) || matches!(prev, Some(Tok::Ident(w)) if is_keyword_before_paren(w)),
                    "?" => matches!(prev, Some(Tok::Punct("+")) | Some(Tok::Punct(":")) | Some(Tok::Punct(","))) && (type_ctx || generic_depth > 0),
                    "," | ";" | "." | "::" => false,
                    ":" => false,
                    "@" => true,
                    "!" => match prev {
                        // `path!(…)` and `macro_rules! name`: tight; `!x` unary otherwise
                        Some(Tok::Ident(w)) if !UNARY_AFTER_KEYWORDS.contains(&w.as_str()) && matches!(next, Some(Tok::MacroBody(_)) | Some(Tok::Ident(_))) => false,
                        _ => space_before_unary(toks, idx),
                    },
                    "&" | "*" | "-" => {
                        if is_unary_prev(prev) { space_before_unary(toks, idx) } else { true }
                    }
                    "<" => {
                        if type_ctx || generic_depth > 0 || matches!(prev, Some(Tok::Punct("::"))) || (header && matches!(prev, Some(Tok::Ident(_)))) { false } else { true }
                    }
                    ">" | ">>" | "<<" => {
                        if type_ctx || generic_depth > 0 { false } else { true }
                    }
                    "=" | "==" | "!=" | "<=" | ">=" | "&&" | "||" | "|" | "^" | "+" | "/" | "%" | "+=" | "-=" | "*=" | "/=" | "%=" | "^=" | "&=" | "|=" | "<<=" | ">>=" | "->" | "=>" => true,
                    _ => true,
                }
            }
            Tok::Ident(w) => match prev {
                None => false,
                Some(Tok::Punct(p)) => match *p {
                    "(" | "[" | "." | "::" | "@" => false,
                    "!" => idx >= 2 && matches!(&toks[idx - 2].tok, Tok::Ident(m) if m == "macro_rules"),
                    "?" => !(type_ctx || generic_depth > 0),
                    "{" => !use_brace,
                    "&" | "*" | "-" => !is_unary_prev(toks.get(idx.wrapping_sub(2)).map(|t| &t.tok)) && !is_unary_use(toks, idx - 1),
                    "<" => !(type_ctx || generic_depth > 0),
                    ".." | "..=" => false,
                    ":" => true,
                    _ => true,
                },
                Some(Tok::Lifetime(_)) => w != "mut" || true,
                Some(Tok::Attr { .. }) => true,
                _ => true,
            },
            Tok::Lifetime(_) => match prev {
                None => false,
                Some(Tok::Punct("&")) | Some(Tok::Punct("<")) | Some(Tok::Punct("(")) | Some(Tok::Punct(",")) => !matches!(prev, Some(Tok::Punct("&")) | Some(Tok::Punct("<")) | Some(Tok::Punct("("))),
                Some(Tok::Punct("::")) => false,
                _ => true,
            },
            Tok::Int(_) | Tok::Float(_) | Tok::Str(_) | Tok::Char(_) => match prev {
                None => false,
                Some(Tok::Punct(p)) => match *p {
                    "(" | "[" | "." | "::" | "!" | ".." | "..=" => false,
                    "{" => true,
                    "-" | "&" | "*" => !is_unary_use(toks, idx - 1),
                    _ => true,
                },
                _ => true,
            },
            Tok::Attr { .. } => prev.is_some(),
            // `name! { … }` keeps a space before a brace body (rustfmt's style); `(`/`[` are tight
            Tok::MacroBody(b) => b.starts_with('{') || (idx >= 2 && matches!(&toks[idx - 2].tok, Tok::Punct("!")) && idx >= 3 && matches!(&toks[idx - 3].tok, Tok::Ident(m) if m == "macro_rules")),
            Tok::RustBlock(_) => !matches!(prev, None | Some(Tok::Punct("(")) | Some(Tok::Punct("[")) | Some(Tok::Punct("!")) | Some(Tok::Punct(".")) | Some(Tok::Punct("::"))),
            _ => prev.is_some(),
        };
        if space_before && !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&text);
        // context updates
        match &t.tok {
            Tok::Punct("(") | Tok::Punct("[") | Tok::Punct("{") => {
                let kind = text.chars().next().unwrap();
                if kind == '{' && matches!(prev, Some(Tok::Punct("::"))) {
                    use_brace = true;
                }
                openers.push(Opener { kind, line_indent, opened_on: line_no, last_line_ends_with_comma: false, is_block_arg: false });
                if kind == '(' && generic_depth == 0 && !type_line {
                    type_ctx = false;
                }
            }
            Tok::Punct(")") | Tok::Punct("]") | Tok::Punct("}") => {
                openers.pop();
                if generic_depth == 0 && !type_line {
                    type_ctx = false;
                }
                if matches!(&t.tok, Tok::Punct("}")) {
                    use_brace = false;
                }
            }
            Tok::Punct(":") => {
                // `x: T` / `-> T` type context; a header colon at line end is handled below
                if next.is_some() {
                    type_ctx = true;
                }
            }
            Tok::Punct("->") => type_ctx = true,
            Tok::Punct("=") | Tok::Punct(",") => {
                if generic_depth == 0 && !type_line {
                    type_ctx = false;
                }
            }
            Tok::Punct("<") => {
                if type_ctx || generic_depth > 0 || matches!(prev, Some(Tok::Punct("::"))) || (header && matches!(prev, Some(Tok::Ident(_)))) {
                    generic_depth += 1;
                }
            }
            Tok::Punct(">") => {
                if generic_depth > 0 {
                    generic_depth -= 1;
                }
            }
            Tok::Punct(">>") => {
                if generic_depth > 0 {
                    generic_depth = (generic_depth - 2).max(0);
                }
            }
            Tok::Ident(w) if w == "impl" || w == "struct" || w == "enum" || w == "trait" || w == "type" || w == "where" || w == "as" => type_ctx = true,
            Tok::Ident(w) if w == "fn" => type_ctx = false,
            _ => {}
        }
        prev = Some(&t.tok);
    }
    // a trailing block argument opens here (D51): last token `=>` or a header colon inside `(`
    if let Some(last) = toks.last() {
        let opens = matches!(&last.tok, Tok::Punct("=>")) || (matches!(&last.tok, Tok::Punct(":")) && header_keyword_on_line(toks));
        if opens {
            if let Some(o) = openers.last_mut() {
                if o.kind == '(' {
                    o.is_block_arg = true;
                }
            }
        }
    }
    let ends_with_comma = matches!(toks.last().map(|t| &t.tok), Some(Tok::Punct(",")));
    (out, ends_with_comma)
}

fn header_keyword_on_line(toks: &[Token]) -> bool {
    // first identifier after the last unclosed `(` on this line, or after the last comma at that level
    let mut depth = 0i32;
    let mut cut = 0usize;
    for (i, t) in toks.iter().enumerate() {
        match &t.tok {
            Tok::Punct("(") | Tok::Punct("[") | Tok::Punct("{") => {
                depth += 1;
                cut = i + 1;
            }
            Tok::Punct(")") | Tok::Punct("]") | Tok::Punct("}") => depth -= 1,
            Tok::Punct(",") if depth > 0 => cut = i + 1,
            _ => {}
        }
    }
    let _ = depth;
    matches!(toks.get(cut).map(|t| &t.tok), Some(Tok::Ident(w)) if w == "match" || w == "if")
}

fn is_keyword_before_paren(w: &str) -> bool {
    matches!(w, "if" | "while" | "for" | "in" | "match" | "return" | "throw" | "break" | "let" | "var" | "move" | "take" | "inout" | "as" | "else" | "impl" | "dyn" | "mut" | "where" | "loop" | "unsafe" | "throws" | "use" | "mod")
}

fn is_unary_prev(prev: Option<&Tok>) -> bool {
    match prev {
        None => true,
        Some(Tok::Punct(p)) => !matches!(*p, ")" | "]" | "}" | "?"),
        Some(Tok::Ident(w)) => UNARY_AFTER_KEYWORDS.contains(&w.as_str()),
        Some(Tok::Attr { .. }) => true,
        _ => false,
    }
}

/// Whether the operator token at index `i` is used as a unary operator.
fn is_unary_use(toks: &[Token], i: usize) -> bool {
    let prev = if i == 0 { None } else { toks.get(i - 1).map(|t| &t.tok) };
    is_unary_prev(prev)
}

/// Space before a unary operator at `idx`: none after an opener or another unary operator,
/// one after a binary operator, a keyword or an operand.
fn space_before_unary(toks: &[Token], idx: usize) -> bool {
    if idx == 0 {
        return false;
    }
    match &toks[idx - 1].tok {
        Tok::Punct(p) => match *p {
            "(" | "[" | "!" | "." | "::" | "<" => false,
            "&" | "*" | "-" => !is_unary_use(toks, idx - 1),
            _ => true,
        },
        _ => true,
    }
}

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Ident(s) | Tok::Lifetime(s) | Tok::Int(s) | Tok::Float(s) | Tok::Str(s) | Tok::Char(s) => s.clone(),
        Tok::Punct(p) => p.to_string(),
        Tok::Attr { inner, name, args, value } => format!(
            "@{}{}{}{}",
            if *inner { "!" } else { "" },
            name,
            args.as_ref().map(|a| format!("({})", a)).unwrap_or_default(),
            value.as_ref().map(|v| format!(" = {}", v)).unwrap_or_default()
        ),
        Tok::RustBlock(s) => if s.starts_with('{') { format!("rust {}", s) } else { s.clone() },
        Tok::MacroBody(s) => s.clone(),
        Tok::Doc(s) => format!("## {}", s),
        Tok::InnerDoc(s) => format!("##! {}", s),
        _ => String::new(),
    }
}

/// Expands `header: inline body [else: inline body]` onto separate lines when too long (§29).
fn expand_inline(toks: &[Token], indent: usize) -> Option<Vec<String>> {
    // find the header colon at depth 0 that is followed by more tokens
    let mut depth = 0i32;
    let mut colon = None;
    // in a signature, `<…>` is a generic list: the colons inside it are bounds, not headers
    let signature = matches!(&toks[0].tok, Tok::Ident(w) if ["fn", "pub", "async", "unsafe", "struct", "enum", "impl", "trait", "type"].contains(&w.as_str()));
    for (i, t) in toks.iter().enumerate() {
        match &t.tok {
            Tok::Punct("(") | Tok::Punct("[") | Tok::Punct("{") => depth += 1,
            Tok::Punct(")") | Tok::Punct("]") | Tok::Punct("}") => depth -= 1,
            Tok::Punct("<") if signature => depth += 1,
            Tok::Punct(">") if signature => depth -= 1,
            Tok::Punct(">>") if signature => depth -= 2,
            Tok::Punct(":") if depth == 0 && i + 1 < toks.len() => {
                // a type annotation colon is followed by a type then `=`; a header colon is not
                let rest = &toks[i + 1..];
                if !rest.iter().any(|t| matches!(&t.tok, Tok::Punct("="))) || matches!(&toks[0].tok, Tok::Ident(w) if ["if", "else", "for", "while", "fn", "pub"].contains(&w.as_str())) {
                    colon = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let c = colon?;
    // Only a whole statement may be expanded. A *continuation* line inside a call — `if ok: "a"
    // else: "b")`, the last argument of a wrapped call — starts with a header word and holds a
    // colon at depth 0, but breaking it after that colon produces text that is not a program at
    // all. A statement's brackets balance; a continuation's do not, so that is the test.
    let mut d = 0i32;
    let mut lowest = 0i32;
    for t in toks {
        match &t.tok {
            Tok::Punct("(") | Tok::Punct("[") | Tok::Punct("{") => d += 1,
            Tok::Punct(")") | Tok::Punct("]") | Tok::Punct("}") => d -= 1,
            _ => {}
        }
        lowest = lowest.min(d);
    }
    if d != 0 || lowest < 0 {
        return None;
    }
    let first_is_header = matches!(&toks[0].tok, Tok::Ident(w) if ["if", "else", "for", "while", "fn", "pub", "match"].contains(&w.as_str())) || matches!(&toks[0].tok, Tok::Str(_) | Tok::Int(_) | Tok::Char(_)) || matches!(&toks[0].tok, Tok::Ident(_));
    if !first_is_header {
        return None;
    }
    let mut lines = Vec::new();
    let mut dummy = Vec::new();
    let (head, _) = render(&toks[..=c], &mut dummy, indent, 0, false);
    // same-line else: split both bodies
    let body = &toks[c + 1..];
    let else_pos = body.iter().position(|t| matches!(&t.tok, Tok::Ident(w) if w == "else"));
    lines.push(format!("{}{}", " ".repeat(indent), head));
    match else_pos {
        Some(e) if body.get(e + 1).map(|t| matches!(&t.tok, Tok::Punct(":"))).unwrap_or(false) => {
            let mut d1 = Vec::new();
            let (b1, _) = render(&body[..e], &mut d1, indent + 4, 0, false);
            lines.push(format!("{}{}", " ".repeat(indent + 4), b1));
            lines.push(format!("{}else:", " ".repeat(indent)));
            let mut d2 = Vec::new();
            let (b2, _) = render(&body[e + 2..], &mut d2, indent + 4, 0, false);
            lines.push(format!("{}{}", " ".repeat(indent + 4), b2));
        }
        _ => {
            let mut d1 = Vec::new();
            let (b1, _) = render(body, &mut d1, indent + 4, 0, false);
            lines.push(format!("{}{}", " ".repeat(indent + 4), b1));
        }
    }
    Some(lines)
}
