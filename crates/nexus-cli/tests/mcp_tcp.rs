//! Integration test: spawn `nexus mcp-serve --tcp 127.0.0.1:0` as a real
//! subprocess and drive the MCP/JSON-RPC protocol over a genuine TCP socket,
//! proving an MCP server hooks in and runs correctly over the network.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn mcp_server_works_over_tcp() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_weft"))
        .args(["mcp-serve", "--tcp", "127.0.0.1:0"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nexus mcp-serve --tcp");

    // The server prints "MCP server listening on tcp://127.0.0.1:PORT" to
    // stderr; read that line to learn the OS-assigned port.
    let mut err = BufReader::new(child.stderr.take().unwrap());
    let mut banner = String::new();
    err.read_line(&mut banner).expect("read banner");
    let addr = banner
        .trim()
        .rsplit("tcp://")
        .next()
        .expect("addr in banner")
        .to_string();

    // Connect and drive ping -> run_app -> shutdown on one connection.
    let mut stream = TcpStream::connect(&addr).expect("connect to mcp tcp");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"id\":1}\n")
        .unwrap();
    stream
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"run_app\",\"params\":{\"source\":\"print 6 * 7\"},\"id\":2}\n")
        .unwrap();
    stream
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"shutdown\",\"id\":3}\n")
        .unwrap();
    stream.flush().unwrap();

    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    let lines: Vec<&str> = resp.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(lines.len() >= 3, "expected 3 responses, got: {resp}");
    assert!(lines[0].contains("\"result\":\"pong\""), "ping: {}", lines[0]);
    assert!(lines[1].contains("\"output\":[\"42\"]"), "run_app: {}", lines[1]);
    assert!(lines[2].contains("bye"), "shutdown: {}", lines[2]);

    child.wait().expect("server should exit after shutdown");
}
