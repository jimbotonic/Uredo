//! Negative fixtures (§27, §34 gate item 4): programs Uredo itself must reject, with the
//! diagnostic anchored at the Uredo line.

fn errors(src: &str) -> Vec<(usize, String)> {
    let out = uredo::compile(src, false);
    out.diags.iter().filter(|d| d.level == uredo::diag::Level::Error).map(|d| (d.line, d.msg.clone())).collect()
}

fn assert_error(src: &str, line: usize, fragment: &str) {
    let errs = errors(src);
    assert!(
        errs.iter().any(|(l, m)| *l == line && m.contains(fragment)),
        "expected an error at line {} containing {:?}; got {:?}",
        line,
        fragment,
        errs
    );
}

fn assert_clean(src: &str) -> String {
    let out = uredo::compile(src, false);
    assert!(!out.has_errors(), "unexpected errors: {:?}", out.diags);
    out.rust
}

#[test]
fn assignment_to_immutable_binding() {
    assert_error("fn main():\n    x = 1\n    x = 2\n", 3, "`x` is immutable");
}

#[test]
fn immutable_argument_to_inout() {
    let src = "fn bump(n: inout i32):\n    *n += 1\nfn main():\n    v = 1\n    bump(v)\n";
    assert_error(src, 5, "cannot be passed to an `inout` parameter");
}

#[test]
fn inout_scalar_needs_deref() {
    assert_error("fn bump(n: inout i32):\n    n += 1\n", 2, "write `*n = …`");
}

#[test]
fn var_binding_satisfies_inout() {
    let src = "fn bump(n: inout i32):\n    *n += 1\nfn main():\n    var v = 1\n    bump(v)\n";
    let rust = assert_clean(src);
    assert!(rust.contains("bump(&mut v)"), "{}", rust);
}

#[test]
fn field_write_needs_self() {
    let src = "struct A:\n    n: i32\n\n    fn f(self: inout):\n        n += 1\n";
    assert_error(src, 5, "field writes are spelled `self.n");
}

#[test]
fn mutating_method_needs_inout_receiver() {
    let src = "struct A:\n    n: i32\n\n    fn f(self):\n        self.n += 1\n";
    assert_error(src, 5, "declare `self: inout`");
}

#[test]
fn tabs_are_rejected() {
    assert_error("fn main():\n\tx = 1\n", 2, "tabs");
}

#[test]
fn chained_comparison_is_rejected() {
    assert_error("fn main():\n    b = 1 < 2 < 3\n", 2, "chained comparisons");
}

#[test]
fn block_closure_after_a_binding() {
    // `=>` ending a line opens an indented block anywhere (D51, §17): here after `=`,
    // anchored at the statement, no closer
    let src = "fn main():\n    f = x =>\n        a = x + 1\n        a\n    print(\"{}\", f(1))\n";
    let rust = assert_clean(src);
    assert!(rust.contains("let f = |x| {"), "{}", rust);
    assert!(rust.contains("f(1)"), "{}", rust);
}

#[test]
fn else_after_a_closure_block_argument_is_an_error() {
    let src = "fn main():\n    v = f(x =>\n        x + 1\n    else:\n        2\n    )\n";
    assert_error(src, 4, "closing the block argument opened on line 2");
}

#[test]
fn inline_if_as_an_argument() {
    let src = "fn pick(s: &'static str) -> &'static str: s\nfn main():\n    c = true\n    print(pick(if c: \"a\" else: \"b\"))\n";
    let rust = assert_clean(src);
    assert!(rust.contains("pick(if c {") && rust.contains("} else {"), "{}", rust);
}

// ----- D51: trailing block arguments -----

#[test]
fn trailing_block_closure_argument() {
    let src = "fn main():\n    s = [1, 2].iter().fold(0, (acc, x) =>\n        y = x * 2\n        acc + y\n    )\n    print(\"{s}\")\n";
    let rust = assert_clean(src);
    assert!(rust.contains("fold(0, |acc, x| {"), "{}", rust);
    assert!(rust.contains("acc + y") && rust.contains("})"), "{}", rust);
}

#[test]
fn trailing_block_in_a_method_chain() {
    let src = "fn main():\n    v: Vec<i32> = (0..3)\n        .map(i =>\n            n = i * 2\n            n + 1\n        )\n        .collect()\n    print(\"{v:?}\")\n";
    let rust = assert_clean(src);
    assert!(rust.contains(".map(|i| {"), "{}", rust);
    assert!(rust.contains("})\n        .collect()") || rust.contains("})\n.collect()") || rust.contains("}).collect()"), "{}", rust);
}

#[test]
fn trailing_match_and_if_arguments() {
    let src = "fn pick(s: &'static str) -> &'static str: s\nfn main():\n    n = 2\n    a = pick(match n:\n        0: \"zero\"\n        _: \"many\"\n    )\n    b = pick(if n > 1:\n        \"big\"\n    else:\n        \"small\"\n    )\n    print(\"{a} {b}\")\n";
    let rust = assert_clean(src);
    assert!(rust.contains("pick(match n {"), "{}", rust);
    assert!(rust.contains("pick(if n > 1 {"), "{}", rust);
}

#[test]
fn block_argument_must_be_last_and_closed_at_the_anchor() {
    assert_error("fn main():\n    v = f(x =>\n        x + 1\n    , 2)\n", 4, "closing the block argument opened on line 2");
    assert_error("fn main():\n    v = f(x =>\n        x + 1\n        )\n", 4, "closing the block argument opened on line 2");
}


#[test]
fn closure_body_on_the_next_line_is_a_block() {
    // a one-expression body on its own line is a (one-line) block, not a continuation (D51)
    let rust = assert_clean("fn main():\n    f = x =>\n        x + 1\n    print(f(1))\n");
    assert!(rust.contains("let f = |x| {") && rust.contains("x + 1"), "{}", rust);
}

#[test]
fn inline_if_body_cannot_be_if() {
    assert_error("fn main():\n    if true: if false: x = 1\n", 2, "D49");
}

#[test]
fn throw_outside_throws() {
    assert_error("fn f():\n    throw 1\n", 2, "`throw` is only allowed");
}

#[test]
fn bare_throws_needs_default_error() {
    assert_error("fn f() throws:\n    return\n", 1, "@!default_error");
}

#[test]
fn mixed_pattern_assignment_is_rejected() {
    assert_error("fn main():\n    a = 1\n    (a, b) = (2, 3)\n", 3, "all-new names or assign all-bound");
}

#[test]
fn intrinsic_shadowing_is_rejected() {
    assert_error("fn main():\n    print = 1\n    print(2)\n", 3, "intrinsic");
}

#[test]
fn colon_less_header_followed_by_block() {
    assert_error("struct A\n    x: i32\n", 2, "expected `:`");
}

#[test]
fn unknown_character_in_a_type_does_not_hang() {
    // `str @a` is not (yet) Uredo syntax; the type lowerer must not loop on it
    let out = uredo::compile("fn choose(x: str @a, y: str @a) -> str @a:\n    if x.len() > y.len(): x else: y\n", false);
    let _ = out;
}


// ----- D50: configuration-invariant lowering -----

#[test]
fn cfg_alternates_with_equal_modes_are_fine() {
    let src = "@cfg(feature = \"a\")\nfn greet(s: str) -> String:\n    s.to_string()\n@cfg(not(feature = \"a\"))\nfn greet(s: str) -> String:\n    s.to_uppercase()\nfn main():\n    print(greet(\"x\"))\n";
    let rust = assert_clean(src);
    assert_eq!(rust.matches("#[cfg(").count(), 2, "{}", rust);
    assert!(rust.contains("greet(\"x\")"), "{}", rust);
}

#[test]
fn cfg_alternates_with_different_modes_are_rejected() {
    let src = "@cfg(feature = \"a\")\nfn take_it(j: take String):\n    drop(j)\n@cfg(not(feature = \"a\"))\nfn take_it(j: String):\n    print(j)\n";
    assert_error(src, 5, "configuration alternates with different passing modes");
}

#[test]
fn conditional_copy_is_rejected() {
    assert_error("@cfg_attr(feature = \"c\", derive(Clone, Copy))\nstruct P:\n    x: i32\n", 2, "may not be derived conditionally");
    let src = "@cfg(feature = \"c\")\n@derive(Clone, Copy)\nstruct P:\n    x: i32\n@cfg(not(feature = \"c\"))\nstruct P:\n    x: i32\n";
    assert_error(src, 6, "differ in `Copy`");
}

#[test]
fn conditional_copy_use_is_rejected() {
    assert_error("@cfg(unix)\n@copy use std::time::Duration\n", 2, "may not be conditional");
}

#[test]
fn copy_inside_a_conditional_module_is_rejected() {
    let src = "@cfg(unix)\nmod imp:\n    @derive(Clone, Copy)\n    pub struct Fd:\n        raw: i32\n";
    assert_error(src, 4, "inside a `@cfg`-guarded module");
}

#[test]
fn module_alternates_must_agree() {
    let ok = "@cfg(unix)\nmod imp:\n    pub fn open(p: str) -> i32:\n        1\n@cfg(windows)\nmod imp:\n    pub fn open(p: str) -> i32:\n        2\n";
    assert_clean(ok);
    let bad = "@cfg(unix)\nmod imp:\n    pub fn open(p: str) -> i32:\n        1\n@cfg(windows)\nmod imp:\n    pub fn open(p: take String) -> i32:\n        2\n";
    assert_error(bad, 6, "module alternate `imp` declares `open`");
}

// ----- D51 boundaries raised by the panel -----

#[test]
fn header_condition_with_a_completed_call_still_opens_a_block() {
    let src = "fn ok(a: i32, b: i32) -> bool: a < b\nfn pick(s: &'static str) -> &'static str: s\nfn main():\n    k = pick(if ok(1, 2):\n        \"yes\"\n    else:\n        \"no\"\n    )\n    print(k)\n";
    let rust = assert_clean(src);
    assert!(rust.contains("pick(if ok(1, 2) {"), "{}", rust);
}

#[test]
fn inner_delimiter_closer_on_a_body_line_is_not_the_owner_closer() {
    let src = "fn g(v: i32) -> i32: v\nfn f(h: i32) -> i32: h\nfn main():\n    v = f(g(\n        1\n    ))\n    w = [1].iter().map(x =>\n        g(\n            *x\n        )\n    ).count()\n    print(\"{v} {w}\")\n";
    let rust = assert_clean(src);
    assert!(rust.contains(".map(|x| {"), "{}", rust);
    assert!(rust.contains("g(*x)") || rust.contains("g(\n"), "{}", rust);
}

#[test]
fn grouped_block_closure_completes_its_own_parenthesis_only() {
    let src = "fn apply(f: impl Fn(i32) -> i32, n: i32) -> i32: f(n)\nfn main():\n    v = apply((x =>\n        x + 1\n    ), 41)\n    print(\"{v}\")\n";
    let rust = assert_clean(src);
    assert!(rust.contains("apply((|x| {") || rust.contains("apply(\n        (|x| {"), "{}", rust);
}

#[test]
fn nested_if_header_inside_a_closure_argument() {
    let src = "fn pick(s: &'static str) -> &'static str: s\nfn main():\n    c = true\n    v = [1].iter().map(x => if c:\n        pick(\"a\")\n    else:\n        pick(\"b\")\n    ).count()\n    print(\"{v}\")\n";
    let rust = assert_clean(src);
    assert!(rust.contains(".map(|x| if c {"), "{}", rust);
}

#[test]
fn closer_on_the_body_line_names_the_header() {
    assert_error("fn main():\n    v = [1].iter().map(x =>\n        x + 1)\n", 3, "opened on line 2");
}

#[test]
fn trailing_comma_before_the_closer_is_rejected() {
    assert_error("fn main():\n    v = [1].iter().map(x =>\n        x + 1,\n    )\n", 4, "opened on line 2");
}

#[test]
fn comment_and_blank_lines_inside_a_block_argument() {
    let src = "fn main():\n    v = [1, 2].iter().map(x =>\n# a comment at column 0\n\n        y = x + 1   # trailing comment\n\n        y\n    ).count()\n    print(\"{v}\")\n";
    let rust = assert_clean(src);
    assert!(rust.contains(".map(|x| {"), "{}", rust);
}

#[test]
fn closer_line_may_continue_the_header_of_an_if() {
    // the corpus's one non-trailing case: a block closure inside an `if` condition, `):`
    let src = "fn main():\n    xs = [1, 2, 3]\n    if xs.iter().any(x =>\n        y = *x * 2\n        y > 4\n    ):\n        print(\"big\")\n    else:\n        print(\"small\")\n";
    let rust = assert_clean(src);
    assert!(rust.contains("if xs.iter().any(|x| {"), "{}", rust);
    assert!(rust.contains("}) {"), "{}", rust);
}

#[test]
fn closer_line_with_nested_closers_and_try() {
    let src = "fn parse(s: str) -> i32 throws String:\n    n = s.len()\n    v = Some(vec![n]).map(xs =>\n        t = xs.len()\n        t + 1\n    ).ok_or(String::from(\"none\"))?\n    v as i32\n";
    let rust = assert_clean(src);
    assert!(rust.contains(".map(|xs| {"), "{}", rust);
    assert!(rust.contains("}).ok_or(String::from(\"none\"))?"), "{}", rust);
}

#[test]
fn closure_inside_brackets_keeps_the_continuation_reading() {
    // only `(` owns a trailing block (D51); inside `[` or `{` the unclosed delimiter still
    // suppresses newlines, so a `=>` at the line end continues a one-expression body
    let rust = assert_clean("fn main():\n    v = [x =>\n        x + 1\n    ]\n    print(\"{}\", v[0](1))\n");
    assert!(rust.contains("[|x| x + 1]"), "{}", rust);
}


// ----- D52: type parameters and `impl Trait` pass by value (P8) -----

#[test]
fn type_parameters_pass_by_value() {
    let src = "fn add_user<N: Into<String>>(name: N) -> String:\n    name.into()\nfn run(f: impl FnOnce() -> i32) -> i32:\n    f()\nfn main():\n    print(add_user(\"ada\"))\n    print(\"{}\", run(() => 1))\n";
    let rust = assert_clean(src);
    assert!(rust.contains("fn add_user<N: Into<String>>(name: N) -> String"), "{}", rust);
    assert!(rust.contains("fn run(f: impl FnOnce() -> i32) -> i32"), "{}", rust);
    assert!(rust.contains("add_user(\"ada\")") && rust.contains("run(|| 1)"), "{}", rust);
}

#[test]
fn unsized_bound_keeps_the_borrow_and_the_general_call_rule() {
    // `?Sized` -> `&T`; a literal is verbatim (D47) and instantiates `T = str`; a binding gets `&`
    let src = "fn byte_len<T: AsRef<[u8]> + ?Sized>(x: T) -> usize:\n    x.as_ref().len()\nfn main():\n    v = Vec::from([1u8])\n    print(\"{} {}\", byte_len(\"hi\"), byte_len(v))\n";
    let rust = assert_clean(src);
    assert!(rust.contains("fn byte_len<T: AsRef<[u8]> + ?Sized>(x: &T) -> usize"), "{}", rust);
    assert!(rust.contains("byte_len(\"hi\")") && rust.contains("byte_len(&v)"), "{}", rust);
}

#[test]
fn byte_string_to_a_generic_parameter_is_verbatim() {
    let src = "fn first<T: AsRef<[u8]>>(x: T) -> u8:\n    x.as_ref()[0]\nfn main():\n    print(\"{}\", first(b\"hi\"))\n";
    let rust = assert_clean(src);
    assert!(rust.contains("first(b\"hi\")"), "{}", rust);
}

#[test]
fn generic_method_and_trait_declaration_parameters() {
    let src = "struct Log:\n    lines: Vec<String>\n\n    fn push<S: Into<String>>(self: inout, s: S):\n        self.lines.push(s.into())\n\ntrait Visit:\n    fn visit<V>(self, v: V) -> usize\n\nfn main():\n    var log = Log { lines: Vec::new() }\n    log.push(\"a\")\n    print(\"{}\", log.lines.len())\n";
    let rust = assert_clean(src);
    assert!(rust.contains("fn push<S: Into<String>>(&mut self, s: S)"), "{}", rust);
    assert!(rust.contains("fn visit<V>(&self, v: V) -> usize;"), "{}", rust);
    assert!(rust.contains("log.push(\"a\")"), "{}", rust);
}

#[test]
fn take_on_a_type_parameter_is_accepted() {
    let rust = assert_clean("fn f<T>(x: take T) -> T:\n    x\nfn main():\n    print(\"{}\", f(1))\n");
    assert!(rust.contains("fn f<T>(x: T) -> T"), "{}", rust);
}

// ----- D53: std-trait parameter modes (G11) -----

#[test]
fn std_trait_methods_take_the_trait_modes() {
    let src = "@derive(Debug, Clone, Copy, PartialEq)\nstruct P:\n    x: i32\n\n    impl std::ops::Add:\n        type Output = P\n        fn add(self: take, rhs: P) -> P: P { x: x + rhs.x }\n\n    impl PartialOrd:\n        fn partial_cmp(self, other: P) -> Option<std::cmp::Ordering>: x.partial_cmp(&other.x)\n\n@derive(Debug)\nstruct E(String)\n\nimpl From<std::num::ParseIntError> for E:\n    fn from(e: std::num::ParseIntError) -> Self: E(e.to_string())\n\nimpl std::fmt::Display for E:\n    fn fmt(self, f: std::fmt::Formatter<'_>) -> std::fmt::Result: write!(f, \"{}\", self.0)\n";
    let rust = assert_clean(src);
    assert!(rust.contains("fn add(self, rhs: P) -> P"), "Add::add takes rhs by value even though P is Copy-by-value anyway\n{}", rust);
    assert!(rust.contains("fn partial_cmp(&self, other: &P) -> Option<std::cmp::Ordering>"), "PartialOrd takes &Rhs even on a Copy type\n{}", rust);
    assert!(rust.contains("fn from(e: std::num::ParseIntError) -> Self"), "From::from takes by value without `take`\n{}", rust);
    assert!(rust.contains("fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result"), "Display::fmt's formatter is &mut without `inout`\n{}", rust);
}

#[test]
fn non_std_trait_impls_keep_the_default_modes() {
    let src = "trait Greet:\n    fn greet(self, who: String) -> String\n\nstruct G\n\nimpl Greet for G:\n    fn greet(self, who: String) -> String: format(\"hi {who}\")\n";
    let rust = assert_clean(src);
    assert!(rust.contains("fn greet(&self, who: &String) -> String"), "{}", rust);
}

// ----- `throws` closures (§17) -----

#[test]
fn throws_closure_returns_a_result() {
    let src = "fn read(n: u32) -> u32 throws String:\n    if n == 0: throw String::from(\"empty\")\n    n * 2\n\nfn main():\n    handler = (req: u32) -> u32 throws String =>\n        v = read(req)?\n        v + 1\n    short = (n: u32) -> u32 throws String => read(n)?\n    print(\"{:?} {:?}\", handler(3), short(0))\n";
    let rust = assert_clean(src);
    assert!(rust.contains("|req: u32| -> ::core::result::Result<u32, String> {"), "{}", rust);
    assert!(rust.contains("::core::result::Result::Ok(v + 1)"), "the tail is wrapped (§15.3)\n{}", rust);
    assert!(rust.contains("match __uredo_owned(read(n))"), "a tail `?` uses the D37 lowering\n{}", rust);
}

#[test]
fn throws_closure_without_a_success_type_is_rejected() {
    assert_error("fn main():\n    f = (n: u32) throws String => n\n", 2, "needs its success type");
}

// ----- D54: an optional parameter follows its payload -----

#[test]
fn optional_with_a_known_copy_payload_passes_by_value() {
    let rust = assert_clean("fn a(x: u32?) -> u32:\n    x.unwrap_or(0)\nfn main():\n    print(\"{}\", a(Some(1)))\n");
    assert!(rust.contains("fn a(x: ::core::option::Option<u32>)"), "{}", rust);
    // and the call site passes it verbatim, not by reference
    assert!(rust.contains("a(Some(1))"), "{}", rust);
}

#[test]
fn optional_with_an_owned_payload_stays_borrowed() {
    let rust = assert_clean("fn b(s: String?) -> bool:\n    s.is_some()\nfn main():\n    print(\"{}\", b(None))\n");
    assert!(rust.contains("fn b(s: &::core::option::Option<String>)"), "{}", rust);
}

#[test]
fn optional_payload_copy_ness_is_recursive() {
    let src = "@derive(Copy, Clone)\nstruct Id:\n    n: u32\nfn c(i: Id?, p: (u32, u32)?, a: [u8; 4]?, n: u32??, u: ()?, r: Result<u32, u8>) -> bool:\n    i.is_some()\nfn main():\n    print(\"{}\", c(None, None, None, None, None, Ok(1)))\n";
    let rust = assert_clean(src);
    for param in ["i: ::core::option::Option<Id>", "p: ::core::option::Option<(u32, u32)>", "a: ::core::option::Option<[u8; 4]>", "u: ::core::option::Option<()>", "r: Result<u32, u8>"] {
        assert!(rust.contains(param), "{} missing from\n{}", param, rust);
    }
    assert!(rust.contains("n: ::core::option::Option<::core::option::Option<u32>>"), "{}", rust);
}

#[test]
fn a_copy_bounded_type_parameter_is_a_known_copy_payload() {
    let rust = assert_clean("fn q<T: Copy>(x: T?) -> bool:\n    x.is_some()\nfn r<T>(x: T?) -> bool:\n    x.is_some()\nfn main():\n    print(\"{}\", r::<u8>(None))\n");
    assert!(rust.contains("fn q<T: Copy>(x: ::core::option::Option<T>)"), "{}", rust);
    assert!(rust.contains("fn r<T>(x: &::core::option::Option<T>)"), "{}", rust);
}

#[test]
fn a_reference_payload_passes_by_value_and_needs_no_parentheses() {
    let rust = assert_clean("fn k(s: &String?) -> bool:\n    s.is_some()\nfn t(o: &(String?)) -> bool:\n    o.is_some()\nfn main():\n    print(\"{}\", k(None))\n");
    assert!(rust.contains("fn k(s: ::core::option::Option<&String>)"), "{}", rust);
    assert!(rust.contains("fn t(o: &::core::option::Option<String>)"), "{}", rust);
}

#[test]
fn a_mutable_reference_payload_is_rejected() {
    assert_error("fn m(r: &mut u32?) -> bool:\n    r.is_some()\n", 1, "cannot be mutated");
}

#[test]
fn take_and_inout_still_reach_both_shapes() {
    let rust = assert_clean("fn m(r: take &mut u32?) -> bool:\n    r.is_some()\nfn n(r: inout u32?) -> bool:\n    r.is_some()\nfn main():\n    print(\"{}\", n(&mut None))\n");
    assert!(rust.contains("fn m(r: ::core::option::Option<&mut u32>)"), "{}", rust);
    assert!(rust.contains("fn n(r: &mut ::core::option::Option<u32>)"), "{}", rust);
}

#[test]
fn a_tuple_and_a_trait_object_keep_their_parentheses() {
    let rust = assert_clean("fn u(p: (u32, u32), q: (u32,)) -> u32:\n    p.0\nfn main():\n    print(\"{}\", u((1, 2), (3,)))\n");
    assert!(rust.contains("p: (u32, u32)") && rust.contains("q: (u32,)"), "{}", rust);
}

// ----- Phase 3 type-level forms (§12.3 D59, §12.4) -----

#[test]
fn an_associated_type_may_carry_bounds_and_a_where_clause() {
    let rust = assert_clean("use std::fmt::Debug\ntrait W:\n    type View<'a>: Debug where Self: 'a\n    fn view(self) -> Self::View<'_>\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("type View<'a>: Debug"), "{}", rust);
    assert!(rust.contains("Self: 'a"), "{}", rust);
    // and on a definition, where rustc also allows it after the `=`
    let rust = assert_clean("struct B:\n    b: Vec<u8>\nimpl W for B:\n    type View<'a> = &'a [u8] where Self: 'a\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("type View<'a> = &'a [u8]") && rust.contains("Self: 'a"), "{}", rust);
}

#[test]
fn a_receiver_may_name_its_own_type() {
    let rust = assert_clean("use std::pin::Pin\nimpl C:\n    fn poll(self: Pin<&mut Self>) -> u32:\n        self.left\n    fn shared(self: Rc<Self>) -> u32:\n        1\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("fn poll(self: Pin<&mut Self>)"), "{}", rust);
    assert!(rust.contains("fn shared(self: Rc<Self>)"), "{}", rust);
}

#[test]
fn a_written_receiver_type_leaves_mutability_to_rustc() {
    // Uredo models no such receiver, so it makes no claim about writes through it (§5.2)
    let rust = assert_clean("use std::pin::Pin\nimpl C:\n    fn tick(self: Pin<&mut Self>):\n        self.left -= 1\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("self.left -= 1"), "{}", rust);
}

#[test]
fn a_receiver_with_nothing_after_the_colon_is_still_rejected() {
    assert_error("impl C:\n    fn f(self: ):\n        1\n", 2, "expected `inout`, `take`, or a receiver type");
}

#[test]
fn a_trait_may_declare_supertraits() {
    // the colon that opens the body is the last one on the line (D60)
    let rust = assert_clean("trait Ord2: PartialOrd + Eq:\n    fn rank(self) -> u32\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("trait Ord2: PartialOrd + Eq {"), "{}", rust);
    let rust = assert_clean("trait Big: Send + Sync + 'static:\n    fn id(self) -> u32\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("trait Big: Send + Sync + 'static {"), "{}", rust);
    // and a declaration with no body keeps its bound
    let rust = assert_clean("trait Marker: Send\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("trait Marker: Send {}"), "{}", rust);
    // a trait with neither is unchanged
    let rust = assert_clean("trait Shape:\n    fn area(self) -> f64\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("trait Shape {"), "{}", rust);
}

#[test]
fn the_rust_prefix_on_an_import_is_documentation() {
    // `use rust::path` reads "this is a Rust item" and means `use path` (§22.1)
    let rust = assert_clean("use rust::std::fmt::Debug\nuse std::collections::HashMap\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("use std::fmt::Debug;"), "{}", rust);
    assert!(!rust.contains("rust::"), "the prefix must not reach the generated Rust:\n{}", rust);
    // it composes with `@copy use`, whose assertion names the real type
    let rust = assert_clean("@copy use rust::std::time::Duration\nfn f(d: Duration) -> u64:\n    d.as_secs()\nfn main():\n    print(\"{}\", f(Duration::from_secs(2)))\n");
    assert!(rust.contains("use std::time::Duration;") && rust.contains("__uredo_assert_copy::<Duration>"), "{}", rust);
    // `f(d)` takes it by value, so the prefix did not disturb the Copy table either
    assert!(rust.contains("fn f(d: Duration)"), "{}", rust);
    // a `rust` segment that is not the prefix is untouched
    let rust = assert_clean("use crate::rust::helpers\nfn main():\n    print(\"x\")\n");
    assert!(rust.contains("use crate::rust::helpers;"), "{}", rust);
}

#[test]
fn a_passing_mode_written_before_the_parameter_name() {
    // `take name: T` parses as a pattern parameter, so before this check the mode word reached
    // rustc inside the generated pattern and the failure was reported as a lowering defect.
    assert_error("fn f(take name: String) -> usize:\n    name.len()\n", 1, "the passing mode goes on the type");
    assert_error("fn f(inout n: i32):\n    *n += 1\n", 1, "write `n: inout T`");
}

#[test]
fn an_optional_of_str_or_a_slice_is_borrowed_like_the_bare_form() {
    // §9.7 borrows a `str`/`[T]` parameter and return type; the optional suffix composes with
    // that rule, since no value can have the unsized `Option<str>` the literal reading gives.
    let rust = assert_clean("fn f(x: str?, y: [u8]?) -> str?:\n    x\n");
    assert!(rust.contains("x: ::core::option::Option<&str>"), "{}", rust);
    assert!(rust.contains("y: ::core::option::Option<&[u8]>"), "{}", rust);
    assert!(rust.contains("-> ::core::option::Option<&str>"), "{}", rust);
    // an array is sized: it keeps its own form, and a holder that wants the unsized type keeps it
    let rust = assert_clean("fn g(x: [u8; 4]?) -> Box<str>:\n    Box::from(\"x\")\n");
    assert!(rust.contains("x: ::core::option::Option<[u8; 4]>"), "{}", rust);
    assert!(rust.contains("-> Box<str>"), "{}", rust);
}

#[test]
fn every_escape_form_of_a_char_literal_lexes() {
    // `'\''` ended one character early — the escaped quote was taken for the closing one — and the
    // stray quote then lexed as a lifetime. Found by porting a tokenizer to Uredo.
    let rust = assert_clean("fn main():\n    q = '\\''\n    n = '\\n'\n    b = '\\\\'\n    u = '\\u{1F600}'\n    x = '\\x41'\n    print(\"{q}{n}{b}{u}{x}\")\n");
    for lit in ["'\\''", "'\\n'", "'\\\\'", "'\\u{1F600}'", "'\\x41'"] {
        assert!(rust.contains(lit), "{} missing from {}", lit, rust);
    }
    // a lifetime is still a lifetime, and an unterminated char literal is still an error
    assert!(assert_clean("fn first<'a>(xs: &'a [u8]) -> &'a u8:\n    &xs[0]\n").contains("<'a>"));
}

#[test]
fn the_four_keywords_with_no_raw_form_cannot_name_a_binding() {
    // §6.1: every other Rust keyword is emitted as `r#name`; these four cannot be, so a binding of
    // that name reached rustc bare and meant something else. Found by writing `crate = …` in Uredo.
    assert_error("fn f(crate: u8) -> u8:\n    1\n", 1, "`crate` cannot name a binding");
    assert_error("fn main():\n    crate = 1\n    print(\"{crate}\")\n", 2, "`crate` cannot name a binding");
    assert_error("fn main():\n    super = 1\n    print(\"{super}\")\n", 2, "`super` cannot name a binding");
    // every other keyword still escapes, as D11 says
    let rust = assert_clean("fn main():\n    box = 1\n    ref = box + 1\n    print(\"{ref}\")\n");
    assert!(rust.contains("r#box"), "{}", rust);
    assert!(rust.contains("r#ref"), "{}", rust);
}

#[test]
fn a_rust_keyword_names_a_field_as_a_raw_identifier() {
    // D11 escapes a keyword used as an identifier. The declaration, the struct literal and the
    // access must all agree; only the access did, so `gen` — reserved in Rust 2024 — reached rustc
    // bare in the other two. Found by porting a benchmark whose data had a `gen` side.
    let rust = assert_clean("@derive(Debug)\nstruct Pair:\n    gen: u32\n    ref: u32\n\nfn main():\n    p = Pair { gen: 1, ref: 2 }\n    print(\"{} {} {:?}\", p.gen, p.ref, p)\n");
    for spelling in ["r#gen: u32", "r#ref: u32", "Pair { r#gen: 1, r#ref: 2 }", "p.r#gen", "p.r#ref"] {
        assert!(rust.contains(spelling), "{} missing from {}", spelling, rust);
    }
    // a punned field keeps the pun, escaped on both halves
    let rust = assert_clean("struct S:\n    gen: u32\n\nfn make(gen: u32) -> S:\n    S { gen }\n\nfn main():\n    print(\"{}\", make(1).gen)\n");
    assert!(rust.contains("S { r#gen }"), "{}", rust);
}

#[test]
fn a_diagnostic_on_an_import_names_the_import_it_is_about() {
    // rustfmt sorts `use` declarations; the provenance map's token walk cannot follow a
    // permutation, so every import in a reordered run pointed at the wrong line of Uredo source.
    let src = "use std::collections::BTreeMap\nuse std::fs\nuse regex::Regex\n\nfn main():\n    var m: BTreeMap<u8, u8> = BTreeMap::new()\n    m.insert(1, 2)\n    print(\"{} {}\", m.len(), fs::read_to_string(\"/nonexistent\").is_err())\n";
    let out = uredo::compile(src, true);
    assert!(!out.has_errors(), "{:?}", out.diags);
    // rustfmt puts `regex` first; each generated import must still name the line it came from
    for (import, ure_line) in [("use std::collections::BTreeMap;", 1), ("use std::fs;", 2), ("use regex::Regex;", 3)] {
        let at = out.rust.lines().position(|l| l.trim() == import).expect(import);
        assert_eq!(out.map.lines.get(at).copied(), Some(ure_line), "{} maps to the wrong line: {:?}", import, out.map.lines.get(at));
    }
}
