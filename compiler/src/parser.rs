//! Recursive-descent parser for the Uredo surface (§6–§23).

use crate::ast::*;
use crate::diag::Diag;
use crate::lexer::{Tok, Token};

pub struct Parser<'a> {
    toks: Vec<Token>,
    pos: usize,
    lines: Vec<&'a str>,
    pub diags: Vec<Diag>,
}

const COMPOUND_OPS: &[&str] = &["+=", "-=", "*=", "/=", "%=", "^=", "&=", "|=", "<<=", ">>="];

const ITEM_KEYWORDS: &[&str] = &[
    "fn", "struct", "enum", "impl", "trait", "use", "const", "static", "mod", "type", "pub", "async", "macro_rules",
];

pub fn parse(src: &str, toks: Vec<Token>) -> (Module, Vec<Diag>) {
    let mut p = Parser {
        toks,
        pos: 0,
        lines: src.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect(),
        diags: Vec::new(),
    };
    let m = p.parse_module();
    (m, p.diags)
}

type PResult<T> = Result<T, ()>;

impl<'a> Parser<'a> {
    // ----- token access -----
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }
    fn peek_at(&self, k: usize) -> &Tok {
        let i = (self.pos + k).min(self.toks.len() - 1);
        &self.toks[i].tok
    }
    fn cur(&self) -> &Token {
        &self.toks[self.pos]
    }
    fn advance(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }
    fn at_punct(&self, p: &str) -> bool {
        self.peek().is_punct(p)
    }
    fn at_kw(&self, k: &str) -> bool {
        self.peek().is_ident(k)
    }
    fn at_newline(&self) -> bool {
        matches!(self.peek(), Tok::Newline)
    }
    fn eat_punct(&mut self, p: &str) -> bool {
        if self.at_punct(p) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, k: &str) -> bool {
        if self.at_kw(k) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn error_here(&mut self, msg: impl Into<String>) {
        let t = self.cur().clone();
        self.diags.push(Diag::error(t.line, t.col, msg));
    }
    fn expect_punct(&mut self, p: &str) -> PResult<()> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            self.error_here(format!("expected `{}`, found {}", p, describe(self.peek())));
            Err(())
        }
    }
    fn expect_ident(&mut self) -> PResult<String> {
        match self.peek().clone() {
            Tok::Ident(s) => {
                self.advance();
                Ok(s)
            }
            t => {
                self.error_here(format!("expected an identifier, found {}", describe(&t)));
                Err(())
            }
        }
    }
    fn expect_newline(&mut self) -> PResult<()> {
        // A block body ends with a Dedent, which already terminates the statement.
        if self.pos > 0 && matches!(self.toks[self.pos - 1].tok, Tok::Dedent) {
            return Ok(());
        }
        if self.at_newline() {
            self.advance();
            Ok(())
        } else if matches!(self.peek(), Tok::Eof | Tok::Dedent) {
            Ok(())
        } else {
            self.error_here(format!("expected end of line, found {}", describe(self.peek())));
            Err(())
        }
    }
    /// Skips to the next statement boundary after an error.
    /// Consumes an indented block, if one begins here: the body of an item whose header did not
    /// parse. Nested blocks go with it.
    fn skip_indented_block(&mut self) {
        if !matches!(self.peek(), Tok::Indent) {
            return;
        }
        let mut depth = 0;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::Indent => {
                    depth += 1;
                    self.advance();
                }
                Tok::Dedent => {
                    depth -= 1;
                    self.advance();
                    if depth == 0 {
                        return;
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn recover(&mut self) {
        let mut depth = 0;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::Newline if depth == 0 => {
                    self.advance();
                    return;
                }
                Tok::Indent => {
                    depth += 1;
                    self.advance();
                }
                Tok::Dedent => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }
    fn slice(&self, from: usize, to: usize) -> String {
        crate::lexer::slice_tokens(&self.lines, &self.toks[from..to])
    }

    // ----- module and items -----
    fn parse_module(&mut self) -> Module {
        let mut m = Module::default();
        let mut docs = Vec::new();
        let mut attrs = Vec::new();
        let mut blank = false;
        loop {
            match self.peek().clone() {
                Tok::Eof => break,
                Tok::Blank => {
                    self.advance();
                    blank = true;
                }
                Tok::Newline => {
                    self.advance();
                }
                Tok::InnerDoc(d) => {
                    self.advance();
                    m.inner_docs.push(d);
                }
                Tok::Doc(d) => {
                    self.advance();
                    docs.push(d);
                }
                Tok::Attr { inner: true, name, args, value } => {
                    let line = self.cur().line;
                    self.advance();
                    m.inner_attrs.push(Attr { inner: true, name, args, value, line });
                }
                Tok::Attr { inner: false, name, args, value } => {
                    let line = self.cur().line;
                    self.advance();
                    attrs.push(Attr { inner: false, name, args, value, line });
                }
                Tok::Indent => {
                    self.error_here("unexpected indentation at module level");
                    self.advance();
                }
                Tok::Dedent => {
                    self.advance();
                }
                _ => {
                    let d = std::mem::take(&mut docs);
                    let a = std::mem::take(&mut attrs);
                    let b = std::mem::replace(&mut blank, false);
                    match self.parse_item(d, a, b) {
                        Ok(items) => m.items.extend(items),
                        // An item header that did not parse is followed by its body, and that body
                        // is not module-level source: reporting each of its lines as "unexpected
                        // indentation" buries the one error that matters. Skip the whole block.
                        Err(()) => {
                            self.recover();
                            self.skip_indented_block();
                        }
                    }
                }
            }
        }
        m
    }

    fn parse_visibility(&mut self) -> Option<String> {
        if self.at_kw("pub") {
            let start = self.pos;
            self.advance();
            if self.at_punct("(") {
                let mut depth = 0;
                loop {
                    if self.at_punct("(") {
                        depth += 1;
                    } else if self.at_punct(")") {
                        depth -= 1;
                        if depth == 0 {
                            self.advance();
                            break;
                        }
                    } else if matches!(self.peek(), Tok::Eof | Tok::Newline) {
                        break;
                    }
                    self.advance();
                }
            }
            Some(self.slice(start, self.pos))
        } else {
            None
        }
    }

    /// Parses one item (a `const:` group yields several).
    fn parse_item(&mut self, docs: Vec<String>, attrs: Vec<Attr>, blank_before: bool) -> PResult<Vec<Item>> {
        let line = self.cur().line;
        // Raw Rust block / macro invocation in item position.
        if let Tok::RustBlock(text) = self.peek().clone() {
            self.advance();
            self.expect_newline()?;
            return Ok(vec![Item { docs, attrs, vis: None, line, blank_before, kind: ItemKind::Rust(text) }]);
        }
        let vis = self.parse_visibility();
        let mk = |kind: ItemKind| Item { docs: docs.clone(), attrs: attrs.clone(), vis: vis.clone(), line, blank_before, kind };
        if self.at_kw("use") {
            self.advance();
            let start = self.pos;
            while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
                self.advance();
            }
            let mut path = self.slice(start, self.pos);
            // `use rust::std::simd` says "this is a Rust item" for the reader's benefit and means
            // the same as `use std::simd` (§22.1); the prefix is documentation, so it is dropped
            // here rather than emitted, where it would name a crate that does not exist.
            if let Some(rest) = path.trim_start().strip_prefix("rust::") {
                path = rest.trim_start().to_string();
            }
            self.expect_newline()?;
            let copy = attrs.iter().any(|a| a.name == "copy");
            return Ok(vec![mk(ItemKind::Use { path, copy })]);
        }
        if self.at_kw("const") || self.at_kw("static") {
            let is_static = self.at_kw("static");
            self.advance();
            if self.eat_punct(":") {
                // declaration group (D46)
                self.expect_newline()?;
                if !matches!(self.peek(), Tok::Indent) {
                    self.error_here("expected an indented block after `const:`");
                    return Err(());
                }
                self.advance();
                let mut items = Vec::new();
                let mut member_docs: Vec<String> = Vec::new();
                let mut member_attrs: Vec<Attr> = Vec::new();
                let mut first = true;
                loop {
                    match self.peek().clone() {
                        Tok::Dedent => {
                            self.advance();
                            break;
                        }
                        Tok::Eof => break,
                        Tok::Blank | Tok::Newline => {
                            self.advance();
                        }
                        Tok::Doc(d) => {
                            self.advance();
                            member_docs.push(d);
                        }
                        Tok::Attr { name, args, value, .. } => {
                            let l = self.cur().line;
                            self.advance();
                            member_attrs.push(Attr { inner: false, name, args, value, line: l });
                        }
                        _ => {
                            let l = self.cur().line;
                            let name = self.expect_ident()?;
                            self.expect_punct(":")?;
                            let ty = self.parse_type(&["="])?;
                            self.expect_punct("=")?;
                            let expr = self.parse_expr()?;
                            self.expect_newline()?;
                            let mut d = if first { docs.clone() } else { Vec::new() };
                            d.extend(std::mem::take(&mut member_docs));
                            let mut a = attrs.clone();
                            a.extend(std::mem::take(&mut member_attrs));
                            items.push(Item {
                                docs: d,
                                attrs: a,
                                vis: vis.clone(),
                                line: l,
                                blank_before: first && blank_before,
                                kind: ItemKind::Const { is_static, name, ty, expr },
                            });
                            first = false;
                        }
                    }
                }
                return Ok(items);
            }
            let name = self.expect_ident()?;
            self.expect_punct(":")?;
            let ty = self.parse_type(&["="])?;
            self.expect_punct("=")?;
            let expr = self.parse_expr()?;
            self.expect_newline()?;
            return Ok(vec![mk(ItemKind::Const { is_static, name, ty, expr })]);
        }
        if self.at_kw("fn") || self.at_kw("async") || (self.at_kw("unsafe") && self.peek_at(1).is_ident("fn")) {
            let f = self.parse_fn()?;
            return Ok(vec![mk(ItemKind::Fn(f))]);
        }
        if self.at_kw("struct") {
            let s = self.parse_struct()?;
            return Ok(vec![mk(ItemKind::Struct(s))]);
        }
        if self.at_kw("enum") {
            let e = self.parse_enum()?;
            return Ok(vec![mk(ItemKind::Enum(e))]);
        }
        if self.at_kw("impl") || (self.at_kw("unsafe") && self.peek_at(1).is_ident("impl")) {
            let i = self.parse_impl()?;
            return Ok(vec![mk(ItemKind::Impl(i))]);
        }
        if self.at_kw("trait") {
            let t = self.parse_trait()?;
            return Ok(vec![mk(t)]);
        }
        if self.at_kw("type") {
            self.advance();
            let name = self.expect_ident()?;
            let generics = self.parse_generics();
            let bounds = if self.eat_punct(":") { Some(self.parse_type(&["=", "where"])?.text) } else { None };
            // a generic associated type carries its bound as a `where` clause, which rustc requires
            // for the `Self::View<'_>` shape and accepts on either side of the `=` (§12.4)
            let mut where_clause = self.parse_where_tail();
            let ty = if self.eat_punct("=") { Some(self.parse_type(&["where"])?) } else { None };
            if where_clause.is_none() {
                where_clause = self.parse_where_tail();
            }
            self.expect_newline()?;
            return Ok(vec![mk(ItemKind::TypeAlias { name, generics, bounds, where_clause, ty })]);
        }
        if self.at_kw("mod") {
            self.advance();
            let name = self.expect_ident()?;
            if self.eat_punct(":") {
                self.expect_newline()?;
                let items = self.parse_item_block()?;
                return Ok(vec![mk(ItemKind::Mod { name, body: Some(items) })]);
            }
            self.expect_newline()?;
            return Ok(vec![mk(ItemKind::Mod { name, body: None })]);
        }
        if self.at_kw("macro_rules") {
            self.advance();
            self.expect_punct("!")?;
            let name = self.expect_ident()?;
            if let Tok::MacroBody(body) = self.peek().clone() {
                self.advance();
                self.expect_newline()?;
                return Ok(vec![mk(ItemKind::MacroRules { name, body })]);
            }
            self.error_here("expected a macro body");
            return Err(());
        }
        // `path!(…)` in item position
        if let Tok::Ident(_) = self.peek() {
            let start = self.pos;
            let mut k = 0;
            while matches!(self.peek_at(k), Tok::Ident(_)) && self.peek_at(k + 1).is_punct("::") {
                k += 2;
            }
            if matches!(self.peek_at(k), Tok::Ident(_)) && self.peek_at(k + 1).is_punct("!") {
                if let Tok::MacroBody(body) = self.peek_at(k + 2).clone() {
                    for _ in 0..=k {
                        self.advance();
                    }
                    let path = self.slice(start, self.pos);
                    self.advance(); // !
                    self.advance(); // body
                    self.expect_newline()?;
                    return Ok(vec![mk(ItemKind::Macro { path, body })]);
                }
            }
        }
        self.error_here(format!("expected an item, found {}", describe(self.peek())));
        Err(())
    }

    /// Parses an indented block of items (module body, impl body, trait body).
    fn parse_item_block(&mut self) -> PResult<Vec<Item>> {
        if !matches!(self.peek(), Tok::Indent) {
            self.error_here("expected an indented block");
            return Err(());
        }
        self.advance();
        let mut items = Vec::new();
        let mut docs = Vec::new();
        let mut attrs = Vec::new();
        let mut blank = false;
        loop {
            match self.peek().clone() {
                Tok::Dedent => {
                    self.advance();
                    break;
                }
                Tok::Eof => break,
                Tok::Blank => {
                    self.advance();
                    blank = true;
                }
                Tok::Newline => {
                    self.advance();
                }
                Tok::Doc(d) => {
                    self.advance();
                    docs.push(d);
                }
                Tok::InnerDoc(_) => {
                    self.error_here("`##!` inner docs are only allowed at the top of a module");
                    self.advance();
                }
                Tok::Attr { name, args, value, .. } => {
                    let line = self.cur().line;
                    self.advance();
                    attrs.push(Attr { inner: false, name, args, value, line });
                }
                _ => {
                    let d = std::mem::take(&mut docs);
                    let a = std::mem::take(&mut attrs);
                    let b = std::mem::replace(&mut blank, false);
                    match self.parse_item(d, a, b) {
                        Ok(its) => items.extend(its),
                        Err(()) => self.recover(),
                    }
                }
            }
        }
        Ok(items)
    }

    /// Optional `<…>` generic parameter list, raw.
    /// `where …` to the end of the line, as written; `None` when the line does not carry one.
    fn parse_where_tail(&mut self) -> Option<String> {
        if !self.at_kw("where") {
            return None;
        }
        let start = self.pos;
        while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
            self.advance();
        }
        Some(self.slice(start, self.pos).trim().to_string())
    }

    fn parse_generics(&mut self) -> Option<String> {
        if !self.at_punct("<") {
            return None;
        }
        let start = self.pos;
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Tok::Punct("<") => depth += 1,
                Tok::Punct(">") => depth -= 1,
                Tok::Punct(">>") => depth -= 2,
                Tok::Eof | Tok::Newline => break,
                _ => {}
            }
            self.advance();
            if depth <= 0 {
                break;
            }
        }
        Some(self.slice(start, self.pos))
    }

    /// Parses a type as raw text until one of `stops` (at bracket depth 0), a newline, or
    /// the keywords `throws`/`where`.
    fn parse_type(&mut self, stops: &[&str]) -> PResult<Type> {
        let start = self.pos;
        let mut depth = 0i32;
        loop {
            let t = self.peek().clone();
            match &t {
                Tok::Newline | Tok::Eof | Tok::Indent | Tok::Dedent => break,
                Tok::Punct(p) => {
                    if depth == 0 && stops.contains(p) {
                        break;
                    }
                    match *p {
                        "(" | "[" | "{" | "<" => depth += 1,
                        ")" | "]" | "}" | ">" => {
                            if depth == 0 {
                                break;
                            }
                            depth -= 1;
                        }
                        ">>" => {
                            depth -= 2;
                            if depth < 0 {
                                break;
                            }
                        }
                        "," if depth == 0 => break,
                        _ => {}
                    }
                }
                Tok::Ident(w) if depth == 0 && (w == "throws" || w == "where") => break,
                _ => {}
            }
            self.advance();
        }
        if self.pos == start {
            self.error_here(format!("expected a type, found {}", describe(self.peek())));
            return Err(());
        }
        Ok(Type::new(self.slice(start, self.pos)))
    }

    /// Raw pattern text until one of `stops` at depth 0 (or the keyword `in` / `if` when listed).
    fn parse_pattern(&mut self, stops: &[&str], stop_kws: &[&str]) -> PResult<String> {
        let start = self.pos;
        let mut depth = 0i32;
        loop {
            let t = self.peek().clone();
            match &t {
                Tok::Newline | Tok::Eof | Tok::Indent | Tok::Dedent => break,
                Tok::Punct(p) => {
                    if depth == 0 && stops.contains(p) {
                        break;
                    }
                    match *p {
                        "(" | "[" | "{" => depth += 1,
                        ")" | "]" | "}" => {
                            if depth == 0 {
                                break;
                            }
                            depth -= 1;
                        }
                        _ => {}
                    }
                }
                Tok::Ident(w) if depth == 0 && stop_kws.contains(&w.as_str()) => break,
                _ => {}
            }
            self.advance();
        }
        if self.pos == start {
            self.error_here(format!("expected a pattern, found {}", describe(self.peek())));
            return Err(());
        }
        Ok(self.slice(start, self.pos))
    }

    fn parse_fn(&mut self) -> PResult<FnDecl> {
        let line = self.cur().line;
        let is_unsafe = self.eat_kw("unsafe");
        let is_async = self.eat_kw("async");
        if !self.eat_kw("fn") {
            self.error_here("expected `fn`");
            return Err(());
        }
        let name = self.expect_ident()?;
        let generics = self.parse_generics();
        self.expect_punct("(")?;
        let mut receiver = None;
        let mut params = Vec::new();
        let mut first = true;
        while !self.at_punct(")") {
            if !first {
                self.expect_punct(",")?;
                if self.at_punct(")") {
                    break;
                }
            }
            first = false;
            let pline = self.cur().line;
            let is_var = self.eat_kw("var");
            // receiver
            if self.at_kw("self") && params.is_empty() && receiver.is_none() {
                self.advance();
                let r = if self.eat_punct(":") {
                    if self.eat_kw("inout") {
                        if is_var {
                            self.error_here("`var self: inout` is not a form; `self: inout` is already mutable");
                        }
                        Receiver::RefMut
                    } else if self.eat_kw("take") {
                        if is_var { Receiver::MutValue } else { Receiver::Value }
                    } else {
                        // `self: Pin<&mut Self>`, `self: Rc<Self>`, … (D59): a receiver type Rust
                        // allows and Uredo does not model, written as it is written in Rust
                        if is_var {
                            self.error_here("`var` does not apply to a written receiver type (§12.3)");
                        }
                        if self.at_punct(")") || self.at_punct(",") {
                            self.error_here("expected `inout`, `take`, or a receiver type after `self:` (§12.3)");
                            return Err(());
                        }
                        let ty = self.parse_type(&[",", ")"])?.text.trim().to_string();
                        Receiver::Explicit(ty)
                    }
                } else {
                    if is_var {
                        self.error_here("`var self` needs `: take` (§12.3)");
                    }
                    Receiver::Ref
                };
                receiver = Some(r);
                continue;
            }
            // `take name: T` puts the mode where the name goes. Without this the pair parses as a
            // pattern parameter and the mode word reaches rustc inside the generated pattern.
            if (self.at_kw("take") || self.at_kw("inout"))
                && matches!(self.peek_at(1), Tok::Ident(_))
                && self.peek_at(2).is_punct(":")
            {
                let kw = if self.at_kw("take") { "take" } else { "inout" };
                let name = match self.peek_at(1) {
                    Tok::Ident(n) => n.clone(),
                    _ => unreachable!(),
                };
                self.error_here(format!("the passing mode goes on the type: write `{}: {} T` (§10.2)", name, kw));
                return Err(());
            }
            let pstart = self.pos;
            let is_pattern = !matches!(self.peek(), Tok::Ident(_)) || !self.peek_at(1).is_punct(":");
            let pat = if is_pattern {
                self.parse_pattern(&[":"], &[])?
            } else {
                let n = self.expect_ident()?;
                n
            };
            let _ = pstart;
            self.expect_punct(":")?;
            let mode = if self.eat_kw("take") {
                Mode::Take
            } else if self.eat_kw("inout") {
                Mode::Inout
            } else {
                Mode::Default
            };
            let ty = self.parse_type(&[","])?;
            params.push(Param { pat, is_pattern, is_var, mode, ty, line: pline });
        }
        self.expect_punct(")")?;
        let mut ret = None;
        if self.eat_punct("->") {
            ret = Some(self.parse_type(&[":"])?);
        }
        let mut throws = None;
        if self.eat_kw("throws") {
            if self.at_punct("->") {
                self.error_here("the return type comes before `throws`: write `-> T throws E` (§15.1)");
                return Err(());
            }
            if self.at_punct(":") || self.at_newline() || self.at_kw("where") {
                throws = Some(None);
            } else {
                throws = Some(Some(self.parse_type(&[":"])?.text));
            }
        }
        let mut where_clause = None;
        if self.at_kw("where") {
            // An inline `where` holds colons of its own — `where T: Clone:` — so the one that opens
            // the body is the one at the end of the line, as it is for a trait's supertraits (D60).
            let start = self.pos;
            while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
                self.advance();
            }
            let line_end = self.pos;
            self.pos = if line_end > start { line_end - 1 } else { start };
            let opens_body = self.pos >= start && self.at_punct(":");
            where_clause = Some(self.slice(start, if opens_body { self.pos } else { line_end }));
            if !opens_body {
                self.pos = line_end;
            }
        }
        // `where:` block form (§18): bounds one per line; the last bound line may end with the
        // body colon, and the body then follows at the same indentation.
        if self.at_newline() && self.peek_at(1).is_ident("where") && self.peek_at(2).is_punct(":") {
            self.advance();
            self.advance();
            self.advance();
            self.expect_newline()?;
            if !matches!(self.peek(), Tok::Indent) {
                self.error_here("expected an indented block of bounds after `where:`");
                return Err(());
            }
            self.advance();
            let mut bounds = Vec::new();
            let mut has_body = false;
            loop {
                match self.peek() {
                    Tok::Dedent => {
                        self.advance();
                        break;
                    }
                    Tok::Eof => break,
                    Tok::Newline | Tok::Blank => {
                        self.advance();
                    }
                    _ => {
                        let start = self.pos;
                        while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
                            self.advance();
                        }
                        let mut end = self.pos;
                        if end > start && self.toks[end - 1].tok.is_punct(":") {
                            end -= 1;
                            has_body = true;
                        }
                        bounds.push(self.slice(start, end));
                        self.expect_newline()?;
                        if has_body {
                            break;
                        }
                    }
                }
            }
            where_clause = Some(format!("where {}", bounds.join(", ")));
            let body = if has_body { Some(self.parse_block_body(line)?) } else { None };
            return Ok(FnDecl { name, generics, receiver, params, ret, throws, where_clause, is_async, is_unsafe, body, line });
        }
        let body = if self.eat_punct(":") {
            Some(self.parse_body()?)
        } else {
            // required trait method: header without a colon
            self.expect_newline()?;
            None
        };
        Ok(FnDecl { name, generics, receiver, params, ret, throws, where_clause, is_async, is_unsafe, body, line })
    }

    /// After a header colon: either an inline body on the same line or an indented block.
    fn parse_body(&mut self) -> PResult<Block> {
        let line = self.cur().line;
        if self.at_newline() {
            self.advance();
            self.parse_block()
        } else {
            let stmt = self.parse_inline_stmt()?;
            Ok(Block { stmts: vec![stmt], line })
        }
    }

    /// One inline body: an expression or a non-block statement (§6.3). Does not consume the newline.
    fn parse_inline_stmt(&mut self) -> PResult<Stmt> {
        let line = self.cur().line;
        let kind = self.parse_simple_stmt_kind(true)?;
        Ok(Stmt { line, blank_before: false, kind })
    }

    fn parse_block(&mut self) -> PResult<Block> {
        let line = self.cur().line;
        if !matches!(self.peek(), Tok::Indent) {
            self.error_here("expected an indented block");
            return Err(());
        }
        self.advance();
        self.parse_block_body(line)
    }

    /// Statements up to the closing Dedent (the Indent has been consumed).
    fn parse_block_body(&mut self, line: usize) -> PResult<Block> {
        let mut stmts = Vec::new();
        let mut blank = false;
        let mut attrs: Vec<Attr> = Vec::new();
        let mut docs: Vec<String> = Vec::new();
        loop {
            match self.peek().clone() {
                Tok::Dedent => {
                    self.advance();
                    break;
                }
                Tok::Eof => break,
                Tok::Blank => {
                    self.advance();
                    blank = true;
                }
                Tok::Newline => {
                    self.advance();
                }
                Tok::Doc(d) => {
                    self.advance();
                    docs.push(d);
                }
                Tok::InnerDoc(_) => {
                    self.error_here("`##!` inner docs are only allowed at the top of a module");
                    self.advance();
                }
                Tok::Attr { name, args, value, .. } => {
                    let l = self.cur().line;
                    self.advance();
                    attrs.push(Attr { inner: false, name, args, value, line: l });
                }
                _ => {
                    let b = std::mem::replace(&mut blank, false);
                    let a = std::mem::take(&mut attrs);
                    let d = std::mem::take(&mut docs);
                    match self.parse_stmt(b, a, d) {
                        Ok(s) => stmts.push(s),
                        Err(()) => self.recover(),
                    }
                }
            }
        }
        Ok(Block { stmts, line })
    }

    fn parse_stmt(&mut self, blank_before: bool, attrs: Vec<Attr>, docs: Vec<String>) -> PResult<Stmt> {
        let line = self.cur().line;
        // Items in blocks.
        let starts_item = match self.peek() {
            Tok::Ident(w) => ITEM_KEYWORDS.contains(&w.as_str()) && !(w == "type" && false),
            Tok::RustBlock(_) => {
                // item-level or statement-level rust block: treat as statement expression
                false
            }
            _ => false,
        } || (self.at_kw("unsafe") && (self.peek_at(1).is_ident("fn") || self.peek_at(1).is_ident("impl")))
            || !attrs.is_empty();
        if starts_item && !self.at_kw("unsafe") || (starts_item && self.at_kw("unsafe") && !self.peek_at(1).is_punct(":")) {
            if !attrs.is_empty() && !matches!(self.peek(), Tok::Ident(_)) {
                self.error_here("attributes must precede an item");
                return Err(());
            }
            // `pub` etc. handled by parse_item
            let items = self.parse_item(docs, attrs, blank_before)?;
            if items.len() != 1 {
                // const group inside a block: emit each as a statement item — wrap the first, rest follow.
                // Simplification: only the first is returned here; groups in blocks are rare.
                self.error_here("declaration groups are not supported inside function bodies");
                return Err(());
            }
            return Ok(Stmt { line, blank_before, kind: StmtKind::Item(items.into_iter().next().unwrap()) });
        }
        // Labelled loops.
        if let Tok::Lifetime(l) = self.peek().clone() {
            if self.peek_at(1).is_punct(":") {
                self.advance();
                self.advance();
                let label = Some(l);
                return self.parse_loop_stmt(label, line, blank_before);
            }
        }
        if self.at_kw("while") || self.at_kw("for") || self.at_kw("loop") {
            return self.parse_loop_stmt(None, line, blank_before);
        }
        if self.at_kw("unsafe") && self.peek_at(1).is_punct(":") {
            self.advance();
            self.advance();
            let body = self.parse_body()?;
            self.expect_newline()?;
            return Ok(Stmt { line, blank_before, kind: StmtKind::Unsafe(body) });
        }
        let kind = self.parse_simple_stmt_kind(false)?;
        self.expect_newline()?;
        Ok(Stmt { line, blank_before, kind })
    }

    fn parse_loop_stmt(&mut self, label: Option<String>, line: usize, blank_before: bool) -> PResult<Stmt> {
        if self.eat_kw("while") {
            if self.eat_kw("let") {
                let pat = self.parse_pattern(&["="], &[])?;
                self.expect_punct("=")?;
                let expr = self.parse_expr()?;
                self.expect_punct(":")?;
                let body = self.parse_body()?;
                self.expect_newline()?;
                return Ok(Stmt { line, blank_before, kind: StmtKind::WhileLet { label, pat, expr, body } });
            }
            let cond = self.parse_expr()?;
            self.expect_punct(":")?;
            let body = self.parse_body()?;
            self.expect_newline()?;
            return Ok(Stmt { line, blank_before, kind: StmtKind::While { label, cond, body } });
        }
        if self.eat_kw("for") {
            let pat = self.parse_pattern(&[], &["in"])?;
            if !self.eat_kw("in") {
                self.error_here("expected `in`");
                return Err(());
            }
            let mode = if self.eat_kw("take") { Mode::Take } else if self.eat_kw("inout") { Mode::Inout } else { Mode::Default };
            let iter = self.parse_expr()?;
            self.expect_punct(":")?;
            let body = self.parse_body()?;
            self.expect_newline()?;
            return Ok(Stmt { line, blank_before, kind: StmtKind::For { label, pat, mode, iter, body } });
        }
        if self.eat_kw("loop") {
            self.expect_punct(":")?;
            let body = self.parse_body()?;
            self.expect_newline()?;
            return Ok(Stmt { line, blank_before, kind: StmtKind::Expr(Expr::Loop { label, body }) });
        }
        self.error_here("expected a loop after the label");
        Err(())
    }

    /// A non-block statement or an expression statement; `inline` restricts to inline forms.
    fn parse_simple_stmt_kind(&mut self, inline: bool) -> PResult<StmtKind> {
        if self.at_kw("return") {
            self.advance();
            if self.at_newline() || self.at_kw("else") || matches!(self.peek(), Tok::Eof | Tok::Dedent) {
                return Ok(StmtKind::Return(None));
            }
            let e = self.parse_expr()?;
            return Ok(StmtKind::Return(Some(e)));
        }
        if self.at_kw("throw") {
            self.advance();
            let e = self.parse_expr()?;
            return Ok(StmtKind::Throw(e));
        }
        if self.at_kw("break") {
            self.advance();
            let mut label = None;
            if let Tok::Lifetime(l) = self.peek().clone() {
                self.advance();
                label = Some(l);
            }
            let value = if self.at_newline() || self.at_kw("else") || matches!(self.peek(), Tok::Eof | Tok::Dedent) {
                None
            } else {
                Some(self.parse_expr()?)
            };
            return Ok(StmtKind::Break { label, value });
        }
        if self.at_kw("continue") {
            self.advance();
            let mut label = None;
            if let Tok::Lifetime(l) = self.peek().clone() {
                self.advance();
                label = Some(l);
            }
            return Ok(StmtKind::Continue { label });
        }
        if self.at_kw("let") || self.at_kw("var") {
            let kind = if self.at_kw("let") { BindKind::Let } else { BindKind::Var };
            self.advance();
            let target = self.parse_bind_target()?;
            let ty = if self.eat_punct(":") { Some(self.parse_type(&["="])?) } else { None };
            self.expect_punct("=")?;
            let init = self.parse_expr()?;
            let mut else_block = None;
            if self.at_kw("else") {
                self.advance();
                self.expect_punct(":")?;
                else_block = Some(self.parse_body()?);
            }
            return Ok(StmtKind::Bind { kind, target, ty, init, else_block });
        }
        // Expression, binding or assignment.
        let target = self.parse_expr()?;
        if self.at_punct("=") {
            self.advance();
            let init = self.parse_expr()?;
            return Ok(StmtKind::Bind { kind: BindKind::Auto, target, ty: None, init, else_block: None });
        }
        if self.at_punct(":") && !inline {
            // typed binding `name: T = expr`
            self.advance();
            let ty = self.parse_type(&["="])?;
            self.expect_punct("=")?;
            let init = self.parse_expr()?;
            return Ok(StmtKind::Bind { kind: BindKind::Auto, target, ty: Some(ty), init, else_block: None });
        }
        if self.at_punct(":") && inline {
            self.advance();
            let ty = self.parse_type(&["="])?;
            self.expect_punct("=")?;
            let init = self.parse_expr()?;
            return Ok(StmtKind::Bind { kind: BindKind::Auto, target, ty: Some(ty), init, else_block: None });
        }
        for op in COMPOUND_OPS {
            if self.at_punct(op) {
                self.advance();
                let value = self.parse_expr()?;
                return Ok(StmtKind::Assign { target, op, value });
            }
        }
        Ok(StmtKind::Expr(target))
    }

    /// The target of `let`/`var`: an identifier or a pattern, returned as an expression-ish node.
    fn parse_bind_target(&mut self) -> PResult<Expr> {
        // `let Some(cfg) = …`, `let (a, b) = …`, `let Point { x, y } = …`, `let name = …`
        if matches!(self.peek(), Tok::Ident(_)) && (self.peek_at(1).is_punct("=") || self.peek_at(1).is_punct(":")) {
            let n = self.expect_ident()?;
            return Ok(Expr::Path(n));
        }
        let pat = self.parse_pattern(&["=", ":"], &[])?;
        Ok(Expr::Path(pat))
    }

    // ----- structs, enums, impls, traits -----
    fn parse_struct(&mut self) -> PResult<StructDecl> {
        self.advance(); // struct
        let name = self.expect_ident()?;
        let generics = self.parse_generics();
        let mut where_clause = None;
        if self.at_kw("where") {
            let start = self.pos;
            while !matches!(self.peek(), Tok::Newline | Tok::Eof) && !self.at_punct(":") && !self.at_punct("(") {
                self.advance();
            }
            where_clause = Some(self.slice(start, self.pos));
        }
        if self.at_punct("(") {
            let start = self.pos;
            let mut depth = 0;
            loop {
                if self.at_punct("(") {
                    depth += 1;
                } else if self.at_punct(")") {
                    depth -= 1;
                    if depth == 0 {
                        self.advance();
                        break;
                    }
                } else if matches!(self.peek(), Tok::Newline | Tok::Eof) {
                    break;
                }
                self.advance();
            }
            let text = self.slice(start, self.pos);
            if self.at_kw("where") {
                let start = self.pos;
                while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
                    self.advance();
                }
                where_clause = Some(self.slice(start, self.pos));
            }
            if self.at_punct(":") {
                self.error_here("a tuple struct has no body (§6.3)");
                return Err(());
            }
            self.expect_newline()?;
            return Ok(StructDecl { name, generics, where_clause, kind: StructKind::Tuple(text), nested: vec![] });
        }
        if !self.at_punct(":") {
            self.expect_newline()?;
            if matches!(self.peek(), Tok::Indent) {
                self.error_here("expected `:` before the struct body");
                return Err(());
            }
            return Ok(StructDecl { name, generics, where_clause, kind: StructKind::Unit, nested: vec![] });
        }
        self.advance(); // :
        self.expect_newline()?;
        let (fields, nested) = self.parse_type_body(false)?;
        let fields = fields.into_iter().map(|f| match f { Member::Field(f) => f, _ => unreachable!() }).collect();
        Ok(StructDecl { name, generics, where_clause, kind: StructKind::Named(fields), nested })
    }

    fn parse_enum(&mut self) -> PResult<EnumDecl> {
        self.advance(); // enum
        let name = self.expect_ident()?;
        let generics = self.parse_generics();
        let mut where_clause = None;
        if self.at_kw("where") {
            let start = self.pos;
            while !matches!(self.peek(), Tok::Newline | Tok::Eof) && !self.at_punct(":") {
                self.advance();
            }
            where_clause = Some(self.slice(start, self.pos));
        }
        if !self.at_punct(":") {
            self.expect_newline()?;
            return Ok(EnumDecl { name, generics, where_clause, variants: vec![], nested: vec![] });
        }
        self.advance();
        self.expect_newline()?;
        let (members, nested) = self.parse_type_body(true)?;
        let variants = members.into_iter().map(|m| match m { Member::Variant(v) => v, _ => unreachable!() }).collect();
        Ok(EnumDecl { name, generics, where_clause, variants, nested })
    }

    /// Fields (or variants) followed by nested methods / trait impls (§12.2).
    fn parse_type_body(&mut self, is_enum: bool) -> PResult<(Vec<Member>, Vec<Nested>)> {
        if !matches!(self.peek(), Tok::Indent) {
            self.error_here("expected an indented body");
            return Err(());
        }
        self.advance();
        let mut members = Vec::new();
        let mut nested = Vec::new();
        let mut docs = Vec::new();
        let mut attrs = Vec::new();
        let mut blank = false;
        loop {
            match self.peek().clone() {
                Tok::Dedent => {
                    self.advance();
                    break;
                }
                Tok::Eof => break,
                Tok::Blank => {
                    self.advance();
                    blank = true;
                }
                Tok::Newline => {
                    self.advance();
                }
                Tok::Doc(d) => {
                    self.advance();
                    docs.push(d);
                }
                Tok::Attr { name, args, value, .. } => {
                    let l = self.cur().line;
                    self.advance();
                    attrs.push(Attr { inner: false, name, args, value, line: l });
                }
                _ => {
                    let line = self.cur().line;
                    let d = std::mem::take(&mut docs);
                    let a = std::mem::take(&mut attrs);
                    let b = std::mem::replace(&mut blank, false);
                    // nested method / impl?
                    let mut k = 0;
                    if self.at_kw("pub") {
                        k = 1;
                        if self.peek_at(1).is_punct("(") {
                            let mut depth = 0;
                            loop {
                                if self.peek_at(k).is_punct("(") {
                                    depth += 1;
                                } else if self.peek_at(k).is_punct(")") {
                                    depth -= 1;
                                    if depth == 0 {
                                        k += 1;
                                        break;
                                    }
                                }
                                k += 1;
                            }
                        }
                    }
                    let is_method = self.peek_at(k).is_ident("fn") || self.peek_at(k).is_ident("async") || (self.peek_at(k).is_ident("unsafe") && self.peek_at(k + 1).is_ident("fn"));
                    if is_method {
                        let mut items = self.parse_item(d, a, b)?;
                        nested.push(Nested::Method(items.remove(0)));
                        continue;
                    }
                    if self.at_kw("impl") {
                        self.advance();
                        let start = self.pos;
                        while !matches!(self.peek(), Tok::Newline | Tok::Eof) && !self.at_punct(":") {
                            self.advance();
                        }
                        let trait_path = self.slice(start, self.pos);
                        let items = if self.eat_punct(":") {
                            self.expect_newline()?;
                            self.parse_item_block()?
                        } else {
                            self.expect_newline()?;
                            vec![]
                        };
                        nested.push(Nested::TraitImpl { attrs: a, trait_path, items, line, blank_before: b });
                        continue;
                    }
                    if !nested.is_empty() {
                        self.error_here("fields and variants must come before nested methods (§12.2)");
                    }
                    if is_enum {
                        let start = self.pos;
                        while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
                            self.advance();
                        }
                        let text = self.slice(start, self.pos);
                        self.expect_newline()?;
                        members.push(Member::Variant(Variant { docs: d, attrs: a, text, line }));
                    } else {
                        let vis = self.parse_visibility();
                        let name = self.expect_ident()?;
                        self.expect_punct(":")?;
                        let ty = self.parse_type(&[])?;
                        self.expect_newline()?;
                        members.push(Member::Field(Field { docs: d, attrs: a, vis, name, ty, line }));
                    }
                }
            }
        }
        Ok((members, nested))
    }

    fn parse_impl(&mut self) -> PResult<ImplDecl> {
        let is_unsafe = self.eat_kw("unsafe");
        self.advance(); // impl
        let generics = self.parse_generics();
        // `impl Trait for Type` or `impl Type`
        let start = self.pos;
        let mut depth = 0i32;
        let mut for_pos = None;
        while !matches!(self.peek(), Tok::Newline | Tok::Eof) && !(depth == 0 && self.at_punct(":")) {
            match self.peek() {
                Tok::Punct("<") | Tok::Punct("(") | Tok::Punct("[") => depth += 1,
                Tok::Punct(">") | Tok::Punct(")") | Tok::Punct("]") => depth -= 1,
                Tok::Punct(">>") => depth -= 2,
                Tok::Ident(w) if depth == 0 && w == "for" => for_pos = Some(self.pos),
                Tok::Ident(w) if depth == 0 && w == "where" => break,
                _ => {}
            }
            self.advance();
        }
        let end = self.pos;
        let (trait_path, self_ty) = match for_pos {
            Some(fp) => (Some(self.slice(start, fp)), self.slice(fp + 1, end)),
            None => (None, self.slice(start, end)),
        };
        let mut where_clause = None;
        if self.at_kw("where") {
            let ws = self.pos;
            while !matches!(self.peek(), Tok::Newline | Tok::Eof) && !self.at_punct(":") {
                self.advance();
            }
            where_clause = Some(self.slice(ws, self.pos));
        }
        let items = if self.eat_punct(":") {
            self.expect_newline()?;
            self.parse_item_block()?
        } else {
            self.expect_newline()?;
            vec![]
        };
        Ok(ImplDecl { generics, trait_path, self_ty, where_clause, items, is_unsafe })
    }

    fn parse_trait(&mut self) -> PResult<ItemKind> {
        self.advance(); // trait
        let name = self.expect_ident()?;
        let generics = self.parse_generics();
        // A trait may carry supertraits, which are written as Rust writes them, so the line holds two
        // kinds of colon: `trait Ord: PartialOrd:`. The one that opens the block is the one at the end
        // of the line; everything before it is the bound list and any `where` clause (D60).
        let start = self.pos;
        while !matches!(self.peek(), Tok::Newline | Tok::Eof) {
            self.advance();
        }
        let line_end = self.pos;
        self.pos = if line_end > start { line_end - 1 } else { start };
        let opens_block = self.pos >= start && self.at_punct(":");
        let rest = self.slice(start, if opens_block { self.pos } else { line_end });
        if !opens_block {
            self.pos = line_end;
        }
        let items = if self.eat_punct(":") {
            self.expect_newline()?;
            self.parse_item_block()?
        } else {
            self.expect_newline()?;
            vec![]
        };
        Ok(ItemKind::Impl(ImplDecl {
            generics,
            trait_path: Some(format!("trait {}", name)),
            self_ty: rest,
            where_clause: None,
            items,
            is_unsafe: false,
        }))
    }

    // ----- expressions -----
    pub fn parse_expr(&mut self) -> PResult<Expr> {
        // closures first
        if self.at_kw("move") {
            self.advance();
            return self.parse_closure(true);
        }
        if self.is_closure_start() {
            return self.parse_closure(false);
        }
        let lhs = self.parse_range()?;
        Ok(lhs)
    }

    fn is_closure_start(&self) -> bool {
        match self.peek() {
            Tok::Ident(_) => self.peek_at(1).is_punct("=>"),
            Tok::Punct("(") => {
                // scan to the matching `)` and check for `=>` or `->`
                let mut depth = 0;
                let mut k = 0;
                loop {
                    match self.peek_at(k) {
                        Tok::Punct("(") => depth += 1,
                        Tok::Punct(")") => {
                            depth -= 1;
                            if depth == 0 {
                                // `(params) =>`, `(params) -> T =>`, or a `throws` closure whose
                                // success type is missing (rejected with a message in parse_closure)
                                return self.peek_at(k + 1).is_punct("=>") || self.peek_at(k + 1).is_punct("->") || self.peek_at(k + 1).is_ident("throws");
                            }
                        }
                        Tok::Eof | Tok::Newline => return false,
                        _ => {}
                    }
                    k += 1;
                }
            }
            _ => false,
        }
    }

    fn parse_closure(&mut self, is_move: bool) -> PResult<Expr> {
        let mut params = Vec::new();
        if self.at_punct("(") {
            self.advance();
            while !self.at_punct(")") {
                let pat = self.parse_pattern(&[":", ","], &[])?;
                let ty = if self.eat_punct(":") { Some(self.parse_type(&[","])?) } else { None };
                params.push(ClosureParam { pat, ty });
                if !self.eat_punct(",") {
                    break;
                }
            }
            self.expect_punct(")")?;
        } else {
            let n = self.expect_ident()?;
            params.push(ClosureParam { pat: n, ty: None });
        }
        let ret = if self.eat_punct("->") { Some(self.parse_type(&["=>"])?) } else { None };
        // `(req: Request) -> Response throws HttpError =>` (§17): the closure returns a `Result`
        let throws = if self.eat_kw("throws") {
            if self.at_punct("=>") { Some(None) } else { Some(Some(self.parse_type(&["=>"])?.text)) }
        } else {
            None
        };
        if throws.is_some() && ret.is_none() {
            self.error_here("a `throws` closure needs its success type: `(x: T) -> U throws E => …` (§17)");
            return Err(());
        }
        self.expect_punct("=>")?;
        // trailing block argument (D51): `f(a, x =>` + indented block + `)` on its own line
        if self.at_newline() && matches!(self.peek_at(1), Tok::Indent) {
            self.advance();
            let block = self.parse_block()?;
            return Ok(Expr::Closure { is_move, params, ret, throws, body: Box::new(Expr::Block(block)) });
        }
        let mut body = self.parse_expr()?;
        // `() => calls += 1`: an assignment as the closure body (Rust allows it as an expression)
        let mut assign_op: Option<&'static str> = None;
        if self.at_punct("=") {
            assign_op = Some("=");
        } else {
            for op in COMPOUND_OPS {
                if self.at_punct(op) {
                    assign_op = Some(op);
                }
            }
        }
        if let Some(op) = assign_op {
            self.advance();
            let value = self.parse_expr()?;
            body = Expr::Assign { target: Box::new(body), op, value: Box::new(value) };
        }
        // `x =>` at a line end continues onto the next line (§6.3); a second, deeper line
        // after that is a block body, which Uredo does not have yet (G9).
        if matches!(self.peek(), Tok::Indent) || (self.at_newline() && matches!(self.peek_at(1), Tok::Indent)) {
            if self.at_newline() {
                self.advance();
            }
            self.error_here("block-bodied closures are not yet supported (G9, §38); use a named function or a `rust { }` expression");
            return Err(());
        }
        Ok(Expr::Closure { is_move, params, ret, throws, body: Box::new(body) })
    }

    fn parse_range(&mut self) -> PResult<Expr> {
        if self.at_punct("..") || self.at_punct("..=") {
            let inclusive = self.at_punct("..=");
            self.advance();
            let hi = if self.expr_can_start() { Some(Box::new(self.parse_binary(0)?)) } else { None };
            return Ok(Expr::Range { lo: None, hi, inclusive });
        }
        let lo = self.parse_binary(0)?;
        if self.at_punct("..") || self.at_punct("..=") {
            let inclusive = self.at_punct("..=");
            self.advance();
            let hi = if self.expr_can_start() { Some(Box::new(self.parse_binary(0)?)) } else { None };
            return Ok(Expr::Range { lo: Some(Box::new(lo)), hi, inclusive });
        }
        Ok(lo)
    }

    fn expr_can_start(&self) -> bool {
        match self.peek() {
            Tok::Ident(w) => !matches!(w.as_str(), "else" | "in" | "as" | "if" | "where" | "throws"),
            Tok::Int(_) | Tok::Float(_) | Tok::Str(_) | Tok::Char(_) | Tok::RustBlock(_) => true,
            Tok::Punct(p) => matches!(*p, "(" | "[" | "-" | "!" | "*" | "&" | "::"),
            _ => false,
        }
    }

    fn binary_prec(&self) -> Option<(&'static str, u8, bool)> {
        // (op, precedence, right-assoc)
        let op = match self.peek() {
            Tok::Punct(p) => *p,
            Tok::Ident(w) if w == "as" => return Some(("as", 11, false)),
            _ => return None,
        };
        let prec = match op {
            "*" | "/" | "%" => 10,
            "+" | "-" => 9,
            "<<" | ">>" => 8,
            "&" => 7,
            "^" => 6,
            "|" => 5,
            "==" | "!=" | "<" | ">" | "<=" | ">=" => 4,
            "&&" => 3,
            "||" => 2,
            _ => return None,
        };
        Some((op, prec, false))
    }

    fn parse_binary(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let Some((op, prec, _)) = self.binary_prec() else { break };
            if prec < min_prec {
                break;
            }
            self.advance();
            if op == "as" {
                let ty = self.parse_cast_type()?;
                lhs = Expr::Cast { expr: Box::new(lhs), ty };
                continue;
            }
            let rhs = self.parse_binary(prec + 1)?;
            if prec == 4 {
                if let Some((op2, 4, _)) = self.binary_prec() {
                    let _ = op2;
                    self.error_here("chained comparisons are not allowed (§7.1); parenthesise");
                    return Err(());
                }
            }
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    /// The type after `as`: a path with optional generics, or a primitive; stops before an operator.
    fn parse_cast_type(&mut self) -> PResult<Type> {
        let start = self.pos;
        // `as &T`, `as *const T`
        if self.at_punct("&") || self.at_punct("*") {
            self.advance();
            self.eat_kw("mut");
            self.eat_kw("const");
        }
        match self.peek() {
            Tok::Ident(_) => {
                self.advance();
                while self.at_punct("::") {
                    self.advance();
                    self.advance();
                }
                if self.at_punct("<") {
                    self.parse_generics();
                }
            }
            Tok::Punct("(") => {
                let mut depth = 0;
                loop {
                    if self.at_punct("(") {
                        depth += 1;
                    } else if self.at_punct(")") {
                        depth -= 1;
                        if depth == 0 {
                            self.advance();
                            break;
                        }
                    }
                    self.advance();
                }
            }
            _ => {
                self.error_here("expected a type after `as`");
                return Err(());
            }
        }
        Ok(Type::new(self.slice(start, self.pos)))
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        if self.at_punct("-") {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::Unary { op: "-", expr: Box::new(e) });
        }
        if self.at_punct("!") {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::Unary { op: "!", expr: Box::new(e) });
        }
        if self.at_punct("*") {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::Unary { op: "*", expr: Box::new(e) });
        }
        if self.at_punct("&") {
            self.advance();
            let mutable = self.eat_kw("mut");
            let e = self.parse_unary()?;
            return Ok(Expr::Ref { mutable, expr: Box::new(e) });
        }
        if self.at_punct("&&") {
            // `&&x` = `&(&x)`
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::Ref { mutable: false, expr: Box::new(Expr::Ref { mutable: false, expr: Box::new(e) }) });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut e = self.parse_primary()?;
        loop {
            if self.at_punct(".") {
                self.advance();
                match self.peek().clone() {
                    Tok::Ident(name) if name == "await" => {
                        self.advance();
                        e = Expr::Await(Box::new(e));
                    }
                    Tok::Ident(name) => {
                        let line = self.cur().line;
                        let col = self.cur().col;
                        self.advance();
                        let mut turbofish = None;
                        if self.at_punct("::") && self.peek_at(1).is_punct("<") {
                            self.advance();
                            turbofish = self.parse_generics();
                        }
                        if self.at_punct("(") {
                            let args = self.parse_args()?;
                            e = Expr::MethodCall { recv: Box::new(e), name, turbofish, args, line, col };
                        } else {
                            if turbofish.is_some() {
                                self.error_here("expected `(` after a turbofish");
                                return Err(());
                            }
                            e = Expr::Field { expr: Box::new(e), name };
                        }
                    }
                    Tok::Int(n) => {
                        self.advance();
                        e = Expr::Field { expr: Box::new(e), name: n };
                    }
                    Tok::Float(f) => {
                        // `t.0.1` lexes as Float("0.1")
                        self.advance();
                        let mut parts = f.split('.');
                        let a = parts.next().unwrap().to_string();
                        let b = parts.next().unwrap_or("").to_string();
                        e = Expr::Field { expr: Box::new(e), name: a };
                        e = Expr::Field { expr: Box::new(e), name: b };
                    }
                    t => {
                        self.error_here(format!("expected a field or method name after `.`, found {}", describe(&t)));
                        return Err(());
                    }
                }
                continue;
            }
            if self.at_punct("(") {
                let line = self.cur().line;
                let col = self.cur().col;
                let args = self.parse_args()?;
                e = Expr::Call { callee: Box::new(e), args, line, col };
                continue;
            }
            if self.at_punct("[") {
                self.advance();
                let idx = self.parse_expr()?;
                self.expect_punct("]")?;
                e = Expr::Index { expr: Box::new(e), index: Box::new(idx) };
                continue;
            }
            if self.at_punct("?") {
                self.advance();
                e = Expr::Try(Box::new(e));
                continue;
            }
            break;
        }
        Ok(e)
    }

    fn parse_args(&mut self) -> PResult<Vec<Expr>> {
        self.expect_punct("(")?;
        let mut args = Vec::new();
        while !self.at_punct(")") {
            args.push(self.parse_expr()?);
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok(args)
    }

    fn parse_path(&mut self) -> PResult<String> {
        let start = self.pos;
        if self.at_punct("::") {
            self.advance();
        }
        self.expect_ident()?;
        loop {
            if self.at_punct("::") {
                if self.peek_at(1).is_punct("<") {
                    self.advance();
                    self.parse_generics();
                    continue;
                }
                if matches!(self.peek_at(1), Tok::Ident(_)) {
                    self.advance();
                    self.advance();
                    continue;
                }
            }
            break;
        }
        Ok(self.slice(start, self.pos))
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let t = self.peek().clone();
        match t {
            Tok::Int(s) => {
                self.advance();
                Ok(Expr::Int(s))
            }
            Tok::Float(s) => {
                self.advance();
                Ok(Expr::Float(s))
            }
            Tok::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Tok::Char(s) => {
                self.advance();
                Ok(Expr::Char(s))
            }
            Tok::RustBlock(s) => {
                self.advance();
                Ok(Expr::Rust(s))
            }
            Tok::Punct("(") => {
                self.advance();
                if self.at_punct(")") {
                    self.advance();
                    return Ok(Expr::Unit);
                }
                let first = self.parse_expr()?;
                if self.at_punct(",") {
                    let mut items = vec![first];
                    while self.eat_punct(",") {
                        if self.at_punct(")") {
                            break;
                        }
                        items.push(self.parse_expr()?);
                    }
                    self.expect_punct(")")?;
                    return Ok(Expr::Tuple(items));
                }
                self.expect_punct(")")?;
                Ok(Expr::Paren(Box::new(first)))
            }
            Tok::Punct("[") => {
                self.advance();
                if self.at_punct("]") {
                    self.advance();
                    return Ok(Expr::Array(vec![]));
                }
                let first = self.parse_expr()?;
                if self.eat_punct(";") {
                    let count = self.parse_expr()?;
                    self.expect_punct("]")?;
                    return Ok(Expr::ArrayRepeat { value: Box::new(first), count: Box::new(count) });
                }
                let mut items = vec![first];
                while self.eat_punct(",") {
                    if self.at_punct("]") {
                        break;
                    }
                    items.push(self.parse_expr()?);
                }
                self.expect_punct("]")?;
                Ok(Expr::Array(items))
            }
            Tok::Ident(w) => {
                match w.as_str() {
                    "true" => {
                        self.advance();
                        return Ok(Expr::Bool(true));
                    }
                    "false" => {
                        self.advance();
                        return Ok(Expr::Bool(false));
                    }
                    "if" => return self.parse_if_expr().map(|i| Expr::If(Box::new(i))),
                    "match" => return self.parse_match(),
                    "loop" => {
                        self.advance();
                        self.expect_punct(":")?;
                        let body = self.parse_body()?;
                        return Ok(Expr::Loop { label: None, body });
                    }
                    "unsafe" if self.peek_at(1).is_punct(":") => {
                        self.advance();
                        self.advance();
                        let body = self.parse_body()?;
                        return Ok(Expr::UnsafeBlock(body));
                    }
                    _ => {}
                }
                let path = self.parse_path()?;
                // macro invocation
                if self.at_punct("!") {
                    if let Tok::MacroBody(body) = self.peek_at(1).clone() {
                        self.advance();
                        self.advance();
                        return Ok(Expr::Macro { path, body });
                    }
                }
                // struct literal
                if self.at_punct("{") {
                    return self.parse_struct_lit(path);
                }
                Ok(Expr::Path(path))
            }
            Tok::Punct("::") => {
                let path = self.parse_path()?;
                if self.at_punct("{") {
                    return self.parse_struct_lit(path);
                }
                Ok(Expr::Path(path))
            }
            Tok::Lifetime(l) if self.peek_at(1).is_punct(":") => {
                self.advance();
                self.advance();
                if self.eat_kw("loop") {
                    self.expect_punct(":")?;
                    let body = self.parse_body()?;
                    return Ok(Expr::Loop { label: Some(l), body });
                }
                self.error_here("a labelled loop expression must be `loop`");
                Err(())
            }
            t => {
                self.error_here(format!("expected an expression, found {}", describe(&t)));
                Err(())
            }
        }
    }

    fn parse_struct_lit(&mut self, path: String) -> PResult<Expr> {
        self.expect_punct("{")?;
        let mut fields = Vec::new();
        let mut base = None;
        loop {
            if self.at_punct("}") {
                break;
            }
            if self.at_punct("..") {
                self.advance();
                base = Some(Box::new(self.parse_expr()?));
                self.eat_punct(",");
                break;
            }
            let mut attrs = Vec::new();
            while let Tok::Attr { name, args, value, .. } = self.peek().clone() {
                let l = self.cur().line;
                self.advance();
                attrs.push(Attr { inner: false, name, args, value, line: l });
            }
            let name = match self.peek().clone() {
                Tok::Ident(n) => {
                    self.advance();
                    n
                }
                Tok::Int(n) => {
                    self.advance();
                    n
                }
                t => {
                    self.error_here(format!("expected a field name, found {}", describe(&t)));
                    return Err(());
                }
            };
            let value = if self.eat_punct(":") { Some(self.parse_expr()?) } else { None };
            fields.push(StructField { attrs, name, value });
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct("}")?;
        Ok(Expr::StructLit { path, fields, base })
    }

    fn parse_if_expr(&mut self) -> PResult<IfExpr> {
        self.advance(); // if
        let mut pat = None;
        if self.eat_kw("let") {
            pat = Some(self.parse_pattern(&["="], &[])?);
            self.expect_punct("=")?;
        }
        let cond = self.parse_expr()?;
        self.expect_punct(":")?;
        let inline = !self.at_newline();
        let then = self.parse_body()?;
        if inline {
            if let Some(Stmt { kind: StmtKind::Expr(Expr::If(_)), .. }) = then.stmts.first() {
                self.error_here("an inline `if` body may not itself be an `if` (D49); use the indented form");
            }
        }
        // else handling: same line (inline) or next line at the anchor indentation
        let mut else_ = None;
        let has_else = if self.at_kw("else") {
            true
        } else if self.at_newline() && self.peek_at(1).is_ident("else") {
            self.advance();
            true
        } else {
            false
        };
        if has_else {
            self.advance(); // else
            if self.at_kw("if") {
                let nested = self.parse_if_expr()?;
                else_ = Some(ElseBranch::ElseIf(Box::new(nested)));
            } else {
                self.expect_punct(":")?;
                let inline_else = !self.at_newline();
                let b = self.parse_body()?;
                if inline_else {
                    if let Some(Stmt { kind: StmtKind::Expr(Expr::If(_)), .. }) = b.stmts.first() {
                        self.error_here("an inline `else` body may not be an `if` (D49); write `else if` or use the indented form");
                    }
                }
                else_ = Some(ElseBranch::Else(b));
            }
        }
        Ok(IfExpr { pat, cond: Box::new(cond), then, else_, inline })
    }

    fn parse_match(&mut self) -> PResult<Expr> {
        self.advance(); // match
        let scrutinee = self.parse_expr()?;
        self.expect_punct(":")?;
        self.expect_newline()?;
        if !matches!(self.peek(), Tok::Indent) {
            self.error_here("expected indented match arms");
            return Err(());
        }
        self.advance();
        let mut arms = Vec::new();
        loop {
            match self.peek().clone() {
                Tok::Dedent => {
                    self.advance();
                    break;
                }
                Tok::Eof => break,
                Tok::Blank | Tok::Newline | Tok::Doc(_) => {
                    self.advance();
                }
                _ => {
                    let line = self.cur().line;
                    let pat = self.parse_pattern(&[":"], &["if"])?;
                    let guard = if self.eat_kw("if") { Some(self.parse_expr()?) } else { None };
                    self.expect_punct(":")?;
                    let inline = !self.at_newline();
                    let body = if inline {
                        let kind = self.parse_simple_stmt_kind(true)?;
                        if matches!(kind, StmtKind::Bind { .. }) {
                            self.error_here("a match arm cannot be a binding; use an indented block");
                            return Err(());
                        }
                        let b = Block { stmts: vec![Stmt { line, blank_before: false, kind }], line };
                        self.expect_newline()?;
                        b
                    } else {
                        self.advance();
                        self.parse_block()?
                    };
                    arms.push(MatchArm { pat, guard, body, inline, line });
                }
            }
        }
        Ok(Expr::Match { scrutinee: Box::new(scrutinee), arms })
    }
}

pub enum Member {
    Field(Field),
    Variant(Variant),
}

pub fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(s) => format!("`{}`", s),
        Tok::Lifetime(s) => format!("`{}`", s),
        Tok::Int(s) | Tok::Float(s) => format!("`{}`", s),
        Tok::Str(_) => "a string literal".into(),
        Tok::Char(_) => "a character literal".into(),
        Tok::Punct(p) => format!("`{}`", p),
        Tok::Attr { name, .. } => format!("attribute `@{}`", name),
        Tok::Doc(_) | Tok::InnerDoc(_) => "a doc comment".into(),
        Tok::Blank => "a blank line".into(),
        Tok::RustBlock(_) => "a `rust { }` block".into(),
        Tok::MacroBody(_) => "a macro body".into(),
        Tok::Newline => "end of line".into(),
        Tok::Indent => "an indented block".into(),
        Tok::Dedent => "end of block".into(),
        Tok::Eof => "end of file".into(),
    }
}
