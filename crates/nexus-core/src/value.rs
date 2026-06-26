//! The Nexus value, type, and expression model.
//!
//! Numbers are exact rationals (i128/i128) rather than floats so that content
//! addressing is deterministic and the SMT verifier reasons over exact linear
//! arithmetic with no rounding error.

use crate::canonical::{Canonical, CanonicalWriter};
use serde::{Deserialize, Serialize};
use std::fmt;

/// An exact rational number `num/den`, kept normalized with `den > 0`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rational {
    pub num: i128,
    pub den: i128,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.max(1)
}

impl Rational {
    pub fn new(num: i128, den: i128) -> Rational {
        assert!(den != 0, "rational denominator must be non-zero");
        let sign = if den < 0 { -1 } else { 1 };
        let num = num * sign;
        let den = den * sign;
        let g = gcd(num, den);
        Rational { num: num / g, den: den / g }
    }

    pub fn int(n: i128) -> Rational {
        Rational { num: n, den: 1 }
    }

    pub fn zero() -> Rational {
        Rational { num: 0, den: 1 }
    }

    /// Parse a decimal string like "0.025" or "5" into an exact rational.
    pub fn parse_decimal(s: &str) -> Option<Rational> {
        let neg = s.starts_with('-');
        let s = s.trim_start_matches('-');
        let (int_part, frac_part) = match s.split_once('.') {
            Some((a, b)) => (a, b),
            None => (s, ""),
        };
        if int_part.is_empty() && frac_part.is_empty() {
            return None;
        }
        let mut num: i128 = if int_part.is_empty() {
            0
        } else {
            int_part.parse().ok()?
        };
        let mut den: i128 = 1;
        for c in frac_part.chars() {
            let d = c.to_digit(10)? as i128;
            num = num.checked_mul(10)?.checked_add(d)?;
            den = den.checked_mul(10)?;
        }
        let r = Rational::new(num, den);
        Some(if neg { Rational::new(-r.num, r.den) } else { r })
    }

    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    pub fn add(self, o: Rational) -> Rational {
        Rational::new(self.num * o.den + o.num * self.den, self.den * o.den)
    }
    pub fn sub(self, o: Rational) -> Rational {
        Rational::new(self.num * o.den - o.num * self.den, self.den * o.den)
    }
    pub fn mul(self, o: Rational) -> Rational {
        Rational::new(self.num * o.num, self.den * o.den)
    }
    pub fn div(self, o: Rational) -> Rational {
        Rational::new(self.num * o.den, self.den * o.num)
    }
    pub fn cmp_val(self, o: Rational) -> std::cmp::Ordering {
        (self.num * o.den).cmp(&(o.num * self.den))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

impl fmt::Debug for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Canonical for Rational {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.tag(0x01);
        w.buf_i128(self.num);
        w.buf_i128(self.den);
    }
}

/// The Nexus type system from the DSL grammar.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Ty {
    String,
    Decimal,
    Uuid,
    Timestamp,
    Bool,
    List(Box<Ty>),
    /// A reference to a user-defined `thing` or interface.
    Named(String),
}

impl Ty {
    pub fn parse(s: &str) -> Ty {
        match s {
            "String" => Ty::String,
            "Decimal" => Ty::Decimal,
            "UUID" => Ty::Uuid,
            "Timestamp" => Ty::Timestamp,
            "Bool" | "Boolean" => Ty::Bool,
            other => {
                if let Some(inner) = other.strip_prefix("List[").and_then(|x| x.strip_suffix(']')) {
                    Ty::List(Box::new(Ty::parse(inner)))
                } else {
                    Ty::Named(other.to_string())
                }
            }
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::String => write!(f, "String"),
            Ty::Decimal => write!(f, "Decimal"),
            Ty::Uuid => write!(f, "UUID"),
            Ty::Timestamp => write!(f, "Timestamp"),
            Ty::Bool => write!(f, "Bool"),
            Ty::List(t) => write!(f, "List[{}]", t),
            Ty::Named(n) => write!(f, "{}", n),
        }
    }
}

impl Canonical for Ty {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.tag(0x10);
        w.str(&self.to_string());
    }
}

/// A literal value appearing in constraints, contracts, and policies.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Literal {
    Number(Rational),
    /// A monetary amount, stored as exact dollars (e.g. `$0.025`).
    Money(Rational),
    /// A byte quantity (e.g. `2GB` -> 2_147_483_648).
    Bytes(u64),
    /// A duration in milliseconds (e.g. `1500ms`).
    Duration(u64),
    Str(String),
    Bool(bool),
    /// A qualified identifier such as `trust.reviewed` or `role.Accounting`.
    Ident(String),
    List(Vec<Literal>),
}

impl Canonical for Literal {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        match self {
            Literal::Number(r) => {
                w.tag(0x20);
                r.write_canonical(w);
            }
            Literal::Money(r) => {
                w.tag(0x21);
                r.write_canonical(w);
            }
            Literal::Bytes(b) => {
                w.tag(0x22).u64(*b);
            }
            Literal::Duration(d) => {
                w.tag(0x23).u64(*d);
            }
            Literal::Str(s) => {
                w.tag(0x24).str(s);
            }
            Literal::Bool(b) => {
                w.tag(0x25).u64(*b as u64);
            }
            Literal::Ident(s) => {
                w.tag(0x26).str(s);
            }
            Literal::List(items) => {
                w.tag(0x27).seq(items.len());
                for it in items {
                    it.write_canonical(w);
                }
            }
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Number(r) => write!(f, "{}", r),
            Literal::Money(r) => write!(f, "${}", r.to_f64()),
            Literal::Bytes(b) => write!(f, "{}B", b),
            Literal::Duration(d) => write!(f, "{}ms", d),
            Literal::Str(s) => write!(f, "\"{}\"", s),
            Literal::Bool(b) => write!(f, "{}", b),
            Literal::Ident(s) => write!(f, "{}", s),
            Literal::List(items) => {
                write!(f, "[")?;
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", it)?;
                }
                write!(f, "]")
            }
        }
    }
}

/// Arithmetic binary operators usable inside constraint expressions.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Comparison operators for constraint predicates.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl CmpOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
        }
    }
}

/// An arithmetic expression over variables and literals.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Expr {
    /// A (possibly dotted) variable path, e.g. `p.balance` or `system_load`.
    Var(String),
    Lit(Literal),
    Bin(BinOp, Box<Expr>, Box<Expr>),
}

impl Canonical for Expr {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        match self {
            Expr::Var(s) => {
                w.tag(0x30).str(s);
            }
            Expr::Lit(l) => {
                w.tag(0x31);
                l.write_canonical(w);
            }
            Expr::Bin(op, a, b) => {
                w.tag(0x32).u64(*op as u64);
                a.write_canonical(w);
                b.write_canonical(w);
            }
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Var(s) => write!(f, "{}", s),
            Expr::Lit(l) => write!(f, "{}", l),
            Expr::Bin(op, a, b) => {
                let o = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                };
                write!(f, "({} {} {})", a, o, b)
            }
        }
    }
}

/// A comparison predicate `lhs op rhs`, the atom of constraints and guarantees.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Predicate {
    pub lhs: Expr,
    pub op: CmpOp,
    pub rhs: Expr,
}

impl Canonical for Predicate {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.tag(0x40).u64(self.op as u64);
        self.lhs.write_canonical(w);
        self.rhs.write_canonical(w);
    }
}

impl fmt::Display for Predicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.lhs, self.op.as_str(), self.rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_normalizes() {
        assert_eq!(Rational::new(2, 4), Rational::new(1, 2));
        assert_eq!(Rational::new(-1, -2), Rational::new(1, 2));
    }

    #[test]
    fn parse_decimal_exact() {
        assert_eq!(Rational::parse_decimal("0.025"), Some(Rational::new(25, 1000)));
        assert_eq!(Rational::parse_decimal("5"), Some(Rational::int(5)));
        assert_eq!(Rational::parse_decimal("0.90"), Some(Rational::new(9, 10)));
    }

    #[test]
    fn rational_arithmetic() {
        let a = Rational::new(1, 3);
        let b = Rational::new(1, 6);
        assert_eq!(a.add(b), Rational::new(1, 2));
        assert_eq!(a.sub(b), Rational::new(1, 6));
        assert_eq!(a.mul(b), Rational::new(1, 18));
    }

    #[test]
    fn type_parsing() {
        assert_eq!(Ty::parse("UUID"), Ty::Uuid);
        assert_eq!(Ty::parse("List[String]"), Ty::List(Box::new(Ty::String)));
        assert_eq!(Ty::parse("Customer"), Ty::Named("Customer".into()));
    }
}
