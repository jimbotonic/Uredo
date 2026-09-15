//! Abstract syntax of the Phase 0 / early Phase 1 surface (§33.1).
//!
//! Types, patterns, macro bodies and Rust blocks are kept as raw source text: Uredo never
//! interprets them beyond the syntactic classification the passing-mode table needs (§5.2).

#[derive(Debug, Clone)]
pub struct Attr {
    pub inner: bool,
    pub name: String,
    pub args: Option<String>,
    /// The name-value form `@name = value` (§23), the value as written.
    pub value: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Module {
    pub inner_docs: Vec<String>,
    pub inner_attrs: Vec<Attr>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub docs: Vec<String>,
    pub attrs: Vec<Attr>,
    pub vis: Option<String>,
    pub line: usize,
    pub blank_before: bool,
    pub kind: ItemKind,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    Use { path: String, copy: bool },
    Const { is_static: bool, name: String, ty: Type, expr: Expr },
    Fn(FnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Impl(ImplDecl),
    Rust(String),
    Macro { path: String, body: String },
    MacroRules { name: String, body: String },
    Mod { name: String, body: Option<Vec<Item>> },
    TypeAlias { name: String, generics: Option<String>, bounds: Option<String>, where_clause: Option<String>, ty: Option<Type> },
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub generics: Option<String>,
    pub receiver: Option<Receiver>,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub throws: Option<Option<String>>,
    pub where_clause: Option<String>,
    pub is_async: bool,
    pub is_unsafe: bool,
    pub body: Option<Block>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Receiver {
    Ref,      // self       -> &self
    RefMut,   // self: inout -> &mut self
    Value,    // self: take  -> self
    MutValue, // var self: take -> mut self
    /// `self: Pin<&mut Self>` and the other receiver types Rust allows (D59): written as-is,
    /// emitted as-is. Uredo models none of them, so what may be done through one is rustc's to say.
    Explicit(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Default,
    Take,
    Inout,
}

#[derive(Debug, Clone)]
pub struct Param {
    /// `name` or a pattern (`(x, y)`, `Point { x, y }`), raw text.
    pub pat: String,
    pub is_pattern: bool,
    pub is_var: bool,
    pub mode: Mode,
    pub ty: Type,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub generics: Option<String>,
    pub where_clause: Option<String>,
    pub kind: StructKind,
    /// Nested methods (sugar for an inherent impl) and nested trait impls, in source order.
    pub nested: Vec<Nested>,
}

#[derive(Debug, Clone)]
pub enum StructKind {
    Unit,
    Tuple(String),
    Named(Vec<Field>),
}

#[derive(Debug, Clone)]
pub struct Field {
    pub docs: Vec<String>,
    pub attrs: Vec<Attr>,
    pub vis: Option<String>,
    pub name: String,
    pub ty: Type,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Option<String>,
    pub where_clause: Option<String>,
    pub variants: Vec<Variant>,
    pub nested: Vec<Nested>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub docs: Vec<String>,
    pub attrs: Vec<Attr>,
    /// Raw text of the variant as written (`Cons(i32, Box<List>)`, `Move { x: i32 }`, `Nil`).
    pub text: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum Nested {
    Method(Item),
    TraitImpl { attrs: Vec<Attr>, trait_path: String, items: Vec<Item>, line: usize, blank_before: bool },
}

#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub generics: Option<String>,
    pub trait_path: Option<String>,
    pub self_ty: String,
    pub where_clause: Option<String>,
    pub items: Vec<Item>,
    pub is_unsafe: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Stmt {
    pub line: usize,
    pub blank_before: bool,
    pub kind: StmtKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindKind {
    /// `name = expr`: binds if unbound, assigns if bound.
    Auto,
    /// `let pat = expr`
    Let,
    /// `var pat = expr`
    Var,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Bind { kind: BindKind, target: Expr, ty: Option<Type>, init: Expr, else_block: Option<Block> },
    Assign { target: Expr, op: &'static str, value: Expr },
    Expr(Expr),
    Return(Option<Expr>),
    Throw(Expr),
    Break { label: Option<String>, value: Option<Expr> },
    Continue { label: Option<String> },
    While { label: Option<String>, cond: Expr, body: Block },
    WhileLet { label: Option<String>, pat: String, expr: Expr, body: Block },
    For { label: Option<String>, pat: String, mode: Mode, iter: Expr, body: Block },
    Item(Item),
    Unsafe(Block),
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    /// `if let PAT = EXPR` when `pat` is Some.
    pub pat: Option<String>,
    pub cond: Box<Expr>,
    pub then: Block,
    pub else_: Option<ElseBranch>,
    pub inline: bool,
}

#[derive(Debug, Clone)]
pub enum ElseBranch {
    ElseIf(Box<IfExpr>),
    Else(Block),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: String,
    pub guard: Option<Expr>,
    pub body: Block,
    pub inline: bool,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct ClosureParam {
    pub pat: String,
    pub ty: Option<Type>,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(String),
    Float(String),
    Str(String),
    Char(String),
    Bool(bool),
    /// Identifier or path (`x`, `Vec::from`, `std::time::Duration::from_secs`, `Self`), raw.
    Path(String),
    Unary { op: &'static str, expr: Box<Expr> },
    Ref { mutable: bool, expr: Box<Expr> },
    Binary { op: &'static str, lhs: Box<Expr>, rhs: Box<Expr> },
    Cast { expr: Box<Expr>, ty: Type },
    Call { callee: Box<Expr>, args: Vec<Expr>, line: usize, col: usize },
    MethodCall { recv: Box<Expr>, name: String, turbofish: Option<String>, args: Vec<Expr>, line: usize, col: usize },
    Field { expr: Box<Expr>, name: String },
    Index { expr: Box<Expr>, index: Box<Expr> },
    Try(Box<Expr>),
    Await(Box<Expr>),
    Paren(Box<Expr>),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    ArrayRepeat { value: Box<Expr>, count: Box<Expr> },
    StructLit { path: String, fields: Vec<StructField>, base: Option<Box<Expr>> },
    Range { lo: Option<Box<Expr>>, hi: Option<Box<Expr>>, inclusive: bool },
    Closure { is_move: bool, params: Vec<ClosureParam>, ret: Option<Type>, throws: Option<Option<String>>, body: Box<Expr> },
    If(Box<IfExpr>),
    Match { scrutinee: Box<Expr>, arms: Vec<MatchArm> },
    Loop { label: Option<String>, body: Block },
    Block(Block),
    /// `unsafe:` block in expression position.
    UnsafeBlock(Block),
    Rust(String),
    Macro { path: String, body: String },
    /// Assignment as an expression: only as a closure body (`() => calls += 1`).
    Assign { target: Box<Expr>, op: &'static str, value: Box<Expr> },
    /// A Uredo identifier `x` at a position where it was recognised as such (same as Path).
    Unit,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub attrs: Vec<Attr>,
    pub name: String,
    pub value: Option<Expr>,
}

impl Type {
    pub fn new(s: impl Into<String>) -> Type {
        Type { text: s.into() }
    }
}
