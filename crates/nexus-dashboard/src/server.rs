//! A tiny, zero-dependency HTTP/1.0 server over `std::net::TcpListener`.
//!
//! It does the bare minimum needed to host [`crate::route`]: parse the request
//! line, skim headers for `Content-Length`, read the body, route, and write a
//! `Connection: close` response. Read/write timeouts keep a stalled client from
//! hanging the accept loop (important for the `serve_n` test path). Every
//! connection is logged through `nexus_core::trace` at target `"dashboard"`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use crate::router::route;

const IO_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BODY: usize = 4 * 1024 * 1024;

/// A bound listener ready to serve the dashboard.
pub struct Server {
    listener: TcpListener,
}

/// Bind a listener. Use `"127.0.0.1:0"` to let the OS choose a free port.
pub fn bind(addr: &str) -> Result<Server, String> {
    let listener = TcpListener::bind(addr).map_err(|e| format!("bind {}: {}", addr, e))?;
    nexus_core::trace::info(
        "dashboard",
        format!("listening on {}", listener.local_addr().map(|a| a.to_string()).unwrap_or_default()),
    );
    Ok(Server { listener })
}

/// Bind `addr` and serve forever.
pub fn serve(addr: &str) -> Result<(), String> {
    bind(addr)?.run()
}

impl Server {
    /// The actual local address (host:port), resolving an OS-assigned port.
    pub fn local_addr(&self) -> String {
        self.listener.local_addr().map(|a| a.to_string()).unwrap_or_default()
    }

    /// Accept and serve connections forever.
    pub fn run(self) -> Result<(), String> {
        for stream in self.listener.incoming() {
            match stream {
                Ok(s) => {
                    if let Err(e) = handle(s) {
                        nexus_core::trace::warn("dashboard", format!("connection error: {}", e));
                    }
                }
                Err(e) => nexus_core::trace::warn("dashboard", format!("accept error: {}", e)),
            }
        }
        Ok(())
    }

    /// Serve exactly `n` accepted connections, then stop. Used by tests.
    ///
    /// We break the instant the nth connection is served rather than looping
    /// back into `incoming()`, which would block on a further `accept` that may
    /// never come.
    pub fn serve_n(self, n: usize) -> Result<(), String> {
        let mut served = 0;
        while served < n {
            match self.listener.accept() {
                Ok((s, _)) => {
                    if let Err(e) = handle(s) {
                        nexus_core::trace::warn("dashboard", format!("connection error: {}", e));
                    }
                    served += 1;
                }
                Err(e) => nexus_core::trace::warn("dashboard", format!("accept error: {}", e)),
            }
        }
        Ok(())
    }
}

/// Read one HTTP/1.0 request off `stream`, route it, and write the response.
fn handle(stream: TcpStream) -> Result<(), String> {
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok();
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();

    let mut reader = BufReader::new(stream);

    // Request line: METHOD PATH HTTP/x
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).map_err(|e| e.to_string())? == 0 {
        return Ok(()); // client closed without sending anything
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    // Headers until a blank line; we only care about Content-Length.
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0).min(MAX_BODY);
            }
        }
    }

    // Body (exactly Content-Length bytes, if any).
    let mut body = String::new();
    if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
        body = String::from_utf8_lossy(&buf).into_owned();
    }

    nexus_core::trace::info(
        "dashboard",
        format!("{} {} {} ({} body bytes)", peer, method, path, content_length),
    );

    let reply = route(&method, &path, &body);
    let mut stream = reader.into_inner();
    let header = format!(
        "HTTP/1.0 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reply.status,
        reply.content_type,
        reply.body.len(),
    );
    stream.write_all(header.as_bytes()).map_err(|e| e.to_string())?;
    stream.write_all(reply.body.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    Ok(())
}
