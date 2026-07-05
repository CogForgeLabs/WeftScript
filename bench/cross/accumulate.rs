// Map accumulation: 300k updates over 3000 keys.
use std::collections::HashMap;

fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut i: i64 = 0;
    while i < 300_000 {
        let k = format!("key_{}", i % 3000);
        *counts.entry(k).or_insert(0) += 1;
        i += 1;
    }
    println!("{}", counts["key_0"]);
}
