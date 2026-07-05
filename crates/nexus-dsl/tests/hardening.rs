//! Regression tests for parser hardening: truncated directives and pathological
//! nesting must return clean parse errors, never panic or overflow the stack.

use nexus_dsl::parse;

/// Truncated directives (a keyword with its arguments missing) previously
/// indexed past the end of the token line and panicked. Each must now be a
/// clean parse error.
#[test]
fn truncated_directives_do_not_panic() {
    // Bare `input` inside a capability signature (no `name: Type` after it).
    let cases = [
        "capability C\n    signature\n        input\n",
        "intent I\n    input\n",
        "agent A\n    tools\n",
        "workflow W\n    step S\n        tools\n",
        "workflow W\n    step S\n        need\n",
        "workflow W\n    step S\n        fallback\n            retry\n",
    ];
    for src in cases {
        // The contract is "does not panic"; an Err is the expected, safe outcome.
        let _ = parse(src);
    }
}

/// A `thing` field with a `policy` keyword but no policy name must error, not
/// index out of bounds.
#[test]
fn field_policy_without_name_does_not_panic() {
    let src = "thing T\n    amount: UUID policy\n";
    let _ = parse(src);
}

/// A deeply nested list type must return a clean error rather than overflowing
/// the stack during parse.
#[test]
fn deeply_nested_list_type_is_a_clean_error() {
    let mut ty = String::from("UUID");
    for _ in 0..500 {
        ty = format!("[{ty}]");
    }
    let src = format!("intent I\n    input x: {ty}\n");
    let res = parse(&src);
    assert!(res.is_err(), "expected a clean error for an over-nested type");
}

/// A long run of unary minus must not overflow the stack (it is now folded in a
/// loop, not per-sign recursion).
#[test]
fn long_unary_minus_run_does_not_overflow() {
    let minuses = "-".repeat(100_000);
    // Use it in a constraint predicate expression position.
    let src = format!("constraint C\n    x == {minuses}5\n");
    // Whether it parses or errors, it must return without a stack overflow.
    let _ = parse(&src);
}
