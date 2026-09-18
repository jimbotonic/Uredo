//! Parameter shapes of every function in a Rust file, for the §37 inference hit rate:
//! `fn_path\tindex\tshape\tgeneric` per parameter, shape ∈ value | ref | refmut, generic = 1 when
//! the parameter's type mentions a type parameter or `impl Trait`.
//!
//! usage: paramshape <file.rs>

use quote::ToTokens;
use syn::visit::{self, Visit};
use syn::*;

struct V {
    prefix: Vec<String>,
    type_params: Vec<String>,
}

fn mentions_generic(ty: &Type, tps: &[String]) -> bool {
    let text = ty.to_token_stream().to_string();
    if text.contains("impl ") {
        return true;
    }
    let words: Vec<String> = text.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).map(|w| w.to_string()).collect();
    words.iter().any(|w| tps.contains(w))
}

impl V {
    fn record(&self, sig: &Signature) {
        let mut tps = self.type_params.clone();
        for p in &sig.generics.params {
            if let GenericParam::Type(t) = p {
                tps.push(t.ident.to_string());
            }
        }
        let path = {
            let mut p = self.prefix.clone();
            p.push(sig.ident.to_string());
            p.join("::")
        };
        let mut index = 0;
        for a in &sig.inputs {
            let FnArg::Typed(t) = a else { continue };
            let shape = match &*t.ty {
                Type::Reference(r) => if r.mutability.is_some() { "refmut" } else { "ref" },
                _ => "value",
            };
            println!("{}\t{}\t{}\t{}", path, index, shape, if mentions_generic(&t.ty, &tps) { 1 } else { 0 });
            index += 1;
        }
    }
}

impl<'ast> Visit<'ast> for V {
    fn visit_item_fn(&mut self, f: &'ast ItemFn) {
        self.record(&f.sig);
        visit::visit_item_fn(self, f);
    }
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let name = match &*i.self_ty {
            Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default(),
            _ => "impl".into(),
        };
        let saved_tp = self.type_params.clone();
        for p in &i.generics.params {
            if let GenericParam::Type(t) = p {
                self.type_params.push(t.ident.to_string());
            }
        }
        self.prefix.push(name);
        for it in &i.items {
            if let ImplItem::Fn(m) = it {
                self.record(&m.sig);
            }
        }
        self.prefix.pop();
        self.type_params = saved_tp;
    }
    fn visit_item_trait(&mut self, t: &'ast ItemTrait) {
        self.prefix.push(t.ident.to_string());
        for it in &t.items {
            if let TraitItem::Fn(m) = it {
                self.record(&m.sig);
            }
        }
        self.prefix.pop();
    }
    fn visit_item_mod(&mut self, m: &'ast ItemMod) {
        self.prefix.push(m.ident.to_string());
        visit::visit_item_mod(self, m);
        self.prefix.pop();
    }
}

fn main() {
    let file = std::env::args().nth(1).expect("file");
    let src = std::fs::read_to_string(&file).expect("read");
    let ast = syn::parse_file(&src).expect("parse");
    let mut v = V { prefix: vec![], type_params: vec![] };
    v.visit_file(&ast);
}
