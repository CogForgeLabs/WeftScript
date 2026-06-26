//! Integration test: spawn the `nexus mcp-serve` binary as a real subprocess
//! and drive it over actual stdin/stdout pipes (JSON-RPC 2.0), proving an MCP
//! client can discover capabilities and execute Nexus programs through it.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn roundtrip(requests: &[&str]) -> Vec<String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_weft"))
        .arg("mcp-serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn nexus mcp-serve");

    {
        let stdin = child.stdin.as_mut().unwrap();
        for r in requests {
            writeln!(stdin, "{r}").unwrap();
        }
        stdin.flush().unwrap();
    }

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    let mut lines = Vec::new();
    for line in reader.lines() {
        let line = line.unwrap();
        if !line.trim().is_empty() {
            lines.push(line);
        }
    }
    child.wait().unwrap();
    lines
}

#[test]
fn server_responds_to_discovery_and_runs_apps() {
    let responses = roundtrip(&[
        r#"{"jsonrpc":"2.0","method":"ping","id":1}"#,
        r#"{"jsonrpc":"2.0","method":"discover","params":{},"id":2}"#,
        r#"{"jsonrpc":"2.0","method":"bind","params":{"capability":"ocr_scanner","objective":"cost"},"id":3}"#,
        r#"{"jsonrpc":"2.0","method":"run_app","params":{"source":"print 2 + 3 * 4"},"id":4}"#,
        r#"{"jsonrpc":"2.0","method":"run_app","params":{"source":"fn sq(x)\n    return x * x\nprint [sq(i) for i in range(1, 5)]"},"id":5}"#,
        r#"{"jsonrpc":"2.0","method":"shutdown","id":6}"#,
    ]);

    assert_eq!(responses.len(), 6, "one response per request: {responses:?}");
    // ping
    assert!(responses[0].contains("\"result\":\"pong\""));
    assert!(responses[0].contains("\"id\":1"));
    // discover lists the ocr capability
    assert!(responses[1].contains("ocr_scanner"));
    // bind chooses the cheapest provider (Tesseract, local, $0)
    assert!(responses[2].contains("Tesseract"));
    // run_app evaluates an expression
    assert!(responses[3].contains("\"output\":[\"14\"]"), "got {}", responses[3]);
    // run_app with a function + comprehension
    assert!(responses[4].contains("[1, 4, 9, 16]"), "got {}", responses[4]);
    // shutdown
    assert!(responses[5].contains("bye"));
}

#[test]
fn server_reports_app_errors_as_json_rpc_errors() {
    let responses = roundtrip(&[
        r#"{"jsonrpc":"2.0","method":"run_app","params":{"source":"print 1 / 0"},"id":9}"#,
        r#"{"jsonrpc":"2.0","method":"shutdown","id":10}"#,
    ]);
    assert!(responses[0].contains("\"error\""));
    assert!(responses[0].contains("division by zero"));
}

#[test]
fn server_runs_mixed_declarative_program_with_proof() {
    // A client asks the server to run a mixed program that formally proves an
    // invariant from inside the executable code.
    let src = "project B\\nguarantee G\\n    assume balance == start - amount\\n    assume start - amount >= 0\\n    ensure balance >= 0\\nprint prove(\\\"G\\\")";
    let req = format!(r#"{{"jsonrpc":"2.0","method":"run_app","params":{{"source":"{src}"}},"id":1}}"#);
    let responses = roundtrip(&[&req, r#"{"jsonrpc":"2.0","method":"shutdown","id":2}"#]);
    assert!(responses[0].contains("\"output\":[\"true\"]"), "got {}", responses[0]);
}
