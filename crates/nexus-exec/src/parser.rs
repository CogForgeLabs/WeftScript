//! Parser for the executable layer: `app` / `fn` / `main` blocks, statements,
//! and a Pratt expression grammar. Reuses the indentation block tree and line
//! lexer from `nexus-dsl`, so the executable surface is the same language.

use crate::ast::*;
use nexus_dsl::block::{build_blocks, Block};
use nexus_dsl::lexer::Tok;
use nexus_core::CmpOp;

#[derive(Debug)]
pub struct ExecParseError {
    pub line: usize,
    pub msg: String,
}

impl std::fmt::Display for ExecParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exec parse error (line {}): {}", self.line, self.msg)
    }
}
impl std::error::Error for ExecParseError {}

type R<T> = Result<T, ExecParseError>;

fn err(line: usize, msg: impl Into<String>) -> ExecParseError {
    ExecParseError { line, msg: msg.into() }
}

/// Parse source text into a runnable [`App`].
///
/// Two forms are accepted:
/// * **App form** — an `app Name` block containing `fn`s and a `main`.
/// * **Script form** — no wrapper at all: top-level `fn` definitions plus bare
///   statements that run directly (the most compact form, ideal for LLMs).
pub fn parse_app(src: &str) -> R<App> {
    let blocks = build_blocks(src).map_err(|e| err(e.line, e.msg))?;
    app_from_blocks(&blocks)
}

/// Build a runnable [`App`] from a pre-built block forest. Declarative blocks
/// (see [`nexus_dsl::DECL_KEYWORDS`]) are ignored, so this works on the
/// executable half of a mixed declarative+executable file.
pub fn app_from_blocks(blocks: &[Block]) -> R<App> {
    if let Some(app_block) = blocks.iter().find(|b| b.head() == Some("app")) {
        let name = match app_block.toks.get(1) {
            Some(Tok::Ident(s)) => s.clone(),
            _ => return Err(err(app_block.line_no, "expected app name")),
        };
        let mut funcs = Vec::new();
        let mut classes = Vec::new();
        let mut main = Vec::new();
        for child in &app_block.children {
            match child.head() {
                Some("fn") => funcs.push(parse_fn(child)?),
                Some("class") => classes.push(parse_class(child)?),
                Some("main") => main = parse_stmts(&child.children)?,
                _ => return Err(err(child.line_no, "expected `fn`, `class`, or `main` inside app")),
            }
        }
        return Ok(App { name, funcs, classes, main });
    }

    // Script form: top-level fns/classes + bare statements (implicit main).
    // Declarative blocks (thing/intent/guarantee/…) are skipped here — they are
    // consumed by the declarative parser in a mixed file.
    let mut funcs = Vec::new();
    let mut classes = Vec::new();
    let mut main_blocks: Vec<Block> = Vec::new();
    for b in blocks {
        match b.head() {
            Some("fn") => funcs.push(parse_fn(b)?),
            Some("class") => classes.push(parse_class(b)?),
            Some(h) if nexus_dsl::DECL_KEYWORDS.contains(&h) => {}
            _ => main_blocks.push(b.clone()),
        }
    }
    let main = parse_stmts(&main_blocks)?;
    Ok(App { name: "main".into(), funcs, classes, main })
}

fn parse_fn(b: &Block) -> R<Func> {
    // fn name ( p1 , p2 )
    let name = match b.toks.get(1) {
        Some(Tok::Ident(s)) => s.clone(),
        _ => return Err(err(b.line_no, "expected function name")),
    };
    let mut params = Vec::new();
    let mut i = 2;
    if b.toks.get(i) == Some(&Tok::LParen) {
        i += 1;
        while let Some(t) = b.toks.get(i) {
            match t {
                Tok::RParen => break,
                Tok::Comma => {}
                Tok::Ident(p) => params.push(p.clone()),
                _ => return Err(err(b.line_no, "bad parameter list")),
            }
            i += 1;
        }
    }
    let body = parse_stmts(&b.children)?;
    let is_generator = contains_yield(&body);
    Ok(Func { name, params, body, is_generator })
}

fn contains_yield(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Yield(_) => true,
        Stmt::If(_, a, b) => contains_yield(a) || contains_yield(b),
        Stmt::While(_, b) | Stmt::For(_, _, b) => contains_yield(b),
        Stmt::Try(a, _, b) => contains_yield(a) || contains_yield(b),
        _ => false,
    })
}

fn parse_class(b: &Block) -> R<crate::ast::Class> {
    let name = match b.toks.get(1) {
        Some(Tok::Ident(s)) => s.clone(),
        _ => return Err(err(b.line_no, "expected class name")),
    };
    let mut methods = Vec::new();
    for child in &b.children {
        match child.head() {
            Some("fn") => methods.push(parse_fn(child)?),
            _ => return Err(err(child.line_no, "only `fn` methods allowed in a class")),
        }
    }
    Ok(crate::ast::Class { name, methods })
}

fn parse_stmts(blocks: &[Block]) -> R<Vec<Stmt>> {
    let mut stmts = Vec::new();
    let mut i = 0;
    while i < blocks.len() {
        let b = &blocks[i];
        match b.head() {
            Some("if") => {
                let cond = parse_expr(&b.toks[1..], b.line_no)?;
                let then = parse_stmts(&b.children)?;
                // Collect any `elif` / `else` siblings into a nested else-chain.
                let mut elifs: Vec<(AExpr, Vec<Stmt>)> = Vec::new();
                let mut els: Vec<Stmt> = Vec::new();
                while let Some(next) = blocks.get(i + 1) {
                    match next.head() {
                        Some("elif") => {
                            let c = parse_expr(&next.toks[1..], next.line_no)?;
                            let body = parse_stmts(&next.children)?;
                            elifs.push((c, body));
                            i += 1;
                        }
                        Some("else") => {
                            els = parse_stmts(&next.children)?;
                            i += 1;
                            break;
                        }
                        _ => break,
                    }
                }
                // Fold elifs from the back into nested If statements.
                let mut else_branch = els;
                for (c, body) in elifs.into_iter().rev() {
                    else_branch = vec![Stmt::If(c, body, else_branch)];
                }
                stmts.push(Stmt::If(cond, then, else_branch));
            }
            Some("else") | Some("elif") => {
                return Err(err(b.line_no, "`else`/`elif` without matching `if`"))
            }
            Some("while") => {
                let cond = parse_expr(&b.toks[1..], b.line_no)?;
                let body = parse_stmts(&b.children)?;
                stmts.push(Stmt::While(cond, body));
            }
            Some("for") => {
                // for <var> in <expr>
                let var = match b.toks.get(1) {
                    Some(Tok::Ident(s)) => s.clone(),
                    _ => return Err(err(b.line_no, "expected loop variable")),
                };
                if b.toks.get(2) != Some(&Tok::Ident("in".into())) {
                    return Err(err(b.line_no, "expected `in` in for-loop"));
                }
                let iter = parse_expr(&b.toks[3..], b.line_no)?;
                let body = parse_stmts(&b.children)?;
                stmts.push(Stmt::For(var, iter, body));
            }
            Some("print") => stmts.push(Stmt::Print(parse_expr(&b.toks[1..], b.line_no)?)),
            Some("throw") => stmts.push(Stmt::Throw(parse_expr(&b.toks[1..], b.line_no)?)),
            Some("yield") => stmts.push(Stmt::Yield(parse_expr(&b.toks[1..], b.line_no)?)),
            Some("try") => {
                let body = parse_stmts(&b.children)?;
                let next = blocks.get(i + 1);
                let (var, handler) = match next {
                    Some(n) if n.head() == Some("catch") => {
                        let var = match n.toks.get(1) {
                            Some(Tok::Ident(v)) => v.clone(),
                            None => "_e".into(),
                            _ => return Err(err(n.line_no, "expected exception variable after catch")),
                        };
                        i += 1;
                        (var, parse_stmts(&n.children)?)
                    }
                    _ => return Err(err(b.line_no, "`try` must be followed by `catch`")),
                };
                stmts.push(Stmt::Try(body, var, handler));
            }
            Some("catch") => return Err(err(b.line_no, "`catch` without matching `try`")),
            Some("return") => {
                let e = if b.toks.len() > 1 {
                    parse_expr(&b.toks[1..], b.line_no)?
                } else {
                    AExpr::Nil
                };
                stmts.push(Stmt::Return(e));
            }
            _ => stmts.push(parse_simple(b)?),
        }
        i += 1;
    }
    Ok(stmts)
}

/// Assignment or bare-expression statement.
fn parse_simple(b: &Block) -> R<Stmt> {
    // Compound assignment: `x += e`, `x -= e`, `x *= e`, `x /= e`.
    if b.toks.len() >= 3 && b.toks.get(2) == Some(&Tok::Assign) {
        if let Tok::Ident(name) = &b.toks[0] {
            let op = match b.toks[1] {
                Tok::Plus => Some(BinOp::Add),
                Tok::Minus => Some(BinOp::Sub),
                Tok::Star => Some(BinOp::Mul),
                Tok::Slash => Some(BinOp::Div),
                _ => None,
            };
            if let Some(op) = op {
                let rhs = parse_expr(&b.toks[3..], b.line_no)?;
                let combined =
                    AExpr::Binary(op, Box::new(AExpr::Var(name.clone())), Box::new(rhs));
                return Ok(Stmt::Assign(name.clone(), combined));
            }
        }
    }
    if let Some(pos) = b.toks.iter().position(|t| *t == Tok::Assign) {
        let lhs = &b.toks[..pos];
        let rhs = parse_expr(&b.toks[pos + 1..], b.line_no)?;
        // `x = expr`
        if lhs.len() == 1 {
            if let Tok::Ident(name) = &lhs[0] {
                return Ok(Stmt::Assign(name.clone(), rhs));
            }
        }
        // `base.field[i].field... = expr` — an lvalue path.
        if let Some(Tok::Ident(var)) = lhs.first() {
            let indices = parse_lvalue_path(&lhs[1..], b.line_no)?;
            if !indices.is_empty() {
                return Ok(Stmt::IndexAssign(var.clone(), indices, rhs));
            }
        }
        return Err(err(b.line_no, "invalid assignment target"));
    }
    Ok(Stmt::Expr(parse_expr(&b.toks, b.line_no)?))
}

/// Parse an lvalue suffix: a chain of `.field` (string-key) and `[expr]`
/// segments after the base variable, into a list of index expressions.
fn parse_lvalue_path(toks: &[Tok], line: usize) -> R<Vec<AExpr>> {
    let mut indices = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Dot => match toks.get(i + 1) {
                Some(Tok::Ident(field)) => {
                    indices.push(AExpr::Str(field.clone()));
                    i += 2;
                }
                _ => return Err(err(line, "expected field name after `.`")),
            },
            Tok::LBracket => {
                let mut depth = 0;
                let mut j = i;
                loop {
                    match toks.get(j) {
                        Some(Tok::LBracket) => depth += 1,
                        Some(Tok::RBracket) => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        None => return Err(err(line, "unclosed `[` in assignment target")),
                        _ => {}
                    }
                    j += 1;
                }
                indices.push(parse_expr(&toks[i + 1..j], line)?);
                i = j + 1;
            }
            _ => return Err(err(line, "invalid assignment target")),
        }
    }
    Ok(indices)
}

/// Parse one or more `[expr]` segments into a list of index expressions.
#[allow(dead_code)]
fn parse_index_path(toks: &[Tok], line: usize) -> R<Vec<AExpr>> {
    let mut indices = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if toks[i] != Tok::LBracket {
            return Err(err(line, "expected `[` in index path"));
        }
        // Find the matching `]` (no nested brackets in index expressions here).
        let mut depth = 0;
        let mut j = i;
        loop {
            match toks.get(j) {
                Some(Tok::LBracket) => depth += 1,
                Some(Tok::RBracket) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                None => return Err(err(line, "unclosed `[` in index path")),
                _ => {}
            }
            j += 1;
        }
        indices.push(parse_expr(&toks[i + 1..j], line)?);
        i = j + 1;
    }
    Ok(indices)
}

// ---- Pratt expression parser --------------------------------------------

struct Cur<'a> {
    toks: &'a [Tok],
    pos: usize,
    line: usize,
}

impl<'a> Cur<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<&Tok> {
        let t = self.toks.get(self.pos);
        self.pos += 1;
        t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
}

/// Parse a complete expression from a token slice.
pub fn parse_expr(toks: &[Tok], line: usize) -> R<AExpr> {
    let mut c = Cur { toks, pos: 0, line };
    let e = parse_bp(&mut c, 0)?;
    if c.pos != c.toks.len() {
        return Err(err(line, "trailing tokens in expression"));
    }
    Ok(e)
}

/// Binary operator binding powers (left, right).
fn binding_power(t: &Tok) -> Option<(BinOp, u8, u8)> {
    Some(match t {
        Tok::Ident(s) if s == "or" => (BinOp::Or, 1, 2),
        Tok::Ident(s) if s == "and" => (BinOp::And, 3, 4),
        Tok::Cmp(CmpOp::Eq) => (BinOp::Eq, 5, 6),
        Tok::Cmp(CmpOp::Ne) => (BinOp::Ne, 5, 6),
        Tok::Cmp(CmpOp::Lt) => (BinOp::Lt, 5, 6),
        Tok::Cmp(CmpOp::Le) => (BinOp::Le, 5, 6),
        Tok::Cmp(CmpOp::Gt) => (BinOp::Gt, 5, 6),
        Tok::Cmp(CmpOp::Ge) => (BinOp::Ge, 5, 6),
        Tok::Plus => (BinOp::Add, 7, 8),
        Tok::Minus => (BinOp::Sub, 7, 8),
        Tok::Star => (BinOp::Mul, 9, 10),
        Tok::Slash => (BinOp::Div, 9, 10),
        Tok::Percent => (BinOp::Mod, 9, 10),
        _ => return None,
    })
}

fn parse_bp(c: &mut Cur, min_bp: u8) -> R<AExpr> {
    let mut lhs = parse_prefix(c)?;
    loop {
        let t = match c.peek() {
            Some(t) => t,
            None => break,
        };
        let (op, lbp, rbp) = match binding_power(t) {
            Some(x) => x,
            None => break,
        };
        if lbp < min_bp {
            break;
        }
        c.next(); // consume operator
        let rhs = parse_bp(c, rbp)?;
        lhs = AExpr::Binary(op, Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}

fn parse_prefix(c: &mut Cur) -> R<AExpr> {
    match c.peek() {
        Some(Tok::Minus) => {
            c.next();
            Ok(AExpr::Unary(UnOp::Neg, Box::new(parse_prefix(c)?)))
        }
        Some(Tok::Ident(s)) if s == "not" => {
            c.next();
            Ok(AExpr::Unary(UnOp::Not, Box::new(parse_prefix(c)?)))
        }
        _ => parse_postfix(c),
    }
}

fn parse_postfix(c: &mut Cur) -> R<AExpr> {
    let mut e = parse_primary(c)?;
    loop {
        match c.peek() {
            Some(Tok::LBracket) => {
                c.next();
                let idx = parse_bp(c, 0)?;
                if !c.eat(&Tok::RBracket) {
                    return Err(err(c.line, "expected `]`"));
                }
                e = AExpr::Index(Box::new(e), Box::new(idx));
            }
            // Method call `recv.method(args)` or attribute access `recv.field`.
            Some(Tok::Dot) => {
                c.next();
                let method = match c.next() {
                    Some(Tok::Ident(m)) => m.clone(),
                    _ => return Err(err(c.line, "expected method or field name after `.`")),
                };
                if c.peek() == Some(&Tok::LParen) {
                    c.next();
                    let mut args = Vec::new();
                    if c.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(parse_bp(c, 0)?);
                            if c.eat(&Tok::Comma) {
                                continue;
                            }
                            break;
                        }
                    }
                    if !c.eat(&Tok::RParen) {
                        return Err(err(c.line, "expected `)` after method arguments"));
                    }
                    e = AExpr::Method(Box::new(e), method, args);
                } else {
                    // Attribute access on a map: `obj.field` == `get(obj, "field")`.
                    e = AExpr::Call("get".into(), vec![e, AExpr::Str(method)]);
                }
            }
            _ => break,
        }
    }
    Ok(e)
}

fn parse_primary(c: &mut Cur) -> R<AExpr> {
    let line = c.line;
    let t = c.next().ok_or_else(|| err(line, "expected expression"))?.clone();
    match t {
        Tok::Number(s) => Ok(AExpr::Num(s.parse().map_err(|_| err(line, "bad number"))?)),
        Tok::Money(s) => Ok(AExpr::Num(s.parse().map_err(|_| err(line, "bad number"))?)),
        Tok::Bytes(b) => Ok(AExpr::Num(b as f64)),
        Tok::Duration(d) => Ok(AExpr::Num(d as f64)),
        Tok::Str(s) => parse_str_literal(&s, line),
        Tok::LParen => {
            let e = parse_bp(c, 0)?;
            if !c.eat(&Tok::RParen) {
                return Err(err(line, "expected `)`"));
            }
            Ok(e)
        }
        Tok::LBrace => {
            // Map literal: { key: value, ... }
            let mut pairs = Vec::new();
            if c.peek() != Some(&Tok::RBrace) {
                loop {
                    let k = parse_bp(c, 0)?;
                    if !c.eat(&Tok::Colon) {
                        return Err(err(line, "expected `:` in map literal"));
                    }
                    let v = parse_bp(c, 0)?;
                    pairs.push((k, v));
                    if c.eat(&Tok::Comma) {
                        continue;
                    }
                    break;
                }
            }
            if !c.eat(&Tok::RBrace) {
                return Err(err(line, "expected `}`"));
            }
            Ok(AExpr::MapLit(pairs))
        }
        Tok::LBracket => {
            if c.peek() == Some(&Tok::RBracket) {
                c.next();
                return Ok(AExpr::List(Vec::new()));
            }
            let first = parse_bp(c, 0)?;
            // List comprehension? `[ elem for v in iter (if cond) ]`
            if c.peek() == Some(&Tok::Ident("for".into())) {
                c.next();
                let var = match c.next() {
                    Some(Tok::Ident(v)) => v.clone(),
                    _ => return Err(err(line, "expected comprehension variable")),
                };
                if c.next() != Some(&Tok::Ident("in".into())) {
                    return Err(err(line, "expected `in` in comprehension"));
                }
                let iter = parse_bp(c, 0)?;
                let cond = if c.peek() == Some(&Tok::Ident("if".into())) {
                    c.next();
                    Some(Box::new(parse_bp(c, 0)?))
                } else {
                    None
                };
                if !c.eat(&Tok::RBracket) {
                    return Err(err(line, "expected `]` to close comprehension"));
                }
                return Ok(AExpr::Comp {
                    elem: Box::new(first),
                    var,
                    iter: Box::new(iter),
                    cond,
                });
            }
            // Plain list literal.
            let mut items = vec![first];
            while c.eat(&Tok::Comma) {
                if c.peek() == Some(&Tok::RBracket) {
                    break;
                }
                items.push(parse_bp(c, 0)?);
            }
            if !c.eat(&Tok::RBracket) {
                return Err(err(line, "expected `]`"));
            }
            Ok(AExpr::List(items))
        }
        Tok::Ident(name) => match name.as_str() {
            "true" => Ok(AExpr::Bool(true)),
            "false" => Ok(AExpr::Bool(false)),
            "nil" => Ok(AExpr::Nil),
            _ => {
                if c.peek() == Some(&Tok::LParen) {
                    c.next();
                    let mut args = Vec::new();
                    if c.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(parse_bp(c, 0)?);
                            if c.eat(&Tok::Comma) {
                                continue;
                            }
                            break;
                        }
                    }
                    if !c.eat(&Tok::RParen) {
                        return Err(err(line, "expected `)` after arguments"));
                    }
                    Ok(AExpr::Call(name, args))
                } else {
                    Ok(AExpr::Var(name))
                }
            }
        },
        other => Err(err(line, format!("unexpected token in expression: {:?}", other))),
    }
}

/// Parse a string literal, recognizing `{expr}` interpolation holes. `{{` and
/// `}}` are literal braces. A string with no holes yields a plain `Str`.
fn parse_str_literal(s: &str, line: usize) -> R<AExpr> {
    let chars: Vec<char> = s.chars().collect();
    let mut parts: Vec<IPart> = Vec::new();
    let mut lit = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => {
                lit.push('{');
                i += 2;
            }
            '}' if chars.get(i + 1) == Some(&'}') => {
                lit.push('}');
                i += 2;
            }
            '{' => {
                if !lit.is_empty() {
                    parts.push(IPart::Lit(std::mem::take(&mut lit)));
                }
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '}' {
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(err(line, "unterminated `{` in interpolated string"));
                }
                let expr_text: String = chars[start..i].iter().collect();
                i += 1; // consume '}'
                let toks = nexus_dsl::lexer::lex_line(&expr_text, line)
                    .map_err(|e| err(line, e.msg))?;
                parts.push(IPart::Expr(parse_expr(&toks, line)?));
            }
            other => {
                lit.push(other);
                i += 1;
            }
        }
    }
    if !lit.is_empty() || parts.is_empty() {
        parts.push(IPart::Lit(lit));
    }
    // Pure literal — no interpolation.
    if parts.len() == 1 {
        if let IPart::Lit(s) = &parts[0] {
            return Ok(AExpr::Str(s.clone()));
        }
    }
    Ok(AExpr::Interp(parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_app() {
        let src = "app T\n    fn add(a, b)\n        return a + b\n    main\n        print add(2, 3)\n";
        let app = parse_app(src).unwrap();
        assert_eq!(app.name, "T");
        assert_eq!(app.funcs.len(), 1);
        assert_eq!(app.funcs[0].params, vec!["a", "b"]);
        assert_eq!(app.main.len(), 1);
    }

    #[test]
    fn parses_control_flow() {
        let src = "app T\n    main\n        x = 5\n        if x > 3\n            print 1\n        else\n            print 2\n";
        let app = parse_app(src).unwrap();
        assert_eq!(app.main.len(), 2);
        assert!(matches!(app.main[1], Stmt::If(_, _, _)));
    }
}
