//! Indentation → nesting. Turns flat source into a tree of blocks where each
//! node owns the more-indented lines beneath it. This cleanly models the
//! reference program's nested structure (`capability` → `signature` → `input`).

use crate::lexer::{lex_line, LexError, Tok};

/// One logical line plus the block lines nested beneath it.
#[derive(Debug, Clone)]
pub struct Block {
    pub line_no: usize,
    pub indent: usize,
    pub toks: Vec<Tok>,
    pub children: Vec<Block>,
}

impl Block {
    /// The first token as an identifier keyword, if any.
    pub fn head(&self) -> Option<&str> {
        match self.toks.first() {
            Some(Tok::Ident(s)) => Some(s.as_str()),
            _ => None,
        }
    }
}

struct Raw {
    line_no: usize,
    indent: usize,
    toks: Vec<Tok>,
}

/// Parse raw source text into a forest of top-level blocks.
pub fn build_blocks(src: &str) -> Result<Vec<Block>, LexError> {
    let mut raws: Vec<Raw> = Vec::new();
    for (idx, raw_line) in src.lines().enumerate() {
        let line_no = idx + 1;
        // Strip comments: everything from an unquoted '#' to end of line.
        let content = strip_comment(raw_line);
        if content.trim().is_empty() {
            continue;
        }
        let indent = content.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let toks = lex_line(content.trim(), line_no)?;
        if toks.is_empty() {
            continue;
        }
        raws.push(Raw { line_no, indent, toks });
    }
    let mut pos = 0;
    Ok(parse_level(&raws, &mut pos, 0))
}

/// Recursively collect blocks whose indent is `>= min_indent`, nesting deeper
/// lines under the block that introduced them.
fn parse_level(raws: &[Raw], pos: &mut usize, min_indent: usize) -> Vec<Block> {
    let mut out = Vec::new();
    while *pos < raws.len() {
        let cur = &raws[*pos];
        if cur.indent < min_indent {
            break;
        }
        // This line owns everything strictly more indented than it.
        let this_indent = cur.indent;
        let line_no = cur.line_no;
        let toks = cur.toks.clone();
        *pos += 1;
        let children = parse_level(raws, pos, this_indent + 1);
        out.push(Block { line_no, indent: this_indent, toks, children });
    }
    out
}

fn strip_comment(line: &str) -> String {
    let mut in_str = false;
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        match c {
            '"' => {
                in_str = !in_str;
                out.push(c);
            }
            '#' if !in_str => break,
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nests_by_indent() {
        let src = "policy P\n    audit true\ncapability C\n    signature\n        input: x: UUID\n";
        let blocks = build_blocks(src).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].head(), Some("policy"));
        assert_eq!(blocks[0].children.len(), 1);
        assert_eq!(blocks[1].head(), Some("capability"));
        // signature is nested, with input nested under it
        assert_eq!(blocks[1].children[0].head(), Some("signature"));
        assert_eq!(blocks[1].children[0].children.len(), 1);
    }

    #[test]
    fn strips_comments_and_blanks() {
        let src = "# header\nproject X\n\n   # indented comment\nevent E\n";
        let blocks = build_blocks(src).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].head(), Some("project"));
        assert_eq!(blocks[1].head(), Some("event"));
    }
}
