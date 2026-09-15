//! Public API manifest (§4.4): every `pub` item's Rust signature, derived only from Uredo
//! declarations. `uredo check --api` compares it with the reviewed baseline `uredo-api.json`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApiEntry {
    /// Module path of the item (`shapes::Rect`, `shapes::Rect::area`, `total_area`).
    pub path: String,
    /// `fn`, `method`, `struct`, `enum`, `trait`, `impl`, `type`, `const`, `static`, `mod`, `use`.
    pub kind: String,
    /// The generated Rust signature (header without body).
    pub signature: String,
    /// Attributes that are part of the API (`derive(...)`, `cfg(...)`, `repr(...)`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attrs: Vec<String>,
    /// Public fields of a struct, variants of an enum.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Manifest {
    pub package: String,
    /// The compiler version that produced it (manifests are compared by content, not version).
    pub uredo: String,
    pub items: Vec<ApiEntry>,
}

impl Manifest {
    pub fn new(package: &str, mut items: Vec<ApiEntry>) -> Manifest {
        items.sort();
        items.dedup();
        Manifest { package: package.to_string(), uredo: env!("CARGO_PKG_VERSION").to_string(), items }
    }
}

#[derive(Debug, Default)]
pub struct Diff {
    pub added: Vec<ApiEntry>,
    pub removed: Vec<ApiEntry>,
    pub changed: Vec<(ApiEntry, ApiEntry)>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    pub fn render(&self, baseline: &str) -> String {
        let mut out = format!("public API differs from {}:\n", baseline);
        for e in &self.added {
            out.push_str(&format!("  + added    {} ({})\n      {}\n", e.path, e.kind, e.signature));
        }
        for e in &self.removed {
            out.push_str(&format!("  - removed  {} ({})\n      {}\n", e.path, e.kind, e.signature));
        }
        for (old, new) in &self.changed {
            out.push_str(&format!("  ~ changed  {} ({})\n      was: {}\n      now: {}\n", new.path, new.kind, describe(old), describe(new)));
        }
        out
    }
}

fn describe(e: &ApiEntry) -> String {
    let mut s = e.signature.clone();
    if !e.attrs.is_empty() {
        s = format!("#[{}] {}", e.attrs.join("] #["), s);
    }
    if !e.members.is_empty() {
        s.push_str(&format!(" {{ {} }}", e.members.join(", ")));
    }
    s
}

/// Compares two manifests by (path, kind).
pub fn diff(old: &Manifest, new: &Manifest) -> Diff {
    let key = |e: &ApiEntry| (e.path.clone(), e.kind.clone());
    let old_map: BTreeMap<_, &ApiEntry> = old.items.iter().map(|e| (key(e), e)).collect();
    let new_map: BTreeMap<_, &ApiEntry> = new.items.iter().map(|e| (key(e), e)).collect();
    let mut d = Diff::default();
    for (k, e) in &new_map {
        match old_map.get(k) {
            None => d.added.push((*e).clone()),
            Some(o) => {
                if o.signature != e.signature || o.attrs != e.attrs || o.members != e.members {
                    d.changed.push(((*o).clone(), (*e).clone()));
                }
            }
        }
    }
    for (k, e) in &old_map {
        if !new_map.contains_key(k) {
            d.removed.push((*e).clone());
        }
    }
    d
}
