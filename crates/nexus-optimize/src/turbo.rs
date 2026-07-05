//! DSPy-style prompt optimization via Trust-Region Bayesian Optimization (TuRBO).
//!
//! The developer declares a structured signature (inputs, outputs, a metric).
//! The optimizer explores instruction prompts and few-shot configurations,
//! represented as a point in a bounded continuous space, and maximizes the
//! metric. A lightweight kernel surrogate with a UCB acquisition function makes
//! it *sample-efficient* — each expensive evaluation (an LLM run) is chosen to
//! be maximally informative, and a shrinking/growing trust region balances
//! exploration and exploitation.

/// A bounded search space.
#[derive(Clone)]
pub struct Bounds {
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
}

impl Bounds {
    pub fn new(lo: Vec<f64>, hi: Vec<f64>) -> Self {
        assert_eq!(lo.len(), hi.len());
        Bounds { lo, hi }
    }
    pub fn dim(&self) -> usize {
        self.lo.len()
    }
    fn clip(&self, x: &mut [f64]) {
        for ((xi, lo), hi) in x.iter_mut().zip(self.lo.iter()).zip(self.hi.iter()) {
            *xi = xi.clamp(*lo, *hi);
        }
    }
}

/// The optimization outcome.
#[derive(Clone, Debug)]
pub struct OptResult {
    pub best_x: Vec<f64>,
    pub best_y: f64,
    pub evals: usize,
    /// Best-so-far value after each evaluation (a monotone non-decreasing curve).
    pub history: Vec<f64>,
}

/// A tiny deterministic xorshift PRNG so runs are reproducible (no `rand` dep,
/// no nondeterminism in content-addressed builds).
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// Uniform in [0,1).
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn dist2(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

/// Maximize `objective` over `bounds` within an evaluation `budget`.
pub fn maximize<F>(objective: F, bounds: &Bounds, budget: usize, seed: u64) -> OptResult
where
    F: Fn(&[f64]) -> f64,
{
    let dim = bounds.dim();
    let mut rng = Rng::new(seed);
    let mut xs: Vec<Vec<f64>> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    let mut history = Vec::new();

    let eval = |x: &[f64], xs: &mut Vec<Vec<f64>>, ys: &mut Vec<f64>, history: &mut Vec<f64>| {
        let y = objective(x);
        xs.push(x.to_vec());
        ys.push(y);
        let best = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        history.push(best);
        y
    };

    // --- Initial design: a handful of random probes. ---
    let init = (2 * dim).max(3).min(budget);
    for _ in 0..init {
        let x: Vec<f64> = (0..dim)
            .map(|i| bounds.lo[i] + rng.unit() * (bounds.hi[i] - bounds.lo[i]))
            .collect();
        eval(&x, &mut xs, &mut ys, &mut history);
    }

    // Trust region length (fraction of each dimension's range).
    let mut tr_len = 0.8f64;
    let mut success = 0u32;
    let mut failure = 0u32;
    let kernel_ell = 0.2f64; // surrogate length-scale (normalized units)

    while ys.len() < budget {
        // Current best.
        let (bi, &by) = ys
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();
        let center = xs[bi].clone();

        // Sample candidates inside the trust region, score by UCB on a kernel
        // surrogate, and evaluate only the most promising one.
        let n_cand = 24;
        let mut best_cand: Option<Vec<f64>> = None;
        let mut best_acq = f64::NEG_INFINITY;
        for _ in 0..n_cand {
            let mut c = vec![0.0; dim];
            for i in 0..dim {
                let half = tr_len * (bounds.hi[i] - bounds.lo[i]) / 2.0;
                c[i] = center[i] + (rng.unit() * 2.0 - 1.0) * half;
            }
            bounds.clip(&mut c);
            let acq = ucb(&c, &xs, &ys, kernel_ell);
            if acq > best_acq {
                best_acq = acq;
                best_cand = Some(c);
            }
        }

        let cand = best_cand.unwrap();
        let y = eval(&cand, &mut xs, &mut ys, &mut history);

        // Trust-region adaptation.
        if y > by + 1e-12 {
            success += 1;
            failure = 0;
        } else {
            failure += 1;
            success = 0;
        }
        if success >= 3 {
            tr_len = (tr_len * 2.0).min(1.6);
            success = 0;
        }
        if failure >= 4 {
            tr_len = (tr_len / 2.0).max(0.02);
            failure = 0;
        }
    }

    let (bi, &by) = ys
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();
    OptResult { best_x: xs[bi].clone(), best_y: by, evals: ys.len(), history }
}

/// Upper-confidence-bound acquisition on a Gaussian-kernel surrogate.
fn ucb(x: &[f64], xs: &[Vec<f64>], ys: &[f64], ell: f64) -> f64 {
    let beta = 1.5;
    let mut wsum = 0.0;
    let mut wy = 0.0;
    for (xi, &yi) in xs.iter().zip(ys) {
        let w = (-dist2(x, xi) / (2.0 * ell * ell)).exp();
        wsum += w;
        wy += w * yi;
    }
    let mean = if wsum > 1e-12 { wy / wsum } else { 0.0 };
    // Uncertainty grows where there is little nearby data.
    let std = 1.0 / (1.0 + wsum);
    mean + beta * std
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_quadratic_optimum() {
        // Maximize -(x-3)^2 -(y+1)^2, optimum at (3,-1), value 0.
        let bounds = Bounds::new(vec![-10.0, -10.0], vec![10.0, 10.0]);
        let res = maximize(
            |x| -((x[0] - 3.0).powi(2) + (x[1] + 1.0).powi(2)),
            &bounds,
            120,
            42,
        );
        assert!(res.best_y > -0.5, "best_y = {}", res.best_y);
        assert!((res.best_x[0] - 3.0).abs() < 0.6);
        assert!((res.best_x[1] + 1.0).abs() < 0.6);
    }

    #[test]
    fn respects_eval_budget() {
        let bounds = Bounds::new(vec![0.0], vec![1.0]);
        let res = maximize(|x| x[0], &bounds, 30, 7);
        assert_eq!(res.evals, 30);
        assert_eq!(res.history.len(), 30);
    }

    #[test]
    fn history_is_monotone() {
        let bounds = Bounds::new(vec![0.0], vec![1.0]);
        let res = maximize(|x| x[0], &bounds, 40, 1);
        for w in res.history.windows(2) {
            assert!(w[1] >= w[0] - 1e-12); // best-so-far never decreases
        }
    }

    #[test]
    fn sample_efficiency_beats_random() {
        // On the same budget, TuRBO should match or beat pure random search.
        let bounds = Bounds::new(vec![-5.0, -5.0], vec![5.0, 5.0]);
        let f = |x: &[f64]| -((x[0] - 2.0).powi(2) + (x[1] - 2.0).powi(2));
        let turbo = maximize(f, &bounds, 60, 123).best_y;

        let mut rng = Rng::new(123);
        let mut rand_best = f64::NEG_INFINITY;
        for _ in 0..60 {
            let x = [
                -5.0 + rng.unit() * 10.0,
                -5.0 + rng.unit() * 10.0,
            ];
            rand_best = rand_best.max(f(&x));
        }
        assert!(turbo >= rand_best - 1e-9, "turbo {} vs random {}", turbo, rand_best);
    }
}
