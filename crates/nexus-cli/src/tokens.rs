//! Token-usage estimation and comparison.
//!
//! A central Nexus claim is *substantially reduced token usage* for humans and
//! LLMs. We quantify it: the LLM token count is estimated with the widely-used
//! ~4-characters-per-token heuristic, and we also report words and lines. The
//! comparison is the Nexus DSL against an equivalent traditional implementation
//! of the very same system.

/// Surface metrics of a source artifact.
#[derive(Clone, Debug)]
pub struct TokenStats {
    pub chars: usize,
    pub words: usize,
    pub lines: usize,
    pub est_tokens: usize,
}

/// Estimate LLM tokens via the ~4 chars/token rule, ignoring pure-whitespace
/// padding so indentation is not over-counted.
pub fn analyze(src: &str) -> TokenStats {
    let non_ws_chars = src.chars().filter(|c| !c.is_whitespace()).count();
    // chars/token ~ 4 over real text; count whitespace at a discount.
    let ws_chars = src.chars().filter(|c| c.is_whitespace()).count();
    let est_tokens = ((non_ws_chars as f64 + ws_chars as f64 * 0.5) / 4.0).ceil() as usize;
    TokenStats {
        chars: src.chars().count(),
        words: src.split_whitespace().count(),
        lines: src.lines().count(),
        est_tokens,
    }
}

/// A side-by-side comparison of two artifacts.
#[derive(Clone, Debug)]
pub struct Comparison {
    pub nexus: TokenStats,
    pub traditional: TokenStats,
}

impl Comparison {
    pub fn new(nexus_src: &str, traditional_src: &str) -> Comparison {
        Comparison { nexus: analyze(nexus_src), traditional: analyze(traditional_src) }
    }

    /// How many times more tokens the traditional approach costs.
    pub fn token_reduction_factor(&self) -> f64 {
        self.traditional.est_tokens as f64 / self.nexus.est_tokens.max(1) as f64
    }

    /// Percentage of tokens saved by Nexus.
    pub fn token_savings_pct(&self) -> f64 {
        100.0 * (1.0 - self.nexus.est_tokens as f64 / self.traditional.est_tokens.max(1) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_counts() {
        let s = analyze("event A\nevent B\n");
        assert_eq!(s.lines, 2);
        assert_eq!(s.words, 4);
        assert!(s.est_tokens > 0);
    }

    #[test]
    fn dsl_uses_far_fewer_tokens() {
        let dsl = nexus_dsl::ENTERPRISE_BILLING;
        let trad = include_str!("../baselines/enterprise_billing_traditional.ts");
        let cmp = Comparison::new(dsl, trad);
        // The DSL must be dramatically more compact than the traditional code.
        assert!(
            cmp.token_reduction_factor() > 2.0,
            "expected >2x reduction, got {:.2}x (dsl={}, trad={})",
            cmp.token_reduction_factor(),
            cmp.nexus.est_tokens,
            cmp.traditional.est_tokens
        );
    }
}
