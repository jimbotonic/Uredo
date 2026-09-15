//! The state, and the only place this service allocates per request beyond the response body.

use std::collections::BTreeMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::Error;
use crate::model::{Health, Item, ItemPatch, NewItem};
use crate::query::Query;

pub struct Store {
    items: RwLock<BTreeMap<u64, Item>>,
    next: AtomicU64,
}

impl Store {
    pub fn new() -> Store {
        Store { items: RwLock::new(BTreeMap::new()), next: AtomicU64::new(1) }
    }

    /// A snapshot of every item, in id order.
    pub fn list(&self) -> Vec<Item> {
        self.items.read().unwrap().values().cloned().collect()
    }

    /// The same, filtered and paged. The query borrows the request's bytes and this borrows the
    /// query, so a filtered list allocates only the items it returns.
    pub fn select(&self, q: &Query) -> Vec<Item> {
        self.items
            .read()
            .unwrap()
            .values()
            .filter(|i| q.done.is_none_or(|d| i.done == d))
            .filter(|i| q.label.is_none_or(|l| i.labels.iter().any(|have| have == l)))
            .filter(|i| q.contains.is_none_or(|c| i.name.contains(c)))
            .skip(q.offset)
            .take(q.limit)
            .cloned()
            .collect()
    }

    pub fn get(&self, id: u64) -> Option<Item> {
        self.items.read().unwrap().get(&id).cloned()
    }

    /// Names are unique, so a repeat is a 409 rather than a second row.
    pub fn create(&self, new: NewItem) -> Result<Item, Error> {
        let mut guard = self.items.write().unwrap();
        if guard.values().any(|i| i.name == new.name) {
            return Err(Error::Conflict(format!("an item named {:?} already exists", new.name)));
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let item = Item::from_new(id, new);
        guard.insert(id, item.clone());
        Ok(item)
    }

    pub fn patch(&self, id: u64, patch: ItemPatch) -> Option<Item> {
        let mut guard = self.items.write().unwrap();
        let existing = guard.get_mut(&id)?;
        existing.apply(patch);
        Some(existing.clone())
    }

    pub fn delete(&self, id: u64) -> bool {
        self.items.write().unwrap().remove(&id).is_some()
    }

    pub fn len(&self) -> usize {
        self.items.read().unwrap().len()
    }

    /// Health as a typed value, so it goes through the same encoder as everything else.
    pub fn health(&self) -> Health {
        Health { status: "ok".to_string(), items: self.len() as u64 }
    }
}
