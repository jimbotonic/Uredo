//! 19. Calling into a hand-written module.

mod mathutil;

use mathutil::{Matrix, dot, scale_in_place};

fn main() {
    let a = [1.0, 2.0, 3.0];
    let b = [4.0, 5.0, 6.0];
    println!("dot {}", dot(&a, &b));
    let mut v = vec![1.0, 2.0];
    scale_in_place(&mut v, 3.0);
    println!("{v:?}");
    let m = Matrix::identity(2);
    println!("{:?} trace {}", m.rows(), m.trace());
    println!("{}", mathutil::describe(&m));
}
