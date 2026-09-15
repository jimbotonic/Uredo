//! §35 compatibility entry: the proc-macro half, which §22.6 says is written in Rust. It stays a
//! plain Rust crate; what is under test is consuming it from Uredo.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// Derives `fn field_names() -> Vec<&'static str>` listing a struct's fields, and an attribute
/// `#[label = "…"]` on the struct chooses the prefix each name carries.
#[proc_macro_derive(FieldNames, attributes(label))]
pub fn field_names(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let mut label = String::new();
    for attr in &ast.attrs {
        if attr.path().is_ident("label") {
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                    label = s.value();
                }
            }
        }
    }
    let fields: Vec<String> = match &ast.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(named) => named
                .named
                .iter()
                .map(|f| format!("{}{}", label, f.ident.as_ref().unwrap()))
                .collect(),
            _ => vec![],
        },
        _ => vec![],
    };
    quote! {
        impl #name {
            pub fn field_names() -> Vec<&'static str> {
                vec![#(#fields),*]
            }
        }
    }
    .into()
}
