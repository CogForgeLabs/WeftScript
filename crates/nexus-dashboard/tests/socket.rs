//! Integration test: a real TCP round-trip through the tiny HTTP/1.0 server.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

use nexus_dashboard::bind;

#[test]
fn serves_one_real_request_over_tcp() {
    let server = bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = server.local_addr();
    assert!(addr.contains("127.0.0.1"));

    let handle = thread::spawn(move || {
        server.serve_n(1).expect("serve one request");
    });

    // Give the accept loop a moment to be ready, then connect.
    let mut stream = {
        let mut attempt = TcpStream::connect(&addr);
        for _ in 0..50 {
            if attempt.is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
            attempt = TcpStream::connect(&addr);
        }
        attempt.expect("connect to dashboard")
    };
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    stream.write_all(b"GET / HTTP/1.0\r\n\r\n").expect("send request");
    stream.flush().ok();

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");

    assert!(response.contains("200"), "status line: {:?}", response.lines().next());
    assert!(response.contains("Nexus"), "body should contain the dashboard title");

    handle.join().expect("server thread joins");
}
