//! Integration tests exercising real cross-domain programs end-to-end.

use nexus_exec::{run_guarded, run_source, Limits};
use std::time::Duration;

#[test]
fn finance_loan_payment() {
    let src = "\
app L
    fn payment(p, ar, m)
        r = ar / 12
        f = pow(1 + r, m)
        return p * r * f / (f - 1)
    main
        print round(payment(250000, 0.065, 360), 2)
";
    assert_eq!(run_source(src).unwrap(), vec!["1580.17"]);
}

#[test]
fn text_word_frequency() {
    let src = "\
app W
    main
        words = split(lower(\"the fox the dog the\"), \" \")
        counts = map()
        for w in words
            counts = set(counts, w, get(counts, w, 0) + 1)
        print get(counts, \"the\")
        print len(keys(counts))
";
    assert_eq!(run_source(src).unwrap(), vec!["3", "3"]);
}

#[test]
fn science_uses_trig() {
    let src = "\
app P
    main
        rad = 45 * pi() / 180
        print round(cos(rad) * sin(rad), 4)
";
    assert_eq!(run_source(src).unwrap(), vec!["0.5"]);
}

#[test]
fn productivity_maps_in_lists() {
    let src = "\
app T
    fn add(ts, name, done)
        t = map()
        t = set(t, \"name\", name)
        t = set(t, \"done\", done)
        return push(ts, t)
    main
        tasks = []
        tasks = add(tasks, \"a\", false)
        tasks = add(tasks, \"b\", true)
        done = 0
        for t in tasks
            if get(t, \"done\")
                done = done + 1
        print done
        print len(tasks)
";
    assert_eq!(run_source(src).unwrap(), vec!["1", "2"]);
}

#[test]
fn control_flow_and_recursion() {
    let src = "\
app R
    fn fib(n)
        if n < 2
            return n
        return fib(n - 1) + fib(n - 2)
    main
        print fib(10)
        total = 0
        for i in range(1, 5)
            total = total + i
        print total
";
    assert_eq!(run_source(src).unwrap(), vec!["55", "10"]);
}

#[test]
fn ergonomics_interpolation_and_methods() {
    // script form, interpolation, method chaining, compound assign
    let out = run_source("x = 5\nx += 3\nprint \"x={x} y={x * 2}\"\nprint \"A,B\".lower().split(\",\")").unwrap();
    assert_eq!(out, vec!["x=8 y=16", "[a, b]"]);
}

#[test]
fn ergonomics_comprehension_and_maplit() {
    let out = run_source(
        "sq = [i * i for i in range(1, 5)]\nprint sq\nm = {\"a\": 1, \"b\": 2}\nprint m.b\nprint sum([n for n in range(11) if n % 2 == 0])",
    )
    .unwrap();
    assert_eq!(out, vec!["[1, 4, 9, 16]", "2", "30"]);
}

#[test]
fn ergonomics_elif_chain() {
    let prog = "x = 2\nif x == 1\n    print \"one\"\nelif x == 2\n    print \"two\"\nelse\n    print \"many\"";
    assert_eq!(run_source(prog).unwrap(), vec!["two"]);
}

#[test]
fn algorithm_quicksort() {
    let src = "fn qs(a)\n    if len(a) <= 1\n        return a\n    p = a[0]\n    r = slice(a, 1, len(a))\n    return qs([x for x in r if x < p]) + [p] + qs([x for x in r if x >= p])\nprint qs([5, 2, 9, 1, 5, 3])";
    assert_eq!(run_source(src).unwrap(), vec!["[1, 2, 3, 5, 5, 9]"]);
}

#[test]
fn algorithm_2d_dp_knapsack() {
    let src = "\
fn knapsack(w, v, cap)
    n = len(w)
    dp = [[0 for c in range(cap + 1)] for i in range(n + 1)]
    for i in range(1, n + 1)
        for c in range(cap + 1)
            dp[i][c] = dp[i - 1][c]
            if w[i - 1] <= c
                take = v[i - 1] + dp[i - 1][c - w[i - 1]]
                if take > dp[i][c]
                    dp[i][c] = take
    return dp[n][cap]
print knapsack([1, 3, 4, 5], [1, 4, 5, 7], 7)";
    assert_eq!(run_source(src).unwrap(), vec!["9"]);
}

#[test]
fn algorithm_caesar_roundtrip() {
    let src = "\
fn sh(c, k)
    code = ord(c)
    if code >= 65 and code <= 90
        return chr(65 + (code - 65 + k) % 26)
    return c
fn caesar(s, k)
    return join([sh(c, k) for c in s], \"\")
e = caesar(\"HELLO\", 3)
print e
print caesar(e, 23)";
    assert_eq!(run_source(src).unwrap(), vec!["KHOOR", "HELLO"]);
}

#[test]
fn vectorized_matmul_cell() {
    // SIMD dot product used as a matrix-multiply cell.
    let src = "print vdot([1, 2], [7, 7])";
    assert_eq!(run_source(src).unwrap(), vec!["21"]);
}

#[test]
fn classes_methods_and_self() {
    let src = "\
class Point
    fn init(self, x, y)
        self.x = x
        self.y = y
    fn dist(self)
        return sqrt(self.x * self.x + self.y * self.y)
p = Point(3, 4)
print p.dist()
print p.x";
    assert_eq!(run_source(src).unwrap(), vec!["5", "3"]);
}

#[test]
fn exceptions_catch_throw_and_runtime() {
    let thrown = "\
try
    throw \"boom\"
catch e
    print e";
    assert_eq!(run_source(thrown).unwrap(), vec!["boom"]);
    let runtime = "\
try
    x = [1]
    print x[5]
catch e
    print \"caught\"";
    assert_eq!(run_source(runtime).unwrap(), vec!["caught"]);
}

#[test]
fn generators_print_materializes_to_list() {
    // Printing a (finite) generator drains and renders it as a list.
    let src = "\
fn evens(n)
    i = 0
    while i < n
        yield i * 2
        i += 1
print evens(4)";
    assert_eq!(run_source(src).unwrap(), vec!["[0, 2, 4, 6]"]);
}

#[test]
fn streaming_infinite_generator_with_take() {
    // An INFINITE generator is fine because the stream is lazy — `take` pulls a
    // finite prefix without the producer running away.
    let src = "\
fn naturals()
    i = 0
    while true
        yield i
        i += 1
print take(naturals(), 5)
print sum(take(naturals(), 100))";
    // sum 0..99 = 4950
    assert_eq!(run_source(src).unwrap(), vec!["[0, 1, 2, 3, 4]", "4950"]);
}

#[test]
fn streaming_for_loop_pulls_lazily() {
    let src = "\
fn count_to(n)
    i = 0
    while i < n
        yield i
        i += 1
total = 0
for x in count_to(6)
    total = total + x
print total
print collect(count_to(4))";
    assert_eq!(run_source(src).unwrap(), vec!["15", "[0, 1, 2, 3]"]);
}

#[test]
fn streaming_pipeline_lazy_transform() {
    // Generators compose: stream -> take -> comprehension transform.
    let src = "\
fn squares()
    i = 1
    while true
        yield i * i
        i += 1
print [x + 1 for x in take(squares(), 5)]";
    assert_eq!(run_source(src).unwrap(), vec!["[2, 5, 10, 17, 26]"]);
}

#[test]
fn parallel_pmap_preserves_order() {
    // pmap fans a 1-arg function across threads; results stay in input order.
    let src = "\
fn sq(x)
    return x * x
print pmap(\"sq\", [1, 2, 3, 4, 5])";
    assert_eq!(run_source(src).unwrap(), vec!["[1, 4, 9, 16, 25]"]);
}

#[test]
fn parallel_runs_named_functions_concurrently() {
    let src = "\
fn a()
    return 10
fn b()
    return 20
fn c()
    return 30
r = parallel([\"a\", \"b\", \"c\"])
print r
print sum(r)";
    assert_eq!(run_source(src).unwrap(), vec!["[10, 20, 30]", "60"]);
}

#[test]
fn transparent_auto_parallel_comprehension_is_correct() {
    // A large comprehension that calls a user function is auto-parallelized by
    // the interpreter (no pmap/amap in sight). The result MUST equal the serial
    // result exactly — order preserved, every element correct.
    let src = "\
fn work(x)
    return x * x + 1
xs = [work(i) for i in range(1000)]
print sum(xs)
print len(xs)
print xs[0]
print xs[999]";
    // sum(i*i+1, 0..999) = 332833500 + 1000 = 332834500
    assert_eq!(run_source(src).unwrap(), vec!["332834500", "1000", "1", "998002"]);
}

#[test]
fn auto_parallel_with_filter_preserves_order() {
    let src = "\
fn keep(x)
    return x % 3 == 0
fn sq(x)
    return x * x
ys = [sq(i) for i in range(600) if keep(i)]
print len(ys)
print first(ys)
print last(ys)";
    // multiples of 3 in 0..599: 0,3,...,597 -> 200 of them; first 0, last 597^2=356409
    assert_eq!(run_source(src).unwrap(), vec!["200", "0", "356409"]);
}

#[test]
fn auto_parallel_skips_impure_bodies() {
    // A comprehension whose body has a SIDE EFFECT (printing) must stay serial,
    // so the emitted lines are in deterministic order — never scrambled across
    // threads. This is the safety guarantee of the purity analysis.
    let src = "\
fn emit(x)
    print x
    return x
done = [emit(i) for i in range(300)]
print \"count={len(done)}\"";
    let out = run_source(src).unwrap();
    assert_eq!(out.len(), 301);
    // The 300 emitted lines are strictly in order 0..299.
    for (i, line) in out.iter().take(300).enumerate() {
        assert_eq!(line, &i.to_string(), "line {i} out of order");
    }
    assert_eq!(out[300], "count=300");
}

#[test]
fn auto_map_small_and_large() {
    // Small input → serial path; large input → parallel path. Same result.
    let small = "fn sq(x)\n    return x * x\nprint amap(\"sq\", [1, 2, 3, 4, 5])";
    assert_eq!(run_source(small).unwrap(), vec!["[1, 4, 9, 16, 25]"]);
    let large = "fn dbl(x)\n    return x * 2\nprint sum(amap(\"dbl\", range(200)))";
    // sum(2*0..199) = 2 * (199*200/2) = 39800
    assert_eq!(run_source(large).unwrap(), vec!["39800"]);
}

#[test]
fn parallel_propagates_worker_errors() {
    let src = "\
fn boom(x)
    return x / 0
print pmap(\"boom\", [1, 2, 3])";
    assert!(run_source(src).is_err());
}

#[test]
fn tool_automation_runs_shell_from_dsl() {
    // A Nexus program drives the terminal: run a command, branch on its result.
    let src = "\
r = sh(\"echo nexus-was-here\")
if r.ok\n    print contains(r.stdout, \"nexus-was-here\")\nelse\n    print \"failed\"";
    assert_eq!(run_source(src).unwrap(), vec!["true"]);
}

#[test]
fn tool_native_interpolation_as_command_template() {
    // The language's own f-strings are the idiomatic command template engine.
    let src = "\
word = \"hi\"
r = sh(\"echo {word}\")
print contains(r.stdout, \"hi\")";
    assert_eq!(run_source(src).unwrap(), vec!["true"]);
}

#[test]
fn tool_template_and_parallel_from_dsl() {
    // Declarative deferred command generation (<key>) + parallel offload.
    let src = "\
p = {\"word\": \"hi\"}
r = tool_run(\"echo <word>\", p)
print r.ok
outs = psh([\"echo one\", \"echo two\"])
print len(outs)";
    assert_eq!(run_source(src).unwrap(), vec!["true", "2"]);
}

#[test]
fn guard_caps_runaway_allocation_and_rolls_back() {
    // A loop that grows a list without bound. The allocation cap trips, the
    // breach is caught, and state rolls back to the last consistent checkpoint
    // (the `safe` assignment that completed before the runaway statement).
    let src = "\
safe = 42
big = []
for i in range(100000)
    big = push(big, i)
print \"unreachable\"";
    let limits = Limits { steps: 50_000_000, alloc_cells: 5_000, wall: None };
    let app = nexus_exec::parse_app(src).unwrap();
    let outcome = nexus_exec::Interp::run_guarded(&app, limits);
    assert!(outcome.recovered, "should have recovered from the breach");
    assert!(outcome.error.as_deref().unwrap_or("").contains("allocation limit"));
    // The pre-breach output was retained, the impossible line never printed.
    assert!(!outcome.output.iter().any(|l| l == "unreachable"));
    assert!(outcome.alloc_cells >= 5_000);
}

#[test]
fn guard_enforces_wall_clock() {
    // An effectively infinite loop is stopped by the time budget, not left to
    // hang. (Step limit is high enough that time is what trips.)
    let src = "\
x = 0
while true
    x = x + 1";
    let limits = Limits { steps: u64::MAX, alloc_cells: u64::MAX, wall: Some(Duration::from_millis(120)) };
    let app = nexus_exec::parse_app(src).unwrap();
    let outcome = nexus_exec::Interp::run_guarded(&app, limits);
    assert!(outcome.recovered);
    assert!(outcome.error.as_deref().unwrap_or("").contains("time limit"));
}

#[test]
fn guard_clean_run_reports_no_recovery() {
    let src = "print 1 + 1";
    let outcome = run_guarded(src, Limits::default()).unwrap();
    assert!(!outcome.recovered);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.output, vec!["2"]);
}

#[test]
fn networking_tcp_from_dsl() {
    // Stand up a localhost line endpoint, then have a Nexus program call it.
    let server = nexus_tool::net::LineServer::spawn("127.0.0.1:0", 1, |line| {
        if line.contains("ping") {
            "pong".to_string()
        } else {
            "?".to_string()
        }
    })
    .unwrap();
    let addr = server.addr.clone();
    let src = format!("print tcp_line(\"{}\", \"ping\")", addr);
    let out = run_source(&src).unwrap();
    assert_eq!(out, vec!["pong"]);
    server.join();
}

#[test]
fn security_blocks_destructive_commands() {
    // The auto-sandbox refuses obviously destructive shell, even with no config.
    assert!(run_source("print sh(\"rm -rf /\")").is_err());
    // sandbox_ok exposes the decision without running anything.
    let out = run_source("print sandbox_ok(\"echo hi\")\nprint sandbox_ok(\"shutdown now\")").unwrap();
    assert_eq!(out, vec!["true", "false"]);
}

#[test]
fn privacy_redaction_and_pii_detection() {
    let out = run_source(
        "msg = \"email alice@example.com or ssn 123-45-6789\"\nprint contains_pii(msg)\nprint redact(msg)",
    )
    .unwrap();
    assert_eq!(out[0], "true");
    assert!(out[1].contains("[REDACTED:email]"), "got {}", out[1]);
    assert!(out[1].contains("[REDACTED:ssn]"), "got {}", out[1]);
    assert!(!out[1].contains("alice@example.com"));
}

#[test]
fn anonymity_stable_pseudonyms_and_tokens() {
    let out = run_source(
        "a = anonymize(\"user-1\")\nb = anonymize(\"user-1\")\nc = anonymize(\"user-2\")\nprint a == b\nprint a == c\nprint starts_with(a, \"anon_\")\nprint anon_token() == anon_token()",
    )
    .unwrap();
    assert_eq!(out, vec!["true", "false", "true", "false"]);
}

#[test]
fn errors_are_reported() {
    assert!(run_source("app E\n    main\n        print 1 / 0\n").is_err());
    assert!(run_source("app E\n    main\n        print undefined_var\n").is_err());
}
