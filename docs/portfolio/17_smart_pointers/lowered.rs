//! 17 — Smart pointers: `Box`, `Rc`, `RefCell`, `Arc`. Every one is spelled, because each
//! is a cost: heap allocation, reference counting, runtime borrow checks (§3.2).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Recursive data needs indirection: `Box` puts the tail on the heap.
#[derive(Debug)]
enum List {
    Cons(i32, Box<List>),
    Nil,
}

impl List {
    fn sum(&self) -> i32 {
        match *self {
            List::Cons(v, ref rest) => v + rest.sum(),
            List::Nil => 0,
        }
    }
}

/// Shared ownership with interior mutability: `Rc<RefCell<T>>`. The borrow checks happen at
/// runtime and can panic; the types say so at every use.
#[derive(Debug)]
struct Node {
    name: String,
    children: Vec<Rc<RefCell<Node>>>,
}

fn add_child(parent: &Rc<RefCell<Node>>, name: &str) {
    let child = Rc::new(RefCell::new(Node {
        name: name.to_string(),
        children: Vec::new(),
    }));
    parent.borrow_mut().children.push(child);
}

fn count(node: &Rc<RefCell<Node>>) -> usize {
    let n = node.borrow();
    ::std::println!("visit {}", n.name);
    1 + n.children.iter().map(|c| count(&c)).sum::<usize>()
}

fn main() {
    let list = List::Cons(
        1,
        Box::new(List::Cons(2, Box::new(List::Cons(3, Box::new(List::Nil))))),
    );
    ::std::println!("{}", list.sum());

    let root = Rc::new(RefCell::new(Node {
        name: String::from("root"),
        children: Vec::new(),
    }));
    add_child(&root, "a");
    add_child(&root, "b");
    add_child(&root.borrow().children[0].clone(), "a1");
    ::std::println!(
        "nodes {} strong refs to root {}",
        count(&root),
        Rc::strong_count(&root)
    );

    let shared = Arc::new(String::from("shared"));
    let handles: Vec<Arc<String>> = (0..3).map(|_| Arc::clone(&shared)).collect();
    ::std::println!("{} refs, value {}", Arc::strong_count(&shared), handles[0]);

    let b = Box::new(41);
    ::std::println!("{}", *b + 1);
}
