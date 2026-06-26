//! Abstract syntax for the executable layer.

/// An expression that evaluates to a [`crate::value::Value`].
#[derive(Clone, Debug)]
pub enum AExpr {
    Num(f64),
    Str(String),
    Bool(bool),
    Nil,
    Var(String),
    List(Vec<AExpr>),
    /// `{ key: value, ... }` map literal.
    MapLit(Vec<(AExpr, AExpr)>),
    /// String interpolation: literal chunks interleaved with `{expr}` holes.
    Interp(Vec<IPart>),
    /// `[ elem for var in iter (if cond) ]` list comprehension.
    Comp {
        elem: Box<AExpr>,
        var: String,
        iter: Box<AExpr>,
        cond: Option<Box<AExpr>>,
    },
    Unary(UnOp, Box<AExpr>),
    Binary(BinOp, Box<AExpr>, Box<AExpr>),
    /// `name(args...)` — a builtin or user function call.
    Call(String, Vec<AExpr>),
    /// `recv.method(args...)` — dispatches to a class method or a builtin.
    Method(Box<AExpr>, String, Vec<AExpr>),
    /// `collection[index]`.
    Index(Box<AExpr>, Box<AExpr>),
}

/// One piece of an interpolated string.
#[derive(Clone, Debug)]
pub enum IPart {
    Lit(String),
    Expr(AExpr),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// A statement.
#[derive(Clone, Debug)]
pub enum Stmt {
    /// `x = expr` (declares or reassigns).
    Assign(String, AExpr),
    /// `base[i][j]... = expr` — a variable followed by one or more index paths.
    IndexAssign(String, Vec<AExpr>, AExpr),
    Print(AExpr),
    Return(AExpr),
    If(AExpr, Vec<Stmt>, Vec<Stmt>),
    While(AExpr, Vec<Stmt>),
    For(String, AExpr, Vec<Stmt>),
    /// A bare expression evaluated for effect.
    Expr(AExpr),
    /// `throw expr` — raise an exception value.
    Throw(AExpr),
    /// `try { body } catch e { handler }`.
    Try(Vec<Stmt>, String, Vec<Stmt>),
    /// `yield expr` — emit a value from a generator function.
    Yield(AExpr),
}

/// A user-defined function.
#[derive(Clone, Debug)]
pub struct Func {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    /// True if the body contains `yield` (an eager generator).
    pub is_generator: bool,
}

/// A class: a named collection of methods (each takes `self` first).
#[derive(Clone, Debug)]
pub struct Class {
    pub name: String,
    pub methods: Vec<Func>,
}

/// A runnable application: classes, functions, and a `main` entry block.
#[derive(Clone, Debug)]
pub struct App {
    pub name: String,
    pub funcs: Vec<Func>,
    pub classes: Vec<Class>,
    pub main: Vec<Stmt>,
}
