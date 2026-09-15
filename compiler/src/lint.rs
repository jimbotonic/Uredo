//! `uredo lint` (§28.1): the idiom checks the spec asks for, each decided from declarations the
//! lowerer already reads, so a lint never needs a second copy of the passing-mode rules.
//!
//! | Lint | What it says | Spec |
//! |---|---|---|
//! | `redundant_take` | `take` where the parameter already passes by value | §10.2 (D52, D54) |
//! | `borrowed_container` | `Vec<T>`/`String` parameters, which pass as `&Vec<T>`/`&String` | §10.2 |
//! | `copy_candidate` | a borrowed parameter whose every use is a deref | §10.1, §10.2 |
//!
//! The lowerer collects them into `Output::lints` on every compile; only `uredo lint` prints them.
//!
//! Beside them it collects `Measures`, the two counts §38 makes the condition for reversing D52.
//! Those are not lints: they say what the by-value rule costs, not that anything is wrong, and
//! `uredo lint --measure` prints them in place of the findings.

use crate::ast::*;
use crate::diag::Diag;

/// The two counts §38 names as the condition that would reverse P8 (D52), collected on every
/// compile and printed by `uredo lint --measure`. Neither is a finding, and neither is a lint: a
/// `.clone()` handed to a by-value parameter may be exactly what its author meant. The question the
/// counts answer is whether a project writes more of them than the `take` annotations the former
/// borrow default would have cost it, which is the point at which P8 stopped paying for itself.
#[derive(Debug, Default, Clone)]
pub struct Measures {
    /// Parameters passing by value because the declared type is a type parameter or `impl Trait`
    /// (P8) — one `take` each under a borrow default, and so the number to beat.
    pub p8_params: usize,
    /// `.clone()` arguments handed to one of those parameters, one diagnostic each.
    pub p8_clones: Vec<Diag>,
    /// What D2 costs: a `&` or `&mut` the author had to write because the callee is a Rust item
    /// whose signature Uredo does not resolve (§10.3). Uredo inserts the borrow for its own
    /// callees, so every one of these is an annotation the surface did not save.
    pub d2_refs: usize,
    /// A `&` or `&mut` written at a call site Uredo *does* resolve, where it would have inserted
    /// the borrow itself and emits what was written verbatim instead (D47). The pair says how much
    /// of the surface's borrow-insertion actually reaches a project's call sites.
    pub own_refs: usize,
    /// Lifetimes written in the source, by the position they appear in — the four buckets the
    /// corpus study counted, so a project's own rate can be set beside real Rust's. §38 makes the
    /// total per 1,000 lines the condition for reopening the notation question, and this is what
    /// produces it rather than a grep.
    pub lifetimes: Lifetimes,
}

/// Written lifetimes, split by position and by whether the name is `'static`.
#[derive(Debug, Default, Clone)]
pub struct Lifetimes {
    /// `&'a T`, `&'static str`
    pub in_reference: usize,
    /// `T: 'a`, `dyn Trait + 'static`
    pub in_bound: usize,
    /// `Cow<'a, str>`, `Token<'a>`, and the `<'a>` that declares one
    pub in_generics: usize,
    /// `'outer: loop`
    pub label: usize,
    /// how many of all of those are `'static`
    pub static_named: usize,
    /// how many are the anonymous `'_`, which is an apostrophe the author still had to write
    pub anonymous: usize,

    /// non-blank source lines, so the rate §38 asks for can be computed
    pub lines: usize,
}

impl Lifetimes {
    pub fn total(&self) -> usize {
        self.in_reference + self.in_bound + self.in_generics + self.label
    }
}

/// Counts the lifetimes of one file from its token stream, which is where they are unambiguous:
/// an apostrophe in a comment or a string literal is not one, and a regex full of them is not one
/// either — a lesson the metrics tokenizer had to learn twice.
pub fn count_lifetimes(toks: &[crate::lexer::Token], src: &str) -> Lifetimes {
    use crate::lexer::Tok;
    let mut out = Lifetimes { lines: src.lines().filter(|l| !l.trim().is_empty()).count(), ..Default::default() };
    for (i, t) in toks.iter().enumerate() {
        let Tok::Lifetime(name) = &t.tok else { continue };
        if name == "'static" {
            out.static_named += 1;
        } else if name == "'_" {
            out.anonymous += 1;
        }
        let before = toks[..i].iter().rev().find(|p| !matches!(p.tok, Tok::Newline));
        let after = toks.get(i + 1).map(|n| &n.tok);
        match before.map(|p| &p.tok) {
            Some(Tok::Punct("&")) => out.in_reference += 1,
            Some(Tok::Punct(":")) | Some(Tok::Punct("+")) => out.in_bound += 1,
            Some(Tok::Punct("<")) | Some(Tok::Punct(",")) => out.in_generics += 1,
            // `'outer: loop` defines a label; `break 'outer` uses one. The corpus study counted
            // occurrences rather than distinct labels, so both are counted here.
            Some(Tok::Ident(w)) if w == "break" || w == "continue" => out.label += 1,
            _ if matches!(after, Some(Tok::Punct(":"))) => out.label += 1,
            // `&mut 'a` is not a thing, but `&'a mut T` is: the `&` may be two tokens back
            _ => out.in_reference += 1,
        }
    }
    out
}

/// A `.clone()` argument at a P8 call site (§38): recorded, never warned about.
pub fn clone_at_by_value(callee: &str, arg: &str, declared: &str, line: usize, col: usize) -> Diag {
    Diag::note_at(line, col, format!("`{}` is cloned into `{}`, whose parameter passes by value (P8, D52)", cloned_value(arg), callee))
        .note(format!("`{}` declares: {}", callee, declared))
        .note("counted by `uredo lint --measure` against the `take` annotations a borrow default would have needed (§38); it is not a finding, and the clone may be right")
}

/// What was cloned, for the message: `(name.clone())` reads as `name`.
fn cloned_value(arg: &str) -> &str {
    let mut a = arg.trim();
    while let (Some(inner), true) = (a.strip_prefix('('), a.ends_with(')')) {
        a = inner[..inner.len() - 1].trim();
    }
    a.strip_suffix(".clone()").unwrap_or(a).trim()
}

/// Every lint's name, for `uredo lint --allow <name>` and for the documentation.
pub const NAMES: &[&str] = &["redundant_take", "borrowed_container", "copy_candidate"];

/// The lint a finding came from: each one ends with a `lint: <name>` note.
pub fn name_of(d: &Diag) -> Option<&str> {
    d.notes.last()?.strip_prefix("lint: ")
}

/// A forwarded `@allow`/`@expect` that names one of *Uredo's* lints (§23, §29.1).
///
/// `@name(args)` forwards verbatim, so the attribute reaches rustc, which does not know the name and
/// warns `unknown lint`; meanwhile the Uredo lint it was meant for keeps firing. The attribute is
/// still forwarded — Uredo never changes what reaches rustc, so that the day rustc ships a lint of
/// the same name a legitimate suppression still works — and this warning says what happened.
pub fn misdirected_allow(a: &Attr) -> Option<Diag> {
    let (kind, args) = match (a.name.as_str(), a.args.as_deref()) {
        ("allow" | "expect", Some(args)) => (a.name.as_str(), args),
        // `@rust(allow(…))` is the exact-forwarding spelling of the same thing
        ("rust", Some(inner)) => {
            let t = inner.trim();
            let kind = if t.starts_with("allow(") { "allow" } else if t.starts_with("expect(") { "expect" } else { return None };
            (kind, t[t.find('(')? + 1..t.rfind(')')?].trim())
        }
        _ => return None,
    };
    let named: Vec<&str> = args
        .split(',')
        .map(str::trim)
        .filter(|n| NAMES.contains(n))
        .collect();
    if named.is_empty() {
        return None;
    }
    let list = named.join("`, `");
    Some(
        Diag::warning(a.line, 1, format!("`{}` names {} lint, but an `{}` attribute forwards to Rust, which does not know it", list, if named.len() == 1 { "a Uredo" } else { "Uredo" }, kind))
            .note(format!("the attribute is passed through unchanged, so rustc will warn `unknown lint: {}` on the generated Rust", named[0]))
            .note(format!("`uredo lint --allow {}` silences the Uredo lint for a run; suppressing one in source is not decided (§38)", named[0])),
    )
}

/// `take` on a parameter that passes by value anyway (§10.2): the annotation says "transfer" where
/// nothing is transferred (a known-Copy type copies) or where the mode is already by value (P8).
pub fn redundant_take(p: &Param, lowered: &str, generic: bool, known_copy: bool) -> Option<Diag> {
    if p.mode != Mode::Take || p.is_pattern {
        return None;
    }
    let written = p.ty.text.trim();
    let (why, what) = if generic {
        ("a type parameter and `impl Trait` already pass by value (P8, D52)", "drop the `take`")
    } else if known_copy {
        ("a known-Copy type passes by value and copies rather than moves (P1)", "drop the `take`")
    } else {
        return None;
    };
    // The only finding whose rewrite provably leaves the generated Rust untouched — that is what
    // the lint says — so it is the only one `uredo fix` may apply (§28.1).
    let bare = written.trim_start_matches("take").trim_start();
    Some(
        Diag::warning(p.line, 1, format!("`take {}` lowers to the same `{}` as `{}` alone: {}", written, lowered, written, why))
            .note(format!("{} — nothing in the signature changes", what))
            .fixed_by(p.line, format!("{}: take {}", p.pat, bare), format!("{}: {}", p.pat, bare), format!("drop the `take` on `{}`", p.pat))
            .note("lint: redundant_take"),
    )
}

/// A parameter whose owned container is borrowed anyway (§10.2): `Vec<T>` passes as `&Vec<T>` and
/// `String` as `&String`, where the slice types accept strictly more arguments at no cost.
pub fn borrowed_container(p: &Param, borrowed: bool) -> Option<Diag> {
    if p.is_pattern || p.mode != Mode::Default {
        return None;
    }
    let written = p.ty.text.trim();
    // both the inferred borrow (`v: Vec<T>` → `&Vec<T>`) and the written one (`v: &Vec<T>`, P6)
    let bare = written.strip_prefix('&').map(str::trim).unwrap_or(written);
    if bare.starts_with("&") || !(borrowed || written.starts_with('&')) {
        return None;
    }
    let (suggest, kind) = if bare == "String" {
        ("str".to_string(), "String")
    } else if let Some(rest) = bare.strip_prefix("Vec<") {
        (format!("[{}]", rest.strip_suffix('>')?.trim()), "Vec")
    } else {
        return None;
    };
    Some(
        Diag::warning(p.line, 1, format!("`{}: {}` passes `&{}`; `{}` accepts more arguments at the same cost", p.pat, written, bare, suggest))
            .note(format!("write `{}: {}` — a `{}` argument still works, and so do slices and literals", p.pat, suggest, kind))
            .note("lint: borrowed_container"),
    )
}

/// A borrowed parameter (P3) whose every use in the body is a deref: the value is being copied out
/// of the reference, which is what a known-Copy type would do by itself (§10.1).
pub fn copy_candidate(p: &Param, body: &Block, declared_in_crate: bool) -> Option<Diag> {
    if p.is_pattern || p.mode != Mode::Default || p.ty.text.trim().starts_with('&') {
        return None;
    }
    let (total, deref) = uses(body, &p.pat);
    if total == 0 || total != deref {
        return None;
    }
    let ty = p.ty.text.trim();
    let fix = if declared_in_crate {
        format!("add `@derive(Copy)` to `{}`", ty)
    } else {
        format!("declare `@copy use …::{}` in this crate", last_segment(ty))
    };
    Some(
        Diag::warning(p.line, 1, format!("every use of `{}` is `*{}`, so `{}` is being copied out of a borrow", p.pat, p.pat, ty))
            .note(format!("if `{}` is `Copy`, {} and the parameter passes by value (§10.1)", ty, fix))
            .note("lint: copy_candidate"),
    )
}

fn last_segment(t: &str) -> &str {
    t.split('<').next().unwrap_or(t).rsplit("::").next().unwrap_or(t).trim()
}

/// `(uses of `name`, uses that are the operand of a `*`)` over a function body.
fn uses(body: &Block, name: &str) -> (usize, usize) {
    let mut c = Count { name, total: 0, deref: 0 };
    c.block(body);
    (c.total, c.deref)
}

struct Count<'a> {
    name: &'a str,
    total: usize,
    deref: usize,
}

impl Count<'_> {
    fn block(&mut self, b: &Block) {
        for s in &b.stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Bind { target, init, else_block, .. } => {
                self.expr(target);
                self.expr(init);
                if let Some(b) = else_block {
                    self.block(b);
                }
            }
            StmtKind::Assign { target, value, .. } => {
                self.expr(target);
                self.expr(value);
            }
            StmtKind::Expr(e) | StmtKind::Throw(e) => self.expr(e),
            StmtKind::Return(e) | StmtKind::Break { value: e, .. } => {
                if let Some(e) = e {
                    self.expr(e);
                }
            }
            StmtKind::Continue { .. } => {}
            StmtKind::While { cond, body, .. } => {
                self.expr(cond);
                self.block(body);
            }
            StmtKind::WhileLet { expr, body, .. } => {
                self.expr(expr);
                self.block(body);
            }
            StmtKind::For { iter, body, .. } => {
                self.expr(iter);
                self.block(body);
            }
            StmtKind::Unsafe(b) => self.block(b),
            // a nested item has its own parameters and body; its uses are not this parameter's
            StmtKind::Item(_) => {}
        }
    }

    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Path(p) => {
                if p == self.name {
                    self.total += 1;
                }
            }
            Expr::Unary { op, expr } => {
                if *op == "*" {
                    if let Expr::Path(p) = &**expr {
                        if p == self.name {
                            self.total += 1;
                            self.deref += 1;
                            return;
                        }
                    }
                }
                self.expr(expr);
            }
            Expr::Ref { expr, .. } | Expr::Cast { expr, .. } | Expr::Field { expr, .. } => self.expr(expr),
            Expr::Try(e) | Expr::Await(e) | Expr::Paren(e) => self.expr(e),
            Expr::Binary { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Expr::Call { callee, args, .. } => {
                self.expr(callee);
                for a in args {
                    self.expr(a);
                }
            }
            Expr::MethodCall { recv, args, .. } => {
                self.expr(recv);
                for a in args {
                    self.expr(a);
                }
            }
            Expr::Index { expr, index } => {
                self.expr(expr);
                self.expr(index);
            }
            Expr::Tuple(xs) | Expr::Array(xs) => {
                for x in xs {
                    self.expr(x);
                }
            }
            Expr::ArrayRepeat { value, count } => {
                self.expr(value);
                self.expr(count);
            }
            Expr::StructLit { fields, base, .. } => {
                for f in fields {
                    if let Some(v) = &f.value {
                        self.expr(v);
                    } else if f.name == self.name {
                        self.total += 1; // field punning uses the binding
                    }
                }
                if let Some(b) = base {
                    self.expr(b);
                }
            }
            Expr::Range { lo, hi, .. } => {
                for x in [lo, hi].into_iter().flatten() {
                    self.expr(x);
                }
            }
            Expr::Closure { body, .. } => self.expr(body),
            Expr::If(i) => self.if_expr(i),
            Expr::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                for a in arms {
                    if let Some(g) = &a.guard {
                        self.expr(g);
                    }
                    self.block(&a.body);
                }
            }
            Expr::Loop { body, .. } | Expr::Block(body) | Expr::UnsafeBlock(body) => self.block(body),
            Expr::Assign { target, value, .. } => {
                self.expr(target);
                self.expr(value);
            }
            // a macro body and a raw Rust block are opaque tokens (§22.5, §6.6); scan their text,
            // so that a parameter used only inside one is not reported as unused by a deref
            Expr::Rust(t) | Expr::Macro { body: t, .. } => {
                if mentions(t, self.name) {
                    self.total += 1;
                }
            }
            Expr::Int(_) | Expr::Float(_) | Expr::Str(_) | Expr::Char(_) | Expr::Bool(_) | Expr::Unit => {}
        }
    }

    fn if_expr(&mut self, i: &IfExpr) {
        self.expr(&i.cond);
        self.block(&i.then);
        match &i.else_ {
            Some(ElseBranch::ElseIf(e)) => self.if_expr(e),
            Some(ElseBranch::Else(b)) => self.block(b),
            None => {}
        }
    }
}

/// Whether opaque text mentions `name` as a whole word.
fn mentions(text: &str, name: &str) -> bool {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == name)
}
