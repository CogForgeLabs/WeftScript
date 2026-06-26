//! End-to-end test: the full enterprise reference program must parse, lower to
//! the content-addressed graph, and round-trip its structure exactly.

use nexus_core::*;
use nexus_dsl::{parse, ENTERPRISE_BILLING};

#[test]
fn parses_full_reference_program() {
    let prog = parse(ENTERPRISE_BILLING).expect("reference program must parse");
    assert_eq!(prog.project, "EnterpriseBillingHub");

    // Counts of each primitive declared in the reference program.
    let count = |k: &str| prog.nodes.iter().filter(|n| n.kind() == k).count();
    assert_eq!(count("policy"), 2);
    assert_eq!(count("thing"), 3);
    assert_eq!(count("capability"), 1);
    assert_eq!(count("contract"), 1);
    assert_eq!(count("constraint"), 2);
    assert_eq!(count("intent"), 1);
    assert_eq!(count("event"), 3);
    assert_eq!(count("agent"), 2);
    assert_eq!(count("workflow"), 1);
    assert_eq!(prog.directives.len(), 4);
}

#[test]
fn lowers_to_store_with_edges() {
    let prog = parse(ENTERPRISE_BILLING).unwrap();
    let store = prog.to_store();

    // Customer.tax_identifier references FinancialCompliance policy.
    let cust = store.by_name("thing", "Customer").unwrap();
    if let Node::Thing(t) = cust {
        let f = t.fields.iter().find(|f| f.name == "tax_identifier").unwrap();
        assert_eq!(f.policy.as_deref(), Some("FinancialCompliance"));
    } else {
        panic!("expected thing");
    }

    // The workflow should wire edges to its intents, agent, contract, need.
    let labels: Vec<&str> = store.edges().iter().map(|e| e.label.as_str()).collect();
    assert!(labels.contains(&"step_intent"));
    assert!(labels.contains(&"step_agent"));
    assert!(labels.contains(&"step_contract"));
    assert!(labels.contains(&"step_need"));
    assert!(labels.contains(&"field_policy"));
}

#[test]
fn contract_limits_parsed_with_units() {
    let prog = parse(ENTERPRISE_BILLING).unwrap();
    let c = prog.nodes.iter().find_map(|n| match n {
        Node::Contract(c) if c.name == "HighPrioritySLA" => Some(c),
        _ => None,
    }).unwrap();
    let get = |k: &str| c.limits.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    assert_eq!(get("max_memory"), Some(Literal::Bytes(2 * 1024 * 1024 * 1024)));
    assert_eq!(get("execution_timeout"), Some(Literal::Duration(1500)));
    assert_eq!(get("financial_budget"), Some(Literal::Money(Rational::new(25, 1000))));
    assert_eq!(get("max_cpu_cores"), Some(Literal::Number(Rational::new(3, 2))));
}

#[test]
fn workflow_steps_and_fallbacks() {
    let prog = parse(ENTERPRISE_BILLING).unwrap();
    let wf = prog.nodes.iter().find_map(|n| match n {
        Node::Workflow(w) => Some(w),
        _ => None,
    }).unwrap();
    assert_eq!(wf.trigger, "Event.InvoiceCreated");
    assert_eq!(wf.steps.len(), 3);

    let charge = &wf.steps[1];
    assert_eq!(charge.name, "ChargeAccount");
    assert_eq!(charge.need.as_deref(), Some("CardProcessor"));
    assert_eq!(charge.contract.as_deref(), Some("HighPrioritySLA"));
    let fb = charge.fallback.as_ref().unwrap();
    assert_eq!(fb.retry, Some(3));
    assert_eq!(fb.backoff.as_deref(), Some("exponential"));
    assert_eq!(fb.on_failure.as_deref(), Some("DispatchAlert"));
}

#[test]
fn parses_formal_guarantee() {
    let prog = parse(ENTERPRISE_BILLING).unwrap();
    assert_eq!(prog.guarantees.len(), 1);
    let g = &prog.guarantees[0];
    assert_eq!(g.name, "NoOverdraft");
    assert_eq!(g.assumptions.len(), 3);
    // ensure: balance >= minimum_reserve
    assert_eq!(g.ensure.op, CmpOp::Ge);
}

#[test]
fn deterministic_lowering_is_stable() {
    // Parsing twice yields an identical graph (same root multiset of CIDs):
    // content addressing makes the build reproducible.
    let a = parse(ENTERPRISE_BILLING).unwrap().to_store();
    let b = parse(ENTERPRISE_BILLING).unwrap().to_store();
    let mut ca: Vec<String> = a.iter().map(|(c, _)| c.to_hex()).collect();
    let mut cb: Vec<String> = b.iter().map(|(c, _)| c.to_hex()).collect();
    ca.sort();
    cb.sort();
    assert_eq!(ca, cb);
    assert_eq!(a.len(), b.len());
}
