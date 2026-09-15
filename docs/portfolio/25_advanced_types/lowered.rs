//! 25 — Advanced types: `PhantomData`, `?Sized`, `Cow`, `TryFrom`, `Index`/`IndexMut`, `Weak`,
//! array repeat. None of this is new syntax: each is a std type or trait spelled as in Rust.
//! What Uredo adds is what it adds everywhere — borrow-by-default parameters (§10.2), colon
//! bodies, explicit receivers, and the call-site rules of §10.3.

use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};
use std::rc::{Rc, Weak};

/// Typed identifiers: `PhantomData<T>` makes `Id<User>` and `Id<Order>` distinct types at zero
/// size. `@derive(Copy)` would add a `T: Copy` bound that `Id<User>` cannot meet, so the impls are
/// hand-written — and a hand-written `Copy` is invisible to the passing table (§10.1): `Id<User>`
/// is *not* known-Copy, so an `id: Id<User>` parameter is still a shared borrow (P3).
struct Id<T> {
    raw: u32,
    _marker: PhantomData<T>,
}

impl<T> Id<T> {
    fn new(raw: u32) -> Self {
        Id {
            raw,
            _marker: PhantomData,
        }
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Id::new(self.raw)
    }
}

impl<T> Copy for Id<T> {}

enum User {}

enum Order {}

fn describe(id: &Id<User>) -> String {
    ::std::format!("user#{}", id.raw)
}

/// `?Sized` lets a generic borrow accept unsized types (`str`, `[T]`). A `?Sized` bound is the one
/// bound the passing table reads: it keeps `x: T` a borrow, `&T` (D52), where P8 would pass by value.
fn byte_len<T: AsRef<[u8]> + ?Sized>(x: &T) -> usize {
    x.as_ref().len()
}

/// `Cow`: borrow when nothing changes, allocate only when something must. The return type says
/// both can happen; no clone is hidden. The `'_` is Rust's elided lifetime, tied to `s` (§9.7).
fn normalize(s: &str) -> Cow<'_, str> {
    if s.contains(' ') {
        Cow::Owned(s.replace(' ', "_"))
    } else {
        Cow::Borrowed(s)
    }
}

/// Fallible conversion: `TryFrom` is an ordinary trait impl. `format` is the intrinsic (§11.3).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Percent(u8);

impl TryFrom<u32> for Percent {
    type Error = String;
    fn try_from(v: u32) -> Result<Self, Self::Error> {
        if v <= 100 {
            Ok(Percent(v as u8))
        } else {
            Err(::std::format!("{} is not a percentage", v))
        }
    }
}

/// Operator traits (see 18): `Index`/`IndexMut` give a type the `[]` syntax. A tuple of scalars
/// is known-Copy (§10.1), so `(x, y)` is a by-value pattern parameter (P7); the receivers and the
/// reference return types are written, because the trait's signature has them.
struct Grid {
    w: usize,
    cells: Vec<u8>,
}

impl Index<(usize, usize)> for Grid {
    type Output = u8;
    fn index(&self, (x, y): (usize, usize)) -> &Self::Output {
        &self.cells[y * self.w + x]
    }
}

impl IndexMut<(usize, usize)> for Grid {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        &mut self.cells[y * self.w + x]
    }
}

/// `Weak`: a parent pointer that does not keep the parent alive, breaking the cycle that
/// `Rc<RefCell<Node>>` in both directions would leak (compare 17).
struct TreeNode {
    name: String,
    parent: RefCell<Weak<TreeNode>>,
    children: RefCell<Vec<Rc<TreeNode>>>,
}

fn adopt(parent: &Rc<TreeNode>, child: &Rc<TreeNode>) {
    *child.parent.borrow_mut() = Rc::downgrade(parent);
    parent.children.borrow_mut().push(Rc::clone(child));
}

fn main() {
    let uid: Id<User> = Id::new(7);
    let oid: Id<Order> = Id::new(7);
    ::std::println!("{} order#{}", describe(&uid), oid.raw);
    let copied = uid;
    ::std::println!("{}", copied.raw + uid.raw);

    ::std::println!(
        "{} {}",
        byte_len("héllo"),
        byte_len(&Vec::from([1u8, 2, 3]))
    );

    for s in ["plain", "has space"] {
        match normalize(&s) {
            Cow::Borrowed(b) => ::std::println!("borrowed {}", b),
            Cow::Owned(o) => ::std::println!("owned {}", o),
        }
    }

    for v in [42u32, 150] {
        match Percent::try_from(v) {
            Ok(p) => ::std::println!("{:?}", p),
            Err(e) => ::std::println!("error: {}", e),
        }
    }

    let mut grid = Grid {
        w: 3,
        cells: Vec::from([0u8; 9]),
    };
    grid[(1, 2)] = 5;
    ::std::println!("cell {} of {}", grid[(1, 2)], grid.cells.len());

    let root = Rc::new(TreeNode {
        name: String::from("root"),
        parent: RefCell::new(Weak::new()),
        children: RefCell::new(Vec::new()),
    });
    let leaf = Rc::new(TreeNode {
        name: String::from("leaf"),
        parent: RefCell::new(Weak::new()),
        children: RefCell::new(Vec::new()),
    });
    adopt(&root, &leaf);
    let parent_name = leaf.parent.borrow().upgrade().map(|p| p.name.clone());
    ::std::println!(
        "leaf's parent {:?}; root strong {} weak {}",
        parent_name,
        Rc::strong_count(&root),
        Rc::weak_count(&root)
    );
}
