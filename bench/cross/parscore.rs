// CPU-bound scoring of 20k items — idiomatic single-thread Rust (std only;
// parallelism would require the rayon crate or manual thread management).
fn score(x: f64) -> f64 {
    let mut s = 0.0;
    let mut i = 0;
    while i < 50 {
        s += (x * i as f64 + 1.0).sqrt();
        i += 1;
    }
    s
}

fn main() {
    let scores: Vec<f64> = (0..20000).map(|x| score(x as f64)).collect();
    let total: f64 = scores.iter().sum();
    println!("{:.3}", total / 1_000_000.0);
}
