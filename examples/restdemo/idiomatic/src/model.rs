//! The types this service carries, and the proof that they are in the accepted set.

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::field::{Field, Map};

#[derive(Debug, Serialize)]
pub struct Health {
    pub status: String,
    pub items: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub id: u64,
    pub priority: i32,
    pub weight: f64,
    pub name: String,
    pub done: bool,
    pub tag: Option<String>,
    pub labels: Vec<String>,
    pub meta: Map<String>,
}

/// What a client may send. `id` is the server's to assign, so it is absent.
#[derive(Debug, Deserialize)]
pub struct NewItem {
    pub name: String,
    pub tag: Option<String>,
    pub priority: Option<i32>,
    pub weight: Option<f64>,
    pub labels: Option<Vec<String>>,
    pub meta: Option<Map<String>>,
}

/// A partial update: every field optional, absent means unchanged.
#[derive(Debug, Deserialize)]
pub struct ItemPatch {
    pub name: Option<String>,
    pub tag: Option<String>,
    pub done: Option<bool>,
    pub priority: Option<i32>,
    pub weight: Option<f64>,
    pub labels: Option<Vec<String>>,
    pub meta: Option<Map<String>>,
}

/// The type set, enforced rather than described: a field outside it stops compiling here.
fn _every_field_is_accepted(item: &Item) {
    fn accept<T: Field>(_value: &T) {}
    accept(&item.id);
    accept(&item.priority);
    accept(&item.weight);
    accept(&item.name);
    accept(&item.done);
    accept(&item.tag);
    accept(&item.labels);
    accept(&item.meta);
}

impl NewItem {
    pub fn validate(&self) -> Result<(), Error> {
        if self.name.is_empty() {
            return Err(Error::BadRequest("name must not be empty".to_string()));
        }
        if self.name.len() > 120 {
            return Err(Error::BadRequest("name is too long".to_string()));
        }
        if let Some(w) = self.weight {
            if !w.is_finite() {
                return Err(Error::BadRequest("weight must be a finite number".to_string()));
            }
        }
        Ok(())
    }
}

impl Item {
    pub fn from_new(id: u64, new: NewItem) -> Item {
        Item {
            id,
            priority: new.priority.unwrap_or(0),
            weight: new.weight.unwrap_or(0.0),
            name: new.name,
            done: false,
            tag: new.tag,
            labels: new.labels.unwrap_or_default(),
            meta: new.meta.unwrap_or_default(),
        }
    }

    /// Applying a patch: absent leaves the field alone.
    pub fn apply(&mut self, patch: ItemPatch) {
        if let Some(name) = patch.name {
            self.name = name;
        }
        if let Some(tag) = patch.tag {
            self.tag = Some(tag);
        }
        if let Some(done) = patch.done {
            self.done = done;
        }
        if let Some(priority) = patch.priority {
            self.priority = priority;
        }
        if let Some(weight) = patch.weight {
            self.weight = weight;
        }
        if let Some(labels) = patch.labels {
            self.labels = labels;
        }
        if let Some(meta) = patch.meta {
            self.meta = meta;
        }
    }
}
