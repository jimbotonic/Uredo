//! Lowering: Uredo AST -> Rust source text (§5, §10, §12, §25).
//!
//! Every elaboration here is decided from Uredo source and Uredo-declared signatures only
//! (§5.2). Where the type of an expression is unknown to Uredo, the expression is emitted
//! verbatim and rustc decides.

use crate::ast::*;
use crate::diag::{Diag, Level};
use crate::lint;
use crate::api::ApiEntry;
use crate::provenance::Elab;
use std::collections::{HashMap, HashSet};

/// Rust keywords that need `r#` when used as Uredo identifiers (§6.1).
const RUST_KEYWORDS: &[&str] = &[
    "abstract", "become", "box", "do", "final", "macro", "override", "priv", "try", "typeof", "unsized",
    "virtual", "yield", "type", "move", "ref", "impl", "dyn", "extern", "where", "mod", "trait", "as",
    "in", "loop", "match", "pub", "unsafe", "use", "while", "struct", "enum", "fn", "let", "mut",
    "static", "const", "if", "else", "for", "return", "break", "continue", "true", "false", "self",
    "Self", "super", "crate", "async", "await", "gen",
];

/// rustdoc's own attributes on a code fence, which say how to treat the block rather than what
/// language it is (§26.4).
const DOC_ATTRS: &[&str] = &["no_run", "should_panic", "compile_fail", "edition2015", "edition2018", "edition2021", "edition2024"];

const SCALARS: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64",
    "bool", "char", "()", "!",
];

/// Prelude Copy table (§10.1): std types passed by value. Matched on the last path segment.
const PRELUDE_COPY: &[&str] = &[
    "Duration", "Instant", "SystemTime", "Ordering", "SocketAddr", "SocketAddrV4", "SocketAddrV6", "IpAddr",
    "Ipv4Addr", "Ipv6Addr", "NonZeroU8", "NonZeroU16", "NonZeroU32", "NonZeroU64", "NonZeroU128",
    "NonZeroUsize", "NonZeroI8", "NonZeroI16", "NonZeroI32", "NonZeroI64", "NonZeroI128", "NonZeroIsize",
    "TypeId", "Layout", "PhantomData", "Wrapping", "Saturating", "Range", "RangeInclusive", "RangeTo",
    "RangeFrom", "RangeFull",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamMode {
    /// P1/P6/P7: the argument is passed as written.
    Verbatim,
    /// P5: `take` — passed as written, ownership moves into the callee.
    Take,
    /// P2/P3: `&arg` unless the argument is syntactically a reference (D47).
    Borrow,
    /// P8 (D52): the parameter's type is a type parameter or `impl Trait`; passed as written,
    /// by value — moved unless the argument's type is `Copy`.
    ByValueGeneric,
    /// P4: `&mut arg`; the argument must be a mutable place.
    BorrowMut,
}

#[derive(Debug, Clone)]
pub struct FnSig {
    pub modes: Vec<ParamMode>,
    pub ret: Option<String>,
    pub receiver: Option<Receiver>,
    /// The declaration as written in Uredo (`fn enqueue(job: take Job) -> Vec<Job>`).
    pub decl: String,
}

#[derive(Debug, Clone, Default)]
pub struct TypeInfo {
    pub is_copy: bool,
    pub fields: Vec<String>,
    pub methods: HashMap<String, FnSig>,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
struct Binding {
    mutable: bool,
    /// Declared type text, when annotated (or a parameter); kept for `explain` (§26.2).
    #[allow(dead_code)]
    ty: Option<String>,
    /// The declared type is syntactically a reference (D47).
    ref_typed: bool,
    /// `&mut T` or `inout` parameter: writes through it are allowed.
    mut_ref: bool,
    /// Inferred Uredo type name (struct literal, annotation, `Type::new`), for method lookups.
    uredo_ty: Option<String>,
    /// Uredo knows this binding's ownership (a declared local, parameter or borrowed loop
    /// variable). Pattern-bound names of unknown type are left to rustc (§8.1).
    known: bool,
}

#[derive(Default)]
struct Scope {
    bindings: HashMap<String, Binding>,
}

#[derive(Clone, Copy, PartialEq)]
enum BlockMode {
    /// Every statement ends with `;`.
    Unit,
    /// The last expression statement is the block's value.
    Value,
}

struct FnCtx {
    receiver: Option<Receiver>,
    self_ty: Option<String>,
    throws: Option<Option<String>>,
    #[allow(dead_code)]
    ret: Option<String>,
}

pub struct Lowerer {
    out: String,
    indent: usize,
    pub diags: Vec<Diag>,
    /// `uredo lint` findings (§28.1); collected during lowering, where the type facts are.
    pub lints: Vec<Diag>,
    /// The module declares `@!no_std`, which changes which intrinsics exist and where they live.
    no_std: bool,
    /// The D52 reversal counts (§38), collected in the same pass.
    pub measures: lint::Measures,
    fns: HashMap<String, FnSig>,
    types: HashMap<String, TypeInfo>,
    copy_types: HashSet<String>,
    scopes: Vec<Scope>,
    fn_stack: Vec<FnCtx>,
    glob_import: bool,
    copy_assert_emitted: bool,
    default_error: Option<String>,
    /// A tail `?` was lowered with the D37 match: emit the `__uredo_owned` helper once.
    needs_owned_helper: bool,
    /// Depth of nested closures with `move` (field sugar disabled inside).
    move_closure_depth: usize,
    /// Uredo line of the construct being emitted (provenance, §5.3).
    cur_line: usize,
    /// Elaboration records for `explain` and diagnostic translation.
    elabs: Vec<Elab>,
    src_lines: Vec<String>,
    /// Declarations of the other `.ure` files of this crate (§10.3 across modules).
    index: CrateIndex,
    /// The package name, so that a binary's `use name::item` resolves into the library root.
    crate_name: String,
    /// Public API entries collected while lowering (§4.4).
    api: Vec<ApiEntry>,
    /// Module path of the item being lowered (file module, then inline `mod` names).
    mod_path: Vec<String>,
    /// Attributes of the item being lowered, as emitted (for the API manifest).
    cur_attrs: Vec<String>,
    /// Depth of enclosing `@cfg`-guarded inline modules (D50: inherited conditionality).
    conditional_depth: usize,
    /// Type parameters in scope (fn, impl and struct generics), with their bound text.
    type_params: Vec<(String, String)>,
    /// The std trait being implemented, when lowering the methods of an `impl Trait for T`
    /// (G11, D53): its methods' parameter modes come from the trait, not from the table.
    trait_impl: Option<String>,
}

/// `@cfg`, `@cfg_attr`, or the same attributes forwarded through `@rust(...)` (D50).
fn is_cfg_attr(a: &Attr) -> bool {
    a.name == "cfg" || a.name == "cfg_attr" || (a.name == "rust" && a.args.as_deref().map(|x| x.trim_start().starts_with("cfg")).unwrap_or(false))
}

/// Signatures of every Uredo-declared item in the crate, keyed by module path relative to
/// the crate root (`shout`, `greet::shout`, `shapes::Rect`), so that a call into another
/// `.ure` file of the same crate is elaborated like a local one (§10.3).
#[derive(Debug, Clone, Default)]
pub struct CrateIndex {
    pub fns: HashMap<String, FnSig>,
    pub types: HashMap<String, TypeInfo>,
    /// The crate root's `@!default_error`, which §15.1 says covers the whole subtree: its example
    /// is annotated "crate root; a module may re-declare for its subtree". Each file was seeded
    /// only from its own inner attributes, so a bare `throws` in any module but the root was
    /// rejected — the spec's own example did not compile across a module boundary.
    pub default_error: Option<String>,
}

impl CrateIndex {
    pub fn merge(&mut self, other: CrateIndex) {
        self.fns.extend(other.fns);
        self.types.extend(other.types);
        // Only the root records one, and it is recorded once; whichever side has it, keep it.
        if self.default_error.is_none() {
            self.default_error = other.default_error;
        }
    }
}

/// Indexes one module's declarations under `prefix` (`""` for the crate root, `"greet"`,
/// `"net::client"`); inline `mod name:` bodies are indexed recursively.
pub fn index_module(m: &Module, prefix: &str) -> CrateIndex {
    let mut l = Lowerer::new("", &CrateIndex::default(), "");
    l.prepass(&m.items);
    let key = |name: &str| if prefix.is_empty() { name.to_string() } else { format!("{}::{}", prefix, name) };
    let mut idx = CrateIndex::default();
    // The crate root's declaration is a crate-wide fact, so it belongs in the index rather than
    // in the file that happens to carry it.
    if prefix.is_empty() {
        idx.default_error = m.inner_attrs.iter().find(|a| a.name == "default_error").and_then(|a| a.args.clone());
    }
    for (n, s) in l.fns {
        idx.fns.insert(key(&n), s);
    }
    for (n, t) in l.types {
        idx.types.insert(key(&n), t);
    }
    for it in &m.items {
        if let ItemKind::Mod { name, body: Some(items) } = &it.kind {
            let sub = Module { inner_docs: vec![], inner_attrs: vec![], items: items.clone() };
            idx.merge(index_module(&sub, &key(name)));
        }
    }
    idx
}

/// Per-line provenance marker appended inside the raw output and stripped at the end.
const MARK: char = '\u{1}';

pub struct Lowered {
    pub api: Vec<ApiEntry>,
    pub raw: String,
    /// `raw_map[i]` = Uredo line of raw line `i + 1` (0 = none).
    pub raw_map: Vec<usize>,
    pub elabs: Vec<Elab>,
    pub diags: Vec<Diag>,
    /// `uredo lint` findings (§28.1, `lint.rs`): warnings, collected but never printed by a build.
    pub lints: Vec<Diag>,
    /// The D52 reversal counts (§38): a measurement, not findings.
    pub measures: lint::Measures,
}

pub fn lower(m: &Module, src: &str) -> Lowered {
    lower_in_crate(m, src, &CrateIndex::default(), "", "")
}

pub fn lower_in_crate(m: &Module, src: &str, index: &CrateIndex, crate_name: &str, module_path: &str) -> Lowered {
    let mut l = Lowerer::new(src, index, crate_name);
    l.mod_path = module_path.split("::").filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
    // A module's own declaration re-declares for its subtree; without one, the crate root's
    // stands (§15.1). A crate *root* takes only its own: a package's lib and bin share one index,
    // so `src/lib.ure` and `src/main.ure` are two roots behind it, and the library's default must
    // not leak into the binary — where `crate::` names a different crate.
    if !module_path.is_empty() {
        l.default_error = index.default_error.clone();
    }
    for a in &m.inner_attrs {
        if a.name == "default_error" {
            l.default_error = a.args.clone();
        }
        if a.name == "no_std" {
            l.no_std = true;
        }
    }
    // `resolve_uses` before `prepass`, and the order is load-bearing. `prepass` records each
    // function's parameter modes, and a mode depends on whether the type is known-`Copy`
    // (§10.1) — which for an imported type is only known once the `use` has been resolved
    // against the crate index. Run the other way round, a `@derive(Copy)` type from another
    // module was recorded P3 at prepass and lowered P1 in the signature, so the declaration took
    // it by value and every call site in the same file inserted a borrow. `prepass` still uses
    // `insert` for its own declarations, so a local one shadows an import exactly as before.
    l.resolve_uses(&m.items);
    l.prepass(&m.items);
    l.check_module_alternates(&m.items);
    l.lower_module(m);
    l.finish()
}

impl Lowerer {
    fn new(src: &str, index: &CrateIndex, crate_name: &str) -> Lowerer {
        Lowerer {
        out: String::new(),
        indent: 0,
        diags: Vec::new(),
        lints: Vec::new(),
        no_std: false,
        measures: lint::Measures::default(),
        fns: HashMap::new(),
        types: HashMap::new(),
        copy_types: PRELUDE_COPY.iter().map(|s| s.to_string()).collect(),
        scopes: vec![Scope::default()],
        fn_stack: Vec::new(),
        glob_import: false,
        copy_assert_emitted: false,
        default_error: None,
        needs_owned_helper: false,
        move_closure_depth: 0,
        cur_line: 0,
        elabs: Vec::new(),
        src_lines: src.lines().map(|s| s.to_string()).collect(),
        index: index.clone(),
        crate_name: crate_name.to_string(),
        api: Vec::new(),
        mod_path: Vec::new(),
        cur_attrs: Vec::new(),
        conditional_depth: 0,
        type_params: Vec::new(),
        trait_impl: None,
        }
    }

    /// The parameter mode a std trait method fixes (D53): `value`, `ref` or `refmut` for the
    /// parameter at `index` (receivers excluded), or None when the trait is not in the table.
    fn std_trait_param_mode(&self, trait_name: &str, method: &str, index: usize) -> Option<&'static str> {
        // `std::ops::Add<Vec2>` -> `Add`; strip the generic arguments before taking the last segment
        let no_generics = trait_name.split('<').next().unwrap_or(trait_name);
        let t = no_generics.rsplit("::").next().unwrap_or(no_generics).trim();
        let m = method;
        let mode = match (t, m) {
            ("From", "from") | ("TryFrom", "try_from") | ("Into", "into") | ("TryInto", "try_into") => "value",
            ("FromStr", "from_str") => "ref",
            ("PartialEq", "eq") | ("PartialEq", "ne") => "ref",
            ("PartialOrd", "partial_cmp") | ("PartialOrd", "lt") | ("PartialOrd", "le") | ("PartialOrd", "gt") | ("PartialOrd", "ge") => "ref",
            ("Ord", "cmp") | ("Ord", "max") | ("Ord", "min") | ("Ord", "clamp") => if m == "cmp" { "ref" } else { "value" },
            ("Hash", "hash") => "refmut",
            ("Hasher", "write") => "ref",
            ("Display", "fmt") | ("Debug", "fmt") | ("LowerHex", "fmt") | ("UpperHex", "fmt") | ("Binary", "fmt") | ("Octal", "fmt") | ("Pointer", "fmt") | ("LowerExp", "fmt") | ("UpperExp", "fmt") => "refmut",
            ("Add", "add") | ("Sub", "sub") | ("Mul", "mul") | ("Div", "div") | ("Rem", "rem") | ("BitAnd", "bitand") | ("BitOr", "bitor") | ("BitXor", "bitxor") | ("Shl", "shl") | ("Shr", "shr") => "value",
            ("AddAssign", "add_assign") | ("SubAssign", "sub_assign") | ("MulAssign", "mul_assign") | ("DivAssign", "div_assign") | ("RemAssign", "rem_assign") | ("BitAndAssign", "bitand_assign") | ("BitOrAssign", "bitor_assign") | ("BitXorAssign", "bitxor_assign") | ("ShlAssign", "shl_assign") | ("ShrAssign", "shr_assign") => "value",
            ("Index", "index") | ("IndexMut", "index_mut") => "value",
            ("Extend", "extend") | ("FromIterator", "from_iter") => "value",
            ("Write", "write") | ("Write", "write_all") => "ref",
            ("Write", "write_fmt") => "value",
            ("Read", "read") | ("Read", "read_exact") => "refmut",
            ("Read", "read_to_end") | ("Read", "read_to_string") => "refmut",
            ("Seek", "seek") => "value",
            ("Clone", "clone_from") => "ref",
            ("Iterator", "nth") | ("Iterator", "step_by") => "value",
            ("Serialize", "serialize") | ("Deserialize", "deserialize") => "value",
            ("Visitor", "visit_str") | ("Visitor", "visit_string") | ("Visitor", "visit_u64") | ("Visitor", "visit_i64") | ("Visitor", "visit_f64") | ("Visitor", "visit_bool") => "value",
            ("Visitor", "expecting") => "refmut",
            ("Error", "provide") => "refmut",
            ("Future", "poll") | ("AsyncRead", "poll_read") | ("AsyncWrite", "poll_write") => "value",
            _ => return None,
        };
        let _ = index;
        Some(mode)
    }

    /// Whether a parameter type is a type parameter in scope or an `impl Trait`, and whether it
    /// carries a `?Sized` bound (which forces a borrow under every rule).
    fn generic_param_kind(&self, t: &str) -> Option<bool> {
        if let Some(rest) = t.strip_prefix("impl ") {
            return Some(rest.contains("?Sized"));
        }
        self.type_params.iter().find(|(n, _)| n == t).map(|(_, b)| b.contains("?Sized"))
    }

    /// D50: two inline modules with the same name (configuration alternates) must declare the
    /// same elaboration properties for same-named items, since the crate is lowered once.
    fn check_module_alternates(&mut self, items: &[Item]) {
        let mut seen: HashMap<String, (usize, CrateIndex)> = HashMap::new();
        for it in items {
            let ItemKind::Mod { name, body: Some(body) } = &it.kind else { continue };
            let sub = Module { inner_docs: vec![], inner_attrs: vec![], items: body.clone() };
            let idx = index_module(&sub, "");
            if let Some((_, prev)) = seen.get(name) {
                for (k, sig) in &idx.fns {
                    if let Some(p) = prev.fns.get(k) {
                        if !Self::sigs_agree(p, sig) {
                            self.diags.push(Diag::error(it.line, 1, format!("module alternate `{}` declares `{}` with different passing modes, receiver or return type than an earlier `mod {}`; Uredo lowers once for all configurations (D50)", name, k, name)));
                        }
                    }
                }
                for (t, info) in &idx.types {
                    if let Some(p) = prev.types.get(t) {
                        if p.is_copy != info.is_copy {
                            self.diags.push(Diag::error(it.line, 1, format!("module alternate `{}` declares `{}` with a different `Copy` than an earlier `mod {}` (D50)", name, t, name)));
                        }
                        for (m, sig) in &info.methods {
                            if let Some(ps) = p.methods.get(m) {
                                if !Self::sigs_agree(ps, sig) {
                                    self.diags.push(Diag::error(it.line, 1, format!("module alternate `{}` declares `{}::{}` with different passing modes than an earlier `mod {}` (D50)", name, t, m, name)));
                                }
                            }
                        }
                    }
                }
            } else {
                seen.insert(name.clone(), (it.line, idx));
            }
        }
    }

    fn api_path(&self, name: &str) -> String {
        let mut p = self.mod_path.clone();
        p.push(name.to_string());
        p.join("::")
    }

    /// Records a public item (§4.4). Only plain `pub` is external API.
    fn api(&mut self, vis: &str, kind: &str, path: String, signature: String, members: Vec<String>) {
        if vis.trim() != "pub" {
            return;
        }
        let attrs: Vec<String> = self.cur_attrs.iter().filter(|a| a.starts_with("[derive") || a.starts_with("[cfg") || a.starts_with("[repr") || a.starts_with("[non_exhaustive") || a.starts_with("[must_use") || a.starts_with("[deprecated")).map(|a| a.trim_start_matches('[').trim_end_matches(']').to_string()).collect();
        self.api.push(ApiEntry { path, kind: kind.to_string(), signature, attrs, members });
    }

    /// Normalises a path to the index's keys: strips `crate::`, `self::` and the package name.
    fn index_key(&self, path: &str) -> String {
        let p = path.trim_start_matches("::");
        let p = p.strip_prefix("crate::").or_else(|| p.strip_prefix("self::")).unwrap_or(p);
        if !self.crate_name.is_empty() {
            if let Some(rest) = p.strip_prefix(&format!("{}::", self.crate_name)) {
                return rest.to_string();
            }
        }
        p.to_string()
    }

    /// `use` items bring other files' declarations into this module's tables (§20.1).
    fn resolve_uses(&mut self, items: &[Item]) {
        for it in items {
            let ItemKind::Use { path, .. } = &it.kind else { continue };
            for (full, local) in use_names(path) {
                let key = self.index_key(&full);
                if let Some(s) = self.index.fns.get(&key).cloned() {
                    self.fns.entry(local.clone()).or_insert(s);
                }
                if let Some(t) = self.index.types.get(&key).cloned() {
                    self.types.entry(local.clone()).or_insert(t);
                }
            }
        }
    }

    /// A qualified callee (`greet::shout`, `shapes::Rect::new`) looked up in the crate index.
    fn index_fn(&self, path: &str) -> Option<FnSig> {
        let key = self.index_key(path);
        if let Some(s) = self.index.fns.get(&key) {
            return Some(s.clone());
        }
        let (ty, f) = key.rsplit_once("::")?;
        self.index.types.get(ty).and_then(|t| t.methods.get(f)).cloned()
    }

    fn finish(self) -> Lowered {
    let l = self;
    // strip the markers into the line map
    let mut raw = String::new();
    let mut raw_map = Vec::new();
    for line in l.out.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let mut ure = 0;
        let mut clean = String::new();
        let mut parts = body.split(MARK);
        clean.push_str(parts.next().unwrap_or(""));
        for p in parts {
            // marker payload: digits, then possibly more text
            let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
            if ure == 0 {
                ure = digits.parse().unwrap_or(0);
            }
            clean.push_str(&p[digits.len()..]);
        }
        raw.push_str(&clean);
        raw.push('\n');
        raw_map.push(ure);
    }
    Lowered { api: l.api, raw, raw_map, elabs: l.elabs, diags: l.diags, lints: l.lints, measures: l.measures }
    }
}

fn last_segment(ty: &str) -> &str {
    let t = ty.trim();
    let t = t.split('<').next().unwrap_or(t);
    t.rsplit("::").next().unwrap_or(t).trim()
}

/// `str` and `[T]` denote `&str` and `&[T]` in a parameter or a return type (§9.7). The optional
/// suffix composes with that rule: `str?` is `Option<&str>`, not the unsized `Option<str>`, which
/// no value can have. Only the suffixed forms are rewritten here; the bare ones are already handled
/// where the mode is chosen. Everything else, `Box<str>` and `[u8; 4]?` included, is returned as it
/// was written.
fn borrow_unsized_source(t: &str) -> String {
    let mut core = t.trim();
    let mut opts = 0;
    while let Some(r) = core.strip_suffix('?') {
        opts += 1;
        core = r.trim_end();
    }
    if opts == 0 || (core != "str" && !is_slice_type(core)) {
        return t.trim().to_string();
    }
    format!("&{}{}", core, "?".repeat(opts))
}

fn is_slice_type(t: &str) -> bool {
    let t = t.trim();
    t.starts_with('[') && t.ends_with(']') && !contains_top_level_semicolon(t)
}

fn contains_top_level_semicolon(t: &str) -> bool {
    let mut depth = 0;
    for c in t.chars() {
        match c {
            '[' | '(' | '<' => depth += 1,
            ']' | ')' | '>' => depth -= 1,
            ';' if depth == 1 => return true,
            _ => {}
        }
    }
    false
}

fn split_top_level(t: &str, sep: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in t.chars() {
        match c {
            '[' | '(' | '<' => {
                depth += 1;
                cur.push(c);
            }
            ']' | ')' | '>' => {
                depth -= 1;
                cur.push(c);
            }
            c if c == sep && depth == 0 => {
                parts.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

impl Lowerer {
    // ----- output helpers -----
    /// The closing brace of a block, attributed to the line its *header* was on.
    ///
    /// Without this a debugger stopping on a closing brace is told it came from the last statement
    /// inside the block, because `cur_line` has advanced by then (D29, §28.2).
    fn close_block(&mut self, header_line: usize) {
        let text = mark_lines("}", header_line);
        self.line(&text);
    }

    fn line(&mut self, s: &str) {
        for (i, piece) in s.split('\n').enumerate() {
            if i == 0 {
                for _ in 0..self.indent {
                    self.out.push_str("    ");
                }
            }
            self.out.push_str(piece);
            if !piece.contains(MARK) && !piece.is_empty() {
                self.out.push(MARK);
                self.out.push_str(&self.cur_line.to_string());
            }
            self.out.push('\n');
        }
    }
    fn elab(&mut self, e: Elab) {
        self.elabs.push(e);
    }
    /// Uredo source text of a call starting at the callee before `(` at (line, col).
    fn call_source(&self, line: usize, col: usize) -> String {
        let Some(src) = self.src_lines.get(line.wrapping_sub(1)) else { return String::new() };
        let chars: Vec<char> = src.chars().collect();
        let open = col.saturating_sub(1).min(chars.len());
        // walk back over the callee path
        let mut start = open;
        while start > 0 && (chars[start - 1].is_alphanumeric() || matches!(chars[start - 1], '_' | ':' | '.' | '<' | '>')) {
            start -= 1;
        }
        let mut depth = 0;
        let mut end = open;
        while end < chars.len() {
            match chars[end] {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end += 1;
                        break;
                    }
                }
                _ => {}
            }
            end += 1;
        }
        chars[start..end.min(chars.len())].iter().collect()
    }
    fn blank(&mut self) {
        self.out.push('\n');
    }
    fn err(&mut self, line: usize, col: usize, msg: impl Into<String>) {
        self.diags.push(Diag::error(line, col, msg));
    }
    fn warn(&mut self, line: usize, col: usize, msg: impl Into<String>) {
        let mut d = Diag::error(line, col, msg);
        d.level = Level::Warning;
        self.diags.push(d);
    }

    // ----- known-Copy and passing modes (§10.1, §10.2) -----
    fn is_known_copy(&self, ty: &str) -> bool {
        let t = ty.trim();
        if SCALARS.contains(&t) {
            return true;
        }
        if t.starts_with('&') && !t.starts_with("&mut") {
            return true;
        }
        if t.starts_with('(') && t.ends_with(')') {
            let inner = &t[1..t.len() - 1];
            return split_top_level(inner, ',').iter().all(|p| self.is_known_copy(p));
        }
        if t.starts_with('[') && t.ends_with(']') && contains_top_level_semicolon(t) {
            let inner = &t[1..t.len() - 1];
            let elem = split_top_level(inner, ';').into_iter().next().unwrap_or_default();
            return self.is_known_copy(&elem);
        }
        // D54: `Option<T>` and `Result<T, E>` are known-Copy exactly when their payloads are —
        // the rule §10.1 already applies to tuples and arrays, and Rust's own impls say the same.
        {
            let seg = last_segment(t);
            if matches!(seg, "Option" | "Result") {
                if let Some(open) = t.find('<') {
                    let inner = &t[open + 1..t.rfind('>').unwrap_or(t.len())];
                    return split_top_level(inner, ',').iter().all(|a| self.is_known_copy(a));
                }
            }
        }
        // D54: a type parameter whose declared bounds include `Copy` is known-Copy. The bound sits
        // in the crate's own text, so the payload of `T?` stays decidable from the declaration (D14).
        if let Some((_, bounds)) = self.type_params.iter().find(|(n, _)| n == t) {
            if bounds.split('+').any(|b| { let b = b.trim(); b == "Copy" || b.ends_with("::Copy") }) {
                return true;
            }
        }
        if std::env::var("UREDO_OPTION_MODE").map(|v| v == "payload").unwrap_or(false) {
            let seg = last_segment(t);
            if matches!(seg, "Option" | "Result") {
                if let Some(open) = t.find('<') {
                    let inner = &t[open + 1..t.rfind('>').unwrap_or(t.len())];
                    return split_top_level(inner, ',').iter().all(|a| self.is_known_copy(a));
                }
            }
        }
        if t == "Self" {
            if let Some(ctx) = self.fn_stack.last() {
                if let Some(st) = &ctx.self_ty {
                    return self.types.get(st).map(|i| i.is_copy).unwrap_or(false);
                }
            }
            return false;
        }
        let seg = last_segment(t);
        if self.types.get(seg).map(|i| i.is_copy).unwrap_or(false) {
            return true;
        }
        // `@copy use path::Type<Args>` declares that one instantiation, not the whole family: a
        // generic foreign type is `Copy` only for some arguments (§10.1, D56)
        if t.contains('<') && self.copy_types.contains(&segment_with_args(t)) {
            return true;
        }
        if self.copy_types.contains(seg) {
            // generic std Copy wrappers are Copy only when their parameter is
            if let Some(inner) = t.find('<').map(|i| &t[i + 1..t.len() - 1]) {
                if matches!(seg, "Wrapping" | "Saturating" | "Range" | "RangeInclusive" | "RangeTo" | "RangeFrom") {
                    return self.is_known_copy(inner);
                }
            }
            return true;
        }
        false
    }

    /// Rust parameter type and call-site mode for a Uredo parameter (§10.2 rows P1–P7).
    fn param_lowering(&self, p: &Param) -> (String, ParamMode, bool) {
        let lowered = lower_type(&borrow_unsized_source(&p.ty.text));
        let t = lowered.as_str();
        if p.is_pattern {
            return (t.to_string(), ParamMode::Verbatim, t.starts_with('&'));
        }
        match p.mode {
            Mode::Take => (t.to_string(), ParamMode::Verbatim, false),
            Mode::Inout => (format!("&mut {}", t), ParamMode::BorrowMut, true),
            Mode::Default => {
                if t.starts_with('&') {
                    return (t.to_string(), ParamMode::Verbatim, true);
                }
                if t == "str" {
                    return ("&str".to_string(), ParamMode::Borrow, true);
                }
                if is_slice_type(t) {
                    return (format!("&{}", t), ParamMode::Borrow, true);
                }
                if t.starts_with("dyn ") {
                    return (format!("&{}", t), ParamMode::Borrow, true);
                }
                if self.is_known_copy(t) {
                    return (t.to_string(), ParamMode::Verbatim, false);
                }
                // P8 (D52): a type parameter or `impl Trait` passes by value; `?Sized` keeps the borrow
                if let Some(unsized_) = self.generic_param_kind(t) {
                    if !unsized_ {
                        return (t.to_string(), ParamMode::ByValueGeneric, false);
                    }
                }
                (format!("&{}", t), ParamMode::Borrow, true)
            }
        }
    }

    /// Doc lines with their code examples lowered (§26.4, D58). A fence with no tag, or tagged
    /// `uredo`, holds Uredo source: it is lowered so that rustdoc compiles and runs it like any
    /// other doctest, and its attributes (`no_run`, `should_panic`, …) are carried across. A fence
    /// tagged `rust` is Rust already and passes through; any other tag (`text`, `ignore`, …) is
    /// rendered and never compiled, by rustdoc's own rule.
    fn doc_lines(&mut self, docs: &[String]) -> Vec<String> {
        if !docs.iter().any(|d| d.trim_start().starts_with("```")) {
            return docs.to_vec();
        }
        let mut out: Vec<String> = Vec::new();
        let mut i = 0;
        while i < docs.len() {
            let line = &docs[i];
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("```") else {
                out.push(line.clone());
                i += 1;
                continue;
            };
            let tags: Vec<&str> = rest.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
            let uredo_example = tags.is_empty() || tags[0] == "uredo" || (tags[0] != "rust" && DOC_ATTRS.contains(&tags[0]));
            let mut body: Vec<String> = Vec::new();
            let mut j = i + 1;
            while j < docs.len() && !docs[j].trim_start().starts_with("```") {
                body.push(docs[j].clone());
                j += 1;
            }
            if !uredo_example || j >= docs.len() {
                // Rust, prose, or an unterminated fence: rendered as written
                out.push(line.clone());
                out.extend(body);
                if j < docs.len() {
                    out.push(docs[j].clone());
                }
                i = j + 1;
                continue;
            }
            let attrs: Vec<&str> = tags.iter().copied().filter(|t| *t != "uredo").collect();
            let fence = if attrs.is_empty() { "```".to_string() } else { format!("```{}", attrs.join(",")) };
            match self.lower_doc_example(&body) {
                Some(rust) => {
                    out.push(fence);
                    out.extend(rust);
                    out.push("```".to_string());
                }
                None => {
                    out.push(line.clone());
                    out.extend(body);
                    out.push("```".to_string());
                }
            }
            i = j + 1;
        }
        out
    }

    /// Lowers one doc example by wrapping it in a function, so the parser sees a body, and taking
    /// the body back out. A example that does not compile is an error on the item's own line.
    fn lower_doc_example(&mut self, body: &[String]) -> Option<Vec<String>> {
        let mut src = String::from("fn __uredo_doc_example():\n");
        for l in body {
            if l.trim().is_empty() {
                src.push('\n');
            } else {
                src.push_str("    ");
                src.push_str(l);
                src.push('\n');
            }
        }
        let out = crate::compile(&src, true);
        if out.has_errors() {
            let first = out.diags.iter().find(|d| d.level == Level::Error);
            let msg = first.map(|d| d.msg.clone()).unwrap_or_else(|| "the example did not lower".into());
            self.diags.push(
                Diag::error(self.cur_line, 1, format!("a documentation example does not compile: {}", msg))
                    .note("a fenced block in a `##` comment is Uredo source and is lowered into a doctest (§26.4)")
                    .note("write ```` ```rust ```` for a block that is already Rust, or ```` ```text ```` for one that is neither"),
            );
            return None;
        }
        let lines: Vec<&str> = out.rust.lines().collect();
        let open = lines.iter().position(|l| l.starts_with("fn __uredo_doc_example()"))?;
        let close = lines.iter().rposition(|l| l.trim_end() == "}")?;
        if close <= open {
            return None;
        }
        Some(lines[open + 1..close].iter().map(|l| l.strip_prefix("    ").unwrap_or(l).to_string()).collect())
    }

    /// The `uredo lint` findings for one parameter (§28.1, `lint.rs`). Called where the passing
    /// mode has just been decided, so a lint reads the same facts the signature does.
    fn collect_lints(&mut self, p: &Param, lowered_ty: &str, mode: ParamMode, body: Option<&Block>) {
        let written = lower_type(p.ty.text.trim());
        let generic = self.generic_param_kind(&written).map(|unsized_| !unsized_).unwrap_or(false);
        if let Some(d) = lint::redundant_take(p, lowered_ty, generic, self.is_known_copy(&written)) {
            self.lints.push(d);
        }
        if let Some(d) = lint::borrowed_container(p, mode == ParamMode::Borrow) {
            self.lints.push(d);
        }
        if mode == ParamMode::Borrow && !is_slice_type(&written) && written != "str" && !written.starts_with("dyn ") {
            if let Some(body) = body {
                let declared = self.types.contains_key(last_segment(&written));
                if let Some(d) = lint::copy_candidate(p, body, declared) {
                    self.lints.push(d);
                }
            }
        }
    }

    fn return_type(&self, t: &Type) -> String {
        let owned = borrow_unsized_source(&t.text);
        let s = owned.as_str();
        if s == "str" {
            return "&str".to_string();
        }
        if is_slice_type(s) {
            return format!("&{}", s);
        }
        lower_type(s)
    }

    /// `@derive(…, Copy, …)`; a conditional derive is rejected (D50).
    fn copy_derive(&mut self, it: &Item) -> bool {
        let conditional_copy = it.attrs.iter().any(|a| (a.name == "cfg_attr" || a.name == "rust") && a.args.as_deref().map(|x| x.contains("derive") && x.contains("Copy")).unwrap_or(false));
        if conditional_copy {
            self.diags.push(Diag::error(it.line, 1, "`Copy` may not be derived conditionally: Copy-ness is an elaboration input and Uredo lowers once for all configurations (D50)"));
        }
        let derives_copy = it.attrs.iter().any(|a| a.name == "derive" && a.args.as_deref().map(|x| x.split(',').any(|d| d.trim() == "Copy")).unwrap_or(false));
        if derives_copy && self.conditional_depth > 0 {
            self.diags.push(Diag::error(it.line, 1, "a `Copy` type may not be declared inside a `@cfg`-guarded module: Copy-ness is an elaboration input and Uredo lowers once for all configurations (D50)"));
        }
        derives_copy
    }

    /// What Uredo elaborates from: parameter modes, receiver, and whether the return is `str`/`[T]`.
    fn sigs_agree(a: &FnSig, b: &FnSig) -> bool {
        let ret_kind = |s: &FnSig| s.ret.as_deref().map(|r| {
            let t = r.trim();
            let n = borrow_unsized_source(t);
            n != t || n == "str" || is_slice_type(&n)
        }).unwrap_or(false);
        a.modes == b.modes && a.receiver == b.receiver && ret_kind(a) == ret_kind(b)
    }

    fn sig_of(&self, f: &FnDecl) -> FnSig {
        let mut me = self.with_generics(f.generics.as_deref(), f.where_clause.as_deref());
        let sig = FnSig {
            modes: f
                .params
                .iter()
                .map(|p| {
                    let mode = me.param_lowering(p).1;
                    if p.mode == Mode::Take { ParamMode::Take } else { mode }
                })
                .collect(),
            ret: f.ret.as_ref().map(|t| t.text.clone()),
            receiver: f.receiver.clone(),
            decl: fn_decl_text(f),
        };
        me.type_params.truncate(self.type_params.len());
        sig
    }

    /// A shallow view of `self` with extra type parameters in scope (for signature computation).
    fn with_generics(&self, generics: Option<&str>, where_clause: Option<&str>) -> GenericsView<'_> {
        let mut tp = self.type_params.clone();
        tp.extend(generic_bounds(generics, where_clause));
        GenericsView { inner: self, type_params: tp }
    }

    // ----- pre-pass: collect declarations -----
    fn prepass(&mut self, items: &[Item]) {
        // First: types (so that `@derive(Copy)` is known when signatures are computed).
        for it in items {
            match &it.kind {
                ItemKind::Struct(s) => {
                    let is_copy = self.copy_derive(it);
                    let fields: Vec<String> = match &s.kind {
                        StructKind::Named(fs) => fs.iter().map(|f| f.name.clone()).collect(),
                        _ => vec![],
                    };
                    if let Some(prev) = self.types.get_mut(&s.name) {
                        // a configuration alternate (D50): Copy-ness must agree; fields are merged
                        if prev.is_copy != is_copy {
                            self.diags.push(Diag::error(it.line, 1, format!("`{}` has configuration alternates that differ in `Copy`; Uredo lowers once for all configurations (D50) — derive `Copy` on both or on neither", s.name)));
                        }
                        for f in fields {
                            if !prev.fields.contains(&f) {
                                prev.fields.push(f);
                            }
                        }
                    } else {
                        self.types.insert(s.name.clone(), TypeInfo { is_copy, fields, methods: HashMap::new(), variants: vec![] });
                    }
                }
                ItemKind::Enum(e) => {
                    let is_copy = self.copy_derive(it);
                    let variants: Vec<String> = e.variants.iter().map(|v| variant_name(&v.text)).collect();
                    if let Some(prev) = self.types.get_mut(&e.name) {
                        if prev.is_copy != is_copy {
                            self.diags.push(Diag::error(it.line, 1, format!("`{}` has configuration alternates that differ in `Copy`; Uredo lowers once for all configurations (D50) — derive `Copy` on both or on neither", e.name)));
                        }
                        for v in variants {
                            if !prev.variants.contains(&v) {
                                prev.variants.push(v);
                            }
                        }
                    } else {
                        self.types.insert(e.name.clone(), TypeInfo { is_copy, fields: vec![], methods: HashMap::new(), variants });
                    }
                }
                ItemKind::Use { path, copy } => {
                    if *copy {
                        if it.attrs.iter().any(is_cfg_attr) || self.conditional_depth > 0 {
                            self.diags.push(Diag::error(it.line, 1, "`@copy use` may not be conditional (nor sit in a `@cfg`-guarded module): Copy-ness is an elaboration input and Uredo lowers once for all configurations (D50)"));
                        }
                        self.copy_types.insert(copy_use_type(path));
                    }
                    if path.trim_end().ends_with('*') {
                        self.glob_import = true;
                    }
                }
                _ => {}
            }
            for a in &it.attrs {
                if a.inner && a.name == "default_error" {
                    self.default_error = a.args.clone();
                }
            }
        }
        // Second: signatures.
        for it in items {
            match &it.kind {
                ItemKind::Fn(f) => {
                    let sig = self.sig_of(f);
                    if let Some(prev) = self.fns.get(&f.name) {
                        if !Self::sigs_agree(prev, &sig) {
                            self.diags.push(Diag::error(f.line, 1, format!("`{}` has configuration alternates with different passing modes, receiver or return type; Uredo lowers once for all configurations (D50) — make the alternates agree, or move the difference into a `rust {{ }}` block", f.name)));
                        }
                    }
                    self.fns.insert(f.name.clone(), sig);
                }
                ItemKind::Struct(s) => {
                    let saved_tp = self.type_params.len();
                    self.type_params.extend(generic_bounds(s.generics.as_deref(), s.where_clause.as_deref()));
                    let mut methods = HashMap::new();
                    for n in &s.nested {
                        match n {
                            Nested::Method(m) => {
                                if let ItemKind::Fn(f) = &m.kind {
                                    methods.insert(f.name.clone(), self.sig_of(f));
                                }
                            }
                            Nested::TraitImpl { items, .. } => {
                                for m in items {
                                    if let ItemKind::Fn(f) = &m.kind {
                                        methods.insert(f.name.clone(), self.sig_of(f));
                                    }
                                }
                            }
                        }
                    }
                    self.types.get_mut(&s.name).unwrap().methods.extend(methods);
                    self.type_params.truncate(saved_tp);
                }
                ItemKind::Enum(e) => {
                    let saved_tp = self.type_params.len();
                    self.type_params.extend(generic_bounds(e.generics.as_deref(), e.where_clause.as_deref()));
                    let mut methods = HashMap::new();
                    for n in &e.nested {
                        match n {
                            Nested::Method(m) => {
                                if let ItemKind::Fn(f) = &m.kind {
                                    methods.insert(f.name.clone(), self.sig_of(f));
                                }
                            }
                            Nested::TraitImpl { items, .. } => {
                                for m in items {
                                    if let ItemKind::Fn(f) = &m.kind {
                                        methods.insert(f.name.clone(), self.sig_of(f));
                                    }
                                }
                            }
                        }
                    }
                    self.types.get_mut(&e.name).unwrap().methods.extend(methods);
                    self.type_params.truncate(saved_tp);
                }
                _ => {}
            }
        }
        // Third: fix up impl-method modes now that everything is known.
        let mut fixes = Vec::new();
        for it in items {
            if let ItemKind::Impl(i) = &it.kind {
                if i.trait_path.as_deref().map(|t| t.starts_with("trait ")).unwrap_or(false) {
                    continue;
                }
                let name = last_segment(&i.self_ty).to_string();
                if self.types.contains_key(&name) {
                    let saved_tp = self.type_params.len();
                    self.type_params.extend(generic_bounds(i.generics.as_deref(), i.where_clause.as_deref()));
                    for m in &i.items {
                        if let ItemKind::Fn(f) = &m.kind {
                            fixes.push((name.clone(), f.name.clone(), self.sig_of(f)));
                        }
                    }
                    self.type_params.truncate(saved_tp);
                }
            }
        }
        for (t, n, sig) in fixes {
            let info = self.types.get_mut(&t).unwrap();
            if let Some(prev) = info.methods.get(&n) {
                if !Self::sigs_agree(prev, &sig) {
                    let line = items.iter().find_map(|it| if let ItemKind::Impl(i) = &it.kind { if last_segment(&i.self_ty) == t { Some(it.line) } else { None } } else { None }).unwrap_or(0);
                    self.diags.push(Diag::error(line, 1, format!("`{}::{}` has configuration alternates with different passing modes, receiver or return type; Uredo lowers once for all configurations (D50)", t, n)));
                }
            }
            info.methods.insert(n, sig);
        }
    }

    // ----- scopes -----
    fn push_scope(&mut self) {
        self.scopes.push(Scope::default());
    }
    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    fn lookup(&self, name: &str) -> Option<&Binding> {
        for s in self.scopes.iter().rev() {
            if let Some(b) = s.bindings.get(name) {
                return Some(b);
            }
        }
        None
    }
    fn bind(&mut self, name: &str, b: Binding) {
        self.scopes.last_mut().unwrap().bindings.insert(name.to_string(), b);
    }
    /// Binds the names of a pattern. Their types are unknown to Uredo (they come from a
    /// scrutinee, an iterator or a closure argument), so writes through them are rustc's call.
    fn bind_pattern_names(&mut self, pat: &str, mutable: bool) {
        for n in pattern_idents(pat) {
            // `ref mut name` binds a mutable reference: writes through it are allowed.
            let mut_ref = pat.contains(&format!("ref mut {}", n));
            self.bind(&n, Binding { mutable, ty: None, ref_typed: false, mut_ref, uredo_ty: None, known: false });
        }
    }

    // ----- module -----
    fn lower_module(&mut self, m: &Module) {
        for d in self.doc_lines(&m.inner_docs) {
            self.line(&format!("//!{}", if d.is_empty() { String::new() } else { format!(" {}", d) }));
        }
        for a in &m.inner_attrs {
            if a.name == "default_error" {
                continue;
            }
            if let Some(d) = lint::misdirected_allow(a) {
                self.diags.push(d);
            }
            let s = attr_text(a);
            self.line(&format!("#!{}", s));
        }
        let mut first = true;
        for it in &m.items {
            if !first && it.blank_before {
                self.blank();
            }
            if first && (!m.inner_docs.is_empty() || !m.inner_attrs.is_empty()) {
                self.blank();
            }
            first = false;
            self.lower_item(it);
        }
        if self.needs_owned_helper {
            self.blank();
            self.line("#[inline(always)]");
            self.line("fn __uredo_owned<T, E>(r: ::core::result::Result<T, E>) -> ::core::result::Result<T, E> {");
            self.line("    r");
            self.line("}");
        }
    }

    fn lower_items_in_block(&mut self, items: &[Item]) {
        let mut first = true;
        for it in items {
            if !first && it.blank_before {
                self.blank();
            }
            first = false;
            self.lower_item(it);
        }
    }

    fn lower_docs_attrs(&mut self, docs: &[String], attrs: &[Attr]) {
        self.cur_attrs = attrs.iter().filter(|a| a.name != "copy" && a.name != "default_error").map(attr_text).collect();
        for d in self.doc_lines(docs) {
            self.line(&format!("///{}", if d.is_empty() { String::new() } else { format!(" {}", d) }));
        }
        for a in attrs {
            if a.name == "copy" || a.name == "default_error" {
                continue;
            }
            if let Some(d) = lint::misdirected_allow(a) {
                self.diags.push(d);
            }
            self.line(&format!("#{}", attr_text(a)));
        }
    }

    fn lower_item(&mut self, it: &Item) {
        self.cur_line = it.line;
        let vis = it.vis.as_deref().map(|v| format!("{} ", v)).unwrap_or_default();
        match &it.kind {
            ItemKind::Use { path, copy } => {
                self.lower_docs_attrs(&it.docs, &it.attrs);
                let import = copy_use_import(path);
                self.line(&format!("{}use {};", vis, import));
                self.api(&vis, "use", self.api_path(&format!("use {}", import.trim())), format!("pub use {};", import.trim()), vec![]);
                if *copy {
                    if !self.copy_assert_emitted {
                        self.line("fn __uredo_assert_copy<T: Copy>() {}");
                        self.copy_assert_emitted = true;
                    }
                    self.line(&format!("const _: fn() = __uredo_assert_copy::<{}>;", copy_use_type(path)));
                }
            }
            ItemKind::Const { is_static, name, ty, expr } => {
                self.lower_docs_attrs(&it.docs, &it.attrs);
                let kw = if *is_static { "static" } else { "const" };
                let t = if ty.text.trim() == "str" { "&'static str".to_string() } else if is_slice_type(&ty.text) { format!("&'static {}", ty.text.trim()) } else { lower_type(ty.text.trim()) };
                let e = self.lower_expr(expr);
                self.line(&format!("{}{} {}: {} = {};", vis, kw, name, t, e));
                self.api(&vis, kw, self.api_path(name), format!("pub {} {}: {}", kw, name, t), vec![]);
            }
            ItemKind::Fn(f) => {
                self.lower_docs_attrs(&it.docs, &it.attrs);
                self.lower_fn(f, &vis, None);
            }
            ItemKind::Struct(s) => self.lower_struct(it, s, &vis),
            ItemKind::Enum(e) => self.lower_enum(it, e, &vis),
            ItemKind::Impl(i) => self.lower_impl(it, i, &vis),
            ItemKind::Rust(text) => {
                self.lower_docs_attrs(&it.docs, &it.attrs);
                let inner = text.trim_start_matches('{').trim_end_matches('}');
                for l in dedent_block(inner).lines() {
                    if l.trim().is_empty() {
                        self.blank();
                    } else {
                        self.line(l);
                    }
                }
            }
            ItemKind::Macro { path, body } => {
                self.lower_docs_attrs(&it.docs, &it.attrs);
                let semi = if body.starts_with('{') { "" } else { ";" };
                self.line(&format!("{}!{}{}", path, body, semi));
            }
            ItemKind::MacroRules { name, body } => {
                self.lower_docs_attrs(&it.docs, &it.attrs);
                let semi = if body.starts_with('{') { "" } else { ";" };
                self.line(&format!("{}macro_rules! {} {}{}", vis, name, body, semi));
            }
            ItemKind::Mod { name, body } => {
                self.lower_docs_attrs(&it.docs, &it.attrs);
                self.api(&vis, "mod", self.api_path(name), format!("pub mod {}", name), vec![]);
                match body {
                    Some(items) => {
                        self.line(&format!("{}mod {} {{", vis, name));
                        self.mod_path.push(name.clone());
                        self.indent += 1;
                        // nested module: its own declarations are visible inside
                        let saved_fns = self.fns.clone();
                        let saved_types = self.types.clone();
                        let guarded = it.attrs.iter().any(is_cfg_attr);
                        if guarded {
                            self.conditional_depth += 1;
                        }
                        self.prepass(items);
                        self.resolve_uses(items);
                        self.check_module_alternates(items);
                        self.push_scope();
                        self.lower_items_in_block(items);
                        self.pop_scope();
                        self.fns = saved_fns;
                        self.types = saved_types;
                        if guarded {
                            self.conditional_depth -= 1;
                        }
                        self.mod_path.pop();
                        self.indent -= 1;
                        self.line("}");
                    }
                    None => self.line(&format!("{}mod {};", vis, name)),
                }
            }
            ItemKind::TypeAlias { name, generics, bounds, where_clause, ty } => {
                self.lower_docs_attrs(&it.docs, &it.attrs);
                let b = bounds.as_deref().map(|b| format!(": {}", b.trim())).unwrap_or_default();
                let w = where_clause.as_deref().map(|w| format!(" {}", w.trim())).unwrap_or_default();
                let sig = match ty {
                    Some(t) => format!("{}type {}{}{} = {}{};", vis, name, generics.as_deref().unwrap_or(""), b, lower_type(&t.text), w),
                    None => format!("{}type {}{}{}{};", vis, name, generics.as_deref().unwrap_or(""), b, w),
                };
                self.line(&sig);
                self.api(&vis, "type", self.api_path(name), sig.trim_end_matches(';').to_string(), vec![]);
            }
        }
    }

    fn lower_struct(&mut self, it: &Item, s: &StructDecl, vis: &str) {
        self.lower_docs_attrs(&it.docs, &it.attrs);
        let g = s.generics.as_deref().unwrap_or("");
        let w = s.where_clause.as_deref().map(|w| format!(" {}", w)).unwrap_or_default();
        let members: Vec<String> = match &s.kind {
            StructKind::Named(fields) => fields.iter().filter(|f| f.vis.as_deref().map(|v| v.trim() == "pub").unwrap_or(false)).map(|f| format!("pub {}: {}", ident(&f.name), lower_type(f.ty.text.trim()))).collect(),
            StructKind::Tuple(t) => vec![t.trim().to_string()],
            StructKind::Unit => vec![],
        };
        self.api(vis, "struct", self.api_path(&s.name), format!("pub struct {}{}{}", s.name, g, w).trim().to_string(), members);
        match &s.kind {
            StructKind::Unit => self.line(&format!("{}struct {}{}{};", vis, s.name, g, w)),
            StructKind::Tuple(t) => self.line(&format!("{}struct {}{}{}{};", vis, s.name, g, inline_field_attrs(t), w)),
            StructKind::Named(fields) => {
                self.line(&format!("{}struct {}{}{} {{", vis, s.name, g, w));
                self.indent += 1;
                for f in fields {
                    self.cur_line = f.line;
                    self.lower_docs_attrs(&f.docs, &f.attrs);
                    let fv = f.vis.as_deref().map(|v| format!("{} ", v)).unwrap_or_default();
                    self.line(&format!("{}{}: {},", fv, ident(&f.name), lower_type(f.ty.text.trim())));
                }
                self.indent -= 1;
                self.line("}");
            }
        }
        self.lower_nested(&s.name, s.generics.as_deref(), s.where_clause.as_deref(), &it.attrs, &s.nested);
    }

    fn lower_enum(&mut self, it: &Item, e: &EnumDecl, vis: &str) {
        self.lower_docs_attrs(&it.docs, &it.attrs);
        let g = e.generics.as_deref().unwrap_or("");
        let w = e.where_clause.as_deref().map(|w| format!(" {}", w)).unwrap_or_default();
        let members: Vec<String> = e.variants.iter().map(|v| lower_type(v.text.trim())).collect();
        self.api(vis, "enum", self.api_path(&e.name), format!("pub enum {}{}{}", e.name, g, w).trim().to_string(), members);
        if e.variants.is_empty() {
            self.line(&format!("{}enum {}{}{} {{}}", vis, e.name, g, w));
        } else {
            self.line(&format!("{}enum {}{}{} {{", vis, e.name, g, w));
            self.indent += 1;
            for v in &e.variants {
                self.cur_line = v.line;
                self.lower_docs_attrs(&v.docs, &v.attrs);
                self.line(&format!("{},", inline_field_attrs(v.text.trim())));
            }
            self.indent -= 1;
            self.line("}");
        }
        self.lower_nested(&e.name, e.generics.as_deref(), e.where_clause.as_deref(), &it.attrs, &e.nested);
    }

    /// Nested methods -> inherent impl; nested `impl Trait:` -> trait impls (§12.2, D45).
    fn lower_nested(&mut self, name: &str, generics: Option<&str>, where_clause: Option<&str>, type_attrs: &[Attr], nested: &[Nested]) {
        let saved_tp = self.type_params.len();
        self.type_params.extend(generic_bounds(generics, where_clause));
        self.lower_nested_inner(name, generics, where_clause, type_attrs, nested);
        self.type_params.truncate(saved_tp);
    }

    fn lower_nested_inner(&mut self, name: &str, generics: Option<&str>, where_clause: Option<&str>, type_attrs: &[Attr], nested: &[Nested]) {
        let g = generics.unwrap_or("");
        let g_args = generic_args(g);
        let w = where_clause.map(|w| format!(" {}", w)).unwrap_or_default();
        let cfgs: Vec<&Attr> = type_attrs.iter().filter(|a| a.name == "cfg").collect();
        let methods: Vec<&Item> = nested.iter().filter_map(|n| if let Nested::Method(m) = n { Some(m) } else { None }).collect();
        if !methods.is_empty() {
            self.blank();
            for a in &cfgs {
                self.line(&format!("#{}", attr_text(a)));
            }
            self.line(&format!("impl{} {}{}{} {{", g, name, g_args, w));
            self.indent += 1;
            let mut first = true;
            for m in &methods {
                if !first && m.blank_before {
                    self.blank();
                }
                first = false;
                if let ItemKind::Fn(f) = &m.kind {
                    self.cur_line = m.line;
                    self.lower_docs_attrs(&m.docs, &m.attrs);
                    let vis = m.vis.as_deref().map(|v| format!("{} ", v)).unwrap_or_default();
                    self.lower_fn(f, &vis, Some(name));
                }
            }
            self.indent -= 1;
            self.line("}");
        }
        for n in nested {
            if let Nested::TraitImpl { attrs, trait_path, items, line, .. } = n {
                self.cur_line = *line;
                self.blank();
                for a in &cfgs {
                    self.line(&format!("#{}", attr_text(a)));
                }
                for a in attrs {
                    self.line(&format!("#{}", attr_text(a)));
                }
                self.line(&format!("impl{} {} for {}{}{} {{", g, trait_path.trim(), name, g_args, w));
                self.api("pub", "impl", self.api_path(&format!("{} for {}", trait_path.trim(), name)), format!("impl{} {} for {}{}{}", g, trait_path.trim(), name, g_args, w), vec![]);
                self.trait_impl = Some(trait_path.trim().to_string());
                self.indent += 1;
                let mut first = true;
                for m in items {
                    if !first && m.blank_before {
                        self.blank();
                    }
                    first = false;
                    self.lower_impl_item(m, Some(name));
                }
                self.trait_impl = None;
                self.indent -= 1;
                self.line("}");
            }
        }
    }

    fn lower_impl_item(&mut self, m: &Item, self_ty: Option<&str>) {
        self.cur_line = m.line;
        match &m.kind {
            ItemKind::Fn(f) => {
                self.lower_docs_attrs(&m.docs, &m.attrs);
                let vis = m.vis.as_deref().map(|v| format!("{} ", v)).unwrap_or_default();
                self.lower_fn(f, &vis, self_ty);
            }
            _ => self.lower_item(m),
        }
    }

    fn lower_impl(&mut self, it: &Item, i: &ImplDecl, vis: &str) {
        let saved_tp = self.type_params.len();
        self.type_params.extend(generic_bounds(i.generics.as_deref(), i.where_clause.as_deref()));
        self.lower_impl_inner(it, i, vis);
        self.type_params.truncate(saved_tp);
    }

    fn lower_impl_inner(&mut self, it: &Item, i: &ImplDecl, vis: &str) {
        self.lower_docs_attrs(&it.docs, &it.attrs);
        let g = i.generics.as_deref().unwrap_or("");
        let w = i.where_clause.as_deref().map(|w| format!(" {}", w)).unwrap_or_default();
        let unsafe_kw = if i.is_unsafe { "unsafe " } else { "" };
        // trait declarations are encoded as ImplDecl with trait_path "trait Name"
        if let Some(tp) = &i.trait_path {
            if let Some(name) = tp.strip_prefix("trait ") {
                let bounds = i.self_ty.trim();
                // `: Supertrait` joins the name directly; a `where` clause needs the space (D60)
                let bounds = match bounds {
                    "" => String::new(),
                    b if b.starts_with(':') => b.to_string(),
                    b => format!(" {}", b),
                };
                if i.items.is_empty() {
                    self.line(&format!("{}trait {}{}{}{} {{}}", vis, name, g, bounds, w));
                    self.api(vis, "trait", self.api_path(name), format!("pub trait {}{}{}{}", name, g, bounds, w), vec![]);
                    return;
                }
                self.line(&format!("{}trait {}{}{}{} {{", vis, name, g, bounds, w));
                self.api(vis, "trait", self.api_path(name), format!("pub trait {}{}{}{}", name, g, bounds, w), vec![]);
                self.indent += 1;
                let mut first = true;
                let trait_name = name.to_string();
                for m in &i.items {
                    if !first && m.blank_before {
                        self.blank();
                    }
                    first = false;
                    self.lower_impl_item(m, Some(&trait_name));
                }
                self.indent -= 1;
                self.line("}");
                return;
            }
        }
        // The impl target is a type position like any other, so `T?` is `Option<T>` here too
        // (§9.3). It was emitted verbatim, and `impl<T: Field> Field for T?` generated Rust that
        // did not parse — found by writing `examples/restdemo`, 2026-09-15.
        let self_ty = lower_type(i.self_ty.trim());
        let trait_path = i.trait_path.as_ref().map(|t| lower_type(t.trim()));
        let header = match &trait_path {
            Some(t) => format!("{}{}impl{} {} for {}{} {{", vis, unsafe_kw, g, t, self_ty, w),
            None => format!("{}{}impl{} {}{} {{", vis, unsafe_kw, g, self_ty, w),
        };
        if let Some(t) = &trait_path {
            self.api("pub", "impl", self.api_path(&format!("{} for {}", t, self_ty)), header.trim_end_matches(" {").to_string(), vec![]);
        }
        if i.items.is_empty() {
            self.line(&format!("{}}}", header));
            return;
        }
        self.line(&header);
        self.trait_impl = i.trait_path.as_ref().map(|t| t.trim().to_string());
        self.indent += 1;
        let self_name = last_segment(&i.self_ty).to_string();
        let mut first = true;
        for m in &i.items {
            if !first && m.blank_before {
                self.blank();
            }
            first = false;
            self.lower_impl_item(m, Some(&self_name));
        }
        self.trait_impl = None;
        self.indent -= 1;
        self.line("}");
    }

    // ----- functions -----
    fn lower_fn(&mut self, f: &FnDecl, vis: &str, self_ty: Option<&str>) {
        self.cur_line = f.line;
        let saved_tp = self.type_params.len();
        self.type_params.extend(generic_bounds(f.generics.as_deref(), f.where_clause.as_deref()));
        let mut params = Vec::new();
        if let Some(r) = &f.receiver {
            params.push(match r {
                Receiver::Ref => "&self".to_string(),
                Receiver::RefMut => "&mut self".to_string(),
                Receiver::Value => "self".to_string(),
                Receiver::MutValue => "mut self".to_string(),
                Receiver::Explicit(t) => format!("self: {}", lower_type(t)),
            });
        }
        self.push_scope();
        if let Some(r) = &f.receiver {
            let mutable = matches!(r, Receiver::MutValue);
            // A written receiver type (D59) is one Uredo does not model, so it makes no claim about
            // what may be done through it: writes are passed on and rustc decides, as §5.2 requires.
            let explicit = matches!(r, Receiver::Explicit(_));
            let mut_ref = explicit || matches!(r, Receiver::RefMut | Receiver::MutValue);
            self.bind("self", Binding { mutable, ty: Some("Self".into()), ref_typed: matches!(r, Receiver::Ref | Receiver::RefMut), mut_ref, uredo_ty: self_ty.map(|s| s.to_string()), known: !explicit });
        }
        for (pi, p) in f.params.iter().enumerate() {
            if !p.is_pattern {
                self.check_bindable(&p.pat, p.line);
            }
            if p.mode == Mode::Default && !p.is_pattern {
                if let Some(payload) = optional_payload(&lower_type(p.ty.text.trim())) {
                    if payload.trim_start().starts_with("&mut") {
                        let d = Diag::error(p.line, 1, format!("`{}` passes `&Option<{}>`, through which the payload cannot be mutated", p.ty.text.trim(), payload.trim()))
                            .note("write `inout` to mutate the optional itself, or `take` to take the mutable reference (§10.2)");
                        self.diags.push(d);
                    }
                }
            }
            let (mut ty, mut mode, mut ref_typed) = self.param_lowering(p);
            self.collect_lints(p, &ty, mode, f.body.as_ref());
            if let Some(tr) = self.trait_impl.clone() {
                if let Some(fixed) = self.std_trait_param_mode(&tr, &f.name, pi) {
                    let base = lower_type(p.ty.text.trim());
                    let base = if base == "str" { "str".to_string() } else { base };
                    match fixed {
                        "value" => {
                            if p.mode != Mode::Inout {
                                ty = base.clone();
                                mode = ParamMode::Verbatim;
                                ref_typed = base.starts_with('&');
                            }
                        }
                        "ref" => {
                            if !base.starts_with('&') {
                                ty = format!("&{}", base);
                                mode = ParamMode::Borrow;
                                ref_typed = true;
                            }
                        }
                        _ => {
                            if !base.starts_with("&mut") {
                                ty = format!("&mut {}", base.trim_start_matches('&').trim_start());
                                mode = ParamMode::BorrowMut;
                                ref_typed = true;
                            }
                        }
                    }
                }
            }
            if mode == ParamMode::ByValueGeneric {
                // one `take` each under the former borrow default (§38)
                self.measures.p8_params += 1;
            }
            let pat = if p.is_var { format!("mut {}", p.pat) } else { p.pat.clone() };
            params.push(format!("{}: {}", pat, ty));
            if p.is_pattern {
                self.bind_pattern_names(&p.pat, p.is_var);
            } else {
                let uredo_ty = if p.mode == Mode::Take || self.is_known_copy(&p.ty.text) || mode == ParamMode::Borrow || mode == ParamMode::BorrowMut {
                    let seg = last_segment(&p.ty.text).to_string();
                    if self.types.contains_key(&seg) { Some(seg) } else { None }
                } else {
                    None
                };
                self.bind(&p.pat, Binding {
                    mutable: p.is_var,
                    ty: Some(ty.clone()),
                    ref_typed,
                    mut_ref: p.mode == Mode::Inout || p.ty.text.trim().starts_with("&mut"),
                    uredo_ty,
                    known: true,
                });
            }
        }
        let ret = match (&f.ret, &f.throws) {
            (Some(t), None) => format!(" -> {}", self.return_type(t)),
            (None, None) => String::new(),
            (ret, Some(err)) => {
                let ok = ret.as_ref().map(|t| self.return_type(t)).unwrap_or_else(|| "()".to_string());
                let e = match err {
                    Some(e) => e.clone(),
                    None => match &self.default_error {
                        Some(d) => d.clone(),
                        None => {
                            self.err(f.line, 1, "bare `throws` needs `@!default_error(Type)` at the crate or module top (§15.1)");
                            "()".to_string()
                        }
                    },
                };
                format!(" -> ::core::result::Result<{}, {}>", ok, e)
            }
        };
        let g = f.generics.as_deref().unwrap_or("");
        let w = f.where_clause.as_deref().map(|w| format!(" {}", w)).unwrap_or_default();
        let asyncness = if f.is_async { "async " } else { "" };
        let unsafeness = if f.is_unsafe { "unsafe " } else { "" };
        let header = format!("{}{}{}fn {}{}({}){}{}", vis, unsafeness, asyncness, ident(&f.name), g, params.join(", "), ret, w);
        {
            let (kind, path) = match self_ty {
                Some(t) => ("method", self.api_path(&format!("{}::{}", t, f.name))),
                None => ("fn", self.api_path(&f.name)),
            };
            // trait items are public with their trait; impl items are public when written `pub`
            let effective_vis = if self_ty.is_some() && vis.is_empty() && f.body.is_none() { "pub" } else { vis.trim() };
            self.api(effective_vis, kind, path, header.trim_start_matches("pub ").to_string(), vec![]);
        }
        match &f.body {
            None => {
                self.line(&format!("{};", header));
            }
            Some(body) => {
                self.line(&format!("{} {{", header));
                self.cur_line = f.line;
                self.indent += 1;
                self.fn_stack.push(FnCtx { receiver: f.receiver.clone(), self_ty: self_ty.map(|s| s.to_string()), throws: f.throws.clone(), ret: f.ret.as_ref().map(|t| t.text.clone()) });
                let mode = if f.ret.is_some() { BlockMode::Value } else { BlockMode::Unit };
                self.check_tail_shape(f);
                let is_throws = f.throws.is_some();
                if is_throws {
                    self.lower_throws_body(body, f.ret.is_some());
                } else {
                    self.lower_stmts(&body.stmts, mode);
                }
                self.fn_stack.pop();
                self.indent -= 1;
                self.cur_line = f.line;
                self.line("}");
            }
        }
        self.pop_scope();
        self.type_params.truncate(saved_tp);
    }

    /// Body of a `throws` function (§15.3, D37): tail values are wrapped in `Ok`.
    /// Two mistakes the *value* half of a signature invites, both decidable from the declaration
    /// and neither diagnosed before.
    ///
    /// `throws` says what a function returns when it fails and nothing about what it returns when
    /// it succeeds, and that split caused four of the thirteen authoring errors in
    /// `examples/restdemo` — one of them at seven sites in a single function. Until now Uredo
    /// lowered all four silently and let rustc complain about generated code: writing `Ok(1)` in a
    /// `throws` body produced `Result::Ok(Ok(1))` and an `expected u32, found Result<…>` pointing
    /// at the *signature line*, never mentioning the rule that caused it.
    fn check_tail_shape(&mut self, f: &FnDecl) {
        let Some(body) = &f.body else { return };
        let Some(last) = body.stmts.last() else { return };
        let StmtKind::Expr(e) = &last.kind else { return };

        // The declared value type, as written. Everything below reads only this.
        let value = f.ret.as_ref().map(|t| t.text.trim().to_string());

        // 1. `Ok(…)`/`Err(…)` by hand in a `throws` body. §15.3 wraps the tail already, so this
        //    is `Ok(Ok(x))`. Correct only when the value type is itself a `Result` or optional,
        //    which is exactly the case the guard keeps.
        if f.throws.is_some() {
            let wraps_itself = value.as_deref().is_some_and(|v| v.starts_with("Result") || v.ends_with('?') || v.starts_with("Option"));
            if !wraps_itself {
                if let Expr::Call { callee, line, col, .. } = e {
                    if let Expr::Path(p) = &**callee {
                        if p == "Ok" || p == "Err" {
                            self.diags.push(
                                Diag::error(*line, *col, format!(
                                    "a `throws` body wraps its tail in `Ok` for you (§15.3), so this returns `Ok({}(…))`",
                                    p
                                ))
                                .note("drop the wrapper and write the value; `throw e` or `e?` carries the error path")
                                .note("`throws` says what this returns when it fails, and nothing about when it succeeds"),
                            );
                        }
                    }
                }
            }
        }

        // 2. `-> T?` whose tail constructs a bare `T`. An optional return is a verbatim `Option`
        //    and gets no wrapping — only a `throws` body does. Restricted to the two forms that
        //    certainly build a `T`: a struct literal and a unit-struct path.
        if f.throws.is_none() {
            if let Some(v) = value.as_deref().and_then(|v| v.strip_suffix('?')) {
                let payload = v.trim();
                let named = |p: &str| p == payload || p.rsplit("::").next() == Some(payload);
                let bad = match e {
                    Expr::StructLit { path, .. } => named(path),
                    Expr::Path(p) => named(p),
                    _ => false,
                };
                if bad {
                    self.diags.push(
                        Diag::error(f.line, 1, format!(
                            "this returns `{}?`, which is `Option<{}>`, but the tail builds a bare `{}`",
                            payload, payload, payload
                        ))
                        .note("an optional return is verbatim: write `Some(…)` or `None`")
                        .note("only a `throws` body has its tail wrapped, and it wraps in `Ok` (§15.3)"),
                    );
                }
            }
        }
    }

    fn lower_throws_body(&mut self, body: &Block, has_value: bool) {
        let n = body.stmts.len();
        for (i, s) in body.stmts.iter().enumerate() {
            let last = i + 1 == n;
            if last {
                match &s.kind {
                    StmtKind::Expr(e) if has_value => {
                        if s.blank_before {
                            self.blank();
                        }
                        let t = self.lower_tail_throws(e);
                        self.emit_multiline(&t, "");
                        return;
                    }
                    StmtKind::Expr(e) if !has_value && is_block_like(e) => {
                        self.lower_stmt(s, BlockMode::Unit, false);
                        if !loop_never_exits(e) {
                            self.line("::core::result::Result::Ok(())");
                        }
                        return;
                    }
                    StmtKind::Return(_) | StmtKind::Throw(_) => {
                        self.lower_stmt(s, BlockMode::Unit, false);
                        return;
                    }
                    _ => {
                        self.lower_stmt(s, BlockMode::Unit, false);
                        if !has_value {
                            self.line("::core::result::Result::Ok(())");
                        } else {
                            self.err(s.line, 1, "a `throws` function with a return type must end with a value expression or `return`");
                        }
                        return;
                    }
                }
            } else {
                self.lower_stmt(s, BlockMode::Unit, false);
            }
        }
        if n == 0 && !has_value {
            self.line("::core::result::Result::Ok(())");
        }
    }

    /// Tail expression of a `throws` body: `e?` -> the D37 match; otherwise `Ok(e)`.
    fn lower_tail_throws(&mut self, e: &Expr) -> String {
        if let Expr::Try(inner) = e {
            let v = self.lower_expr(inner);
            self.needs_owned_helper = true;
            self.elab(Elab {
                line: self.cur_line,
                col: 1,
                kind: "throws-tail".into(),
                rule: "D37 (tail `?` in a `throws` body: matched on an owned operand, the error converted with `From::from`; alias-identical to a direct return)".into(),
                before: self.src_lines.get(self.cur_line.wrapping_sub(1)).map(|s| s.trim().to_string()).unwrap_or_default(),
                after: format!("match __uredo_owned({}) {{ Ok(v) => Ok(v), Err(x) => Err(From::from(x)) }}", v),
                inserted: "borrow: none · clone: none · allocation: none · dispatch: none · sync: none".into(),
                ..Default::default()
            });
            return format!(
                "match __uredo_owned({}) {{\n    ::core::result::Result::Ok(__uredo_v) => ::core::result::Result::Ok(__uredo_v),\n    ::core::result::Result::Err(__uredo_x) => ::core::result::Result::Err(::core::convert::From::from(__uredo_x)),\n}}",
                v
            );
        }
        if is_block_like(e) {
            // `if`/`match` tails: wrap each branch value is complex; wrap the whole expression.
            let v = self.lower_expr_mode(e, BlockMode::Value);
            return format!("::core::result::Result::Ok({})", v);
        }
        let v = self.lower_expr(e);
        format!("::core::result::Result::Ok({})", v)
    }

    // ----- statements -----
    fn lower_stmts(&mut self, stmts: &[Stmt], mode: BlockMode) {
        let n = stmts.len();
        for (i, s) in stmts.iter().enumerate() {
            let is_tail = mode == BlockMode::Value && i + 1 == n;
            self.lower_stmt(s, mode, is_tail);
        }
    }

    fn emit_multiline(&mut self, text: &str, suffix: &str) {
        let lines: Vec<&str> = text.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            let last = i + 1 == lines.len();
            if last {
                self.line(&format!("{}{}", l, suffix));
            } else {
                self.line(l);
            }
        }
    }

    fn lower_stmt(&mut self, s: &Stmt, _mode: BlockMode, is_tail: bool) {
        self.cur_line = s.line;
        if s.blank_before {
            self.blank();
        }
        match &s.kind {
            StmtKind::Bind { kind, target, ty, init, else_block } => self.lower_bind(s, *kind, target, ty.as_ref(), init, else_block.as_ref()),
            StmtKind::Assign { target, op, value } => {
                self.check_assignable(target, s.line);
                let t = self.lower_place(target);
                let v = self.lower_expr(value);
                self.line(&format!("{} {} {};", t, op, v));
            }
            StmtKind::Expr(e) => {
                if is_block_like(e) {
                    let text = self.lower_expr_mode(e, if is_tail { BlockMode::Value } else { BlockMode::Unit });
                    self.emit_multiline(&text, "");
                } else {
                    let text = self.lower_expr(e);
                    let suffix = if is_tail { "" } else { ";" };
                    self.emit_multiline(&text, suffix);
                }
            }
            StmtKind::Return(e) => {
                let in_throws = self.fn_stack.last().map(|c| c.throws.is_some()).unwrap_or(false);
                match e {
                    None => {
                        if in_throws {
                            self.line("return ::core::result::Result::Ok(());");
                        } else {
                            self.line("return;");
                        }
                    }
                    Some(e) => {
                        if in_throws {
                            let t = self.lower_tail_throws(e);
                            self.emit_multiline(&format!("return {}", t), ";");
                        } else {
                            let t = self.lower_expr(e);
                            self.emit_multiline(&format!("return {}", t), ";");
                        }
                    }
                }
            }
            StmtKind::Throw(e) => {
                let t = self.lower_expr(e);
                if !self.fn_stack.last().map(|c| c.throws.is_some()).unwrap_or(false) {
                    self.err(s.line, 1, "`throw` is only allowed in a function declared with `throws` (§15.1)");
                }
                self.line(&format!("return ::core::result::Result::Err({});", t));
            }
            StmtKind::Break { label, value } => {
                let mut t = "break".to_string();
                if let Some(l) = label {
                    t.push(' ');
                    t.push_str(l);
                }
                if let Some(v) = value {
                    t.push(' ');
                    t.push_str(&self.lower_expr(v));
                }
                self.line(&format!("{};", t));
            }
            StmtKind::Continue { label } => match label {
                Some(l) => self.line(&format!("continue {};", l)),
                None => self.line("continue;"),
            },
            StmtKind::While { label, cond, body } => {
                let header_line = self.cur_line;
                let c = self.lower_expr(cond);
                let lbl = label.as_ref().map(|l| format!("{}: ", l)).unwrap_or_default();
                self.line(&format!("{}while {} {{", lbl, c));
                self.lower_block_body(body, BlockMode::Unit);
                self.close_block(header_line);
            }
            StmtKind::WhileLet { label, pat, expr, body } => {
                let header_line = self.cur_line;
                let e = self.lower_expr(expr);
                let lbl = label.as_ref().map(|l| format!("{}: ", l)).unwrap_or_default();
                self.line(&format!("{}while let {} = {} {{", lbl, self.lower_pattern(pat, None), e));
                self.push_scope();
                self.bind_pattern_names(pat, false);
                self.lower_block_body(body, BlockMode::Unit);
                self.pop_scope();
                self.close_block(header_line);
            }
            StmtKind::For { label, pat, mode, iter, body } => {
                let header_line = self.cur_line;
                let it = self.lower_for_source(iter, *mode);
                let lbl = label.as_ref().map(|l| format!("{}: ", l)).unwrap_or_default();
                self.line(&format!("{}for {} in {} {{", lbl, self.lower_pattern(pat, None), it));
                self.push_scope();
                self.bind_pattern_names(pat, false);
                let is_place = match iter {
                    Expr::Path(p) if !p.contains("::") => self.lookup(p).map(|b| !b.ref_typed).unwrap_or(false),
                    Expr::Field { .. } | Expr::Index { .. } => true,
                    _ => false,
                };
                if *mode == Mode::Inout || (is_place && *mode == Mode::Default) {
                    // the loop variable is `&mut T` (inout) or `&T` (borrowed place): known
                    for n in pattern_idents(pat) {
                        self.bind(&n, Binding { mutable: false, ty: None, ref_typed: true, mut_ref: *mode == Mode::Inout, uredo_ty: None, known: true });
                    }
                }
                self.lower_block_body(body, BlockMode::Unit);
                self.pop_scope();
                self.close_block(header_line);
            }
            StmtKind::Item(it) => {
                // items in blocks: register signatures locally
                if let ItemKind::Fn(f) = &it.kind {
                    let sig = self.sig_of(f);
                    self.fns.insert(f.name.clone(), sig);
                }
                self.lower_item(it);
            }
            StmtKind::Unsafe(b) => {
                self.line("unsafe {");
                self.lower_block_body(b, if is_tail { BlockMode::Value } else { BlockMode::Unit });
                self.line("}");
            }
        }
    }

    fn lower_block_body(&mut self, b: &Block, mode: BlockMode) {
        self.indent += 1;
        self.push_scope();
        self.lower_stmts(&b.stmts, mode);
        self.pop_scope();
        self.indent -= 1;
    }

    /// `for` source (§16): a place is borrowed unless `take`/`inout`; other expressions verbatim.
    fn lower_for_source(&mut self, iter: &Expr, mode: Mode) -> String {
        let text = self.lower_expr(iter);
        if mode == Mode::Take {
            return text;
        }
        // an `inout` parameter is already `&mut T`: reborrow it for the loop
        if mode == Mode::Inout {
            if let Expr::Path(p) = iter {
                if !p.contains("::") && self.lookup(p).map(|b| b.mut_ref && b.known).unwrap_or(false) {
                    self.elab(Elab { line: self.cur_line, col: 1, kind: "for-borrow".into(), rule: "§16 (`for … in inout param` reborrows the `&mut` parameter)".into(), before: text.clone(), after: format!("&mut *{}", text), inserted: "borrow: mutable (reborrow) · clone: none · allocation: none · dispatch: none · sync: none".into(), ..Default::default() });
                    return format!("&mut *{}", text);
                }
            }
        }
        let is_place = match iter {
            Expr::Path(p) if !p.contains("::") => self.lookup(p).map(|b| !b.ref_typed).unwrap_or(false),
            Expr::Field { .. } | Expr::Index { .. } => true,
            _ => false,
        };
        if !is_place {
            if mode == Mode::Inout {
                return format!("&mut {}", text);
            }
            return text;
        }
        if mode == Mode::Inout {
            let (l, c) = expr_pos(iter);
            if let Some(root) = root_ident(iter) {
                if let Some(b) = self.lookup(&root).cloned() {
                    if !(b.mutable || b.mut_ref) {
                        self.err(l, c, format!("`{}` is immutable; `for … in inout {}` needs a `var` binding", root, root));
                    }
                }
            }
            self.elab(Elab { line: self.cur_line, col: 1, kind: "for-borrow".into(), rule: "§16 (`for … in inout place` borrows the place mutably)".into(), before: text.clone(), after: format!("&mut {}", text), inserted: "borrow: mutable · clone: none · allocation: none · dispatch: none · sync: none".into(), ..Default::default() });
            return format!("&mut {}", text);
        }
        self.elab(Elab { line: self.cur_line, col: 1, kind: "for-borrow".into(), rule: "§16 (a `for` source that is a place is borrowed; `take` consumes it)".into(), before: text.clone(), after: format!("&{}", text), inserted: "borrow: shared · clone: none · allocation: none · dispatch: none · sync: none".into(), ..Default::default() });
        format!("&{}", text)
    }

    /// `self`, `Self`, `super` and `crate` are the four Rust keywords that cannot be written as
    /// raw identifiers (§6.1), so a binding of that name has no spelling in the generated Rust and
    /// would reach rustc bare, where it means something else entirely. Reject it in Uredo terms.
    fn check_bindable(&mut self, name: &str, line: usize) {
        let n = name.trim();
        if matches!(n, "self" | "Self" | "super" | "crate") {
            self.diags.push(
                Diag::error(line, 1, format!("`{}` cannot name a binding: it is one of the four Rust keywords that has no raw form (§6.1)", n))
                    .note(format!("every other Rust keyword is emitted as `r#{}`; these four cannot be, so the name has to change", n)),
            );
        }
    }

    fn lower_bind(&mut self, s: &Stmt, kind: BindKind, target: &Expr, ty: Option<&Type>, init: &Expr, else_block: Option<&Block>) {
        let line = s.line;
        if let Expr::Path(name) = target {
            self.check_bindable(name, line);
        }
        // `Vec<T>` annotation with an array literal (§9.4)
        let init_text = match (ty, init) {
            (Some(t), Expr::Array(_)) if t.text.trim().starts_with("Vec<") || t.text.trim() == "Vec" => {
                format!("Vec::from({})", self.lower_expr(init))
            }
            _ => self.lower_expr_mode(init, BlockMode::Value),
        };
        let ty_text = ty.map(|t| format!(": {}", lower_type(t.text.trim()))).unwrap_or_default();
        let inferred = self.infer_uredo_type(init, ty);
        let ref_typed = ty.map(|t| t.text.trim().starts_with('&')).unwrap_or(false);
        match kind {
            BindKind::Let | BindKind::Var => {
                let mutable = kind == BindKind::Var;
                let pat = match target {
                    Expr::Path(p) => p.clone(),
                    _ => unreachable!(),
                };
                let names = pattern_idents(&pat);
                let pat_text = if mutable {
                    if names.len() == 1 && names[0] == pat.trim() {
                        format!("mut {}", ident(&pat))
                    } else if pat.trim().starts_with('(') {
                        // `(a, b)` -> `(mut a, mut b)`
                        let inner = &pat.trim()[1..pat.trim().len() - 1];
                        let parts: Vec<String> = split_top_level(inner, ',').iter().map(|p| if p == "_" { p.clone() } else { format!("mut {}", p) }).collect();
                        format!("({})", parts.join(", "))
                    } else {
                        self.err(line, 1, "`var` with this pattern is not supported; bind the names separately");
                        pat.clone()
                    }
                } else if names.len() == 1 && names[0] == pat.trim() {
                    ident(&pat)
                } else {
                    self.lower_pattern(&pat, None)
                };
                match else_block {
                    None => self.line(&format!("let {}{} = {};", pat_text, ty_text, init_text)),
                    Some(eb) => {
                        self.line(&format!("let {}{} = {} else {{", pat_text, ty_text, init_text));
                        self.lower_block_body(eb, BlockMode::Unit);
                        self.line("};");
                    }
                }
                let single = names.len() == 1 && names[0] == pat.trim();
                for n in names {
                    self.bind(&n, Binding { mutable, ty: ty.map(|t| t.text.clone()), ref_typed, mut_ref: ty.map(|t| t.text.trim().starts_with("&mut")).unwrap_or(false), uredo_ty: inferred.clone(), known: single || ty.is_some() });
                }
            }
            BindKind::Auto => {
                match target {
                    Expr::Path(name) if !name.contains("::") && !name.contains(' ') => {
                        if name == "_" {
                            self.line(&format!("let _{} = {};", ty_text, init_text));
                            return;
                        }
                        if self.lookup(name).is_some() {
                            if ty.is_some() {
                                self.err(line, 1, format!("`{}` is already bound; a type annotation belongs on a `let`/`var` binding", name));
                            }
                            self.check_assignable(target, line);
                            self.line(&format!("{} = {};", ident(name), init_text));
                        } else {
                            if let Some(ctx) = self.fn_stack.last() {
                                if ctx.receiver.is_some() && self.self_has_field(name) {
                                    self.warn(line, 1, format!("this binds a new local `{}`; write `self.{} = …` to assign the field (§12.2)", name, name));
                                }
                            }
                            self.line(&format!("let {}{} = {};", ident(name), ty_text, init_text));
                            self.bind(name, Binding { mutable: false, ty: ty.map(|t| t.text.clone()), ref_typed, mut_ref: false, uredo_ty: inferred, known: true });
                        }
                    }
                    Expr::Tuple(items) => {
                        let names: Vec<String> = items.iter().flat_map(expr_idents).collect();
                        let bound: Vec<bool> = names.iter().map(|n| n == "_" || self.lookup(n).is_some()).collect();
                        let all_bound = bound.iter().all(|b| *b) && names.iter().any(|n| n != "_");
                        let none_bound = names.iter().zip(&bound).all(|(n, b)| n == "_" || !*b);
                        let pat = self.lower_expr(target);
                        if all_bound {
                            for it in items {
                                self.check_assignable(it, line);
                            }
                            self.line(&format!("{} = {};", pat, init_text));
                        } else if none_bound {
                            self.line(&format!("let {}{} = {};", pat, ty_text, init_text));
                            for n in names {
                                if n != "_" {
                                    self.bind(&n, Binding { mutable: false, ty: None, ref_typed: false, mut_ref: false, uredo_ty: None, known: false });
                                }
                            }
                        } else {
                            self.err(line, 1, "a pattern must bind all-new names or assign all-bound names (§8.1); use `let` to rebind");
                        }
                    }
                    Expr::StructLit { .. } => {
                        let names = expr_idents(target);
                        let none_bound = names.iter().all(|n| self.lookup(n).is_none());
                        let pat = self.lower_expr(target);
                        if none_bound {
                            self.line(&format!("let {}{} = {};", pat, ty_text, init_text));
                            for n in names {
                                self.bind(&n, Binding { mutable: false, ty: None, ref_typed: false, mut_ref: false, uredo_ty: None, known: false });
                            }
                        } else {
                            self.line(&format!("{} = {};", pat, init_text));
                        }
                    }
                    Expr::Field { .. } | Expr::Index { .. } | Expr::Unary { op: "*", .. } => {
                        self.check_assignable(target, line);
                        let t = self.lower_place(target);
                        self.line(&format!("{} = {};", t, init_text));
                    }
                    _ => {
                        self.err(line, 1, "invalid assignment target");
                    }
                }
            }
        }
    }

    fn self_has_field(&self, name: &str) -> bool {
        if let Some(ctx) = self.fn_stack.last() {
            if let Some(st) = &ctx.self_ty {
                if let Some(info) = self.types.get(st) {
                    return info.fields.iter().any(|f| f == name);
                }
            }
        }
        false
    }

    /// Mutability check for assignment targets (§8.1, §12.3).
    fn check_assignable(&mut self, target: &Expr, line: usize) {
        let root = root_ident(target);
        let Some(root) = root else { return };
        if root == "self" {
            let ok = self.fn_stack.last().map(|c| matches!(c.receiver, Some(Receiver::RefMut) | Some(Receiver::MutValue) | Some(Receiver::Explicit(_)))).unwrap_or(true);
            if !ok {
                let field = match target {
                    Expr::Field { name, .. } => name.clone(),
                    _ => "a field".to_string(),
                };
                self.err(line, 1, format!("this method mutates `{}`; declare `self: inout` (§12.3)", field));
            }
            return;
        }
        match self.lookup(&root) {
            Some(b) if !b.known => {}
            Some(b) => {
                let through_ref = matches!(target, Expr::Field { .. } | Expr::Index { .. } | Expr::Unary { op: "*", .. });
                if !(b.mutable || (through_ref && b.mut_ref)) {
                    if through_ref {
                        self.err(line, 1, format!("cannot assign through `{}`: it is not a `var` binding or an `inout` parameter (§8.1)", root));
                    } else if b.mut_ref {
                        self.err(line, 1, format!("`{}` is an `inout` parameter, a `&mut` reference; write `*{} = …` to assign through it", root, root));
                    } else {
                        self.err(line, 1, format!("`{}` is immutable; declare it with `var {} = …` to allow assignment (§8.1)", root, root));
                    }
                }
            }
            None => {
                if self.fn_stack.last().map(|c| c.receiver.is_some()).unwrap_or(false) && self.self_has_field(&root) {
                    if let Expr::Path(_) = target {
                        self.err(line, 1, format!("field writes are spelled `self.{} …` (§12.2)", root));
                    }
                }
            }
        }
    }

    fn lower_place(&mut self, e: &Expr) -> String {
        self.lower_expr(e)
    }

    fn infer_uredo_type(&self, init: &Expr, ty: Option<&Type>) -> Option<String> {
        if let Some(t) = ty {
            let seg = last_segment(&t.text).to_string();
            if self.types.contains_key(&seg) {
                return Some(seg);
            }
            return None;
        }
        match init {
            Expr::StructLit { path, .. } => {
                let seg = if path == "Self" { self.fn_stack.last().and_then(|c| c.self_ty.clone()) } else { Some(last_segment(path).to_string()) };
                seg.filter(|s| self.types.contains_key(s))
            }
            Expr::Call { callee, .. } => {
                if let Expr::Path(p) = &**callee {
                    if let Some((ty, f)) = p.rsplit_once("::") {
                        let tyname = if ty == "Self" { self.fn_stack.last().and_then(|c| c.self_ty.clone()) } else { Some(last_segment(ty).to_string()) };
                        if let Some(tn) = tyname {
                            if let Some(info) = self.types.get(&tn) {
                                if let Some(sig) = info.methods.get(f) {
                                    if let Some(r) = &sig.ret {
                                        if r.trim() == "Self" || last_segment(r) == tn {
                                            return Some(tn);
                                        }
                                    }
                                }
                            }
                        }
                    } else if let Some(sig) = self.fns.get(p) {
                        if let Some(r) = &sig.ret {
                            let seg = last_segment(r).to_string();
                            if self.types.contains_key(&seg) && !r.contains('<') && !r.contains('&') {
                                return Some(seg);
                            }
                        }
                    }
                }
                None
            }
            Expr::MethodCall { recv, name, .. } => {
                let rt = self.expr_uredo_type(recv)?;
                let info = self.types.get(&rt)?;
                let sig = info.methods.get(name)?;
                let r = sig.ret.as_ref()?;
                if r.trim() == "Self" { Some(rt) } else { let seg = last_segment(r).to_string(); if self.types.contains_key(&seg) { Some(seg) } else { None } }
            }
            Expr::Paren(e) => self.infer_uredo_type(e, None),
            _ => None,
        }
    }

    fn expr_uredo_type(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Path(p) if p == "self" => self.fn_stack.last().and_then(|c| c.self_ty.clone()),
            Expr::Path(p) if !p.contains("::") => self.lookup(p).and_then(|b| b.uredo_ty.clone()),
            Expr::Paren(e) | Expr::Unary { op: "*", expr: e } | Expr::Ref { expr: e, .. } => self.expr_uredo_type(e),
            _ => self.infer_uredo_type(e, None),
        }
    }

    // ----- expressions -----
    fn lower_expr(&mut self, e: &Expr) -> String {
        self.lower_expr_mode(e, BlockMode::Value)
    }

    fn lower_expr_mode(&mut self, e: &Expr, mode: BlockMode) -> String {
        match e {
            Expr::Int(s) | Expr::Float(s) | Expr::Str(s) | Expr::Char(s) => s.clone(),
            Expr::Bool(b) => b.to_string(),
            Expr::Unit => "()".to_string(),
            Expr::Path(p) => self.lower_path(p),
            Expr::Unary { op, expr } => format!("{}{}", op, self.lower_expr(expr)),
            Expr::Ref { mutable, expr } => format!("&{}{}", if *mutable { "mut " } else { "" }, self.lower_expr(expr)),
            Expr::Binary { op, lhs, rhs } => format!("{} {} {}", self.lower_expr(lhs), op, self.lower_expr(rhs)),
            Expr::Cast { expr, ty } => format!("{} as {}", self.lower_expr(expr), lower_type(ty.text.trim())),
            Expr::Call { callee, args, line, col } => self.lower_call(callee, args, *line, *col),
            Expr::MethodCall { recv, name, turbofish, args, line, col } => {
                let r = self.lower_expr(recv);
                let recv_ty = self.expr_uredo_type(recv);
                let sig = recv_ty.as_ref().and_then(|t| self.types.get(t)).and_then(|i| i.methods.get(name)).cloned();
                let modes: Option<Vec<ParamMode>> = sig.as_ref().map(|s| s.modes.clone());
                let named = sig.as_ref().map(|s| (name.clone(), s.decl.clone()));
                let (a, notes, takes, by_value) = self.lower_args(args, modes.as_deref(), (*line, *col), named.as_ref().map(|(n, d)| (n.as_str(), d.as_str())));
                let text = format!("{}.{}{}({})", r, ident(name), turbofish.as_deref().map(|t| format!("::{}", t)).unwrap_or_default(), a);
                let before = self.call_source(*line, *col);
                match &sig {
                    Some(s) => {
                        let recv_note = match &s.receiver {
                            Some(Receiver::RefMut) => " receiver: `self: inout` (needs a `var` binding or an `inout` parameter);",
                            Some(Receiver::Value) | Some(Receiver::MutValue) => " receiver: `self: take` (the receiver is consumed);",
                            Some(Receiver::Ref) => " receiver: `self` (shared borrow);",
                            // a written receiver type is Rust's; rustc's own message is the useful one
                            Some(Receiver::Explicit(_)) | None => "",
                        };
                        let mut rule = format!("method `{}` of Uredo type `{}`:{}{}", name, recv_ty.clone().unwrap_or_default(), recv_note, if notes.is_empty() { " no arguments".to_string() } else { format!(" {}", notes.join("; ")) });
                        if rule.ends_with(';') {
                            rule.pop();
                        }
                        self.elab(Elab { line: *line, col: *col, kind: "method".into(), rule, before, after: text.clone(), inserted: inserted_summary(&notes), callee: Some(name.clone()), declared: Some(s.decl.clone()), takes, by_value });
                    }
                    None => {
                        if !args.is_empty() {
                            self.elab(Elab { line: *line, col: *col, kind: "method".into(), rule: "arguments passed verbatim (receiver type not known to Uredo; §10.3)".into(), before, after: text.clone(), inserted: "borrow: none · clone: none · allocation: none · dispatch: none · sync: none".into(), callee: Some(name.clone()), ..Default::default() });
                        }
                    }
                }
                text
            }
            Expr::Field { expr, name } => format!("{}.{}", self.lower_expr(expr), if name.chars().all(|c| c.is_ascii_digit()) { name.clone() } else { ident(name) }),
            Expr::Index { expr, index } => format!("{}[{}]", self.lower_expr(expr), self.lower_expr(index)),
            Expr::Try(e) => format!("{}?", self.lower_expr(e)),
            Expr::Await(e) => format!("{}.await", self.lower_expr(e)),
            Expr::Paren(e) => format!("({})", self.lower_expr(e)),
            Expr::Tuple(items) => {
                let parts: Vec<String> = items.iter().map(|i| self.lower_expr(i)).collect();
                if parts.len() == 1 { format!("({},)", parts[0]) } else { format!("({})", parts.join(", ")) }
            }
            Expr::Array(items) => {
                let parts: Vec<String> = items.iter().map(|i| self.lower_expr(i)).collect();
                format!("[{}]", parts.join(", "))
            }
            Expr::ArrayRepeat { value, count } => format!("[{}; {}]", self.lower_expr(value), self.lower_expr(count)),
            Expr::StructLit { path, fields, base } => {
                let mut parts = Vec::new();
                for f in fields {
                    let mut s = String::new();
                    for a in &f.attrs {
                        s.push_str(&format!("#{} ", attr_text(a)));
                    }
                    match &f.value {
                        Some(v) => s.push_str(&format!("{}: {}", ident(&f.name), self.lower_expr(v))),
                        None => {
                            // punned field: field sugar may apply to the value
                            let v = self.lower_path(&f.name);
                            if v == f.name || v == ident(&f.name) {
                                s.push_str(&ident(&f.name));
                            } else {
                                s.push_str(&format!("{}: {}", ident(&f.name), v));
                            }
                        }
                    }
                    parts.push(s);
                }
                if let Some(b) = base {
                    parts.push(format!("..{}", self.lower_expr(b)));
                }
                if parts.is_empty() { format!("{} {{}}", path) } else { format!("{} {{ {} }}", path, parts.join(", ")) }
            }
            Expr::Range { lo, hi, inclusive } => {
                let l = lo.as_ref().map(|e| self.lower_expr(e)).unwrap_or_default();
                let h = hi.as_ref().map(|e| self.lower_expr(e)).unwrap_or_default();
                format!("{}{}{}", l, if *inclusive { "..=" } else { ".." }, h)
            }
            Expr::Closure { is_move, params, ret, throws, body } => {
                self.push_scope();
                if *is_move {
                    self.move_closure_depth += 1;
                }
                let mut ps = Vec::new();
                for p in params {
                    let ty_text = p.ty.as_ref().map(|t| {
                        let s = t.text.trim();
                        if s == "str" { "&str".to_string() } else if is_slice_type(s) { format!("&{}", s) } else { lower_type(s) }
                    });
                    // `var acc` on a closure parameter is Rust's `mut acc` (D31)
                    let (pat, is_var) = match p.pat.trim().strip_prefix("var ") {
                        Some(rest) => (format!("mut {}", rest.trim()), true),
                        None => (p.pat.clone(), false),
                    };
                    match &ty_text {
                        Some(t) => ps.push(format!("{}: {}", pat, t)),
                        None => ps.push(pat.clone()),
                    }
                    self.bind_pattern_names(p.pat.trim().strip_prefix("var ").unwrap_or(&p.pat), is_var);
                    for n in pattern_idents(&p.pat) {
                        if let Some(t) = &ty_text {
                            let rt = t.starts_with('&');
                            self.bind(&n, Binding { mutable: false, ty: Some(t.clone()), ref_typed: rt, mut_ref: t.starts_with("&mut"), uredo_ty: None, known: true });
                        }
                    }
                }
                let is_block = matches!(**body, Expr::Block(_));
                // a `throws` closure returns a `Result` and its body is wrapped like a `throws`
                // function body (§15.3, §17)
                let (b, ret_text) = match throws {
                    Some(err) => {
                        let e = match err {
                            Some(e) => e.clone(),
                            None => match &self.default_error {
                                Some(d) => d.clone(),
                                None => {
                                    self.err(self.cur_line, 1, "a bare `throws` closure needs `@!default_error(Type)` in scope (§15.1)");
                                    "()".to_string()
                                }
                            },
                        };
                        let ok = ret.as_ref().map(|r| lower_type(r.text.trim())).unwrap_or_else(|| "()".to_string());
                        let ctx = FnCtx { receiver: self.fn_stack.last().and_then(|c| c.receiver.clone()), self_ty: self.fn_stack.last().and_then(|c| c.self_ty.clone()), throws: Some(Some(e.clone())), ret: ret.as_ref().map(|r| r.text.clone()) };
                        self.fn_stack.push(ctx);
                        let text = match &**body {
                            Expr::Block(blk) => {
                                let inner = self.capture(|l| l.lower_throws_body(blk, true));
                                format!("{{\n{}}}", indent_text(&inner, 1))
                            }
                            e => {
                                let t = self.lower_tail_throws(e);
                                format!("{{ {} }}", t)
                            }
                        };
                        self.fn_stack.pop();
                        (text, Some(format!("::core::result::Result<{}, {}>", ok, e)))
                    }
                    None => (self.lower_expr(body), ret.as_ref().map(|r| lower_type(r.text.trim()))),
                };
                if *is_move {
                    self.move_closure_depth -= 1;
                }
                self.pop_scope();
                let mv = if *is_move { "move " } else { "" };
                let block_body = is_block || throws.is_some();
                match ret_text {
                    Some(r) if block_body => format!("{}|{}| -> {} {}", mv, ps.join(", "), r, b),
                    Some(r) => format!("{}|{}| -> {} {{ {} }}", mv, ps.join(", "), r, b),
                    None => format!("{}|{}| {}", mv, ps.join(", "), b),
                }
            }
            Expr::If(i) => self.lower_if(i, mode),
            Expr::Match { scrutinee, arms } => {
                let s = self.lower_expr(scrutinee);
                let scrut_ty = self.expr_uredo_type(scrutinee);
                let header_line = match expr_pos(scrutinee) {
                    (0, _) => self.cur_line,
                    (l, _) => l,
                };
                let mut out = mark_lines(&format!("match {} {{", s), header_line);
                out.push('\n');
                for arm in arms {
                    self.cur_line = arm.line;
                    let pat = self.lower_pattern(&arm.pat, scrut_ty.as_deref());
                    self.push_scope();
                    self.bind_pattern_names(&arm.pat, false);
                    let guard = match &arm.guard {
                        Some(g) => format!(" if {}", self.lower_expr(g)),
                        None => String::new(),
                    };
                    if arm.inline {
                        let first = arm.body.stmts.first().unwrap();
                        let v = if let StmtKind::Expr(e) = &first.kind {
                            self.lower_expr_mode(e, mode)
                        } else {
                            // `return`, `throw`, `break`, `continue`, an assignment: statements that
                            // are expressions in Rust; drop the statement terminator.
                            let t = strip_markers(&self.capture(|l| l.lower_stmt(first, BlockMode::Unit, false)));
                            t.trim_end().trim_end_matches(';').to_string()
                        };
                        out.push_str(&mark_lines(&indent_text(&format!("{}{} => {},\n", pat, guard, v), 1), arm.line));
                    } else {
                        let body = self.capture(|l| {
                            l.lower_stmts(&arm.body.stmts, mode);
                        });
                        out.push_str(&mark_lines(&indent_text(&format!("{}{} => {{\n{}}}\n", pat, guard, indent_text(&body, 1)), 1), arm.line));
                    }
                    self.pop_scope();
                }
                out.push_str(&mark_lines("}", header_line));
                out
            }
            Expr::Loop { label, body } => {
                let lbl = label.as_ref().map(|l| format!("{}: ", l)).unwrap_or_default();
                let b = self.capture(|l| {
                    l.push_scope();
                    l.lower_stmts(&body.stmts, BlockMode::Unit);
                    l.pop_scope();
                });
                format!("{}loop {{\n{}}}", lbl, indent_text(&b, 1))
            }
            Expr::Block(b) => {
                let body = self.capture(|l| {
                    l.push_scope();
                    l.lower_stmts(&b.stmts, mode);
                    l.pop_scope();
                });
                format!("{{\n{}}}", indent_text(&body, 1))
            }
            Expr::UnsafeBlock(b) => {
                let body = self.capture(|l| {
                    l.push_scope();
                    l.lower_stmts(&b.stmts, mode);
                    l.pop_scope();
                });
                format!("unsafe {{\n{}}}", indent_text(&body, 1))
            }
            Expr::Rust(text) => text.clone(),
            Expr::Macro { path, body } => format!("{}!{}", path, body),
            Expr::Assign { target, op, value } => {
                self.check_assignable(target, 0);
                format!("{} {} {}", self.lower_place(target), op, self.lower_expr(value))
            }
        }
    }

    /// Runs `f` with a fresh output buffer at indentation 0 and returns what it emitted.
    fn capture(&mut self, f: impl FnOnce(&mut Lowerer)) -> String {
        let saved_out = std::mem::take(&mut self.out);
        let saved_indent = self.indent;
        self.indent = 0;
        f(self);
        self.indent = saved_indent;
        std::mem::replace(&mut self.out, saved_out)
    }

    fn lower_if(&mut self, i: &IfExpr, mode: BlockMode) -> String {
        // taken before anything is lowered: lowering the body advances `cur_line` past the header
        let header_line = match expr_pos(&i.cond) {
            (0, _) => self.cur_line,
            (l, _) => l,
        };
        let cond = match &i.pat {
            Some(p) => format!("let {} = {}", self.lower_pattern(p, None), self.lower_expr(&i.cond)),
            None => self.lower_expr(&i.cond),
        };
        let then = self.capture(|l| {
            l.push_scope();
            if let Some(p) = &i.pat {
                l.bind_pattern_names(p, false);
            }
            l.lower_stmts(&i.then.stmts, mode);
            l.pop_scope();
        });
        // the header and the braces are the *header's* line, not wherever the body ended up
        let mut out = mark_lines(&format!("if {} {{", cond), header_line);
        out.push('\n');
        out.push_str(&indent_text(&then, 1));
        out.push_str(&mark_lines("}", header_line));
        match &i.else_ {
            None => {}
            Some(ElseBranch::ElseIf(n)) => {
                let rest = self.lower_if(n, mode);
                out.push_str(" else ");
                out.push_str(&rest);
            }
            Some(ElseBranch::Else(b)) => {
                let eb = self.capture(|l| {
                    l.push_scope();
                    l.lower_stmts(&b.stmts, mode);
                    l.pop_scope();
                });
                out.push_str(&mark_lines(" else {", header_line));
                out.push('\n');
                out.push_str(&indent_text(&eb, 1));
                out.push_str(&mark_lines("}", header_line));
            }
        }
        out
    }

    /// Patterns are Rust's (§13.1); bare variant names of a known scrutinee type are qualified.
    fn lower_pattern(&self, pat: &str, scrut_ty: Option<&str>) -> String {
        let p = pat.trim();
        if let Some(t) = scrut_ty {
            if let Some(info) = self.types.get(t) {
                // qualify bare variants: `Quit` -> `Msg::Quit`, `Text(t)` -> `Msg::Text(t)`, `A | B`
                let parts = split_top_level(p, '|');
                let mut out = Vec::new();
                for part in parts {
                    let head: String = part.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                    if info.variants.iter().any(|v| *v == head) && !part.starts_with("::") {
                        out.push(format!("{}::{}", t, part));
                    } else {
                        out.push(part);
                    }
                }
                return out.join(" | ");
            }
        }
        p.to_string()
    }

    fn lower_path(&mut self, p: &str) -> String {
        if p.contains("::") || p.contains(' ') || p.contains('<') {
            return p.to_string();
        }
        if p == "self" || p == "Self" {
            return p.to_string();
        }
        // field sugar (§12.2, D41)
        if self.lookup(p).is_none() && !self.glob_import && self.move_closure_depth == 0 && !self.fns.contains_key(p) && !self.types.contains_key(p) {
            if let Some(ctx) = self.fn_stack.last() {
                if ctx.receiver.is_some() && self.self_has_field(p) {
                    self.elab(Elab { line: self.cur_line, col: 1, kind: "field-sugar".into(), rule: "D41 (bare field name in an instance method reads `self.field`)".into(), before: p.to_string(), after: format!("self.{}", ident(p)), inserted: "borrow: none · clone: none · allocation: none · dispatch: none · sync: none".into(), ..Default::default() });
                    return format!("self.{}", ident(p));
                }
            }
        }
        ident(p)
    }

    fn lower_call(&mut self, callee: &Expr, args: &[Expr], line: usize, col: usize) -> String {
        if let Expr::Path(p) = callee {
            // intrinsics (§11.3)
            // In a `@!no_std` module the intrinsics that *can* work are the ones `core` and `alloc`
            // provide: `format` is `alloc`'s and `panic` is `core`'s, while `print` and `eprint`
            // want a standard output that a `no_std` target does not have (§11.3, §35).
            if (p == "print" || p == "eprint") && self.no_std {
                self.err(line, col, format!("`{}` is not available in a `@!no_std` module: there is no standard output to write to (§11.3)", p));
                self.diags.last_mut().unwrap().notes.push("note: `format` still works and lowers to `::alloc::format!`; writing it somewhere is the target's business".into());
                return String::new();
            }
            if let Some(mac) = match (p.as_str(), self.no_std) {
                ("print", _) => Some("::std::println!"),
                ("eprint", _) => Some("::std::eprintln!"),
                ("format", false) => Some("::std::format!"),
                ("format", true) => Some("::alloc::format!"),
                ("panic", false) => Some("::std::panic!"),
                ("panic", true) => Some("::core::panic!"),
                _ => None,
            } {
                if self.lookup(p).is_some() {
                    self.err(line, col, format!("`{}` is an intrinsic; a local of that name cannot be called bare (§11.3)", p));
                }
                if args.is_empty() {
                    if p == "print" || p == "eprint" {
                        return format!("{}()", mac);
                    }
                    self.err(line, col, format!("`{}` needs at least one argument", p));
                    return format!("{}()", mac);
                }
                let rest: Vec<String> = args.iter().skip(1).map(|a| self.lower_expr(a)).collect();
                let alloc = if p == "format" { "allocation: String (declared by the intrinsic's name, §11.3)" } else { "allocation: none" };
                let text = if let Expr::Str(s) = &args[0] {
                    let mut all = vec![s.clone()];
                    all.extend(rest);
                    format!("{}({})", mac, all.join(", "))
                } else {
                    if !rest.is_empty() {
                        self.err(line, col, format!("`{}` with several arguments needs a format-string literal first (§11.3)", p));
                    }
                    let a = self.lower_expr(&args[0]);
                    format!("{}(\"{{}}\", {})", mac, a)
                };
                let before = self.call_source(line, col);
                self.elab(Elab { line, col, kind: "intrinsic".into(), rule: format!("§11.3 (`{}` is an intrinsic: the literal is a Rust format string; other arguments are Uredo expressions)", p), before, after: text.clone(), inserted: format!("borrow: none · clone: none · {} · dispatch: none · sync: none", alloc), callee: Some(p.clone()), ..Default::default() });
                return text;
            }
            // Uredo-declared function or associated function
            let (sig, why_verbatim): (Option<FnSig>, &str) = if !p.contains("::") {
                if self.lookup(p).is_some() {
                    (None, "callee is a closure or function-valued binding") // verbatim (§10.3)
                } else {
                    match self.fns.get(p).cloned() {
                        Some(s) => (Some(s), ""),
                        None => (None, "callee is a Rust item"),
                    }
                }
            } else if let Some((ty, f)) = p.rsplit_once("::") {
                let tn = if ty == "Self" { self.fn_stack.last().and_then(|c| c.self_ty.clone()) } else { Some(last_segment(ty).to_string()) };
                match tn.and_then(|t| self.types.get(&t)).and_then(|i| i.methods.get(f)).cloned().or_else(|| self.index_fn(p)) {
                    Some(s) => (Some(s), ""),
                    None => (None, "callee is a Rust item"),
                }
            } else {
                (None, "callee is a Rust item")
            };
            let modes: Option<Vec<ParamMode>> = sig.as_ref().map(|s| s.modes.clone());
            let named = sig.as_ref().map(|s| (p.clone(), s.decl.clone()));
            let c = self.lower_path(p);
            let (a, notes, takes, by_value) = self.lower_args(args, modes.as_deref(), (line, col), named.as_ref().map(|(n, d)| (n.as_str(), d.as_str())));
            let text = format!("{}({})", c, a);
            let before = self.call_source(line, col);
            match sig {
                Some(s) => {
                    let rule = if notes.is_empty() { format!("Uredo fn `{}`: no arguments", p) } else { format!("{} (callee is Uredo fn `{}`)", notes.join("; "), p) };
                    self.elab(Elab { line, col, kind: "call".into(), rule, before, after: text.clone(), inserted: inserted_summary(&notes), callee: Some(p.clone()), declared: Some(s.decl.clone()), takes, by_value });
                }
                None => {
                    if !args.is_empty() {
                        self.elab(Elab { line, col, kind: "call".into(), rule: format!("arguments passed verbatim ({}; §10.3)", why_verbatim), before, after: text.clone(), inserted: "borrow: none · clone: none · allocation: none · dispatch: none · sync: none".into(), callee: Some(p.clone()), ..Default::default() });
                    }
                }
            }
            return text;
        }
        let c = self.lower_expr(callee);
        let (a, _, _, _) = self.lower_args(args, None, (line, col), None);
        format!("{}({})", c, a)
    }

    /// Call-site rule (§10.3, D47).
    /// Returns the argument list, one note per argument (for `explain`), and the arguments
    /// whose ownership moved (`take` parameters). `callee` is the resolved name and declaration,
    /// which the P8 clone count needs to say which parameter a clone was written for (§38).
    fn lower_args(&mut self, args: &[Expr], modes: Option<&[ParamMode]>, call_pos: (usize, usize), callee: Option<(&str, &str)>) -> (String, Vec<String>, Vec<String>, Vec<String>) {
        let mut out = Vec::new();
        let mut notes = Vec::new();
        let mut takes = Vec::new();
        let mut by_value = Vec::new();
        let pos_of = |a: &Expr| {
            let p = expr_pos(a);
            if p.0 == 0 { call_pos } else { p }
        };
        for (i, a) in args.iter().enumerate() {
            let text = self.lower_expr(a);
            let mode = modes.and_then(|m| m.get(i).copied()).unwrap_or(ParamMode::Verbatim);
            let n = i + 1;
            match mode {
                ParamMode::Verbatim => {
                    if modes.is_some() {
                        notes.push(format!("arg {}: by value (P1/P6/P7: scalar, known-Copy, explicit reference or pattern parameter)", n));
                    } else if matches!(a, Expr::Ref { .. }) {
                        // D2: the callee is a Rust item, so its arguments are Rust's to spell and
                        // this `&` is one the surface did not save (§10.3)
                        self.measures.d2_refs += 1;
                    }
                    out.push(text)
                }
                ParamMode::Take => {
                    notes.push(format!("arg {}: `take` (P5: ownership of `{}` moves into the callee; no clone inserted, §10.4)", n, text));
                    takes.push(text.clone());
                    out.push(text)
                }
                ParamMode::ByValueGeneric => {
                    notes.push(format!("arg {}: P8 by value (the parameter's type is a type parameter or `impl Trait`; `{}` is moved unless its type is `Copy` — D52)", n, text));
                    if let (Some((cname, decl)), true) = (callee, is_clone_call(a)) {
                        let (l, c) = pos_of(a);
                        self.measures.p8_clones.push(lint::clone_at_by_value(cname, &text, decl, l, c));
                    }
                    by_value.push(text.clone());
                    out.push(text)
                }
                ParamMode::Borrow => {
                    if self.is_syntactic_reference(a) {
                        if matches!(a, Expr::Ref { .. }) {
                            self.measures.own_refs += 1;
                        }
                        notes.push(format!("arg {}: P2/P3 shared borrow, argument already a reference: passed verbatim (D47)", n));
                        out.push(text);
                    } else {
                        notes.push(format!("arg {}: P2/P3 default shared borrow inserted (`&{}`)", n, text));
                        out.push(format!("&{}", text));
                    }
                }
                ParamMode::BorrowMut => {
                    if let Expr::Ref { mutable: true, .. } = a {
                        self.measures.own_refs += 1;
                        notes.push(format!("arg {}: P4 `inout`, explicit `&mut` passed verbatim", n));
                        out.push(text);
                        continue;
                    }
                    if let Some(root) = root_ident(a) {
                        if root == "self" {
                            let ok = self.fn_stack.last().map(|c| matches!(c.receiver, Some(Receiver::RefMut) | Some(Receiver::MutValue) | Some(Receiver::Explicit(_)))).unwrap_or(true);
                            if !ok {
                                let (l, c) = pos_of(a);
                                self.err(l, c, "passing `self` to an `inout` parameter needs `self: inout` (§12.3)");
                            }
                        } else if let Some(b) = self.lookup(&root).cloned() {
                            let through = !matches!(a, Expr::Path(_));
                            if !(b.mutable || b.mut_ref) {
                                let (l, c) = pos_of(a);
                                self.err(l, c, format!("`{}` is immutable and cannot be passed to an `inout` parameter; declare it with `var` (§10.3)", root));
                            }
                            if b.mut_ref && !through && !b.mutable {
                                // an `inout` parameter passed on: already `&mut T`, reborrow verbatim
                                notes.push(format!("arg {}: P4 `inout`, an `inout` parameter passed on: verbatim (already `&mut`)", n));
                                out.push(text);
                                continue;
                            }
                        }
                    }
                    notes.push(format!("arg {}: P4 `inout` mutable borrow inserted (`&mut {}`)", n, text));
                    out.push(format!("&mut {}", text));
                }
            }
        }
        (out.join(", "), notes, takes, by_value)
    }

    /// D47: the argument is syntactically already a reference.
    fn is_syntactic_reference(&self, a: &Expr) -> bool {
        match a {
            Expr::Ref { .. } => true,
            Expr::Str(_) => true,
            Expr::Path(p) if p == "self" => self.fn_stack.last().map(|c| matches!(c.receiver, Some(Receiver::Ref) | Some(Receiver::RefMut))).unwrap_or(false),
            Expr::Path(p) if !p.contains("::") => self.lookup(p).map(|b| b.ref_typed).unwrap_or(false),
            Expr::Paren(e) => self.is_syntactic_reference(e),
            _ => false,
        }
    }
}

/// A bare `.clone()`, through any parentheses: the shape §38 counts at a P8 call site.
fn is_clone_call(a: &Expr) -> bool {
    match a {
        Expr::MethodCall { name, args, .. } => name == "clone" && args.is_empty(),
        Expr::Paren(e) => is_clone_call(e),
        _ => false,
    }
}

/// Expands a `use` path into (full path, local name) pairs: `a::{B, c as d}` -> [(a::B, B), (a::c, d)].
pub fn use_names(path: &str) -> Vec<(String, String)> {
    let p = path.trim().trim_end_matches(';');
    let mut out = Vec::new();
    if let Some(open) = p.find('{') {
        let prefix = p[..open].trim_end_matches("::").trim();
        let inner = p[open + 1..p.rfind('}').unwrap_or(p.len())].to_string();
        for part in split_top_level(&inner, ',') {
            for (full, local) in use_names(&part) {
                let full = if full == "self" { prefix.to_string() } else if prefix.is_empty() { full } else { format!("{}::{}", prefix, full) };
                out.push((full, local));
            }
        }
        return out;
    }
    if p.ends_with('*') {
        return out;
    }
    let (full, local) = match p.split_once(" as ") {
        Some((f, l)) => (f.trim().to_string(), l.trim().to_string()),
        None => (p.to_string(), p.rsplit("::").next().unwrap_or(p).to_string()),
    };
    out.push((full, local));
    out
}

/// The declaration as the Uredo programmer wrote it, for diagnostics (§27).
pub fn fn_decl_text(f: &FnDecl) -> String {
    let mut ps = Vec::new();
    if let Some(r) = &f.receiver {
        ps.push(match r {
            Receiver::Ref => "self".to_string(),
            Receiver::RefMut => "self: inout".to_string(),
            Receiver::Value => "self: take".to_string(),
            Receiver::MutValue => "var self: take".to_string(),
            Receiver::Explicit(t) => format!("self: {}", t),
        });
    }
    for p in &f.params {
        let m = match p.mode {
            Mode::Default => "",
            Mode::Take => "take ",
            Mode::Inout => "inout ",
        };
        ps.push(format!("{}{}: {}{}", if p.is_var { "var " } else { "" }, p.pat, m, p.ty.text.trim()));
    }
    let ret = f.ret.as_ref().map(|t| format!(" -> {}", t.text.trim())).unwrap_or_default();
    let throws = match &f.throws {
        Some(Some(e)) => format!(" throws {}", e),
        Some(None) => " throws".to_string(),
        None => String::new(),
    };
    format!("fn {}{}({}){}{}", f.name, f.generics.as_deref().unwrap_or(""), ps.join(", "), ret, throws)
}

/// The `Inserted by Uredo` line of `explain` (§26.2), from the per-argument notes.
fn inserted_summary(notes: &[String]) -> String {
    let shared = notes.iter().filter(|n| n.contains("shared borrow inserted")).count();
    let mutable = notes.iter().filter(|n| n.contains("mutable borrow inserted")).count();
    let borrow = match (shared, mutable) {
        (0, 0) => "none".to_string(),
        (s, 0) => format!("shared ×{}", s),
        (0, m) => format!("mutable ×{}", m),
        (s, m) => format!("shared ×{}, mutable ×{}", s, m),
    };
    format!("borrow: {} · clone: none · allocation: none · dispatch: none · sync: none", borrow)
}


/// Type-position lowering: `T?` -> `::core::option::Option<T>` (§9.3), applied to every
/// atom, so `Vec<u32?>`, `(String?, u8)` and `&User?` (= `Option<&User>`, D54) all work.
pub fn lower_type(t: &str) -> String {
    let chars: Vec<char> = t.chars().collect();
    let mut pos = 0;
    let out = lower_type_seq(&chars, &mut pos, None);
    out
}

/// Lowers a sequence of type atoms and separators up to `stop` (or the end).
fn lower_type_seq(c: &[char], pos: &mut usize, stop: Option<char>) -> String {
    let mut out = String::new();
    while *pos < c.len() {
        let ch = c[*pos];
        if Some(ch) == stop {
            break;
        }
        if ch.is_whitespace() || ch == ',' || ch == '+' || ch == ';' || ch == ':' || ch == '=' || ch == '|' {
            out.push(ch);
            *pos += 1;
            continue;
        }
        if ch == '-' && c.get(*pos + 1) == Some(&'>') {
            out.push_str("->");
            *pos += 2;
            continue;
        }
        out.push_str(&lower_type_atom(c, pos));
    }
    out
}

fn lower_type_atom(c: &[char], pos: &mut usize) -> String {
    let mut prefix = String::new();
    // reference / pointer prefixes
    loop {
        if c.get(*pos) == Some(&'&') {
            prefix.push('&');
            *pos += 1;
            if c.get(*pos) == Some(&'\'') {
                while *pos < c.len() && !c[*pos].is_whitespace() {
                    prefix.push(c[*pos]);
                    *pos += 1;
                }
                prefix.push(' ');
                *pos += 1;
            }
            if starts_with_word(c, *pos, "mut") {
                prefix.push_str("mut ");
                *pos += 4;
            }
            continue;
        }
        if c.get(*pos) == Some(&'*') {
            prefix.push('*');
            *pos += 1;
            for w in ["const", "mut"] {
                if starts_with_word(c, *pos, w) {
                    prefix.push_str(w);
                    prefix.push(' ');
                    *pos += w.len() + 1;
                }
            }
            continue;
        }
        for w in ["dyn", "impl"] {
            if starts_with_word(c, *pos, w) {
                prefix.push_str(w);
                prefix.push(' ');
                *pos += w.len() + 1;
            }
        }
        break;
    }
    let mut core = String::new();
    match c.get(*pos) {
        Some('(') | Some('[') => {
            let open = c[*pos];
            let close = if open == '(' { ')' } else { ']' };
            *pos += 1;
            let inner = lower_type_seq(c, pos, Some(close));
            *pos += 1; // close
            // `&(T?)` — the spelling for a borrow of a whole optional (§9.3) — needs no parentheses
            // in the generated Rust. A tuple (a top-level comma) and a bound list (`+`) keep theirs.
            if open == '(' && !inner.is_empty() && split_top_level(&inner, ',').len() == 1
                && split_top_level(&inner, '+').len() == 1 && !inner.ends_with(',')
            {
                core.push_str(inner.trim());
            } else {
                core.push(open);
                core.push_str(&inner);
                core.push(close);
            }
        }
        Some(_) => {
            // path segment(s), lifetimes, literals, with generic lists
            while *pos < c.len() {
                let ch = c[*pos];
                if ch == '<' {
                    *pos += 1;
                    let inner = lower_type_seq(c, pos, Some('>'));
                    *pos += 1;
                    core.push('<');
                    core.push_str(&inner);
                    core.push('>');
                    continue;
                }
                if ch == '(' && (core.ends_with("Fn") || core.ends_with("FnMut") || core.ends_with("FnOnce")) {
                    *pos += 1;
                    let inner = lower_type_seq(c, pos, Some(')'));
                    *pos += 1;
                    core.push('(');
                    core.push_str(&inner);
                    core.push(')');
                    continue;
                }
                if ch.is_alphanumeric() || ch == '_' || ch == ':' || ch == '\'' || ch == '!' {
                    // `::` path separator or lifetime; a lone `:` (bound) ends the atom
                    if ch == ':' && c.get(*pos + 1) != Some(&':') && !core.ends_with(':') {
                        break;
                    }
                    core.push(ch);
                    *pos += 1;
                    continue;
                }
                break;
            }
        }
        None => {}
    }
    if core.is_empty() && prefix.is_empty() {
        // an unexpected character: keep it and move on (never loop)
        if let Some(ch) = c.get(*pos) {
            core.push(*ch);
            *pos += 1;
        }
    }
    // `&T?` is `Option<&T>` (§10.2, D54): the wrap applies to
    // the whole prefixed atom.
    let mut result = format!("{}{}", prefix, core);
    while c.get(*pos) == Some(&'?') {
        *pos += 1;
        result = format!("::core::option::Option<{}>", result);
    }
    result
}

fn starts_with_word(c: &[char], pos: usize, w: &str) -> bool {
    let wc: Vec<char> = w.chars().collect();
    if pos + wc.len() > c.len() {
        return false;
    }
    if c[pos..pos + wc.len()] != wc[..] {
        return false;
    }
    match c.get(pos + wc.len()) {
        Some(ch) => !(ch.is_alphanumeric() || *ch == '_'),
        None => true,
    }
}

/// Removes provenance markers from captured text that is re-embedded as an expression.
/// Attributes every physical line of `text` that does not already carry a marker to `line`.
///
/// A multi-line construct built as one string — a `match`, an `if` — reaches `line()` as a single
/// piece, and `line()` can only attribute the whole piece to whatever `cur_line` happens to be by
/// then, which for a `match` was its *last* arm: a debugger stopping on the header was told it
/// came from the bottom of the block (found 2026-09-13, writing the GDB helpers). A nested
/// construct has already marked its own lines, and those keep their own attribution.
fn mark_lines(text: &str, line: usize) -> String {
    text.split('\n')
        .map(|l| if l.is_empty() || l.contains(MARK) { l.to_string() } else { format!("{}{}{}", l, MARK, line) })
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_markers(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == MARK {
            while chars.peek().map(|d| d.is_ascii_digit()).unwrap_or(false) {
                chars.next();
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Does this `loop` have no way out?
///
/// A `throws` body ending in `loop { … }` with no `break` cannot fall through, so §15.3's
/// `Ok(())` after it is unreachable and rustc says so. Hand-written Rust would let the loop be
/// the tail — `loop` has type `!`, which coerces to the return type — and write nothing.
///
/// Deliberately conservative: any `break` anywhere inside, even one belonging to a nested loop,
/// makes this `false` and the wrapping is emitted as before. A false negative costs the
/// unreachable line that was always there; a false positive would drop a needed return.
fn loop_never_exits(e: &Expr) -> bool {
    fn block_has_break(b: &Block) -> bool {
        b.stmts.iter().any(|s| match &s.kind {
            StmtKind::Break { .. } => true,
            StmtKind::While { body, .. } | StmtKind::WhileLet { body, .. } | StmtKind::For { body, .. } | StmtKind::Unsafe(body) => block_has_break(body),
            StmtKind::Bind { else_block: Some(b), .. } => block_has_break(b),
            StmtKind::Expr(e) => expr_has_break(e),
            _ => false,
        })
    }
    fn expr_has_break(e: &Expr) -> bool {
        match e {
            Expr::Loop { body, .. } | Expr::Block(body) | Expr::UnsafeBlock(body) => block_has_break(body),
            Expr::If(i) => {
                block_has_break(&i.then)
                    || match &i.else_ {
                        Some(ElseBranch::Else(b)) => block_has_break(b),
                        Some(ElseBranch::ElseIf(n)) => expr_has_break(&Expr::If(n.clone())),
                        None => false,
                    }
            }
            Expr::Match { arms, .. } => arms.iter().any(|a| block_has_break(&a.body)),
            _ => false,
        }
    }
    matches!(e, Expr::Loop { body, .. } if !block_has_break(body))
}

fn is_block_like(e: &Expr) -> bool {
    matches!(e, Expr::If(_) | Expr::Match { .. } | Expr::Loop { .. } | Expr::Block(_) | Expr::UnsafeBlock(_))
        || matches!(e, Expr::Rust(t) if t.starts_with('{'))
        || matches!(e, Expr::Macro { body, .. } if body.starts_with('{'))
}

fn root_ident(e: &Expr) -> Option<String> {
    match e {
        Expr::Path(p) if !p.contains("::") => Some(p.clone()),
        Expr::Field { expr, .. } | Expr::Index { expr, .. } => root_ident(expr),
        Expr::Unary { op: "*", expr } => root_ident(expr),
        Expr::Paren(e) => root_ident(e),
        _ => None,
    }
}

fn expr_pos(e: &Expr) -> (usize, usize) {
    match e {
        Expr::Call { line, col, .. } | Expr::MethodCall { line, col, .. } => (*line, *col),
        _ => (0, 0),
    }
}

/// Identifiers bound by a pattern (Rust pattern syntax, syntactic approximation).
pub fn pattern_idents(pat: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_was_path = false;
    let chars: Vec<char> = pat.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphanumeric() || c == '_' {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                let next_non_space = chars[i..].iter().find(|c| !c.is_whitespace()).copied();
                let is_path_seg = next_non_space == Some(':') && chars[i..].iter().skip_while(|c| c.is_whitespace()).nth(1) == Some(&':');
                let is_ctor = next_non_space == Some('(') || next_non_space == Some('{');
                push_ident(&mut out, &cur, prev_was_path, is_path_seg || is_ctor);
                prev_was_path = is_path_seg;
                cur.clear();
            }
            if c == ':' && i + 1 < chars.len() && chars[i + 1] == ':' {
                i += 2;
                continue;
            }
            if c != ':' {
                prev_was_path = false;
            }
        }
        i += 1;
    }
    if !cur.is_empty() {
        push_ident(&mut out, &cur, prev_was_path, false);
    }
    out
}

fn push_ident(out: &mut Vec<String>, cur: &str, after_path: bool, is_path_or_ctor: bool) {
    if is_path_or_ctor || after_path {
        return;
    }
    if cur == "_" || cur == "ref" || cur == "mut" || cur == "true" || cur == "false" || cur == "None" || cur == "Some" || cur == "Ok" || cur == "Err" {
        return;
    }
    if cur.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return;
    }
    // Upper-case initial = a unit variant / constant, not a binding (Rust convention).
    if cur.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        return;
    }
    if !out.iter().any(|o| o == cur) {
        out.push(cur.to_string());
    }
}

fn expr_idents(e: &Expr) -> Vec<String> {
    match e {
        Expr::Path(p) if !p.contains("::") => vec![p.clone()],
        Expr::Tuple(items) => items.iter().flat_map(expr_idents).collect(),
        Expr::StructLit { fields, .. } => fields.iter().flat_map(|f| match &f.value { Some(v) => expr_idents(v), None => vec![f.name.clone()] }).collect(),
        Expr::Paren(e) => expr_idents(e),
        _ => vec![],
    }
}

fn variant_name(text: &str) -> String {
    text.trim().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect()
}

/// Type parameters with their bound text: `<T: AsRef<[u8]> + ?Sized, U>` + `where U: Copy`
/// -> [(T, "AsRef<[u8]> + ?Sized"), (U, "Copy")].
/// The type a `@copy use` declares known-Copy: the last path segment, with its type arguments if the
/// declaration carried them (`nalgebra::Vector3<f64>` → `Vector3<f64>`; `uuid::Uuid` → `Uuid`).
fn copy_use_type(path: &str) -> String {
    let p = strip_spaces(path);
    let p = p.as_str();
    let head = p.split('<').next().unwrap_or(p);
    let seg = head.rsplit("::").next().unwrap_or(head).trim();
    match p.find('<') {
        Some(i) => format!("{}{}", seg, p[i..].trim()),
        None => seg.to_string(),
    }
}

/// What a `@copy use` imports: the path without the type arguments, since a `use` cannot carry them.
fn copy_use_import(path: &str) -> String {
    let p = path.trim();
    match p.find('<') {
        Some(i) => p[..i].trim().to_string(),
        None => p.to_string(),
    }
}

/// A type's text without whitespace, so `Vector3 < f64 >` and `Vector3<f64>` compare equal.
fn strip_spaces(t: &str) -> String {
    t.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The last path segment of a type, keeping its arguments (`nalgebra::Vector3<f64>` → `Vector3<f64>`).
fn segment_with_args(t: &str) -> String {
    let t = strip_spaces(t);
    let t = t.as_str();
    let head = t.split('<').next().unwrap_or(t);
    let seg = head.rsplit("::").next().unwrap_or(head).trim();
    match t.find('<') {
        Some(i) => format!("{}{}", seg, t[i..].trim()),
        None => seg.to_string(),
    }
}

/// `@attr` written on a field of a tuple struct or an enum variant, where the field has no line of
/// its own to carry it (§23, D55): `Parse(@from io::Error)` becomes `Parse(#[from] io::Error)`. Only
/// an `@` that begins a field is rewritten — after `(`, `{` or `,` — so nothing inside a type is
/// touched, and `@rust(tokens)` forwards its tokens unchanged as everywhere else.
fn inline_field_attrs(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut field_start = false; // the next non-space character begins a field
    while i < c.len() {
        let ch = c[i];
        if ch == '"' {
            out.push(ch);
            i += 1;
            while i < c.len() {
                out.push(c[i]);
                if c[i] == '\\' && i + 1 < c.len() {
                    out.push(c[i + 1]);
                    i += 2;
                    continue;
                }
                if c[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            field_start = false;
            continue;
        }
        if ch == '@' && field_start {
            let mut j = i + 1;
            while j < c.len() && (c[j].is_alphanumeric() || c[j] == '_') {
                j += 1;
            }
            let name: String = c[i + 1..j].iter().collect();
            if name.is_empty() {
                out.push(ch);
                i += 1;
                continue;
            }
            let mut args = String::new();
            if c.get(j) == Some(&'(') {
                let mut depth = 0;
                let start = j;
                while j < c.len() {
                    if c[j] == '(' {
                        depth += 1;
                    } else if c[j] == ')' {
                        depth -= 1;
                        if depth == 0 {
                            j += 1;
                            break;
                        }
                    }
                    j += 1;
                }
                args = c[start + 1..j - 1].iter().collect();
            }
            if name == "rust" {
                out.push_str(&format!("#[{}]", args));
            } else if args.is_empty() {
                out.push_str(&format!("#[{}]", name));
            } else {
                out.push_str(&format!("#[{}({})]", name, args));
            }
            i = j;
            // several attributes may sit on one field
            continue;
        }
        if !ch.is_whitespace() {
            field_start = matches!(ch, '(' | '{' | ',');
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// The payload of a lowered `Option<…>` type, or `None` for anything else.
fn optional_payload(t: &str) -> Option<&str> {
    let t = t.trim();
    if last_segment(t) != "Option" {
        return None;
    }
    let open = t.find('<')?;
    let close = t.rfind('>')?;
    Some(&t[open + 1..close])
}

fn generic_bounds(g: Option<&str>, where_clause: Option<&str>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    if let Some(g) = g {
        let inner = &g[1..g.len() - 1];
        for p in split_top_level(inner, ',') {
            if p.starts_with('\'') || p.starts_with("const ") {
                continue;
            }
            let (n, b) = match p.split_once(':') {
                Some((n, b)) => (n.trim().to_string(), b.trim().to_string()),
                None => (p.trim().to_string(), String::new()),
            };
            out.push((n, b));
        }
    }
    if let Some(w) = where_clause {
        let w = w.trim().strip_prefix("where").unwrap_or(w).trim();
        for p in split_top_level(w, ',') {
            if let Some((n, b)) = p.split_once(':') {
                let n = n.trim();
                if let Some(e) = out.iter_mut().find(|(x, _)| x == n) {
                    if !e.1.is_empty() {
                        e.1.push_str(" + ");
                    }
                    e.1.push_str(b.trim());
                }
            }
        }
    }
    out
}

/// `self` plus extra type parameters, for computing a signature outside its lowering context.
struct GenericsView<'a> {
    inner: &'a Lowerer,
    type_params: Vec<(String, String)>,
}

impl<'a> GenericsView<'a> {
    fn param_lowering(&self, p: &Param) -> (String, ParamMode, bool) {
        // delegate with the extended parameter list
        let mut tmp = Lowerer::new("", &CrateIndex::default(), "");
        tmp.types = self.inner.types.clone();
        tmp.copy_types = self.inner.copy_types.clone();
        tmp.fn_stack = Vec::new();
        tmp.type_params = self.type_params.clone();
        tmp.param_lowering(p)
    }
}


fn generic_args(g: &str) -> String {
    // `<T: Clone, 'a>` -> `<T, 'a>`
    if g.is_empty() {
        return String::new();
    }
    let inner = &g[1..g.len() - 1];
    let parts: Vec<String> = split_top_level(inner, ',').iter().map(|p| p.split(':').next().unwrap().trim().to_string()).collect();
    format!("<{}>", parts.join(", "))
}

pub fn attr_text(a: &Attr) -> String {
    if a.name == "rust" {
        return format!("[{}]", a.args.as_deref().unwrap_or(""));
    }
    if let Some(v) = &a.value {
        // the name-value form (§23, D57): `@must_use = "…"` → `#[must_use = "…"]`
        return format!("[{} = {}]", a.name, v);
    }
    match &a.args {
        Some(args) => format!("[{}({})]", a.name, args),
        None => format!("[{}]", a.name),
    }
}

pub fn ident(name: &str) -> String {
    let n = name.trim();
    if n.starts_with("r#") {
        return n.to_string();
    }
    if RUST_KEYWORDS.contains(&n) && !matches!(n, "self" | "Self" | "super" | "crate") {
        return format!("r#{}", n);
    }
    n.to_string()
}

fn indent_text(s: &str, levels: usize) -> String {
    let pad = "    ".repeat(levels);
    let mut out = String::new();
    for l in s.lines() {
        if l.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&pad);
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

fn dedent_block(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let min = lines.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let mut out = String::new();
    let mut started = false;
    for l in &lines {
        if !started && l.trim().is_empty() {
            continue;
        }
        started = true;
        if l.trim().is_empty() {
            out.push('\n');
        } else {
            out.push_str(&l[min.min(l.len())..]);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}
