//! Every function parameter of a Rust tree with its passing shape and type text, for the
//! `@value use` question (§38): one JSON line per parameter with `repo`, `shape`
//! (value | ref | refmut), `generic` (the type mentions a type parameter or `impl Trait`) and
//! the type as written. A syn pass, so `impl Into<String>` and `F` are not mistaken for concrete
//! types the way a textual pass does.
//!
//! usage: valparams <dir> > valparams.jsonl

use quote::ToTokens;
use syn::visit::{self, Visit};
use syn::*;

struct V {
    repo: String,
    type_params: Vec<String>,
}

fn mentions_generic(ty: &Type, tps: &[String]) -> bool {
    let text = ty.to_token_stream().to_string();
    if text.contains("impl ") || text.contains("dyn ") {
        return true;
    }
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .any(|w| tps.iter().any(|t| t == w))
}

impl V {
    fn record(&self, sig: &Signature) {
        let mut tps = self.type_params.clone();
        for p in &sig.generics.params {
            if let GenericParam::Type(t) = p {
                tps.push(t.ident.to_string());
            }
        }
        for a in &sig.inputs {
            let FnArg::Typed(t) = a else { continue };
            let shape = match &*t.ty {
                Type::Reference(r) => {
                    if r.mutability.is_some() {
                        "refmut"
                    } else {
                        "ref"
                    }
                }
                _ => "value",
            };
            let rec = serde_json::json!({
                "repo": self.repo,
                "shape": shape,
                "generic": mentions_generic(&t.ty, &tps),
                "type": t.ty.to_token_stream().to_string().replace(' ', ""),
            });
            println!("{}", rec);
        }
    }
}

impl<'ast> Visit<'ast> for V {
    fn visit_item_fn(&mut self, f: &'ast ItemFn) {
        self.record(&f.sig);
        visit::visit_item_fn(self, f);
    }
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let saved = self.type_params.clone();
        for p in &i.generics.params {
            if let GenericParam::Type(t) = p {
                self.type_params.push(t.ident.to_string());
            }
        }
        for it in &i.items {
            if let ImplItem::Fn(m) = it {
                self.record(&m.sig);
            }
        }
        visit::visit_item_impl(self, i);
        self.type_params = saved;
    }
    fn visit_item_trait(&mut self, t: &'ast ItemTrait) {
        let saved = self.type_params.clone();
        for p in &t.generics.params {
            if let GenericParam::Type(tp) = p {
                self.type_params.push(tp.ident.to_string());
            }
        }
        for it in &t.items {
            if let TraitItem::Fn(m) = it {
                self.record(&m.sig);
            }
        }
        visit::visit_item_trait(self, t);
        self.type_params = saved;
    }
}

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    for e in walkdir::WalkDir::new(&dir).into_iter().flatten() {
        let p = e.path();
        if p.extension().map(|x| x == "rs").unwrap_or(false) {
            let Ok(src) = std::fs::read_to_string(p) else { continue };
            let Ok(file) = syn::parse_file(&src) else { continue };
            let rel = p.strip_prefix(&dir).unwrap_or(p).to_string_lossy().to_string();
            let repo = rel.split('/').next().unwrap_or("").to_string();
            let mut v = V { repo, type_params: vec![] };
            v.visit_file(&file);
        }
    }
}
