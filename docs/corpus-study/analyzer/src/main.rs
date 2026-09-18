use std::collections::{BTreeMap, HashSet};
use syn::visit::{self, Visit};
use syn::*;
use quote::ToTokens;

type Counts = BTreeMap<String, u64>;

struct Collector<'a> { c: Counts, local_fns: &'a HashSet<String>, copy_types: &'a HashSet<String>, all_types: &'a HashSet<String>, scopes: Vec<HashSet<String>> }
impl<'a> Collector<'a> {
    fn inc(&mut self, k: &str) { *self.c.entry(k.to_string()).or_insert(0) += 1; }
    fn add(&mut self, k: &str, n: u64) { *self.c.entry(k.to_string()).or_insert(0) += n; }
    fn declare(&mut self, name: &str) {
        let in_same = self.scopes.last().map(|s| s.contains(name)).unwrap_or(false);
        let in_outer = self.scopes.iter().rev().skip(1).any(|s| s.contains(name));
        if in_same { self.inc("let.shadow_same_block"); } else if in_outer { self.inc("let.shadow_outer_scope"); }
        if let Some(s) = self.scopes.last_mut() { s.insert(name.to_string()); }
    }
    fn classify_type(&mut self, ty: &Type, prefix: &str) {
        match ty {
            Type::Reference(r) => {
                let inner = &*r.elem;
                let key = if r.mutability.is_some() { "ref_mut" } else {
                    match inner {
                        Type::Path(p) if p.path.is_ident("str") => "ref_str",
                        Type::Slice(_) => "ref_slice",
                        Type::Path(p) => { let id = p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
                            if id == "String" || id == "Vec" || id == "PathBuf" { "ref_owned_container" } else { "ref_shared" } }
                        Type::TraitObject(_) => "ref_dyn",
                        _ => "ref_shared",
                    } };
                self.inc(&format!("{prefix}.{key}"));
            }
            Type::ImplTrait(_) => self.inc(&format!("{prefix}.impl_trait")),
            Type::Path(p) => {
                let id = p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
                let scalar = ["bool","char","i8","i16","i32","i64","i128","isize","u8","u16","u32","u64","u128","usize","f32","f64"];
                if scalar.contains(&id.as_str()) { self.inc(&format!("{prefix}.owned_scalar")); }
                else if p.path.segments.len()==1 && p.path.segments[0].ident.to_string().len()==1 { self.inc(&format!("{prefix}.owned_typeparam")); }
                else {
                    self.inc(&format!("{prefix}.owned_other")); self.inc(&format!("{prefix}.owned_type.{id}"));
                    let std_copy = ["Duration","Instant","Ordering","SocketAddr","Ipv4Addr","Ipv6Addr","TypeId","NonZeroU32","NonZeroUsize","NonZeroU64","Wrapping","Range","RangeInclusive","Self"];
                    let k = if id=="Option" { "owned_option" } else if id=="Box"||id=="Vec"||id=="String"||id=="PathBuf"||id=="HashMap"||id=="Arc"||id=="Rc" { "owned_std_noncopy" }
                        else if self.copy_types.contains(&id) { "owned_local_copy" } else if self.all_types.contains(&id) { "owned_local_noncopy" }
                        else if std_copy.contains(&id.as_str()) { "owned_std_copy" } else { "owned_foreign_unknown" };
                    self.inc(&format!("{prefix}.class.{k}"));
                }
            }
            Type::Tuple(t) if t.elems.is_empty() => self.inc(&format!("{prefix}.unit")),
            Type::Tuple(_) => self.inc(&format!("{prefix}.owned_tuple")),
            Type::Array(_) => self.inc(&format!("{prefix}.owned_array")),
            _ => self.inc(&format!("{prefix}.other")),
        }
    }
    fn sig(&mut self, sig: &Signature, kind: &str) {
        self.inc("fn.total"); self.inc(&format!("fn.kind.{kind}"));
        if sig.asyncness.is_some() { self.inc("fn.async"); }
        if sig.unsafety.is_some() { self.inc("fn.unsafe"); }
        if !sig.generics.params.is_empty() { self.inc("fn.generic");
            if sig.generics.params.iter().any(|p| matches!(p, GenericParam::Lifetime(_))) { self.inc("fn.explicit_lifetime"); } }
        if sig.generics.where_clause.is_some() { self.inc("fn.where_clause"); }
        let mut has_self=false;
        for a in &sig.inputs { match a {
            FnArg::Receiver(r) => { has_self=true; let k = if r.reference.is_some() { if r.mutability.is_some() {"ref_mut_self"} else {"ref_self"} } else if r.mutability.is_some() {"mut_self"} else {"self"}; self.inc(&format!("fn.recv.{k}")); }
            FnArg::Typed(t) => { self.inc("param.total"); self.classify_type(&t.ty, "param"); if let Pat::Ident(pi)=&*t.pat { if pi.mutability.is_some() { self.inc("param.mut_binding"); } } else { self.inc("param.pattern"); } }
        } }
        if !has_self && kind != "free" { self.inc("fn.recv.none_assoc"); }
        match &sig.output { ReturnType::Default => self.inc("ret.unit"), ReturnType::Type(_, ty) => {
            self.inc("ret.total");
            if let Type::Path(p) = &**ty { let id = p.path.segments.last().unwrap().ident.to_string();
                if id=="Result" { self.inc("ret.result"); if let PathArguments::AngleBracketed(a)=&p.path.segments.last().unwrap().arguments { if let Some(GenericArgument::Type(Type::Tuple(t)))=a.args.first() { if t.elems.is_empty() { self.inc("ret.result_unit"); } } } }
                else if id=="Option" { self.inc("ret.option"); } }
            if let Type::Reference(_) = &**ty { self.inc("ret.reference"); }
            if let Type::ImplTrait(_) = &**ty { self.inc("ret.impl_trait"); }
        } }
    }
    fn callee_kind(&self, e: &Expr) -> &'static str {
        if let Expr::Path(p) = e { let segs=&p.path.segments; let last=segs.last().unwrap().ident.to_string();
            if last=="Ok"||last=="Err"||last=="Some" { return "ctor"; }
            if segs.len()==1 { if self.local_fns.contains(&last) { return "local_fn" } else { return "single_unknown" } }
            if last.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) { return "tuple_ctor_or_variant" }
            let first = segs[0].ident.to_string();
            if first=="self"||first=="crate"||first=="super" { if self.local_fns.contains(&last) { return "local_fn_path" } }
            "path_call"
        } else { "expr_callee" }
    }
}

impl<'a, 'ast> Visit<'ast> for Collector<'a> {
    fn visit_item_fn(&mut self, i: &'ast ItemFn) { self.sig(&i.sig, "free"); self.scopes.push(HashSet::new()); for a in &i.sig.inputs { if let FnArg::Typed(t)=a { if let Pat::Ident(pi)=&*t.pat { let n=pi.ident.to_string(); self.scopes.last_mut().unwrap().insert(n);} } } visit::visit_item_fn(self, i); self.scopes.pop(); }
    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) { self.sig(&i.sig, "method"); self.scopes.push(HashSet::new()); for a in &i.sig.inputs { if let FnArg::Typed(t)=a { if let Pat::Ident(pi)=&*t.pat { let n=pi.ident.to_string(); self.scopes.last_mut().unwrap().insert(n);} } } visit::visit_impl_item_fn(self, i); self.scopes.pop(); }
    fn visit_trait_item_fn(&mut self, i: &'ast TraitItemFn) { self.sig(&i.sig, "trait"); visit::visit_trait_item_fn(self, i); }
    fn visit_block(&mut self, b: &'ast Block) { self.inc("block.total"); self.scopes.push(HashSet::new()); visit::visit_block(self, b); self.scopes.pop(); }
    fn visit_local(&mut self, l: &'ast Local) {
        self.inc("let.total");
        let (pat, typed) = match &l.pat { Pat::Type(pt) => (&*pt.pat, true), p => (p, false) };
        if typed { self.inc("let.typed"); }
        if let Pat::Ident(pi) = pat { if pi.mutability.is_some() { self.inc("let.mut"); } self.declare(&pi.ident.to_string()); } else { self.inc("let.pattern"); }
        if let Some(init)=&l.init { if init.diverge.is_some() { self.inc("let.let_else"); } } else { self.inc("let.uninit"); }
        visit::visit_local(self, l);
    }
    fn visit_expr_try(&mut self, e: &'ast ExprTry) { self.inc("try.total"); if let Expr::Await(_) = &*e.expr { self.inc("try.after_await"); } if let Expr::MethodCall(_) = &*e.expr { self.inc("try.on_method_call"); } visit::visit_expr_try(self, e); }
    fn visit_expr_method_call(&mut self, e: &'ast ExprMethodCall) {
        self.inc("mcall.total");
        if e.turbofish.is_some() { self.inc("mcall.turbofish"); }
        match &*e.receiver { Expr::Try(_) => self.inc("try.in_chain_as_receiver"), Expr::Await(a) => { if let Expr::Try(_)=&*a.base { self.inc("try.in_chain_as_receiver"); } } _ => {} }
        let m = e.method.to_string();
        for name in ["clone","to_string","to_owned","as_ref","as_str","as_mut","iter","iter_mut","into_iter","collect","unwrap","expect","map","and_then","unwrap_or","ok_or","take","borrow","borrow_mut","lock","into","try_into","parse","get","push","insert","len","is_empty","cloned","copied","as_deref","ok","await"] { if m==name { self.inc(&format!("mcall.name.{m}")); } }
        for a in &e.args { if let Expr::Reference(r)=a { self.inc(if r.mutability.is_some() {"arg.ref_mut.method"} else {"arg.ref.method"}); } }
        visit::visit_expr_method_call(self, e);
    }
    fn visit_expr_field(&mut self, e: &'ast ExprField) { if let Expr::Try(_) = &*e.base { self.inc("try.in_chain_as_field_base"); } visit::visit_expr_field(self, e); }
    fn visit_expr_await(&mut self, e: &'ast ExprAwait) { self.inc("await.total"); if let Expr::Try(_) = &*e.base { self.inc("try.before_await"); } visit::visit_expr_await(self, e); }
    fn visit_expr_call(&mut self, e: &'ast ExprCall) {
        self.inc("call.total");
        let kind = self.callee_kind(&e.func); self.inc(&format!("call.kind.{kind}"));
        if let Expr::Path(p) = &*e.func { let last = p.path.segments.last().unwrap().ident.to_string();
            if last=="Ok" { self.inc("ok.total"); if let Some(Expr::Tuple(t))=e.args.first() { if t.elems.is_empty() { self.inc("ok.unit"); } } }
            if last=="Err" { self.inc("err.total"); }
            if last=="Some" { self.inc("some.total"); }
            if p.path.segments.len()>=2 { let seg=p.path.segments.last().unwrap(); if let PathArguments::AngleBracketed(_)=seg.arguments { self.inc("call.turbofish"); } let prev=&p.path.segments[p.path.segments.len()-2]; if let PathArguments::AngleBracketed(_)=prev.arguments { self.inc("call.turbofish"); } }
        }
        for a in &e.args { if let Expr::Reference(r)=a { let k = if r.mutability.is_some() {"ref_mut"} else {"ref"}; self.inc(&format!("arg.{k}.call.{kind}")); } }
        visit::visit_expr_call(self, e);
    }
    fn visit_expr_closure(&mut self, e: &'ast ExprClosure) { self.inc("closure.total"); if e.capture.is_some() { self.inc("closure.move"); } self.add("closure.params", e.inputs.len() as u64); if let Expr::Block(_) = &*e.body { self.inc("closure.block_body"); } if e.inputs.iter().any(|p| matches!(p, Pat::Type(_))) { self.inc("closure.typed_param"); } visit::visit_expr_closure(self, e); }
    fn visit_expr_match(&mut self, e: &'ast ExprMatch) { self.inc("match.total"); self.add("match.arms", e.arms.len() as u64); for a in &e.arms { if let Expr::Block(_)=&*a.body { self.inc("match.arm_block"); } if a.guard.is_some() { self.inc("match.guard"); } } visit::visit_expr_match(self, e); }
    fn visit_expr_if(&mut self, e: &'ast ExprIf) { self.inc("if.total"); if let Expr::Let(_)=&*e.cond { self.inc("if.if_let"); } visit::visit_expr_if(self, e); }
    fn visit_expr_while(&mut self, e: &'ast ExprWhile) { self.inc("while.total"); if let Expr::Let(_)=&*e.cond { self.inc("while.while_let"); } visit::visit_expr_while(self, e); }
    fn visit_expr_loop(&mut self, e: &'ast ExprLoop) { self.inc("loop.total"); visit::visit_expr_loop(self, e); }
    fn visit_expr_for_loop(&mut self, e: &'ast ExprForLoop) {
        self.inc("for.total");
        let k = match &*e.expr { Expr::Reference(r) => if r.mutability.is_some() {"ref_mut"} else {"ref"}, Expr::Path(_) => "place_path", Expr::Field(_) => "place_field", Expr::Range(_) => "range", Expr::MethodCall(m) => { let n=m.method.to_string(); if n=="iter" {"iter()"} else if n=="iter_mut" {"iter_mut()"} else if n=="into_iter" {"into_iter()"} else {"method_chain"} }, Expr::Call(_) => "call", _ => "other" };
        self.inc(&format!("for.src.{k}")); visit::visit_expr_for_loop(self, e);
    }
    fn visit_expr_cast(&mut self, e: &'ast ExprCast) { self.inc("cast.total"); visit::visit_expr_cast(self, e); }
    fn visit_expr_struct(&mut self, e: &'ast ExprStruct) { self.inc("structlit.total"); if e.rest.is_some() { self.inc("structlit.with_rest"); } self.add("structlit.fields", e.fields.len() as u64); self.add("structlit.shorthand_fields", e.fields.iter().filter(|f| f.colon_token.is_none()).count() as u64); visit::visit_expr_struct(self, e); }
    fn visit_expr_reference(&mut self, e: &'ast ExprReference) { self.inc(if e.mutability.is_some() {"refexpr.mut"} else {"refexpr.shared"}); visit::visit_expr_reference(self, e); }
    fn visit_expr_return(&mut self, e: &'ast ExprReturn) { self.inc("return.total"); if let Some(Expr::Call(c))=e.expr.as_deref() { if let Expr::Path(p)=&*c.func { let l=p.path.segments.last().unwrap().ident.to_string(); if l=="Ok" {self.inc("return.ok");} if l=="Err" {self.inc("return.err");} } } visit::visit_expr_return(self, e); }
    fn visit_expr_unsafe(&mut self, e: &'ast ExprUnsafe) { self.inc("unsafe.block"); visit::visit_expr_unsafe(self, e); }
    fn visit_expr_index(&mut self, e: &'ast ExprIndex) { self.inc("index.total"); visit::visit_expr_index(self, e); }
    fn visit_macro(&mut self, m: &'ast Macro) { let n=m.path.segments.last().unwrap().ident.to_string(); self.inc("macro.total"); self.inc(&format!("macro.name.{n}")); visit::visit_macro(self, m); }
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) { self.inc(if i.trait_.is_some() {"impl.trait"} else {"impl.inherent"}); visit::visit_item_impl(self, i); }
    fn visit_item_trait(&mut self, i: &'ast ItemTrait) { self.inc("item.trait"); visit::visit_item_trait(self, i); }
    fn visit_item_struct(&mut self, i: &'ast ItemStruct) { self.inc("item.struct"); if !i.generics.params.is_empty() { self.inc("item.struct_generic"); } if i.generics.params.iter().any(|p| matches!(p, GenericParam::Lifetime(_))) { self.inc("item.struct_lifetime"); } visit::visit_item_struct(self, i); }
    fn visit_item_enum(&mut self, i: &'ast ItemEnum) { self.inc("item.enum"); for v in &i.variants { match &v.fields { Fields::Unit=>self.inc("enum.variant.unit"), Fields::Unnamed(_)=>self.inc("enum.variant.tuple"), Fields::Named(_)=>self.inc("enum.variant.struct") } } visit::visit_item_enum(self, i); }
    fn visit_item_mod(&mut self, i: &'ast ItemMod) { self.inc(if i.content.is_some() {"mod.inline"} else {"mod.decl"}); visit::visit_item_mod(self, i); }
    fn visit_type_trait_object(&mut self, t: &'ast TypeTraitObject) { self.inc("type.dyn"); visit::visit_type_trait_object(self, t); }
    fn visit_type_path(&mut self, t: &'ast TypePath) { let id=t.path.segments.last().unwrap().ident.to_string(); for n in ["Box","Arc","Rc","Mutex","RwLock","RefCell","Cell","Option","Result","Vec","HashMap","String","Cow","PhantomData"] { if id==n { self.inc(&format!("type.name.{n}")); } } visit::visit_type_path(self, t); }
    fn visit_pat_struct(&mut self, p: &'ast PatStruct) { self.inc("pat.struct"); visit::visit_pat_struct(self, p); }
    fn visit_pat_tuple_struct(&mut self, p: &'ast PatTupleStruct) { self.inc("pat.tuple_struct"); visit::visit_pat_tuple_struct(self, p); }
    fn visit_pat_reference(&mut self, p: &'ast PatReference) { self.inc("pat.reference"); visit::visit_pat_reference(self, p); }
    fn visit_pat_ident(&mut self, p: &'ast PatIdent) { if p.by_ref.is_some() { self.inc("pat.ref_binding"); } visit::visit_pat_ident(self, p); }
}

struct FnNames { names: HashSet<String>, copy_types: HashSet<String>, all_types: HashSet<String> }
fn derives_copy(attrs: &[Attribute]) -> bool { attrs.iter().any(|a| a.path().is_ident("derive") && a.meta.to_token_stream().to_string().contains("Copy")) }
impl<'ast> Visit<'ast> for FnNames {
    fn visit_item_fn(&mut self, i: &'ast ItemFn) { self.names.insert(i.sig.ident.to_string()); visit::visit_item_fn(self, i); }
    fn visit_item_struct(&mut self, i: &'ast ItemStruct) { self.all_types.insert(i.ident.to_string()); if derives_copy(&i.attrs) { self.copy_types.insert(i.ident.to_string()); } visit::visit_item_struct(self, i); }
    fn visit_item_enum(&mut self, i: &'ast ItemEnum) { self.all_types.insert(i.ident.to_string()); if derives_copy(&i.attrs) { self.copy_types.insert(i.ident.to_string()); } visit::visit_item_enum(self, i); }
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) { if let Some((_, path, _)) = &i.trait_ { if path.is_ident("Copy") { if let Type::Path(tp) = &*i.self_ty { self.copy_types.insert(tp.path.segments.last().unwrap().ident.to_string()); } } } visit::visit_item_impl(self, i); }
}

fn token_stats(ts: &proc_macro2::TokenStream, c: &mut Counts) {
    use proc_macro2::{TokenTree, Delimiter};
    let mut prev_colon=false;
    for tt in ts.clone() { match tt {
        TokenTree::Group(g) => { *c.entry("tok.total".into()).or_insert(0)+=2; match g.delimiter() { Delimiter::Brace=>*c.entry("tok.brace_pairs".into()).or_insert(0)+=1, Delimiter::Parenthesis=>*c.entry("tok.paren_pairs".into()).or_insert(0)+=1, Delimiter::Bracket=>*c.entry("tok.bracket_pairs".into()).or_insert(0)+=1, _=>{} } token_stats(&g.stream(), c); prev_colon=false; }
        TokenTree::Punct(p) => { *c.entry("tok.total".into()).or_insert(0)+=1; let ch=p.as_char(); let k=match ch {';'=>"semi",'&'=>"amp",'?'=>"question",'!'=>"bang",'|'=>"pipe",','=>"comma",'>'=>"gt",'<'=>"lt",'='=>"eq",_=>""}; if !k.is_empty() { *c.entry(format!("tok.punct.{k}")).or_insert(0)+=1; } if ch==':' { if prev_colon { *c.entry("tok.punct.pathsep".into()).or_insert(0)+=1; prev_colon=false; } else { prev_colon=true; } } else { prev_colon=false; } }
        _ => { *c.entry("tok.total".into()).or_insert(0)+=1; prev_colon=false; }
    } }
}

fn main() {
    let root = std::env::args().nth(1).expect("corpus dir");
    let mut out = serde_json::Map::new();
    for repo in std::fs::read_dir(&root).unwrap() {
        let repo = repo.unwrap().path(); if !repo.is_dir() { continue; }
        let name = repo.file_name().unwrap().to_string_lossy().to_string();
        let mut files=Vec::new();
        for e in walkdir::WalkDir::new(&repo) { let e=e.unwrap(); let p=e.path(); if p.extension().map(|x| x=="rs").unwrap_or(false) && !p.to_string_lossy().contains("/target/") { if let Ok(src)=std::fs::read_to_string(p) { files.push(src); } } }
        let mut asts=Vec::new(); let mut fails=0u64;
        for src in &files { match syn::parse_file(src) { Ok(f)=>asts.push((src, f)), Err(_)=>fails+=1 } }
        let mut fnn = FnNames{names:HashSet::new(), copy_types:HashSet::new(), all_types:HashSet::new()}; for (_,f) in &asts { fnn.visit_file(f); }
        let mut total: Counts = BTreeMap::new();
        total.insert("meta.files".into(), files.len() as u64); total.insert("meta.parse_failures".into(), fails);
        total.insert("meta.lines".into(), files.iter().map(|s| s.lines().count() as u64).sum());
        total.insert("meta.nonws_chars".into(), files.iter().map(|s| s.chars().filter(|c| !c.is_whitespace()).count() as u64).sum());
        for (src,f) in &asts { let mut c=Collector{c:BTreeMap::new(), local_fns:&fnn.names, copy_types:&fnn.copy_types, all_types:&fnn.all_types, scopes:vec![]}; c.visit_file(f); for (k,v) in c.c { *total.entry(k).or_insert(0)+=v; } if let Ok(ts)=src.parse::<proc_macro2::TokenStream>() { token_stats(&ts, &mut total); } }
        out.insert(name, serde_json::to_value(total).unwrap());
        eprintln!("done {}", repo.display());
    }
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
