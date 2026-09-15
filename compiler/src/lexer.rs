//! Lexer: turns Uredo source into a token stream with INDENT/DEDENT/NEWLINE tokens (§6).
//!
//! Layout rules implemented here:
//! - 4-space indentation, tabs rejected, CRLF normalised (§6.3);
//! - a newline ends a statement unless a delimiter is open, the line ends with a
//!   continuation operator, or the next line is deeper and begins with `.`, `?` or a
//!   binary operator (§6.3);
//! - `#` comments, `##` outer docs, `##!` inner docs (§6.4);
//! - `rust { … }` and `path!(…)` switch to Rust token mode until the balanced close (§6.6).

use crate::diag::Diag;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Lifetime(String),
    Int(String),
    Float(String),
    Str(String),
    Char(String),
    Punct(&'static str),
    /// `@name(args)`, `@!name(args)` or the name-value form `@name = value` (§23): `args` is the
    /// raw text between the parentheses, `value` the raw text after `=` to the end of the line.
    Attr { inner: bool, name: String, args: Option<String>, value: Option<String> },
    /// A `##` doc line (text after `##`, one leading space stripped).
    Doc(String),
    /// A `##!` inner doc line.
    InnerDoc(String),
    /// An empty line (kept so the generator can reproduce paragraph breaks).
    Blank,
    /// Raw Rust text of a `rust { … }` block, including the braces.
    RustBlock(String),
    /// Raw text of a macro invocation body, including its delimiters.
    MacroBody(String),
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    pub col: usize,
    /// Byte range within `line` (for slicing raw text back out of the source).
    pub start: usize,
    pub end: usize,
}

const PUNCTS: &[&str] = &[
    "..=", "<<=", ">>=", "...", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "..", "+=",
    "-=", "*=", "/=", "%=", "^=", "&=", "|=", "<<", ">>", "(", ")", "[", "]", "{", "}", ",", ":",
    ";", ".", "=", "<", ">", "+", "-", "*", "/", "%", "!", "&", "|", "^", "?", "@", "$", "~",
];

/// Operators that, at the end of a line, continue the statement on the next line.
const TRAILING_CONTINUERS: &[&str] = &[
    ",", "=", "->", "+", "-", "/", "%", "&&", "||", "&", "|", "^", "<<", "==", "!=",
    "<=", ">=", "+=", "-=", "*=", "/=", "..", "..=",
];
// `*`, `<`, `>` and `>>` are deliberately absent: they end `use x::*` and generic lists.
// `=>` is absent too: a closure header ending a line opens an indented block (D51, §17).

/// Operators that, at the start of a deeper line, continue the previous statement.
const LEADING_CONTINUERS: &[&str] = &[
    ".", "?", "+", "/", "%", "&&", "||", "|", "^", "<<", ">>", "==", "!=", "<=", ">=", "<", ">",
];
// `*`, `-`, `&` and `!` are absent: a line may begin with a deref, a negation or a borrow.

pub struct Lexer<'a> {
    src: &'a str,
    lines: Vec<&'a str>,
    pub tokens: Vec<Token>,
    diags: Vec<Diag>,
    indent_stack: Vec<usize>,
    /// Open `(`/`[`/`{` count across lines.
    depth: i32,
    /// Depth of an open turbofish `::<`.
    generic_depth: i32,
    /// The previous logical line ended with a trailing continuation operator.
    trailing_cont: bool,
    /// Newline/Blank tokens emitted but not yet committed (dropped by a leading continuation).
    pending: Vec<Token>,
    statement_indent: usize,
    in_statement: bool,
    /// Kinds of the unclosed delimiters (`(`, `[`, `{`), innermost last.
    delims: Vec<char>,
    /// Where the outermost unclosed delimiter was opened, for the diagnostic.
    unclosed_at: usize,
    unclosed_col: usize,
    /// Trailing block arguments currently open (G9, D51), innermost last.
    block_args: Vec<BlockArg>,
}

/// A trailing block argument: `f(a, x =>` or `f(match e:` at the end of a line inside an
/// unclosed `(`/`[` opens an indented block laid out like any other; the block ends at the
/// first line at the header line's indentation, which must start with the closing delimiter.
struct BlockArg {
    anchor: usize,
    saved_depth: i32,
    saved_generic: i32,
    saved_statement_indent: usize,
    saved_in_statement: bool,
    saved_delims: Vec<char>,
    pushed_anchor: bool,
    header_line: usize,
    /// The header is an `if`: an `else:` at the anchor continues it instead of closing.
    is_if: bool,
}

pub fn lex(src: &str) -> (Vec<Token>, Vec<Diag>) {
    let mut lx = Lexer {
        src,
        lines: src.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect(),
        tokens: Vec::new(),
        diags: Vec::new(),
        indent_stack: vec![0],
        depth: 0,
        generic_depth: 0,
        unclosed_at: 0,
        unclosed_col: 0,
        trailing_cont: false,
        pending: Vec::new(),
        statement_indent: 0,
        in_statement: false,
        delims: Vec::new(),
        block_args: Vec::new(),
    };
    lx.run();
    (lx.tokens, lx.diags)
}

fn is_ident_start(c: char) -> bool {
    c == '_' || unicode_ident_start(c)
}
fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}
fn unicode_ident_start(c: char) -> bool {
    c.is_alphabetic()
}

impl<'a> Lexer<'a> {
    fn err(&mut self, line: usize, col: usize, msg: impl Into<String>) {
        self.diags.push(Diag::error(line, col, msg));
    }

    fn push(&mut self, tok: Tok, line: usize, start: usize, end: usize) {
        self.tokens.push(Token { tok, line, col: start + 1, start, end });
    }

    fn flush_pending(&mut self) {
        let p = std::mem::take(&mut self.pending);
        self.tokens.extend(p);
    }

    /// Commits pending tokens around an indentation change: statement-ending newlines first,
    /// then the Indent/Dedent tokens, then blank lines — so that a blank line before a
    /// dedented item belongs to that item, not to the block that ended.
    fn flush_and_indent(&mut self, indent: usize, lineno: usize) {
        let p = std::mem::take(&mut self.pending);
        let (newlines, blanks): (Vec<Token>, Vec<Token>) = p.into_iter().partition(|t| matches!(t.tok, Tok::Newline));
        self.tokens.extend(newlines);
        self.handle_indent(indent, lineno);
        self.tokens.extend(blanks);
    }

    fn run(&mut self) {
        let n = self.lines.len();
        let mut i = 0;
        while i < n {
            let line = self.lines[i];
            let lineno = i + 1;
            // Indentation.
            let mut indent = 0;
            for c in line.chars() {
                match c {
                    ' ' => indent += 1,
                    '\t' => {
                        self.err(lineno, indent + 1, "tabs are not allowed for indentation (§6.3)");
                        indent += 4;
                    }
                    _ => break,
                }
            }
            let content = &line[line.char_indices().nth(indent).map(|(b, _)| b).unwrap_or(line.len())..];
            let content_trim = content.trim_end();

            // Blank and comment-only lines.
            if content_trim.is_empty() || content_trim.starts_with('#') {
                if content_trim.starts_with("##!") {
                    let text = strip_doc(&content_trim[3..]);
                    if self.depth == 0 && !self.trailing_cont {
                        self.flush_and_indent(indent, lineno);
                    }
                    self.push(Tok::InnerDoc(text), lineno, indent, line.len());
                    self.push(Tok::Newline, lineno, line.len(), line.len());
                } else if content_trim.starts_with("##") {
                    let text = strip_doc(&content_trim[2..]);
                    if self.depth == 0 && !self.trailing_cont {
                        self.flush_and_indent(indent, lineno);
                    }
                    self.push(Tok::Doc(text), lineno, indent, line.len());
                    self.push(Tok::Newline, lineno, line.len(), line.len());
                } else if content_trim.is_empty() && self.depth == 0 && !self.trailing_cont {
                    self.pending.push(Token { tok: Tok::Blank, line: lineno, col: 1, start: 0, end: 0 });
                }
                i += 1;
                continue;
            }

            // End of a trailing block argument (D51): a line at or above the anchor closes it,
            // unless it is the `else` of an `if` header (which stays at the anchor).
            if let Some(b) = self.block_args.last() {
                let at_anchor = indent <= b.anchor && !(b.is_if && indent == b.anchor && starts_with_word(content_trim, "else"));
                // a deeper line that begins with the closing delimiter is a misplaced closing line
                let misplaced_close = indent > b.anchor && content_trim.starts_with(')');
                let closes = self.depth == 0 && self.generic_depth == 0 && !self.trailing_cont && (at_anchor || misplaced_close);
                if closes {
                    let b = self.block_args.pop().unwrap();
                    self.flush_and_indent(b.anchor, lineno);
                    if b.pushed_anchor {
                        self.indent_stack.pop();
                    }
                    self.depth = b.saved_depth;
                    self.generic_depth = b.saved_generic;
                    self.statement_indent = b.saved_statement_indent;
                    self.in_statement = b.saved_in_statement;
                    self.delims = b.saved_delims;
                    let expected = ")";
                    if indent != b.anchor || !content_trim.starts_with(expected) {
                        self.err(lineno, indent + 1, format!("expected `{}` closing the block argument opened on line {}, at that line's indentation (D51)", expected, b.header_line));
                    }
                    let consumed = self.lex_line(i, indent);
                    i += consumed;
                    continue;
                }
            }

            // An unclosed bracket makes every following line a continuation, which in a layout
            // language means one missing `)` swallows the rest of the file: the error lands on the
            // next item's header and every item after it is lost. No valid program has an item at
            // column 0 inside brackets, so a line that starts one closes the run instead, and the
            // diagnostic points at the bracket rather than at the innocent line after it.
            if (self.depth > 0 || self.generic_depth > 0 || self.trailing_cont) && indent == 0 && starts_item(content_trim) {
                if self.depth > 0 || self.generic_depth > 0 {
                    let opener = self.delims.last().copied().unwrap_or('(');
                    self.err(self.unclosed_at, self.unclosed_col, format!("unclosed `{}`", opener));
                } else {
                    self.err(lineno.saturating_sub(1), 1, "this line ends in an operator, so the next line continues it — but the next line starts an item");
                }
                self.depth = 0;
                self.generic_depth = 0;
                self.delims.clear();
                self.trailing_cont = false;
            }
            let continues_previous = self.depth > 0 || self.generic_depth > 0 || self.trailing_cont;
            let leading_cont = !continues_previous
                && self.in_statement
                && indent > self.statement_indent
                && starts_with_leading_continuer(content_trim);

            if continues_previous || leading_cont {
                if leading_cont {
                    // Retract the statement-ending newline emitted for the previous line.
                    self.pending.clear();
                }
            } else {
                self.flush_and_indent(indent, lineno);
                self.statement_indent = indent;
                self.in_statement = true;
            }

            // Tokenise the content; may consume further lines for rust blocks / macros.
            let consumed = self.lex_line(i, indent);
            i += consumed;
        }
        // End of file.
        while let Some(b) = self.block_args.pop() {
            self.err(self.lines.len(), 1, format!("the block argument opened on line {} is never closed: expected `)` at that line's indentation (D51)", b.header_line));
            self.depth = b.saved_depth;
        }
        self.flush_pending();
        if self.in_statement {
            let last = self.lines.len();
            self.push(Tok::Newline, last, 0, 0);
        }
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            let last = self.lines.len();
            self.push(Tok::Dedent, last, 0, 0);
        }
        let last = self.lines.len();
        self.push(Tok::Eof, last, 0, 0);
    }

    fn handle_indent(&mut self, indent: usize, lineno: usize) {
        let top = *self.indent_stack.last().unwrap();
        if indent > top {
            if indent - top != 4 {
                self.err(lineno, 1, format!("indentation must step by 4 spaces (found {} after {})", indent, top));
            }
            self.indent_stack.push(indent);
            self.push(Tok::Indent, lineno, 0, indent);
        } else if indent < top {
            while *self.indent_stack.last().unwrap() > indent {
                self.indent_stack.pop();
                self.push(Tok::Dedent, lineno, 0, indent);
            }
            if *self.indent_stack.last().unwrap() != indent {
                self.err(lineno, 1, "inconsistent indentation: this line matches no enclosing block");
            }
        }
    }

    /// Lexes the content of line `i` (0-based). Returns the number of physical lines consumed.
    fn lex_line(&mut self, i: usize, indent: usize) -> usize {
        let line = self.lines[i];
        let lineno = i + 1;
        let bytes = line.as_bytes();
        let mut pos = line.char_indices().nth(indent).map(|(b, _)| b).unwrap_or(line.len());
        let mut consumed = 1;
        let mut last_punct: Option<&'static str> = None;
        let mut last_was_punct = false;
        while pos < line.len() {
            let c = line[pos..].chars().next().unwrap();
            if c == ' ' {
                pos += 1;
                continue;
            }
            if c == '#' {
                // trailing comment
                break;
            }
            let start = pos;
            // Attributes: `@` directly followed by a name (or `!`); a spaced `@` is the
            // pattern binding operator (`rest @ ..`).
            if c == '@' && (bytes.get(pos + 1) == Some(&b'!') || line[pos + 1..].chars().next().map(is_ident_start).unwrap_or(false)) {
                let mut p = pos + 1;
                let inner = bytes.get(p) == Some(&b'!');
                if inner {
                    p += 1;
                }
                let name_start = p;
                while p < line.len() && line[p..].chars().next().map(is_ident_continue).unwrap_or(false) {
                    p += line[p..].chars().next().unwrap().len_utf8();
                }
                let name = line[name_start..p].to_string();
                if name.is_empty() {
                    self.err(lineno, start + 1, "expected an attribute name after `@`");
                    pos = p;
                    continue;
                }
                let mut args = None;
                if bytes.get(p) == Some(&b'(') {
                    match find_balanced_rust(&self.lines, i, p) {
                        Some((end_line, end_pos, text)) => {
                            if end_line != i {
                                self.err(lineno, p + 1, "attribute arguments must be on one line");
                            }
                            args = Some(text[1..text.len() - 1].to_string());
                            p = end_pos;
                        }
                        None => {
                            self.err(lineno, p + 1, "unclosed `(` in attribute");
                            p = line.len();
                        }
                    }
                }
                // `@name = value` (§23): Rust's name-value attribute, the rest of the line verbatim
                let mut value = None;
                if args.is_none() {
                    let mut q = p;
                    while q < line.len() && line[q..].starts_with(' ') {
                        q += 1;
                    }
                    if bytes.get(q) == Some(&b'=') && bytes.get(q + 1) != Some(&b'=') {
                        let text = line[q + 1..].trim();
                        if text.is_empty() {
                            self.err(lineno, q + 1, "expected a value after `=` in the attribute");
                        }
                        value = Some(text.to_string());
                        p = line.len();
                    }
                }
                self.push(Tok::Attr { inner, name, args, value }, lineno, start, p);
                pos = p;
                last_was_punct = false;
                last_punct = None;
                continue;
            }
            // Strings, byte strings, raw strings.
            if let Some((len, kind)) = string_prefix(&line[pos..]) {
                match lex_string(&self.lines, i, pos, kind) {
                    Some((end_line, end_pos)) => {
                        let text = if end_line == i {
                            line[pos..end_pos].to_string()
                        } else {
                            let mut t = String::new();
                            t.push_str(&self.lines[i][pos..]);
                            for l in i + 1..end_line {
                                t.push('\n');
                                t.push_str(self.lines[l]);
                            }
                            t.push('\n');
                            t.push_str(&self.lines[end_line][..end_pos]);
                            t
                        };
                        self.push(Tok::Str(text), lineno, pos, end_pos);
                        if end_line != i {
                            // Continue lexing on the end line.
                            consumed = end_line - i + 1;
                            let rest = self.lex_rest(end_line, end_pos);
                            return consumed + rest - 1;
                        }
                        pos = end_pos;
                    }
                    None => {
                        self.err(lineno, pos + 1, "unterminated string literal");
                        pos = line.len();
                    }
                }
                let _ = len;
                last_was_punct = false;
                last_punct = None;
                continue;
            }
            // Byte char literal `b'x'`.
            if c == 'b' && line[pos + 1..].starts_with('\'') {
                if let Some(end) = lex_char(line, pos + 1) {
                    self.push(Tok::Char(line[pos..end].to_string()), lineno, pos, end);
                    pos = end;
                    last_was_punct = false;
                    last_punct = None;
                    continue;
                }
            }
            // Char literal or lifetime.
            if c == '\'' {
                if let Some(end) = lex_char(line, pos) {
                    self.push(Tok::Char(line[pos..end].to_string()), lineno, pos, end);
                    pos = end;
                } else {
                    // lifetime
                    let mut p = pos + 1;
                    while p < line.len() && line[p..].chars().next().map(is_ident_continue).unwrap_or(false) {
                        p += line[p..].chars().next().unwrap().len_utf8();
                    }
                    if p == pos + 1 {
                        self.err(lineno, pos + 1, "unexpected `'`");
                        pos += 1;
                        continue;
                    }
                    self.push(Tok::Lifetime(line[pos..p].to_string()), lineno, pos, p);
                    pos = p;
                }
                last_was_punct = false;
                last_punct = None;
                continue;
            }
            // Numbers.
            if c.is_ascii_digit() {
                let (end, is_float) = lex_number(line, pos);
                let text = line[pos..end].to_string();
                self.push(if is_float { Tok::Float(text) } else { Tok::Int(text) }, lineno, pos, end);
                pos = end;
                last_was_punct = false;
                last_punct = None;
                continue;
            }
            // Identifiers, keywords, raw identifiers, `rust {`, macros.
            if is_ident_start(c) || (c == 'r' && line[pos..].starts_with("r#")) {
                let mut p = pos;
                if line[pos..].starts_with("r#") {
                    p += 2;
                }
                while p < line.len() && line[p..].chars().next().map(is_ident_continue).unwrap_or(false) {
                    p += line[p..].chars().next().unwrap().len_utf8();
                }
                let word = &line[pos..p];
                // `rust {` block; `async {` / `async move {` blocks are Rust source too (§21)
                let async_block = word == "async" && {
                    let rest = line[p..].trim_start();
                    rest.starts_with('{') || (rest.starts_with("move") && rest[4..].trim_start().starts_with('{'))
                };
                if (word == "rust" && line[p..].trim_start().starts_with('{')) || async_block {
                    let brace = p + line[p..].find('{').unwrap();
                    let prefix = if async_block { line[pos..brace].trim_end().to_string() + " " } else { String::new() };
                    match find_balanced_rust(&self.lines, i, brace) {
                        Some((end_line, end_pos, text)) => {
                            self.push(Tok::RustBlock(format!("{}{}", prefix, text)), lineno, pos, end_pos);
                            if end_line != i {
                                consumed = end_line - i + 1;
                                let rest = self.lex_rest(end_line, end_pos);
                                return consumed + rest - 1;
                            }
                            pos = end_pos;
                            last_was_punct = false;
                            last_punct = None;
                            continue;
                        }
                        None => {
                            self.err(lineno, brace + 1, "unclosed `rust {` block (§6.6)");
                            return self.lines.len() - i;
                        }
                    }
                }
                self.push(Tok::Ident(word.to_string()), lineno, pos, p);
                pos = p;
                last_was_punct = false;
                last_punct = None;
                // Macro invocation: ident followed directly by `!` and a delimiter (§6.6).
                if line[pos..].starts_with('!') && !line[pos..].starts_with("!=") {
                    let after = &line[pos + 1..];
                    let mut open = pos + 1 + (after.len() - after.trim_start().len());
                    if word == "macro_rules" {
                        // `macro_rules! name { … }`
                        self.push(Tok::Punct("!"), lineno, pos, pos + 1);
                        let ns = open;
                        let mut q = ns;
                        while q < line.len() && line[q..].chars().next().map(is_ident_continue).unwrap_or(false) {
                            q += line[q..].chars().next().unwrap().len_utf8();
                        }
                        if q == ns {
                            self.err(lineno, ns + 1, "expected `macro_rules! name { … }`");
                            pos = q;
                            continue;
                        }
                        self.push(Tok::Ident(line[ns..q].to_string()), lineno, ns, q);
                        open = q + (line[q..].len() - line[q..].trim_start().len());
                    }
                    let delim = line[open..].chars().next();
                    if matches!(delim, Some('(') | Some('[') | Some('{')) {
                        if word != "macro_rules" {
                            self.push(Tok::Punct("!"), lineno, pos, pos + 1);
                        }
                        match find_balanced_rust(&self.lines, i, open) {
                            Some((end_line, end_pos, text)) => {
                                self.push(Tok::MacroBody(text), lineno, open, end_pos);
                                if end_line != i {
                                    consumed = end_line - i + 1;
                                    let rest = self.lex_rest(end_line, end_pos);
                                    return consumed + rest - 1;
                                }
                                pos = end_pos;
                            }
                            None => {
                                self.err(lineno, open + 1, "unclosed macro invocation");
                                return self.lines.len() - i;
                            }
                        }
                    } else if word == "macro_rules" {
                        self.err(lineno, open + 1, "expected `macro_rules! name { … }`");
                    }
                }
                continue;
            }
            // Punctuation.
            let mut matched = None;
            for p in PUNCTS {
                if line[pos..].starts_with(p) {
                    matched = Some(*p);
                    break;
                }
            }
            match matched {
                Some(p) => {
                    // Turbofish tracking: `::<` opens a generic list that suppresses newlines.
                    if p == "::" && line[pos + 2..].starts_with('<') {
                        self.push(Tok::Punct("::"), lineno, pos, pos + 2);
                        self.push(Tok::Punct("<"), lineno, pos + 2, pos + 3);
                        self.generic_depth += 1;
                        pos += 3;
                        last_was_punct = true;
                        last_punct = Some("<");
                        continue;
                    }
                    if self.generic_depth > 0 {
                        if p == "<" {
                            self.generic_depth += 1;
                        } else if p == ">" {
                            self.generic_depth -= 1;
                        } else if p == ">>" {
                            self.generic_depth -= 2;
                        }
                    }
                    match p {
                        "(" | "[" | "{" => {
                            if self.depth == 0 {
                                self.unclosed_at = lineno;
                                self.unclosed_col = pos + 1;
                            }
                            self.depth += 1;
                            self.delims.push(p.chars().next().unwrap());
                        }
                        ")" | "]" | "}" => {
                            self.depth -= 1;
                            self.delims.pop();
                            if self.depth < 0 {
                                match self.block_args.last() {
                                    Some(b) => self.err(lineno, pos + 1, format!("the block argument opened on line {} must be closed by `{}` on its own line at that line's indentation (D51)", b.header_line, p)),
                                    None => self.err(lineno, pos + 1, format!("unmatched `{}`", p)),
                                }
                                self.depth = 0;
                            }
                        }
                        _ => {}
                    }
                    self.push(Tok::Punct(p), lineno, pos, pos + p.len());
                    pos += p.len();
                    last_was_punct = true;
                    last_punct = Some(p);
                }
                None => {
                    self.err(lineno, pos + 1, format!("unexpected character `{}`", c));
                    pos += c.len_utf8();
                }
            }
        }
        // End of physical line.
        // A closure header (`=>`) or a `match`/`if` header colon at the end of a line inside an
        // unclosed `(`/`[` opens a trailing block argument (D51): layout resumes for the block.
        let header_kw = if last_was_punct && last_punct == Some(":") { line_header_keyword(line) } else { None };
        let opens_block = self.depth > 0
            && self.generic_depth == 0
            && matches!(self.delims.last(), Some('('))
            && last_was_punct
            && (last_punct == Some("=>") || header_kw.is_some());
        if opens_block {
            let top = *self.indent_stack.last().unwrap();
            let pushed_anchor = indent > top;
            if pushed_anchor {
                self.indent_stack.push(indent);
            }
            self.block_args.push(BlockArg {
                anchor: indent,
                saved_depth: self.depth,
                saved_generic: self.generic_depth,
                saved_statement_indent: self.statement_indent,
                saved_in_statement: self.in_statement,
                saved_delims: std::mem::take(&mut self.delims),
                pushed_anchor,
                header_line: lineno,
                is_if: header_kw == Some("if"),
            });
            self.depth = 0;
            self.generic_depth = 0;
            self.statement_indent = indent;
            self.in_statement = true;
            self.trailing_cont = false;
            self.pending.push(Token { tok: Tok::Newline, line: lineno, col: line.len() + 1, start: line.len(), end: line.len() });
            return consumed;
        }
        self.trailing_cont = last_was_punct && last_punct.map(|p| TRAILING_CONTINUERS.contains(&p)).unwrap_or(false);
        if self.depth == 0 && self.generic_depth == 0 && !self.trailing_cont {
            self.pending.push(Token { tok: Tok::Newline, line: lineno, col: line.len() + 1, start: line.len(), end: line.len() });
        }
        if self.generic_depth < 0 {
            self.generic_depth = 0;
        }
        consumed
    }

    /// Continues lexing a line from `pos` (after a multi-line string/rust block ended there).
    fn lex_rest(&mut self, i: usize, pos: usize) -> usize {
        // Reuse lex_line by pretending the indentation is `pos` characters.
        let line = self.lines[i];
        let indent_chars = line[..pos].chars().count();
        self.lex_line(i, indent_chars)
    }
}

fn starts_with_word(s: &str, w: &str) -> bool {
    s.starts_with(w) && s[w.len()..].chars().next().map(|c| !(c.is_alphanumeric() || c == '_')).unwrap_or(true)
}

/// Whether a line's last colon belongs to a `match`/`if`/`else` header rather than a type or
/// field (only such headers open a trailing block argument).
fn line_header_keyword(line: &str) -> Option<&'static str> {
    let t = line.trim_end().trim_end_matches(':').trim_end();
    // the header keyword is the first word after the last *unclosed* `(`/`[`, or after the
    // last `,` at that level (strings are skipped)
    // one cut position per open delimiter level: the opener, or the last comma at that level
    let mut cuts: Vec<usize> = Vec::new();
    let mut in_str = false;
    let mut prev = ' ';
    for (i, c) in t.char_indices() {
        if in_str {
            if c == '"' && prev != '\\' {
                in_str = false;
            }
        } else {
            match c {
                '"' => in_str = true,
                '(' | '[' | '{' => cuts.push(i + 1),
                ')' | ']' | '}' => {
                    cuts.pop();
                }
                ',' => {
                    if let Some(top) = cuts.last_mut() {
                        *top = i + 1;
                    }
                }
                _ => {}
            }
        }
        prev = c;
    }
    let cut = cuts.last().copied().unwrap_or(0);
    let head = t[cut..].trim_start();
    // only `match` and `if` headers open a trailing block argument (D51)
    ["match", "if"].iter().copied().find(|k| starts_with_word(head, k))
}

/// The words an item may begin with. A line at column 0 starting with one of these is an item,
/// whatever brackets are still open above it.
fn starts_item(line: &str) -> bool {
    const ITEM: &[&str] = &[
        "fn", "pub", "struct", "enum", "impl", "trait", "use", "mod", "type", "const", "static",
        "async", "unsafe", "extern", "macro_rules",
    ];
    let word: String = line.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    ITEM.contains(&word.as_str())
}

fn strip_doc(s: &str) -> String {
    let s = s.strip_prefix(' ').unwrap_or(s);
    s.to_string()
}

fn starts_with_leading_continuer(content: &str) -> bool {
    if content.starts_with("..") {
        return false;
    }
    for p in LEADING_CONTINUERS {
        if content.starts_with(p) {
            // `.` must not be `..`; `?` fine.
            return true;
        }
    }
    false
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StrKind {
    Normal,
    Raw(usize),
}

/// Recognises a string literal prefix at the start of `s`: `"`, `b"`, `r"`, `r#"`, `br#"`, `c"`.
fn string_prefix(s: &str) -> Option<(usize, StrKind)> {
    let b = s.as_bytes();
    let mut p = 0;
    if b.get(p) == Some(&b'b') || b.get(p) == Some(&b'c') {
        p += 1;
    }
    if b.get(p) == Some(&b'r') {
        let mut hashes = 0;
        let mut q = p + 1;
        while b.get(q) == Some(&b'#') {
            hashes += 1;
            q += 1;
        }
        if b.get(q) == Some(&b'"') {
            return Some((q + 1, StrKind::Raw(hashes)));
        }
        return None;
    }
    if b.get(p) == Some(&b'"') {
        return Some((p + 1, StrKind::Normal));
    }
    None
}

/// Returns (end_line, end_pos) just past the closing quote.
fn lex_string(lines: &[&str], mut li: usize, pos: usize, kind: StrKind) -> Option<(usize, usize)> {
    let (prefix_len, _) = string_prefix(&lines[li][pos..])?;
    let mut p = pos + prefix_len;
    loop {
        let line = lines[li];
        let bytes = line.as_bytes();
        while p < line.len() {
            match kind {
                StrKind::Normal => {
                    if bytes[p] == b'\\' {
                        p += 2;
                        continue;
                    }
                    if bytes[p] == b'"' {
                        return Some((li, p + 1));
                    }
                }
                StrKind::Raw(h) => {
                    if bytes[p] == b'"' {
                        let mut q = p + 1;
                        let mut n = 0;
                        while bytes.get(q) == Some(&b'#') && n < h {
                            n += 1;
                            q += 1;
                        }
                        if n == h {
                            return Some((li, q));
                        }
                    }
                }
            }
            p += 1;
        }
        li += 1;
        if li >= lines.len() {
            return None;
        }
        p = 0;
    }
}

/// A char literal `'x'`, `'\n'`, `'\u{1F600}'`; returns end position or None if it is a lifetime.
fn lex_char(line: &str, pos: usize) -> Option<usize> {
    let rest = &line[pos + 1..];
    let mut chars = rest.char_indices();
    let (_, c) = chars.next()?;
    if c == '\\' {
        // An escape sequence, then the closing quote. Searching for the next `'` instead would end
        // `'\''` one character early and leave a stray quote behind (found 2026-09-13).
        let after_backslash = &rest[1..];
        let escaped = if let Some(brace) = after_backslash.strip_prefix("u{") {
            // `\u{1F600}`
            2 + brace.find('}')? + 1
        } else if after_backslash.starts_with('x') {
            // `\x41`
            3
        } else {
            after_backslash.chars().next()?.len_utf8()
        };
        let close = 1 + escaped;
        if !rest[close..].starts_with('\'') {
            return None;
        }
        return Some(pos + 1 + close + 1);
    }
    let after = pos + 1 + c.len_utf8();
    if line[after..].starts_with('\'') {
        return Some(after + 1);
    }
    None
}

fn lex_number(line: &str, pos: usize) -> (usize, bool) {
    let b = line.as_bytes();
    let mut p = pos;
    let mut is_float = false;
    if line[pos..].starts_with("0x") || line[pos..].starts_with("0o") || line[pos..].starts_with("0b") {
        p += 2;
        while p < b.len() && (b[p].is_ascii_hexdigit() || b[p] == b'_') {
            p += 1;
        }
    } else {
        while p < b.len() && (b[p].is_ascii_digit() || b[p] == b'_') {
            p += 1;
        }
        // fraction: `.` followed by a digit (not `..`, not a method call `.sqrt`)
        if p < b.len() && b[p] == b'.' && p + 1 < b.len() && b[p + 1].is_ascii_digit() {
            is_float = true;
            p += 1;
            while p < b.len() && (b[p].is_ascii_digit() || b[p] == b'_') {
                p += 1;
            }
        } else if p < b.len() && b[p] == b'.' && !(p + 1 < b.len() && (b[p + 1] == b'.' || is_ident_start(line[p + 1..].chars().next().unwrap_or(' ')))) {
            // `1.` trailing-dot float
            is_float = true;
            p += 1;
        }
        // exponent
        if p < b.len() && (b[p] == b'e' || b[p] == b'E') {
            let mut q = p + 1;
            if q < b.len() && (b[q] == b'+' || b[q] == b'-') {
                q += 1;
            }
            if q < b.len() && b[q].is_ascii_digit() {
                is_float = true;
                p = q;
                while p < b.len() && (b[p].is_ascii_digit() || b[p] == b'_') {
                    p += 1;
                }
            }
        }
    }
    // suffix
    let suffix_start = p;
    while p < b.len() && (b[p].is_ascii_alphanumeric() || b[p] == b'_') {
        p += 1;
    }
    let suffix = &line[suffix_start..p];
    if suffix.starts_with('f') {
        is_float = true;
    }
    (p, is_float)
}

/// From an opening delimiter at (line `li`, byte `pos`), finds the balanced close using Rust
/// lexical rules (strings, chars, lifetimes, comments). Returns (end_line, end_pos_exclusive, text).
pub fn find_balanced_rust(lines: &[&str], li0: usize, pos0: usize) -> Option<(usize, usize, String)> {
    let mut li = li0;
    let mut p = pos0;
    let mut depth: i32 = 0;
    let mut text = String::new();
    let mut in_block_comment = false;
    let mut multiline;
    loop {
        let line = lines[li];
        let b = line.as_bytes();
        multiline = false;
        while p < line.len() {
            if in_block_comment {
                if line[p..].starts_with("*/") {
                    in_block_comment = false;
                    text.push_str("*/");
                    p += 2;
                } else {
                    let c = line[p..].chars().next().unwrap();
                    text.push(c);
                    p += c.len_utf8();
                }
                continue;
            }
            if line[p..].starts_with("//") {
                text.push_str(&line[p..]);
                break;
            }
            if line[p..].starts_with("/*") {
                in_block_comment = true;
                text.push_str("/*");
                p += 2;
                continue;
            }
            if let Some((_, kind)) = string_prefix(&line[p..]) {
                match lex_string(lines, li, p, kind) {
                    Some((el, ep)) => {
                        if el == li {
                            text.push_str(&line[p..ep]);
                            p = ep;
                        } else {
                            text.push_str(&line[p..]);
                            for l in li + 1..el {
                                text.push('\n');
                                text.push_str(lines[l]);
                            }
                            text.push('\n');
                            text.push_str(&lines[el][..ep]);
                            li = el;
                            p = ep;
                            // the string ended on a later line: refresh the line binding
                            multiline = true;
                            break;
                        }
                        continue;
                    }
                    None => return None,
                }
            }
            if b[p] == b'\'' {
                if let Some(end) = lex_char(line, p) {
                    text.push_str(&line[p..end]);
                    p = end;
                    continue;
                }
            }
            let c = line[p..].chars().next().unwrap();
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        text.push(c);
                        return Some((li, p + 1, text));
                    }
                    if depth < 0 {
                        return None;
                    }
                }
                _ => {}
            }
            text.push(c);
            p += c.len_utf8();
        }
        if multiline {
            // continue on the line the string ended on, without inserting a newline
            continue;
        }
        li += 1;
        if li >= lines.len() {
            return None;
        }
        text.push('\n');
        p = 0;
    }
}

/// Raw source text of the token range [a, b] on a single line, or tokens joined if multi-line.
pub fn slice_tokens(src_lines: &[&str], toks: &[Token]) -> String {
    if toks.is_empty() {
        return String::new();
    }
    let first = &toks[0];
    let last = &toks[toks.len() - 1];
    if first.line == last.line {
        let line = src_lines[first.line - 1];
        return line[first.start..last.end].to_string();
    }
    let mut out = String::new();
    let mut prev: Option<&Token> = None;
    for t in toks {
        if let Some(p) = prev {
            if p.line == t.line {
                let line = src_lines[t.line - 1];
                out.push_str(&line[p.end..t.start]);
            } else {
                out.push(' ');
            }
        }
        let line = src_lines[t.line - 1];
        out.push_str(&line[t.start..t.end]);
        prev = Some(t);
    }
    out
}

#[allow(dead_code)]
pub fn source_lines(src: &str) -> Vec<&str> {
    src.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect()
}

#[allow(dead_code)]
pub fn _unused(_: &str) -> &str {
    ""
}

impl Tok {
    pub fn is_punct(&self, p: &str) -> bool {
        matches!(self, Tok::Punct(q) if *q == p)
    }
    pub fn is_ident(&self, s: &str) -> bool {
        matches!(self, Tok::Ident(q) if q == s)
    }
}

#[allow(dead_code)]
fn _src_unused<'a>(l: &Lexer<'a>) -> &'a str {
    l.src
}
