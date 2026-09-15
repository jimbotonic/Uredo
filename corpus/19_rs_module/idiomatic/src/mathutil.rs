//! Hand-written Rust: a module the Uredo program calls into with Rust conventions.

#[derive(Debug, Clone)]
pub struct Matrix {
    n: usize,
    data: Vec<f64>,
}

impl Matrix {
    pub fn identity(n: usize) -> Matrix {
        let mut data = vec![0.0; n * n];
        for i in 0..n {
            data[i * n + i] = 1.0;
        }
        Matrix { n, data }
    }
    pub fn rows(&self) -> usize {
        self.n
    }
    pub fn trace(&self) -> f64 {
        (0..self.n).map(|i| self.data[i * self.n + i]).sum()
    }
}

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn scale_in_place(v: &mut Vec<f64>, k: f64) {
    for x in v.iter_mut() {
        *x *= k;
    }
}

pub fn describe(m: &Matrix) -> String {
    format!("{}x{} matrix", m.n, m.n)
}
