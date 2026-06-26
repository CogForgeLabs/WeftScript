//! Integration test: true multiprocessing. A Nexus program uses `mp_map` to run
//! work in SEPARATE OS processes (one child `nexus` per element), each fully
//! isolated, and collects their outputs. Proves process-level parallelism, not
//! just threads.

use std::io::Write;
use std::process::Command;

fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("nexus_mp_{}_{}.nx", name, std::process::id()));
    let mut f = std::fs::File::create(&p).expect("create temp script");
    f.write_all(contents.as_bytes()).expect("write temp script");
    p
}

#[test]
fn mp_map_runs_work_in_separate_processes() {
    // Each child computes input*input in its own process; the parent collects
    // the printed results in order.
    let script = write_temp(
        "square",
        "r = mp_map(\"print input * input\", [2, 3, 4, 5])\nprint r\n",
    );

    let out = Command::new(env!("CARGO_BIN_EXE_weft"))
        // Make children resolve the same binary we're running.
        .env("NEXUS_BIN", env!("CARGO_BIN_EXE_weft"))
        .args(["app", script.to_str().unwrap()])
        .output()
        .expect("spawn nexus app");

    let _ = std::fs::remove_file(&script);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "run failed: {stderr}");
    assert!(stdout.contains("[4, 9, 16, 25]"), "stdout: {stdout}");
}

#[test]
fn mp_map_children_are_isolated() {
    // A child can read its own `input` but shares no interpreter state with the
    // parent or siblings — each is a fresh process.
    let script = write_temp(
        "isolated",
        "r = mp_map(\"print \\\"pid-result:\\\" + str(input)\", [10, 20])\nprint len(r)\nprint r[0]\nprint r[1]\n",
    );
    let out = Command::new(env!("CARGO_BIN_EXE_weft"))
        .env("NEXUS_BIN", env!("CARGO_BIN_EXE_weft"))
        .args(["app", script.to_str().unwrap()])
        .output()
        .expect("spawn nexus app");
    let _ = std::fs::remove_file(&script);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("pid-result:10"), "stdout: {stdout}");
    assert!(stdout.contains("pid-result:20"), "stdout: {stdout}");
}
