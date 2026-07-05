//! Symbolic execution and invariant proving.
//!
//! Given a set of path assumptions (constraints holding on an execution path)
//! and a safety guarantee, the verifier proves the guarantee by showing that
//! `assumptions ∧ ¬guarantee` is unsatisfiable. If a satisfying assignment
//! exists, it is a concrete counterexample — the violating path the spec says
//! the compiler must surface as a target test case.

use crate::solver::{solve, Atom, Feasibility, LinExpr};
use nexus_core::{BinOp, CmpOp, Expr, Literal, Predicate, Rational};
use std::collections::BTreeMap;

/// Outcome of attempting to prove a guarantee.
#[derive(Debug)]
pub enum ProofResult {
    /// The guarantee holds on every path (no counterexample exists).
    Proven,
    /// The guarantee can be violated; here is a concrete counterexample.
    Violated { model: BTreeMap<String, Rational> },
    /// The guarantee/assumptions use constructs outside the supported fragment
    /// (e.g. non-arithmetic operators, or too many bilinear factors).
    Unsupported { reason: String },
    /// A nonlinear guarantee whose sign-relaxation is too weak to either prove
    /// or exhibit a genuine counterexample. Soundly inconclusive.
    Unknown { reason: String },
}

impl ProofResult {
    pub fn is_proven(&self) -> bool {
        matches!(self, ProofResult::Proven)
    }
}

/// Maps variable names (dotted paths) to dense solver indices.
#[derive(Default)]
struct VarMap {
    to_idx: BTreeMap<String, usize>,
    names: Vec<String>,
}

impl VarMap {
    fn idx(&mut self, name: &str) -> usize {
        if let Some(i) = self.to_idx.get(name) {
            return *i;
        }
        let i = self.names.len();
        self.to_idx.insert(name.to_string(), i);
        self.names.push(name.to_string());
        i
    }
    fn len(&self) -> usize {
        self.names.len()
    }
    fn name(&self, idx: usize) -> &str {
        &self.names[idx]
    }
}

fn lit_to_lin(l: &Literal) -> Option<LinExpr> {
    Some(match l {
        Literal::Number(r) | Literal::Money(r) => LinExpr::constant(*r),
        Literal::Bytes(b) => LinExpr::constant(Rational::int(*b as i128)),
        Literal::Duration(d) => LinExpr::constant(Rational::int(*d as i128)),
        _ => return None,
    })
}

/// Conversion context: owns the variable map and the running list of bilinear
/// product obligations discovered while linearizing expressions.
///
/// A product of two non-constant linear expressions `a * b` cannot be expressed
/// in linear arithmetic, so it is replaced by a FRESH product variable `p` and
/// the relation `p == a * b` is recorded as an obligation. Identical products
/// (same factors up to ordering) reuse the same fresh variable. Because `a` and
/// `b` are themselves linear expressions (possibly already containing product
/// variables), nested products compose naturally.
#[derive(Default)]
struct Conv {
    vm: VarMap,
    /// Each entry is `(p_idx, a, b)` meaning solver-variable `p_idx == a * b`.
    products: Vec<(usize, LinExpr, LinExpr)>,
}

impl Conv {
    /// Return the fresh product variable representing `a * b`, allocating (and
    /// recording the obligation) on first use; deduped up to factor ordering.
    fn product_var(&mut self, a: LinExpr, b: LinExpr) -> usize {
        for (p, x, y) in &self.products {
            if (*x == a && *y == b) || (*x == b && *y == a) {
                return *p;
            }
        }
        let name = format!("__prod_{}", self.products.len());
        let p = self.vm.idx(&name);
        self.products.push((p, a, b));
        p
    }
}

fn expr_to_lin(e: &Expr, ctx: &mut Conv) -> Option<LinExpr> {
    Some(match e {
        Expr::Var(name) => LinExpr::var(ctx.vm.idx(name)),
        Expr::Lit(l) => lit_to_lin(l)?,
        Expr::Bin(op, a, b) => {
            let la = expr_to_lin(a, ctx)?;
            let lb = expr_to_lin(b, ctx)?;
            match op {
                BinOp::Add => la.add(&lb),
                BinOp::Sub => la.sub(&lb),
                BinOp::Mul => {
                    // If either side is constant, the product is linear.
                    if lb.terms.is_empty() {
                        la.scale(lb.constant)
                    } else if la.terms.is_empty() {
                        lb.scale(la.constant)
                    } else {
                        // Bilinear: introduce/reuse a fresh product variable.
                        // (Both factors are already reduced to linear form, so
                        // any sub-product they contained is a product var here.)
                        let p = ctx.product_var(la, lb);
                        LinExpr::var(p)
                    }
                }
                BinOp::Div => {
                    if lb.terms.is_empty() && lb.constant.num != 0 {
                        la.scale(Rational::new(lb.constant.den, lb.constant.num))
                    } else {
                        return None;
                    }
                }
            }
        }
    })
}

/// Convert `lhs op rhs` into a conjunction of atoms (a single disjunct).
fn pred_to_atoms(p: &Predicate, ctx: &mut Conv) -> Option<Vec<Atom>> {
    let l = expr_to_lin(&p.lhs, ctx)?;
    let rr = expr_to_lin(&p.rhs, ctx)?;
    let diff = l.sub(&rr); // lhs - rhs
    Some(match p.op {
        CmpOp::Ge => vec![Atom::ge(diff)],
        CmpOp::Gt => vec![Atom::gt(diff)],
        CmpOp::Le => vec![Atom::ge(diff.scale(Rational::int(-1)))],
        CmpOp::Lt => vec![Atom::gt(diff.scale(Rational::int(-1)))],
        CmpOp::Eq => Atom::eq(diff).to_vec(),
        CmpOp::Ne => return None, // disjunctive; not a single conjunct
    })
}

/// The negation of a guarantee, expressed as a set of disjuncts. The guarantee
/// holds iff `assumptions ∧ disjunct` is UNSAT for *every* disjunct.
fn negate(p: &Predicate, ctx: &mut Conv) -> Option<Vec<Vec<Atom>>> {
    let l = expr_to_lin(&p.lhs, ctx)?;
    let rr = expr_to_lin(&p.rhs, ctx)?;
    let diff = l.sub(&rr); // lhs - rhs
    let neg = diff.scale(Rational::int(-1)); // rhs - lhs
    Some(match p.op {
        // ¬(lhs >= rhs) == lhs < rhs == rhs - lhs > 0
        CmpOp::Ge => vec![vec![Atom::gt(neg)]],
        // ¬(lhs > rhs) == lhs <= rhs == rhs - lhs >= 0
        CmpOp::Gt => vec![vec![Atom::ge(neg)]],
        // ¬(lhs <= rhs) == lhs > rhs
        CmpOp::Le => vec![vec![Atom::gt(diff)]],
        // ¬(lhs < rhs) == lhs >= rhs
        CmpOp::Lt => vec![vec![Atom::ge(diff)]],
        // ¬(lhs == rhs) == lhs > rhs OR lhs < rhs
        CmpOp::Eq => vec![vec![Atom::gt(diff)], vec![Atom::gt(neg)]],
        // ¬(lhs != rhs) == lhs == rhs
        CmpOp::Ne => vec![Atom::eq(diff).to_vec()],
    })
}

/// Prove that `guarantee` holds under all the `assumptions`.
///
/// Refutation-based: the guarantee holds iff `assumptions ∧ ¬guarantee` is
/// unsatisfiable. Purely-linear problems are decided directly. Nonlinear
/// (bilinear) problems are handled by a SOUND sign relaxation — see the body.
pub fn prove(assumptions: &[Predicate], guarantee: &Predicate) -> ProofResult {
    let mut ctx = Conv::default();
    let mut base = Vec::new();
    for a in assumptions {
        match pred_to_atoms(a, &mut ctx) {
            Some(atoms) => base.extend(atoms),
            None => {
                return ProofResult::Unsupported {
                    reason: format!("assumption `{}` is not linear", a),
                }
            }
        }
    }
    let disjuncts = match negate(guarantee, &mut ctx) {
        Some(d) => d,
        None => {
            return ProofResult::Unsupported {
                reason: format!("guarantee `{}` is not linear", guarantee),
            }
        }
    };

    let nvars = ctx.vm.len();
    let products = std::mem::take(&mut ctx.products);

    // ---- Purely-linear fast path: behaviour identical to the original. ----
    if products.is_empty() {
        for disj in &disjuncts {
            let mut atoms = base.clone();
            atoms.extend(disj.iter().cloned());
            if let Feasibility::Sat(model) = solve(&atoms, nvars) {
                // A satisfying assignment to the negation = a violating path.
                return ProofResult::Violated { model: name_model(&model, &ctx.vm) };
            }
        }
        return ProofResult::Proven;
    }

    // ---- Nonlinear path: exhaustive sign case-split over bilinear factors. ----
    //
    // Soundness sketch: for every product obligation `p == a*b` and any fixed
    // signs of `a` and `b`, the sign of `p` is entailed (equal signs ⇒ p>=0,
    // mixed ⇒ p<=0). Adding those entailed facts only ENLARGES the feasible
    // region relative to the true (nonlinear) one, so the truth set is a subset
    // of the relaxation. Hence: if every case is UNSAT, the truth is UNSAT and
    // the guarantee holds (Proven). A SAT case may be spurious, so we only ever
    // report Violated after checking the model satisfies every product exactly.

    // Distinct bilinear factors (the linear expressions appearing as a or b),
    // deduped structurally.
    let mut factors: Vec<LinExpr> = Vec::new();
    for (_, a, b) in &products {
        for f in [a, b] {
            if !factors.iter().any(|g| g == f) {
                factors.push(f.clone());
            }
        }
    }
    if factors.len() > 6 {
        return ProofResult::Unsupported {
            reason: format!(
                "nonlinear guarantee has {} distinct bilinear factors (max 6)",
                factors.len()
            ),
        };
    }

    // For each factor, restrict the enumerated signs to those not contradicted
    // by an *entailed* one-sided bound from the base assumptions. This is a
    // sound tightening: if `base ⊢ f >= 0` then the `f >= 0` half already covers
    // every feasible point (including f == 0), so dropping the `f <= 0` case
    // loses nothing — yet it removes degenerate `f == 0` relaxation models in
    // which the decoupled product variable could take a spurious value.
    let mut sign_opts: Vec<Vec<bool>> = Vec::with_capacity(factors.len());
    for f in &factors {
        let neg_f = f.scale(Rational::int(-1));
        let mut probe_pos = base.clone();
        probe_pos.push(Atom::gt(neg_f)); // f < 0
        let nonneg_entailed = matches!(solve(&probe_pos, nvars), Feasibility::Unsat);
        if nonneg_entailed {
            sign_opts.push(vec![true]);
            continue;
        }
        let mut probe_neg = base.clone();
        probe_neg.push(Atom::gt(f.clone())); // f > 0
        let nonpos_entailed = matches!(solve(&probe_neg, nvars), Feasibility::Unsat);
        if nonpos_entailed {
            sign_opts.push(vec![false]);
        } else {
            sign_opts.push(vec![true, false]); // true = f>=0, false = f<=0
        }
    }

    let total: usize = sign_opts.iter().map(|v| v.len()).product();
    let mut all_unsat = true;
    for combo_idx in 0..total {
        // Decode `combo_idx` (mixed-radix) into a sign per factor.
        let mut rem = combo_idx;
        let mut signs: Vec<bool> = Vec::with_capacity(factors.len());
        for opts in &sign_opts {
            signs.push(opts[rem % opts.len()]);
            rem /= opts.len();
        }

        let mut case_atoms: Vec<Atom> = Vec::new();
        // Sign atom for each factor.
        for (i, f) in factors.iter().enumerate() {
            if signs[i] {
                case_atoms.push(Atom::ge(f.clone())); // f >= 0
            } else {
                case_atoms.push(Atom::ge(f.scale(Rational::int(-1)))); // f <= 0
            }
        }
        // Entailed sign of each product variable in this case.
        for (p, a, b) in &products {
            // Invariant: `factors` was built (above) by pushing every `a` and
            // `b` of every product, deduped by the SAME `==` used here, so both
            // lookups must succeed. Rather than `unwrap()` — which would panic
            // on untrusted (`.nexus`-authored) input if that invariant were ever
            // broken by a refactor — degrade soundly: decline to prove. Bailing
            // to `Unsupported` is always safe (it never claims a false Proven).
            let (Some(ia), Some(ib)) = (
                factors.iter().position(|f| f == a),
                factors.iter().position(|f| f == b),
            ) else {
                return ProofResult::Unsupported {
                    reason: "internal: bilinear factor missing from factor set".into(),
                };
            };
            let pv = LinExpr::var(*p);
            if signs[ia] == signs[ib] {
                case_atoms.push(Atom::ge(pv)); // p >= 0
            } else {
                case_atoms.push(Atom::ge(pv.scale(Rational::int(-1)))); // p <= 0
            }
        }

        for disj in &disjuncts {
            let mut atoms = base.clone();
            atoms.extend(case_atoms.iter().cloned());
            atoms.extend(disj.iter().cloned());
            if let Feasibility::Sat(model) = solve(&atoms, nvars) {
                all_unsat = false;
                // Only a genuine counterexample if the relaxed model happens to
                // satisfy every product relation exactly.
                if products_exact(&products, &model) {
                    return ProofResult::Violated {
                        model: name_model_stripped(&model, &ctx.vm),
                    };
                }
            }
        }
    }

    if all_unsat {
        ProofResult::Proven
    } else {
        ProofResult::Unknown {
            reason: "nonlinear sign relaxation is satisfiable but no exact \
                     counterexample was found; cannot prove or refute"
                .into(),
        }
    }
}

/// Evaluate a linear expression under a (solver-indexed) model.
fn eval_lin(e: &LinExpr, model: &BTreeMap<usize, Rational>) -> Rational {
    let mut v = e.constant;
    for (i, c) in &e.terms {
        let x = model.get(i).copied().unwrap_or(Rational::zero());
        v = v.add(c.mul(x));
    }
    v
}

/// True iff every product obligation `p == a*b` holds *exactly* under `model`.
fn products_exact(
    products: &[(usize, LinExpr, LinExpr)],
    model: &BTreeMap<usize, Rational>,
) -> bool {
    products.iter().all(|(p, a, b)| {
        let pv = model.get(p).copied().unwrap_or(Rational::zero());
        eval_lin(a, model).mul(eval_lin(b, model)).cmp_val(pv) == std::cmp::Ordering::Equal
    })
}

/// Name a solver model, keeping all variables (linear path — unchanged output).
fn name_model(model: &BTreeMap<usize, Rational>, vm: &VarMap) -> BTreeMap<String, Rational> {
    model
        .iter()
        .map(|(i, v)| (vm.name(*i).to_string(), *v))
        .collect()
}

/// Name a solver model, dropping the synthetic `__prod_*` product variables.
fn name_model_stripped(
    model: &BTreeMap<usize, Rational>,
    vm: &VarMap,
) -> BTreeMap<String, Rational> {
    model
        .iter()
        .filter(|(i, _)| !vm.name(**i).starts_with("__prod_"))
        .map(|(i, v)| (vm.name(*i).to_string(), *v))
        .collect()
}

/// Pretty-print a counterexample model for human/AI consumption.
pub fn format_model(model: &BTreeMap<String, Rational>) -> String {
    let mut parts: Vec<String> = model.iter().map(|(k, v)| format!("{k} = {v}")).collect();
    parts.sort();
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::{Expr, Literal};

    fn var(n: &str) -> Expr {
        Expr::Var(n.into())
    }
    fn num(n: i128) -> Expr {
        Expr::Lit(Literal::Number(Rational::int(n)))
    }
    fn pred(l: Expr, op: CmpOp, r: Expr) -> Predicate {
        Predicate { lhs: l, op, rhs: r }
    }

    #[test]
    fn proves_no_negative_balance() {
        // Assumptions modeling a safe settlement transition:
        //   balance == start - amount
        //   start >= amount + minimum_reserve   (guard the code enforces)
        //   minimum_reserve >= 0
        // Guarantee: balance >= minimum_reserve  -> PROVEN
        let assumptions = vec![
            // balance - start + amount == 0
            pred(
                Expr::Bin(
                    BinOp::Add,
                    Box::new(Expr::Bin(BinOp::Sub, Box::new(var("balance")), Box::new(var("start")))),
                    Box::new(var("amount")),
                ),
                CmpOp::Eq,
                num(0),
            ),
            // start - amount - minimum_reserve >= 0
            pred(
                Expr::Bin(
                    BinOp::Sub,
                    Box::new(Expr::Bin(BinOp::Sub, Box::new(var("start")), Box::new(var("amount")))),
                    Box::new(var("minimum_reserve")),
                ),
                CmpOp::Ge,
                num(0),
            ),
            pred(var("minimum_reserve"), CmpOp::Ge, num(0)),
        ];
        let guarantee = pred(var("balance"), CmpOp::Ge, var("minimum_reserve"));
        assert!(prove(&assumptions, &guarantee).is_proven());
    }

    #[test]
    fn catches_violating_path() {
        // Without the guard, an overdraft path exists -> counterexample.
        let assumptions = vec![
            pred(
                Expr::Bin(
                    BinOp::Add,
                    Box::new(Expr::Bin(BinOp::Sub, Box::new(var("balance")), Box::new(var("start")))),
                    Box::new(var("amount")),
                ),
                CmpOp::Eq,
                num(0),
            ),
            pred(var("amount"), CmpOp::Gt, num(0)),
            pred(var("start"), CmpOp::Ge, num(0)),
            pred(var("minimum_reserve"), CmpOp::Ge, num(0)),
        ];
        let guarantee = pred(var("balance"), CmpOp::Ge, var("minimum_reserve"));
        match prove(&assumptions, &guarantee) {
            ProofResult::Violated { model } => {
                // The counterexample must actually violate the guarantee.
                let bal = model.get("balance").copied().unwrap_or(Rational::zero());
                let res = model.get("minimum_reserve").copied().unwrap_or(Rational::zero());
                assert!(bal.cmp_val(res) == std::cmp::Ordering::Less);
            }
            other => panic!("expected a counterexample, got {:?}", other),
        }
    }

    #[test]
    fn proves_budget_guard() {
        // monthly_budget_remaining > 5 AND spend == 0 -> remaining stays > 5 ... trivial proof
        let assumptions = vec![pred(var("monthly_budget_remaining"), CmpOp::Gt, num(5))];
        let guarantee = pred(var("monthly_budget_remaining"), CmpOp::Gt, num(0));
        assert!(prove(&assumptions, &guarantee).is_proven());
    }

    fn mul(a: Expr, b: Expr) -> Expr {
        Expr::Bin(BinOp::Mul, Box::new(a), Box::new(b))
    }

    #[test]
    fn proves_product_of_nonnegatives() {
        // x>=0, y>=0  =>  x*y >= 0
        let assumptions = vec![
            pred(var("x"), CmpOp::Ge, num(0)),
            pred(var("y"), CmpOp::Ge, num(0)),
        ];
        let guarantee = pred(mul(var("x"), var("y")), CmpOp::Ge, num(0));
        assert!(prove(&assumptions, &guarantee).is_proven());
    }

    #[test]
    fn proves_nonnegative_tax() {
        // The user's case: tax == income*rate, income>=0, rate>=0  =>  tax >= 0
        let assumptions = vec![
            pred(var("tax"), CmpOp::Eq, mul(var("income"), var("rate"))),
            pred(var("income"), CmpOp::Ge, num(0)),
            pred(var("rate"), CmpOp::Ge, num(0)),
        ];
        let guarantee = pred(var("tax"), CmpOp::Ge, num(0));
        assert!(prove(&assumptions, &guarantee).is_proven());
    }

    #[test]
    fn proves_square_nonnegative() {
        // No assumptions:  x*x >= 0  (sign split on x covers both halves)
        let guarantee = pred(mul(var("x"), var("x")), CmpOp::Ge, num(0));
        assert!(prove(&[], &guarantee).is_proven());
    }

    #[test]
    fn unknown_or_refuted_when_relaxation_too_weak() {
        // x>=0, y>=0  =>  x*y >= 1 is FALSE (x=y=0 refutes it). Must NOT be Proven.
        let assumptions = vec![
            pred(var("x"), CmpOp::Ge, num(0)),
            pred(var("y"), CmpOp::Ge, num(0)),
        ];
        let guarantee = pred(mul(var("x"), var("y")), CmpOp::Ge, num(1));
        let res = prove(&assumptions, &guarantee);
        assert!(!res.is_proven(), "must not prove a false nonlinear guarantee: {:?}", res);
    }

    #[test]
    fn nonlinear_violation_is_never_spurious() {
        // x>=0, y>=0  =>  x*y >= 1  is false. A sound prover must either return a
        // counterexample whose product is satisfied EXACTLY, or be inconclusive
        // (Unknown) — but it must never claim Proven and never report a model
        // that does not actually satisfy the product relation.
        let assumptions = vec![
            pred(var("x"), CmpOp::Ge, num(0)),
            pred(var("y"), CmpOp::Ge, num(0)),
        ];
        let guarantee = pred(mul(var("x"), var("y")), CmpOp::Ge, num(1));
        match prove(&assumptions, &guarantee) {
            ProofResult::Violated { model } => {
                let x = model.get("x").copied().unwrap_or(Rational::zero());
                let y = model.get("y").copied().unwrap_or(Rational::zero());
                // synthetic product vars stripped from the reported model
                assert!(model.keys().all(|k| !k.starts_with("__prod_")));
                // and the model genuinely violates x*y >= 1
                assert!(x.mul(y).cmp_val(Rational::int(1)) == std::cmp::Ordering::Less);
            }
            ProofResult::Unknown { .. } => {} // sound: relaxation too weak
            other => panic!("must not prove a false guarantee, got {:?}", other),
        }
    }

    // ---- Regression: the bilinear factor-position lookup must never panic. ----
    //
    // Lines ~294 formerly did `factors.iter().position(...).unwrap()`. These
    // cases drive the nonlinear sign-split path with the shapes most likely to
    // expose an off-by-one in the factor set (distinct products sharing a
    // factor, repeated factors, nested products, and a self-product/square).
    // They assert the prover returns a *sound* result (never a false Proven)
    // and, above all, does not panic while resolving product factor indices.

    #[test]
    fn nonlinear_shared_factor_does_not_panic() {
        // Two products that share the factor `x`: x*y and x*z.
        // Reaches the factor-position lookup for x (twice), y, and z.
        let assumptions = vec![
            pred(var("x"), CmpOp::Ge, num(0)),
            pred(var("y"), CmpOp::Ge, num(0)),
            pred(var("z"), CmpOp::Ge, num(0)),
        ];
        // guarantee: x*y + x*z >= 0  (true for nonneg vars) — must be sound.
        let guarantee = pred(
            Expr::Bin(
                BinOp::Add,
                Box::new(mul(var("x"), var("y"))),
                Box::new(mul(var("x"), var("z"))),
            ),
            CmpOp::Ge,
            num(0),
        );
        // Must not panic; must not falsely refute a true guarantee.
        assert!(prove(&assumptions, &guarantee).is_proven());
    }

    #[test]
    fn nonlinear_repeated_and_square_factors_do_not_panic() {
        // A product and its mirror x*y / y*x plus a square x*x: exercises the
        // factor dedup (up to ordering) and the a == b self-product branch that
        // makes both position() lookups resolve to the same factor entry.
        let guarantee = pred(
            Expr::Bin(
                BinOp::Add,
                Box::new(Expr::Bin(
                    BinOp::Sub,
                    Box::new(mul(var("x"), var("y"))),
                    Box::new(mul(var("y"), var("x"))),
                )),
                Box::new(mul(var("x"), var("x"))),
            ),
            CmpOp::Ge,
            num(0),
        );
        // x*y - y*x + x*x == x*x >= 0. Whatever the prover concludes, it must
        // not panic and must not report a spurious model.
        match prove(&[], &guarantee) {
            ProofResult::Proven | ProofResult::Unknown { .. } => {}
            ProofResult::Violated { model } => {
                assert!(model.keys().all(|k| !k.starts_with("__prod_")));
            }
            other => panic!("unexpected sound-but-wrong result: {:?}", other),
        }
    }

    #[test]
    fn nonlinear_nested_product_does_not_panic() {
        // (x*y)*z — a nested bilinear whose inner product becomes a product
        // variable that then appears as a factor of the outer product. This is
        // the case where a factor is itself synthetic; the position lookup must
        // still find it (or degrade to Unsupported) rather than unwrap-panic.
        let assumptions = vec![
            pred(var("x"), CmpOp::Ge, num(0)),
            pred(var("y"), CmpOp::Ge, num(0)),
            pred(var("z"), CmpOp::Ge, num(0)),
        ];
        let guarantee = pred(
            mul(mul(var("x"), var("y")), var("z")),
            CmpOp::Ge,
            num(0),
        );
        // Must terminate without panicking and never falsely claim Proven if it
        // cannot soundly do so; here it may be Proven or Unknown — both sound.
        let res = prove(&assumptions, &guarantee);
        assert!(
            matches!(res, ProofResult::Proven | ProofResult::Unknown { .. }),
            "nested product must be sound (Proven/Unknown), got {:?}",
            res
        );
    }

    #[test]
    fn linear_violation_still_reported() {
        // The original linear refutation path is unchanged.
        let assumptions = vec![
            pred(
                Expr::Bin(
                    BinOp::Add,
                    Box::new(Expr::Bin(BinOp::Sub, Box::new(var("balance")), Box::new(var("start")))),
                    Box::new(var("amount")),
                ),
                CmpOp::Eq,
                num(0),
            ),
            pred(var("amount"), CmpOp::Gt, num(0)),
            pred(var("start"), CmpOp::Ge, num(0)),
        ];
        let guarantee = pred(var("balance"), CmpOp::Ge, num(0));
        assert!(matches!(prove(&assumptions, &guarantee), ProofResult::Violated { .. }));
    }
}
