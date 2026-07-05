// Dot product of two 2M-element vectors — idiomatic single-thread Rust.
fn main() {
    let n = 2_000_000usize;
    let a: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let total: f64 = a.iter().map(|x| x * x).sum();
    println!("{:.3}", total / 1e15);
}
