//! Integration tests for the language upgrades: `break`/`continue`, multi-line
//! collection literals, higher-order builtins, JSON, files, and time.

use nexus_exec::run_source;

// ---- break / continue ------------------------------------------------------

#[test]
fn break_exits_while_loop() {
    let src = "\
i = 0
while true
    i += 1
    if i == 5
        break
print i
";
    assert_eq!(run_source(src).unwrap(), vec!["5"]);
}

#[test]
fn continue_skips_iteration() {
    let src = "\
total = 0
for x in range(10)
    if x % 2 == 0
        continue
    total += x
print total
";
    // 1+3+5+7+9
    assert_eq!(run_source(src).unwrap(), vec!["25"]);
}

#[test]
fn break_only_exits_innermost_loop() {
    let src = "\
hits = 0
for i in range(3)
    for j in range(10)
        if j == 2
            break
        hits += 1
print hits
";
    assert_eq!(run_source(src).unwrap(), vec!["6"]);
}

#[test]
fn break_in_for_over_list() {
    let src = "\
found = nil
for w in [\"a\", \"needle\", \"b\"]
    if w == \"needle\"
        found = w
        break
print found
";
    assert_eq!(run_source(src).unwrap(), vec!["needle"]);
}

#[test]
fn break_outside_loop_is_an_error() {
    let err = run_source("break\n").unwrap_err();
    assert!(err.contains("break"), "unexpected error: {err}");
}

#[test]
fn continue_outside_loop_is_an_error() {
    let err = run_source("continue\n").unwrap_err();
    assert!(err.contains("continue"), "unexpected error: {err}");
}

#[test]
fn break_is_catchable_boundary_with_functions() {
    // A function body is a fresh loop context: break inside a function called
    // from a loop must NOT break the caller's loop.
    let src = "\
fn f()
    x = 0
    while true
        break
    return 7
total = 0
for i in range(3)
    total += f()
print total
";
    assert_eq!(run_source(src).unwrap(), vec!["21"]);
}

// ---- multi-line literals ---------------------------------------------------

#[test]
fn multiline_list_literal() {
    let src = "\
xs = [
    1,
    2,
    3,
]
print sum(xs)
";
    assert_eq!(run_source(src).unwrap(), vec!["6"]);
}

#[test]
fn multiline_map_literal() {
    let src = "\
m = {
    \"a\": 1,
    \"b\": 2,
}
print m.a + m.b
";
    assert_eq!(run_source(src).unwrap(), vec!["3"]);
}

#[test]
fn multiline_nested_structures() {
    let src = "\
data = [
    {\"name\": \"ada\", \"score\": 90},
    {\"name\": \"bob\",
     \"score\": 80},
]
print data[0].name
print data[1].score
";
    assert_eq!(run_source(src).unwrap(), vec!["ada", "80"]);
}

#[test]
fn multiline_call_arguments() {
    let src = "\
fn add3(a, b, c)
    return a + b + c
print add3(
    1,
    2,
    3
)
";
    assert_eq!(run_source(src).unwrap(), vec!["6"]);
}

#[test]
fn unclosed_bracket_reports_error() {
    let err = run_source("xs = [1, 2\nprint 1\n").unwrap_err();
    assert!(err.contains("unclosed"), "unexpected error: {err}");
}

// ---- higher-order builtins ---------------------------------------------------

#[test]
fn filter_by_function_name() {
    let src = "\
fn keep(x)
    return x % 2 == 0
print filter(\"keep\", range(10))
";
    assert_eq!(run_source(src).unwrap(), vec!["[0, 2, 4, 6, 8]"]);
}

#[test]
fn reduce_by_function_name() {
    let src = "\
fn add(acc, x)
    return acc + x
print reduce(\"add\", range(1, 5), 0)
print reduce(\"add\", [], 42)
";
    assert_eq!(run_source(src).unwrap(), vec!["10", "42"]);
}

#[test]
fn any_and_all() {
    let src = "\
print any([false, true])
print any([])
print all([1, 2, 3])
print all([1, 0])
";
    assert_eq!(run_source(src).unwrap(), vec!["true", "false", "true", "false"]);
}

#[test]
fn enumerate_and_items() {
    let src = "\
for pair in enumerate([\"x\", \"y\"])
    print \"{pair[0]}:{pair[1]}\"
m = {\"a\": 1}
for kv in items(m)
    print \"{kv[0]}={kv[1]}\"
";
    assert_eq!(run_source(src).unwrap(), vec!["0:x", "1:y", "a=1"]);
}

#[test]
fn del_removes_a_key() {
    let src = "\
m = {\"a\": 1, \"b\": 2}
m = del(m, \"a\")
print has(m, \"a\")
print has(m, \"b\")
";
    assert_eq!(run_source(src).unwrap(), vec!["false", "true"]);
}

#[test]
fn type_of_values() {
    let src = "\
print type(1)
print type(\"s\")
print type([1])
print type({\"a\": 1})
print type(nil)
print type(true)
";
    assert_eq!(run_source(src).unwrap(), vec!["num", "str", "list", "map", "nil", "bool"]);
}

// ---- JSON --------------------------------------------------------------------

#[test]
fn json_round_trip() {
    let src = "\
v = {\"name\": \"ada\", \"scores\": [1, 2.5], \"ok\": true}
s = json_encode(v)
back = json_decode(s)
print back.name
print back.scores[1]
print back.ok
";
    assert_eq!(run_source(src).unwrap(), vec!["ada", "2.5", "true"]);
}

#[test]
fn json_decode_error_is_catchable() {
    let src = "\
try
    x = json_decode(\"not json\")
catch e
    print \"caught\"
";
    assert_eq!(run_source(src).unwrap(), vec!["caught"]);
}

// ---- files -------------------------------------------------------------------

#[test]
fn file_write_read_append() {
    let dir = std::env::temp_dir().join(format!("weft_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.txt");
    let p = path.to_string_lossy().replace('\\', "/");
    let src = format!(
        "write_file(\"{p}\", \"hello\")\nappend_file(\"{p}\", \" world\")\nprint read_file(\"{p}\")\n"
    );
    assert_eq!(run_source(&src).unwrap(), vec!["hello world"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_missing_file_is_catchable() {
    let src = "\
try
    x = read_file(\"definitely/not/a/real/path.txt\")
catch e
    print \"caught\"
";
    assert_eq!(run_source(src).unwrap(), vec!["caught"]);
}

// ---- numeric literals ----------------------------------------------------------

#[test]
fn scientific_notation_literals() {
    let src = "\
print 1e3
print 2.5e-3
print 1.5E2 + 50
";
    assert_eq!(run_source(src).unwrap(), vec!["1000", "0.0025", "200"]);
}

// ---- misc builtins -------------------------------------------------------------

#[test]
fn lines_splits_text() {
    let src = "print len(lines(\"a\\nb\\nc\"))\n";
    assert_eq!(run_source(src).unwrap(), vec!["3"]);
}

#[test]
fn now_ms_is_monotonic_enough() {
    let src = "\
a = now_ms()
b = now_ms()
print b >= a
print a > 0
";
    assert_eq!(run_source(src).unwrap(), vec!["true", "true"]);
}

// ---- performance-shaped regressions -------------------------------------------
// These are the accumulate patterns that used to be O(n^2); they must finish
// comfortably inside the default limits at meaningful sizes.

#[test]
fn map_accumulation_scales() {
    let src = "\
counts = map()
i = 0
while i < 30000
    k = \"k{i % 50}\"
    counts[k] = get(counts, k, 0) + 1
    i += 1
print get(counts, \"k0\")
";
    assert_eq!(run_source(src).unwrap(), vec!["600"]);
}

#[test]
fn list_push_accumulation_scales() {
    let src = "\
acc = []
i = 0
while i < 30000
    acc = push(acc, i)
    i += 1
print len(acc)
print acc[29999]
";
    assert_eq!(run_source(src).unwrap(), vec!["30000", "29999"]);
}
