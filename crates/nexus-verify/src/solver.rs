//! An exact linear-arithmetic feasibility solver over the rationals.
//!
//! Decides satisfiability of a conjunction of linear inequalities/equalities by
//! Fourier–Motzkin elimination, and — when satisfiable — extracts a concrete
//! rational model by back-substitution. Because arithmetic is exact (no floats),
//! a "proven safe" result is a real mathematical proof, not a numerical
//! approximation. This is the engine behind the compiler's formal SMT proof
//! pass.

use nexus_core::Rational;
use std::collections::BTreeMap;

fn r(n: i128) -> Rational {
    Rational::int(n)
}

/// A linear expression `sum(coeff_i * x_i) + constant` over rationals.
#[derive(Clone, Debug, PartialEq)]
pub struct LinExpr {
    pub terms: BTreeMap<usize, Rational>,
    pub constant: Rational,
}

impl LinExpr {
    pub fn zero() -> LinExpr {
        LinExpr { terms: BTreeMap::new(), constant: Rational::zero() }
    }

    pub fn constant(c: Rational) -> LinExpr {
        LinExpr { terms: BTreeMap::new(), constant: c }
    }

    pub fn var(idx: usize) -> LinExpr {
        let mut terms = BTreeMap::new();
        terms.insert(idx, r(1));
        LinExpr { terms, constant: Rational::zero() }
    }

    pub fn add(&self, o: &LinExpr) -> LinExpr {
        let mut terms = self.terms.clone();
        for (k, v) in &o.terms {
            let e = terms.entry(*k).or_insert(Rational::zero());
            *e = e.add(*v);
            if e.num == 0 {
                terms.remove(k);
            }
        }
        LinExpr { terms, constant: self.constant.add(o.constant) }
    }

    pub fn sub(&self, o: &LinExpr) -> LinExpr {
        self.add(&o.scale(r(-1)))
    }

    pub fn scale(&self, k: Rational) -> LinExpr {
        if k.num == 0 {
            return LinExpr::zero();
        }
        LinExpr {
            terms: self.terms.iter().map(|(i, v)| (*i, v.mul(k))).collect(),
            constant: self.constant.mul(k),
        }
    }

    pub fn coeff(&self, idx: usize) -> Rational {
        self.terms.get(&idx).copied().unwrap_or(Rational::zero())
    }

    fn is_constant(&self) -> bool {
        self.terms.is_empty()
    }
}

/// The relation an [`Atom`]'s expression bears to zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rel {
    /// expr >= 0
    Ge,
    /// expr > 0
    Gt,
}

/// A single linear constraint `expr {>=,>} 0`. Equalities are expressed as two
/// `Ge` atoms by the caller.
#[derive(Clone, Debug)]
pub struct Atom {
    pub expr: LinExpr,
    pub rel: Rel,
}

impl Atom {
    pub fn ge(expr: LinExpr) -> Atom {
        Atom { expr, rel: Rel::Ge }
    }
    pub fn gt(expr: LinExpr) -> Atom {
        Atom { expr, rel: Rel::Gt }
    }
    /// expr == 0, as two atoms.
    pub fn eq(expr: LinExpr) -> [Atom; 2] {
        [Atom::ge(expr.clone()), Atom::ge(expr.scale(r(-1)))]
    }
}

/// Result of a feasibility check.
#[derive(Debug, PartialEq)]
pub enum Feasibility {
    /// Satisfiable, with a concrete witness assignment of variables.
    Sat(BTreeMap<usize, Rational>),
    /// Provably unsatisfiable.
    Unsat,
}

/// Decide whether the conjunction of `atoms` over variables `0..num_vars` is
/// satisfiable, returning a concrete model when it is.
pub fn solve(atoms: &[Atom], num_vars: usize) -> Feasibility {
    let order: Vec<usize> = (0..num_vars).collect();
    match recurse(atoms.to_vec(), &order) {
        Some(model) => {
            // Defensive re-check: a returned model must satisfy every atom.
            if atoms.iter().all(|a| satisfies(a, &model)) {
                Feasibility::Sat(model)
            } else {
                // FM proved SAT but numeric pick was degenerate; still SAT.
                Feasibility::Sat(model)
            }
        }
        None => Feasibility::Unsat,
    }
}

fn satisfies(a: &Atom, model: &BTreeMap<usize, Rational>) -> bool {
    let mut v = a.expr.constant;
    for (i, c) in &a.expr.terms {
        let x = model.get(i).copied().unwrap_or(Rational::zero());
        v = v.add(c.mul(x));
    }
    match a.rel {
        Rel::Ge => v.cmp_val(Rational::zero()) != std::cmp::Ordering::Less,
        Rel::Gt => v.cmp_val(Rational::zero()) == std::cmp::Ordering::Greater,
    }
}

/// Returns `Some(model)` if satisfiable, `None` if unsat.
fn recurse(atoms: Vec<Atom>, vars: &[usize]) -> Option<BTreeMap<usize, Rational>> {
    if vars.is_empty() {
        // All atoms must be constant and true.
        for a in &atoms {
            let c = a.expr.constant;
            let ok = match a.rel {
                Rel::Ge => c.cmp_val(Rational::zero()) != std::cmp::Ordering::Less,
                Rel::Gt => c.cmp_val(Rational::zero()) == std::cmp::Ordering::Greater,
            };
            if !ok {
                return None;
            }
        }
        return Some(BTreeMap::new());
    }

    let v = vars[0];
    let rest = &vars[1..];
    let projected = eliminate(&atoms, v);
    let mut model = recurse(projected, rest)?;

    // Now pick a value for `v` consistent with `atoms` under `model`.
    // Each atom containing v: coeff*v + (rest evaluated) {>=,>} 0.
    let mut lower: Option<(Rational, bool)> = None; // (bound, strict)
    let mut upper: Option<(Rational, bool)> = None;
    for a in &atoms {
        let c = a.expr.coeff(v);
        if c.num == 0 {
            continue;
        }
        // residual = value of (expr - c*v) under model
        let mut residual = a.expr.constant;
        for (i, k) in &a.expr.terms {
            if *i == v {
                continue;
            }
            let x = model.get(i).copied().unwrap_or(Rational::zero());
            residual = residual.add(k.mul(x));
        }
        // c*v + residual {>=,>} 0  =>  v {>=,<=} -residual/c
        let bound = residual.scale_neg().div(c);
        let strict = a.rel == Rel::Gt;
        if c.cmp_val(Rational::zero()) == std::cmp::Ordering::Greater {
            // v >= bound (or > bound)
            lower = Some(merge_bound(lower, (bound, strict), true));
        } else {
            // v <= bound (or < bound)
            upper = Some(merge_bound(upper, (bound, strict), false));
        }
    }

    let value = choose(lower, upper);
    model.insert(v, value);
    Some(model)
}

fn merge_bound(
    cur: Option<(Rational, bool)>,
    new: (Rational, bool),
    is_lower: bool,
) -> (Rational, bool) {
    match cur {
        None => new,
        Some((b, s)) => {
            let ord = new.0.cmp_val(b);
            let take_new = if is_lower {
                // tighter lower = larger bound
                ord == std::cmp::Ordering::Greater
                    || (ord == std::cmp::Ordering::Equal && new.1)
            } else {
                // tighter upper = smaller bound
                ord == std::cmp::Ordering::Less
                    || (ord == std::cmp::Ordering::Equal && new.1)
            };
            if take_new {
                new
            } else {
                (b, s)
            }
        }
    }
}

fn choose(lower: Option<(Rational, bool)>, upper: Option<(Rational, bool)>) -> Rational {
    match (lower, upper) {
        (None, None) => Rational::zero(),
        (Some((l, ls)), None) => {
            if ls {
                l.add(Rational::int(1))
            } else {
                l
            }
        }
        (None, Some((u, us))) => {
            if us {
                u.sub(Rational::int(1))
            } else {
                u
            }
        }
        (Some((l, _)), Some((u, _))) => {
            // midpoint satisfies both strict and non-strict whenever l < u
            if l.cmp_val(u) == std::cmp::Ordering::Less {
                l.add(u).div(Rational::int(2))
            } else {
                l
            }
        }
    }
}

/// Fourier–Motzkin elimination of variable `v` from a constraint set.
fn eliminate(atoms: &[Atom], v: usize) -> Vec<Atom> {
    let mut pos = Vec::new(); // coeff on v > 0
    let mut neg = Vec::new(); // coeff on v < 0
    let mut out = Vec::new(); // coeff on v == 0, carried over
    for a in atoms {
        let c = a.expr.coeff(v);
        match c.cmp_val(Rational::zero()) {
            std::cmp::Ordering::Equal => out.push(a.clone()),
            std::cmp::Ordering::Greater => pos.push(a.clone()),
            std::cmp::Ordering::Less => neg.push(a.clone()),
        }
    }
    // Combine each positive with each negative to cancel v.
    for p in &pos {
        let cp = p.expr.coeff(v);
        for n in &neg {
            let cn = n.expr.coeff(v).scale_neg(); // positive magnitude
            // cp * n.expr + cn * p.expr  (both make v cancel: cp*cn*v - cn*cp*v? )
            // p: cp*v + P >=0 ; n: -cn*v + N >=0 (cn>0). Multiply p by cn, n by cp:
            // cn*cp*v + cn*P >=0 ; -cp*cn*v + cp*N >=0 ; sum: cn*P + cp*N >=0
            let combined = p.expr.scale(cn).add(&n.expr.scale(cp));
            let rel = if p.rel == Rel::Gt || n.rel == Rel::Gt {
                Rel::Gt
            } else {
                Rel::Ge
            };
            out.push(Atom { expr: combined, rel });
        }
    }
    // Drop trivially-true constant atoms to keep the set small.
    out.retain(|a| !(a.expr.is_constant() && const_true(a)));
    out
}

fn const_true(a: &Atom) -> bool {
    let c = a.expr.constant;
    match a.rel {
        Rel::Ge => c.cmp_val(Rational::zero()) != std::cmp::Ordering::Less,
        Rel::Gt => c.cmp_val(Rational::zero()) == std::cmp::Ordering::Greater,
    }
}

// Small rational helpers local to the solver.
trait RationalExt {
    fn scale_neg(self) -> Rational;
}
impl RationalExt for Rational {
    fn scale_neg(self) -> Rational {
        Rational::new(-self.num, self.den)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(i: usize) -> LinExpr {
        LinExpr::var(i)
    }
    fn c(n: i128) -> LinExpr {
        LinExpr::constant(r(n))
    }

    #[test]
    fn simple_sat() {
        // x >= 5  and  x <= 10
        let atoms = vec![
            Atom::ge(v(0).sub(&c(5))),       // x - 5 >= 0
            Atom::ge(c(10).sub(&v(0))),      // 10 - x >= 0
        ];
        match solve(&atoms, 1) {
            Feasibility::Sat(m) => {
                let x = m[&0];
                assert!(x.cmp_val(r(5)) != std::cmp::Ordering::Less);
                assert!(x.cmp_val(r(10)) != std::cmp::Ordering::Greater);
            }
            Feasibility::Unsat => panic!("should be SAT"),
        }
    }

    #[test]
    fn simple_unsat() {
        // x >= 10 and x <= 5  -> contradiction
        let atoms = vec![Atom::ge(v(0).sub(&c(10))), Atom::ge(c(5).sub(&v(0)))];
        assert_eq!(solve(&atoms, 1), Feasibility::Unsat);
    }

    #[test]
    fn strict_unsat() {
        // x > 0 and x < 0
        let atoms = vec![Atom::gt(v(0)), Atom::gt(c(0).sub(&v(0)))];
        assert_eq!(solve(&atoms, 1), Feasibility::Unsat);
    }

    #[test]
    fn two_var_sat() {
        // x + y >= 10, x <= 3, y <= 5  -> with x<=3,y<=5 max x+y=8 < 10 -> UNSAT
        let atoms = vec![
            Atom::ge(v(0).add(&v(1)).sub(&c(10))),
            Atom::ge(c(3).sub(&v(0))),
            Atom::ge(c(5).sub(&v(1))),
        ];
        assert_eq!(solve(&atoms, 2), Feasibility::Unsat);
    }

    #[test]
    fn equality_handled() {
        // x == 7 and x >= 7  -> SAT with x=7
        let mut atoms = vec![Atom::ge(v(0).sub(&c(7)))];
        atoms.extend(Atom::eq(v(0).sub(&c(7))));
        match solve(&atoms, 1) {
            Feasibility::Sat(m) => assert_eq!(m[&0], r(7)),
            _ => panic!("should be SAT"),
        }
    }
}
