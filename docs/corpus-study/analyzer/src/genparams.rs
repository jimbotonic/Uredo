//! Per-parameter records for the generic-`T` review (§38): for every function parameter whose
//! type is a type parameter, an `impl Trait`, or an `Option`, emit one JSON line with its shape
//! as written in idiomatic Rust and the bounds that apply, so that candidate Uredo rules can be
//! scored for their inference hit rate.
//!
//! usage: genparams <dir> > records.jsonl

use quote::ToTokens;
use std::collections::HashMap;
use syn::visit::{self, Visit};
use syn::*;

struct V {
    repo: String,
    file: String,
    /// type-parameter name -> bounds (last segment of each trait path), from the impl and fn
    impl_bounds: HashMap<String, Vec<String>>,
    in_trait: bool,
}

fn bound_names(bounds: &punctuated::Punctuated<TypeParamBound, Token![+]>) -> Vec<String> {
    bounds
        .iter()
        .filter_map(|b| match b {
            TypeParamBound::Trait(t) => t.path.segments.last().map(|s| {
                let id = s.ident.to_string();
                // keep the first generic argument of Fn-family bounds out; keep `Into<String>` as `Into`
                id
            }),
            _ => None,
        })
        .collect()
}

fn generics_bounds(g: &Generics) -> HashMap<String, Vec<String>> {
    let mut m: HashMap<String, Vec<String>> = HashMap::new();
    for p in &g.params {
        if let GenericParam::Type(tp) = p {
            m.entry(tp.ident.to_string()).or_default().extend(bound_names(&tp.bounds));
        }
    }
    if let Some(w) = &g.where_clause {
        for pred in &w.predicates {
            if let WherePredicate::Type(pt) = pred {
                if let Type::Path(p) = &pt.bounded_ty {
                    if let Some(id) = p.path.get_ident() {
                        m.entry(id.to_string()).or_default().extend(bound_names(&pt.bounds));
                    }
                }
            }
        }
    }
    m
}

fn last_ident(p: &TypePath) -> String {
    p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default()
}

fn first_type_arg(p: &TypePath) -> Option<&Type> {
    if let PathArguments::AngleBracketed(a) = &p.path.segments.last()?.arguments {
        for ga in &a.args {
            if let GenericArgument::Type(t) = ga {
                return Some(t);
            }
        }
    }
    None
}

impl V {
    fn record(&self, sig: &Signature, kind: &str) {
        let mut bounds = self.impl_bounds.clone();
        for (k, v) in generics_bounds(&sig.generics) {
            bounds.entry(k).or_default().extend(v);
        }
        let type_params: Vec<String> = bounds.keys().cloned().collect();
        for a in &sig.inputs {
            let FnArg::Typed(t) = a else { continue };
            let (shape, tp, extra_bounds) = classify(&t.ty, &type_params);
            let Some(shape) = shape else { continue };
            let b: Vec<String> = match &tp {
                Some(name) => bounds.get(name).cloned().unwrap_or_default(),
                None => extra_bounds,
            };
            let rec = serde_json::json!({
                "repo": self.repo, "file": self.file, "fn": sig.ident.to_string(), "kind": kind,
                "trait_decl": self.in_trait,
                "shape": shape, "type_param": tp, "bounds": b,
                "type": t.ty.to_token_stream().to_string().replace(" ", ""),
            });
            println!("{}", rec);
        }
    }
}

/// A coarse class for an `Option`'s payload: whether passing the `Option` by value can move
/// anything the caller might still want. `copy` = scalar/reference/known-Copy std type;
/// `owned` = String/Vec/Box/collection/other; `unknown` = a name we cannot classify.
fn payload_class(ty: &Type) -> &'static str {
    let text = ty.to_token_stream().to_string().replace(' ', "");
    let head = text.split('<').next().unwrap_or(&text).rsplit("::").next().unwrap_or("").to_string();
    const SCALARS: &[&str] = &["bool", "char", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "NonZeroUsize", "NonZeroU32", "NonZeroU64"];
    const COPYISH: &[&str] = &["Duration", "Instant", "Ordering", "SocketAddr", "Ipv4Addr", "Ipv6Addr", "TypeId", "Range", "RangeInclusive", "Layout", "Entity", "NodeId", "Id"];
    const OWNED: &[&str] = &["String", "Vec", "Box", "HashMap", "BTreeMap", "HashSet", "PathBuf", "OsString", "Arc", "Rc", "VecDeque", "Cow"];
    if text.starts_with('&') {
        return "copy";
    }
    if SCALARS.contains(&head.as_str()) || COPYISH.contains(&head.as_str()) {
        return "copy";
    }
    if OWNED.contains(&head.as_str()) {
        return "owned";
    }
    "unknown"
}

/// Returns (shape, type parameter name if the outer type is one, bounds for impl Trait).
fn classify(ty: &Type, type_params: &[String]) -> (Option<&'static str>, Option<String>, Vec<String>) {
    match ty {
        Type::Path(p) => {
            let id = last_ident(p);
            if p.path.segments.len() == 1 && type_params.contains(&id) {
                return (Some("T"), Some(id), vec![]);
            }
            if id == "Option" {
                if let Some(inner) = first_type_arg(p) {
                    return match inner {
                        Type::Reference(r) => (Some(if r.mutability.is_some() { "Option<&mut X>" } else { "Option<&X>" }), None, vec![]),
                        Type::Path(ip) if ip.path.segments.len() == 1 && type_params.contains(&last_ident(ip)) => (Some("Option<T>"), Some(last_ident(ip)), vec![]),
                        _ => (Some("Option<X>"), Some(format!("payload:{}", payload_class(inner))), vec![]),
                    };
                }
            }
            (None, None, vec![])
        }
        Type::ImplTrait(it) => (Some("impl"), None, bound_names(&it.bounds)),
        Type::Reference(r) => match &*r.elem {
            Type::Path(p) => {
                let id = last_ident(p);
                if p.path.segments.len() == 1 && type_params.contains(&id) {
                    return (Some(if r.mutability.is_some() { "&mut T" } else { "&T" }), Some(id), vec![]);
                }
                if id == "Option" {
                    return (Some(if r.mutability.is_some() { "&mut Option<X>" } else { "&Option<X>" }), None, vec![]);
                }
                (None, None, vec![])
            }
            Type::ImplTrait(it) => (Some(if r.mutability.is_some() { "&mut impl" } else { "&impl" }), None, bound_names(&it.bounds)),
            Type::Slice(s) => {
                if let Type::Path(p) = &*s.elem {
                    if p.path.segments.len() == 1 && type_params.contains(&last_ident(p)) {
                        return (Some("&[T]"), Some(last_ident(p)), vec![]);
                    }
                }
                (None, None, vec![])
            }
            _ => (None, None, vec![]),
        },
        _ => (None, None, vec![]),
    }
}

impl<'ast> Visit<'ast> for V {
    fn visit_item_fn(&mut self, f: &'ast ItemFn) {
        self.record(&f.sig, "free");
        visit::visit_item_fn(self, f);
    }
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let saved = self.impl_bounds.clone();
        self.impl_bounds = generics_bounds(&i.generics);
        for it in &i.items {
            if let ImplItem::Fn(m) = it {
                self.record(&m.sig, if i.trait_.is_some() { "trait_impl" } else { "method" });
            }
        }
        visit::visit_item_impl(self, i);
        self.impl_bounds = saved;
    }
    fn visit_item_trait(&mut self, t: &'ast ItemTrait) {
        let saved = self.impl_bounds.clone();
        self.impl_bounds = generics_bounds(&t.generics);
        self.in_trait = true;
        for it in &t.items {
            if let TraitItem::Fn(m) = it {
                self.record(&m.sig, "trait_decl");
            }
        }
        self.in_trait = false;
        visit::visit_item_trait(self, t);
        self.impl_bounds = saved;
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
            let mut v = V { repo, file: rel, impl_bounds: HashMap::new(), in_trait: false };
            v.visit_file(&file);
        }
    }
}
