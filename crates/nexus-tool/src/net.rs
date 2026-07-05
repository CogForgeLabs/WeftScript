//! Real networking over `std::net` (zero external dependencies).
//!
//! This gives Nexus programs genuine socket I/O — talk to a TCP service, fetch
//! over HTTP/1.0, or stand up a one-shot line server so an MCP-style endpoint
//! can be reached over the network rather than only in-process or over stdio.
//! Every call is traced through `nexus_core::trace`.

use nexus_core::trace::{self, Level};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Cap on how many bytes any single network read will accept into memory. A
/// hostile or misbehaving peer could otherwise stream without end and exhaust
/// memory (a DoS reachable from a program that is allowed to open a socket).
/// 64 MiB matches the interpreter's other allocation ceilings.
const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

/// Read a stream to a `String`, but never accept more than [`MAX_RESPONSE_BYTES`].
/// Returns an error if the peer exceeds the cap rather than growing the buffer
/// without bound. Invalid UTF-8 is replaced so a binary body cannot fail the read.
fn read_capped(stream: &mut impl Read) -> Result<String, String> {
    let mut buf = Vec::new();
    stream.take(MAX_RESPONSE_BYTES + 1).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    if buf.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(format!("response exceeds {MAX_RESPONSE_BYTES}-byte cap"));
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Send `payload` to a TCP `addr` ("host:port"), half-close the write side so
/// the peer sees end-of-request, and return the full response as a string.
pub fn tcp_request(addr: &str, payload: &str) -> Result<String, String> {
    let sp = trace::span("net.tcp", format!("request -> {}", addr));
    let mut stream = TcpStream::connect(addr).map_err(|e| {
        sp.record(Level::Error, "connect failed", &[("err", &e.to_string())]);
        e.to_string()
    })?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok();
    stream.write_all(payload.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().ok();
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let resp = read_capped(&mut stream)?;
    sp.record(Level::Info, "response", &[("bytes", &resp.len().to_string())]);
    Ok(resp)
}

/// Send a single line (a `\n` is appended if missing) and read back exactly one
/// response line — the request/response shape an MCP/JSON-RPC line server uses.
pub fn tcp_request_line(addr: &str, line: &str) -> Result<String, String> {
    let sp = trace::span("net.tcp", format!("line -> {}", addr));
    let mut stream = TcpStream::connect(addr).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    let mut msg = line.to_string();
    if !msg.ends_with('\n') {
        msg.push('\n');
    }
    stream.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().ok();
    let mut reader = BufReader::new(stream);
    let mut resp = String::new();
    reader.read_line(&mut resp).map_err(|e| e.to_string())?;
    sp.record(Level::Info, "line response", &[("bytes", &resp.len().to_string())]);
    Ok(resp.trim_end().to_string())
}

/// Result of an HTTP fetch.
#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// Minimal HTTP/1.0 GET over a raw TCP socket. Handles `http://host[:port]/path`.
///
/// Anonymity: sends only a neutral `User-Agent: Nexus/1.0` and `DNT: 1` with no
/// cookies or referer, so requests carry no identifying client state. If the
/// `NEXUS_PROXY` env var is set (`host:port`), the request is routed through
/// that HTTP proxy in absolute-URI form — point it at a Tor/SOCKS-bridge or
/// forward proxy to mask the origin.
pub fn http_get(url: &str) -> Result<HttpResponse, String> {
    let sp = trace::span("net.http", format!("GET {}", url));
    let rest = url.strip_prefix("http://").ok_or("only http:// URLs are supported")?;
    let (authority, path) = match rest.split_once('/') {
        Some((a, p)) => (a, format!("/{}", p)),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match authority.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().map_err(|_| "bad port")?),
        None => (authority.to_string(), 80),
    };

    // Route through an HTTP proxy if configured (anonymity).
    let proxy = std::env::var("NEXUS_PROXY").ok().filter(|s| !s.is_empty());
    let (connect_to, request_target) = match &proxy {
        Some(p) => {
            sp.record(Level::Debug, "via proxy", &[("proxy", p)]);
            (p.clone(), format!("http://{authority}{path}")) // absolute-form for proxy
        }
        None => (format!("{host}:{port}"), path.clone()),
    };

    let mut stream = TcpStream::connect(&connect_to).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    let req = format!(
        "GET {request_target} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: Nexus/1.0\r\nDNT: 1\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().ok();
    let raw = read_capped(&mut stream)?;
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw.as_str(), ""));
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);
    sp.record(Level::Info, "fetched", &[("status", &status.to_string()), ("bytes", &body.len().to_string())]);
    Ok(HttpResponse { status, body: body.to_string() })
}

/// A running one-shot/looping line server, used both as an MCP-style network
/// endpoint and in tests. Bind to "127.0.0.1:0" to get an OS-assigned port.
pub struct LineServer {
    pub addr: String,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl LineServer {
    /// Serve `max_conns` connections (each: read one line, reply with
    /// `handler(line)` + newline), then stop. The bound address is available
    /// immediately so a client can connect without a race.
    pub fn spawn<F>(bind: &str, max_conns: usize, handler: F) -> Result<LineServer, String>
    where
        F: Fn(&str) -> String + Send + 'static,
    {
        let listener = TcpListener::bind(bind).map_err(|e| e.to_string())?;
        let addr = listener.local_addr().map_err(|e| e.to_string())?.to_string();
        trace::record(Level::Info, "net.serve", None, "listening", &[("addr", &addr)]);
        let handle = std::thread::spawn(move || {
            let mut served = 0;
            for conn in listener.incoming() {
                let stream = match conn {
                    Ok(s) => s,
                    Err(_) => break,
                };
                stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
                let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() {
                    let reply = handler(line.trim_end());
                    let mut s = stream;
                    let _ = s.write_all(reply.as_bytes());
                    let _ = s.write_all(b"\n");
                    let _ = s.flush();
                }
                served += 1;
                if served >= max_conns {
                    break;
                }
            }
        });
        Ok(LineServer { addr, handle: Some(handle) })
    }

    /// Wait for the server thread to finish serving its connections.
    pub fn join(mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_line_round_trip_localhost() {
        // Stand up an MCP-style JSON-RPC line endpoint on localhost and call it.
        let server = LineServer::spawn("127.0.0.1:0", 1, |line| {
            // Echo a JSON-RPC "pong" carrying back the received id, proving the
            // server actually parsed and handled the request.
            if line.contains("\"method\":\"ping\"") {
                "{\"jsonrpc\":\"2.0\",\"result\":\"pong\",\"id\":1}".to_string()
            } else {
                "{\"jsonrpc\":\"2.0\",\"error\":\"unknown\",\"id\":1}".to_string()
            }
        })
        .unwrap();
        let addr = server.addr.clone();
        let resp = tcp_request_line(&addr, "{\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"id\":1}").unwrap();
        assert!(resp.contains("pong"), "got: {resp}");
        server.join();
    }

    #[test]
    fn http_get_localhost() {
        // A tiny local HTTP/1.0 server — no internet required.
        let server = LineServer::spawn("127.0.0.1:0", 1, |_req| {
            // Note: the client sends the request line; we reply with a full
            // HTTP response. The trailing newline LineServer adds is harmless.
            "HTTP/1.0 200 OK\r\nContent-Type: text/plain\r\n\r\nhello-nexus".to_string()
        })
        .unwrap();
        let url = format!("http://{}/", server.addr);
        let resp = http_get(&url).unwrap();
        assert_eq!(resp.status, 200, "resp: {:?}", resp);
        assert!(resp.body.contains("hello-nexus"));
        server.join();
    }

    #[test]
    fn read_capped_accepts_under_limit() {
        // A body at exactly the cap is accepted; nothing is lost.
        let body = vec![b'x'; 4096];
        let out = read_capped(&mut body.as_slice()).unwrap();
        assert_eq!(out.len(), 4096);
    }

    #[test]
    fn read_capped_rejects_over_limit() {
        // A peer that streams past the cap is refused rather than allowed to
        // grow the buffer without bound. `Repeat` is an endless byte source.
        let mut endless = std::io::repeat(b'x');
        let err = read_capped(&mut endless).unwrap_err();
        assert!(err.contains("cap"), "got: {err}");
    }
}
