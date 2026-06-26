//! Integration test: process-level resource protection. A child workload that
//! breaches its memory budget is contained in its own process — it exits with
//! the distinct "breach contained" code (7), retains the output produced before
//! the breach, and leaves the parent (this test) completely unaffected. This is
//! the out-of-process half of the OOM/rollback protection.

use std::io::Write;
use std::process::Command;

fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("nexus_guard_{}_{}.nx", name, std::process::id()));
    let mut f = std::fs::File::create(&p).expect("create temp script");
    f.write_all(contents.as_bytes()).expect("write temp script");
    p
}

#[test]
fn runaway_allocation_is_contained_in_child_process() {
    // Prints a safe line, then grows a list without bound. Under a tiny alloc
    // budget the breach trips; the impossible final line must never print.
    let script = write_temp(
        "runaway",
        "print \"started-ok\"\nbig = []\nfor i in range(100000)\n    big = push(big, i)\nprint \"NEVER\"\n",
    );

    let out = Command::new(env!("CARGO_BIN_EXE_weft"))
        .args(["run-guarded", script.to_str().unwrap()])
        .env("NEXUS_ALLOC_CELLS", "5000")
        .output()
        .expect("spawn nexus run-guarded");

    let _ = std::fs::remove_file(&script);

    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Distinct exit code for "breach contained, rolled back".
    assert_eq!(code, 7, "expected containment exit 7; stderr: {stderr}");
    // Pre-breach output retained; post-breach line never reached.
    assert!(stdout.contains("started-ok"), "stdout: {stdout}");
    assert!(!stdout.contains("NEVER"), "stdout: {stdout}");
    // The breach was reported as a contained rollback, not a hard crash.
    assert!(stderr.contains("breach contained"), "stderr: {stderr}");
    assert!(stderr.contains("allocation limit"), "stderr: {stderr}");
}

#[test]
fn clean_program_exits_zero_under_guard() {
    let script = write_temp("clean", "print 2 + 2\n");
    let out = Command::new(env!("CARGO_BIN_EXE_weft"))
        .args(["run-guarded", script.to_str().unwrap()])
        .output()
        .expect("spawn nexus run-guarded");
    let _ = std::fs::remove_file(&script);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("4"));
}
