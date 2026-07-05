// Capability-parity pipeline in idiomatic Rust (standard library only):
// schema validation, constraint check, PARALLEL CPU-bound scoring, PII
// redaction, file output, invariant verification. With no crates allowed
// (serde/rayon/regex), validation, the thread pool, and the email masker are
// all hand-rolled; with crates it needs a Cargo project and a build step.
use std::fs;
use std::thread;

#[derive(Clone)]
struct Order {
    id: String,
    amount: f64,
    email: String,
}

fn make(i: usize) -> Order {
    Order {
        id: format!("ord-{}", i),
        amount: ((i * 7) % 500) as f64,
        email: format!("user{}@corp.com", i),
    }
}

fn validate(o: &Order) -> bool {
    // Field presence/types are enforced by the struct; mirror the semantic
    // checks the dynamic languages perform.
    !o.id.is_empty() && o.amount.is_finite() && !o.email.is_empty()
}

fn non_negative(o: &Order) -> bool {
    o.amount >= 0.0
}

fn score(o: &Order) -> f64 {
    let mut s = 0.0;
    let mut j = 0;
    while j < 200 {
        s += (o.amount * j as f64 + 1.0).sqrt();
        j += 1;
    }
    (s * 10.0).round() / 10.0
}

/// Mask anything that looks like an email address (no regex crate).
fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('@') {
        let (before, after) = rest.split_at(at);
        let local_start = before
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || "._%+-".contains(c)))
            .map(|p| p + 1)
            .unwrap_or(0);
        let domain_len = after[1..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || ".-".contains(c)))
            .unwrap_or(after.len() - 1);
        out.push_str(&before[..local_start]);
        out.push_str("[REDACTED:email]");
        rest = &after[1 + domain_len..];
    }
    out.push_str(rest);
    out
}

fn parallel_scores(valid: &[Order]) -> Vec<f64> {
    let n_workers = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let chunk = valid.len().div_ceil(n_workers);
    let mut handles = Vec::new();
    for part in valid.chunks(chunk) {
        let part: Vec<Order> = part.to_vec();
        handles.push(thread::spawn(move || {
            part.iter().map(score).collect::<Vec<f64>>()
        }));
    }
    let mut scores = Vec::with_capacity(valid.len());
    for h in handles {
        scores.extend(h.join().expect("worker panicked"));
    }
    scores
}

fn main() {
    let orders: Vec<Order> = (0..2000).map(make).collect();
    let valid: Vec<Order> = orders
        .into_iter()
        .filter(|o| validate(o) && non_negative(o))
        .collect();

    let scores = parallel_scores(&valid);

    let mut top = 0.0f64;
    for &s in &scores {
        if s > top {
            top = s;
        }
    }

    let report = format!(
        "orders={} top={} contact={}",
        valid.len(),
        top,
        valid[0].email
    );
    println!("{}", redact(&report));
    fs::write("pipeline_report.txt", redact(&report)).expect("write failed");
    let invariant = valid.iter().all(|o| o.amount >= 0.0);
    println!("invariant holds: {}", invariant);
}
