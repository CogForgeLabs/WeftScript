//! Auto-parallelization must work in mixed (host-backed) programs — that is
//! how every `weft app` run executes — while comprehensions that call
//! host-provided builtins (which cannot cross threads) stay serial and correct.

use nexus_app::run_mixed;

#[test]
fn autopar_fires_for_pure_user_fn_under_host() {
    // 2000 items ≥ the 256 threshold; body calls a pure user function, so the
    // interpreter is allowed to fan this across threads. Output must be
    // correct and ordered either way.
    let src = "\
project P

fn sq(x)
    return x * x

xs = [sq(x) for x in range(2000)]
print xs[0]
print xs[1999]
print len(xs)
";
    let out = run_mixed(src).unwrap();
    assert_eq!(out, vec!["0", "3996001", "2000"]);
}

#[test]
fn host_builtin_bodies_stay_serial_and_correct() {
    // The comprehension calls `validate` (host-provided). The purity analysis
    // must refuse to parallelize it — a worker thread has no host and would
    // fail with "unknown function". Correct output proves it stayed serial.
    let src = "\
project P

thing Item
    id: UUID
    n: Decimal

fn make(i)
    return {\"id\": \"x{i}\", \"n\": i}

items = [make(i) for i in range(600)]
ok = [it for it in items if validate(it, \"Item\")]
print len(ok)
";
    let out = run_mixed(src).unwrap();
    assert_eq!(out, vec!["600"]);
}

#[test]
fn heavy_mixed_pipeline_matches_serial_reference() {
    // Same program with auto-par on (default) — results must be identical to
    // the arithmetic reference regardless of scheduling.
    let src = "\
project P

fn score(x)
    s = 0
    j = 0
    while j < 10
        s += x * j
        j += 1
    return s

scores = [score(x) for x in range(1000)]
print sum(scores)
";
    // sum over x of x * (0+1+...+9) = 45 * sum(x) = 45 * 499500
    let out = run_mixed(src).unwrap();
    assert_eq!(out, vec![(45.0_f64 * 499_500.0).to_string().replace(".0", "")]);
}
