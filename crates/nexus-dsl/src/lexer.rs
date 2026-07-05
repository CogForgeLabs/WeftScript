//! Line-oriented lexer.
//!
//! The Nexus DSL is indentation-structured (like the reference program), so we
//! lex one logical line at a time into a token vector and let the block builder
//! handle nesting. Dotted identifiers (`trust.reviewed`), money (`$0.025`),
//! byte sizes (`2GB`), and durations (`1500ms`) are recognized as single
//! tokens to keep the grammar — and the token count an LLM must emit — small.

use nexus_core::CmpOp;

#[derive(Clone, PartialEq, Debug)]
pub enum Tok {
    /// Identifier or keyword, including dotted paths like `Event.InvoiceCreated`.
    Ident(String),
    Str(String),
    /// A raw numeric literal text, e.g. "0.90" or "1.5".
    Number(String),
    /// A monetary literal's numeric text, e.g. `$0.025` -> "0.025".
    Money(String),
    /// A byte quantity already normalized to bytes, e.g. `2GB`.
    Bytes(u64),
    /// A duration already normalized to milliseconds, e.g. `1500ms`.
    Duration(u64),
    Colon,
    Arrow,    // ->
    FatArrow, // =>
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Cmp(CmpOp),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LParen,
    RParen,
    /// `=` assignment (distinct from `==` comparison).
    Assign,
    /// `.` member/method access.
    Dot,
}

#[derive(Debug)]
pub struct LexError {
    pub line: usize,
    pub msg: String,
}

/// Lex a single line's text (comments already stripped) into tokens.
pub fn lex_line(text: &str, line_no: usize) -> Result<Vec<Tok>, LexError> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    let err = |msg: String| LexError { line: line_no, msg };

    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\r' => {
                i += 1;
            }
            '[' => {
                out.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                out.push(Tok::RBracket);
                i += 1;
            }
            '{' => {
                out.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                out.push(Tok::RBrace);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            ':' => {
                out.push(Tok::Colon);
                i += 1;
            }
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '*' => {
                out.push(Tok::Star);
                i += 1;
            }
            '/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            '%' => {
                out.push(Tok::Percent);
                i += 1;
            }
            '.' => {
                out.push(Tok::Dot);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            '-' => {
                if chars.get(i + 1) == Some(&'>') {
                    out.push(Tok::Arrow);
                    i += 2;
                } else {
                    out.push(Tok::Minus);
                    i += 1;
                }
            }
            '=' => {
                if chars.get(i + 1) == Some(&'>') {
                    out.push(Tok::FatArrow);
                    i += 2;
                } else if chars.get(i + 1) == Some(&'=') {
                    out.push(Tok::Cmp(CmpOp::Eq));
                    i += 2;
                } else {
                    out.push(Tok::Assign);
                    i += 1;
                }
            }
            '!' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Tok::Cmp(CmpOp::Ne));
                    i += 2;
                } else {
                    return Err(err("unexpected '!'".into()));
                }
            }
            '<' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Tok::Cmp(CmpOp::Le));
                    i += 2;
                } else {
                    out.push(Tok::Cmp(CmpOp::Lt));
                    i += 1;
                }
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Tok::Cmp(CmpOp::Ge));
                    i += 2;
                } else {
                    out.push(Tok::Cmp(CmpOp::Gt));
                    i += 1;
                }
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        // Escape sequences.
                        let esc = chars[i + 1];
                        s.push(match esc {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '"' => '"',
                            '\\' => '\\',
                            other => other,
                        });
                        i += 2;
                        continue;
                    }
                    s.push(chars[i]);
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(err("unterminated string".into()));
                }
                i += 1; // closing quote
                out.push(Tok::Str(s));
            }
            '$' => {
                i += 1;
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                if i == start {
                    return Err(err("expected number after '$'".into()));
                }
                out.push(Tok::Money(chars[start..i].iter().collect()));
            }
            d if d.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                // Scientific notation: `1e15`, `2.5E-3` — an `e`/`E` followed by
                // an optionally-signed digit run extends the number literal.
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    let mut j = i + 1;
                    if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                        j += 1;
                    }
                    if j < chars.len() && chars[j].is_ascii_digit() {
                        i = j;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        out.push(Tok::Number(chars[start..i].iter().collect()));
                        continue;
                    }
                }
                let num: String = chars[start..i].iter().collect();
                // Optional unit suffix immediately following the digits.
                let suffix_start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let suffix: String = chars[suffix_start..i].iter().collect();
                out.push(classify_number(&num, &suffix).map_err(err)?);
            }
            c if is_ident_start(c) => {
                let start = i;
                while i < chars.len() && is_ident_part(chars[i]) {
                    i += 1;
                }
                out.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            other => {
                return Err(err(format!("unexpected character '{}'", other)));
            }
        }
    }
    Ok(out)
}

fn classify_number(num: &str, suffix: &str) -> Result<Tok, String> {
    if suffix.is_empty() {
        return Ok(Tok::Number(num.to_string()));
    }
    let whole: u64 = num.parse().map_err(|_| {
        format!("unit suffix '{}' requires an integer, got '{}'", suffix, num)
    })?;
    match suffix {
        "B" => Ok(Tok::Bytes(whole)),
        "KB" => Ok(Tok::Bytes(whole * 1024)),
        "MB" => Ok(Tok::Bytes(whole * 1024 * 1024)),
        "GB" => Ok(Tok::Bytes(whole * 1024 * 1024 * 1024)),
        "TB" => Ok(Tok::Bytes(whole * 1024 * 1024 * 1024 * 1024)),
        "ms" => Ok(Tok::Duration(whole)),
        "s" => Ok(Tok::Duration(whole * 1000)),
        "m" => Ok(Tok::Duration(whole * 60_000)),
        other => Err(format!("unknown unit suffix '{}'", other)),
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_directive_arrow() {
        let t = lex_line("restrict write -> role.AccountingSystem", 1).unwrap();
        assert_eq!(
            t,
            vec![
                Tok::Ident("restrict".into()),
                Tok::Ident("write".into()),
                Tok::Arrow,
                Tok::Ident("role".into()),
                Tok::Dot,
                Tok::Ident("AccountingSystem".into()),
            ]
        );
    }

    #[test]
    fn lexes_units_and_money() {
        assert_eq!(lex_line("max_memory: 2GB", 1).unwrap()[2], Tok::Bytes(2 * 1024 * 1024 * 1024));
        assert_eq!(lex_line("t: 1500ms", 1).unwrap()[2], Tok::Duration(1500));
        assert_eq!(lex_line("b: $0.025", 1).unwrap()[2], Tok::Money("0.025".into()));
    }

    #[test]
    fn lexes_scientific_notation() {
        assert_eq!(lex_line("1e15", 1).unwrap(), vec![Tok::Number("1e15".into())]);
        assert_eq!(lex_line("2.5E-3", 1).unwrap(), vec![Tok::Number("2.5E-3".into())]);
        assert_eq!(lex_line("x = 6.02e+23", 1).unwrap()[2], Tok::Number("6.02e+23".into()));
        // A bare `e` after digits is still a unit-suffix error, not an exponent.
        assert!(lex_line("3e", 1).is_err());
        // Unit suffixes are unaffected.
        assert_eq!(lex_line("t: 1500ms", 1).unwrap()[2], Tok::Duration(1500));
    }

    #[test]
    fn lexes_comparison() {
        let t = lex_line("system_load < 0.90", 1).unwrap();
        assert_eq!(t[1], Tok::Cmp(CmpOp::Lt));
        assert_eq!(t[2], Tok::Number("0.90".into()));
    }

    #[test]
    fn lexes_list() {
        let t = lex_line("tools: [A, B]", 1).unwrap();
        assert_eq!(t[0], Tok::Ident("tools".into()));
        assert_eq!(t[2], Tok::LBracket);
        assert_eq!(t[4], Tok::Comma);
    }
}
