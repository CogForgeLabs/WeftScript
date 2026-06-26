//! Pure, socket-free request router — the testable core of the dashboard.
//!
//! Everything the dashboard serves is decided here as a plain
//! `(method, path, body) -> HttpReply` function with no I/O, so the entire HTTP
//! surface can be exercised by ordinary unit tests. The tiny TCP server in
//! [`crate::server`] is just a transport that calls into this.

use crate::output;

/// A fully-formed HTTP reply: status code, content type, and body. The server
/// turns this into bytes on the wire; tests inspect it directly.
pub struct HttpReply {
    pub status: u16,
    pub content_type: String,
    pub body: String,
}

impl HttpReply {
    fn new(status: u16, content_type: &str, body: String) -> Self {
        HttpReply {
            status,
            content_type: content_type.to_string(),
            body,
        }
    }
}

/// The auto-generated dashboard page: embedded HTML + CSS + vanilla JS, no
/// external assets or CDNs. The script polls `/api/trace` and `/api/output`
/// every second and POSTs source to `/api/run`.
const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Nexus Dashboard</title>
<style>
  :root { color-scheme: dark; }
  * { box-sizing: border-box; }
  body {
    margin: 0; font-family: ui-monospace, "Cascadia Code", Menlo, Consolas, monospace;
    background: #0d1117; color: #c9d1d9;
  }
  header {
    padding: 14px 20px; border-bottom: 1px solid #21262d;
    display: flex; align-items: baseline; gap: 12px;
    background: #161b22;
  }
  header h1 { font-size: 16px; margin: 0; color: #58a6ff; letter-spacing: .5px; }
  header .sub { font-size: 12px; color: #8b949e; }
  main { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; padding: 16px; }
  @media (max-width: 900px) { main { grid-template-columns: 1fr; } }
  section { background: #161b22; border: 1px solid #21262d; border-radius: 8px; overflow: hidden; }
  section > h2 {
    font-size: 13px; margin: 0; padding: 10px 14px; background: #1c2330;
    border-bottom: 1px solid #21262d; color: #d2a8ff; text-transform: uppercase; letter-spacing: .8px;
  }
  .pane { padding: 12px 14px; max-height: 42vh; overflow: auto; }
  textarea {
    width: 100%; min-height: 140px; resize: vertical;
    background: #0d1117; color: #c9d1d9; border: 1px solid #30363d; border-radius: 6px;
    padding: 10px; font: inherit; font-size: 13px;
  }
  button {
    margin-top: 10px; padding: 8px 18px; border: 0; border-radius: 6px;
    background: #238636; color: #fff; font: inherit; font-weight: 600; cursor: pointer;
  }
  button:hover { background: #2ea043; }
  button:disabled { background: #30363d; cursor: default; }
  pre { margin: 8px 0 0; white-space: pre-wrap; word-break: break-word; font-size: 13px; }
  .log { font-size: 12px; line-height: 1.5; }
  .log .line { display: block; }
  table { width: 100%; border-collapse: collapse; font-size: 12px; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid #21262d; vertical-align: top; }
  th { color: #8b949e; position: sticky; top: 0; background: #161b22; }
  td.msg { word-break: break-word; }
  .lvl { font-weight: 600; }
  .lvl.INFO { color: #58a6ff; } .lvl.DEBUG { color: #8b949e; } .lvl.TRACE { color: #6e7681; }
  .lvl.WARN { color: #d29922; } .lvl.ERROR { color: #f85149; }
  .err { color: #f85149; }
  .muted { color: #6e7681; }
</style>
</head>
<body>
<header>
  <h1>Nexus Dashboard</h1>
  <span class="sub">live program output &amp; structured trace &mdash; <span id="status" class="muted">connecting&hellip;</span></span>
</header>
<main>
  <section>
    <h2>Run</h2>
    <div class="pane">
      <textarea id="src" spellcheck="false" placeholder="print 2 + 3">print 2 + 3</textarea>
      <button id="run">Run</button>
      <pre id="runResult" class="muted">Submit Nexus source to POST /api/run.</pre>
    </div>
  </section>
  <section>
    <h2>Output</h2>
    <div class="pane log" id="output"><span class="muted">waiting for /api/output&hellip;</span></div>
  </section>
  <section style="grid-column: 1 / -1;">
    <h2>Trace</h2>
    <div class="pane">
      <table>
        <thead><tr><th>seq</th><th>+us</th><th>level</th><th>target</th><th>span</th><th>message</th><th>fields</th></tr></thead>
        <tbody id="trace"><tr><td colspan="7" class="muted">waiting for /api/trace&hellip;</td></tr></tbody>
      </table>
    </div>
  </section>
</main>
<script>
"use strict";
function esc(s) {
  return String(s).replace(/[&<>]/g, function (c) {
    return c === "&" ? "&amp;" : c === "<" ? "&lt;" : "&gt;";
  });
}
function setStatus(ok) {
  var el = document.getElementById("status");
  el.textContent = ok ? "live" : "disconnected";
  el.className = ok ? "" : "err";
}
function renderOutput(lines) {
  var box = document.getElementById("output");
  if (!lines || !lines.length) { box.innerHTML = '<span class="muted">(no output)</span>'; return; }
  box.innerHTML = lines.map(function (l) { return '<span class="line">' + esc(l) + '</span>'; }).join("");
}
function renderTrace(recs) {
  var body = document.getElementById("trace");
  if (!recs || !recs.length) { body.innerHTML = '<tr><td colspan="7" class="muted">(empty)</td></tr>'; return; }
  body.innerHTML = recs.map(function (r) {
    var f = r.fields || {};
    var fk = Object.keys(f).map(function (k) { return k + "=" + f[k]; }).join(", ");
    return "<tr>" +
      "<td>" + esc(r.seq) + "</td>" +
      "<td>" + esc(r.elapsed_us) + "</td>" +
      '<td class="lvl ' + esc(r.level) + '">' + esc(r.level) + "</td>" +
      "<td>" + esc(r.target) + "</td>" +
      "<td>" + (r.span === null ? '<span class="muted">-</span>' : esc(r.span)) + "</td>" +
      '<td class="msg">' + esc(r.message) + "</td>" +
      "<td>" + esc(fk) + "</td>" +
      "</tr>";
  }).join("");
}
function poll() {
  fetch("/api/output").then(function (r) { return r.json(); }).then(function (d) {
    setStatus(true); renderOutput(d.output);
  }).catch(function () { setStatus(false); });
  fetch("/api/trace").then(function (r) { return r.json(); }).then(renderTrace).catch(function () {});
}
document.getElementById("run").addEventListener("click", function () {
  var btn = this, src = document.getElementById("src").value, res = document.getElementById("runResult");
  btn.disabled = true; res.className = "muted"; res.textContent = "running…";
  fetch("/api/run", { method: "POST", body: src }).then(function (r) { return r.json(); }).then(function (d) {
    if (d.error) { res.className = "err"; res.textContent = "error: " + d.error; }
    else { res.className = ""; res.textContent = (d.output || []).join("\n") || "(no output)"; }
    poll();
  }).catch(function (e) {
    res.className = "err"; res.textContent = "request failed: " + e;
  }).finally(function () { btn.disabled = false; });
});
poll();
setInterval(poll, 1000);
</script>
</body>
</html>
"##;

/// Escape a string for inclusion inside a JSON string literal.
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// `{"output":["...","..."]}` for a slice of lines.
fn output_json(lines: &[String]) -> String {
    let mut s = String::from("{\"output\":[");
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('"');
        s.push_str(&json_escape(l));
        s.push('"');
    }
    s.push_str("]}");
    s
}

/// `{"error":"..."}` for a failed run.
fn error_json(msg: &str) -> String {
    format!("{{\"error\":\"{}\"}}", json_escape(msg))
}

/// Serialize the current trace snapshot as a JSON array of record objects.
fn trace_json() -> String {
    let recs = nexus_core::trace::snapshot();
    let mut s = String::from("[");
    for (i, r) in recs.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"seq\":");
        s.push_str(&r.seq.to_string());
        s.push_str(",\"elapsed_us\":");
        s.push_str(&r.elapsed_us.to_string());
        s.push_str(",\"level\":\"");
        s.push_str(&json_escape(r.level.as_str()));
        s.push_str("\",\"target\":\"");
        s.push_str(&json_escape(&r.target));
        s.push_str("\",\"span\":");
        match r.span {
            Some(id) => s.push_str(&id.to_string()),
            None => s.push_str("null"),
        }
        s.push_str(",\"message\":\"");
        s.push_str(&json_escape(&r.message));
        s.push_str("\",\"fields\":{");
        for (j, (k, v)) in r.fields.iter().enumerate() {
            if j > 0 {
                s.push(',');
            }
            s.push('"');
            s.push_str(&json_escape(k));
            s.push_str("\":\"");
            s.push_str(&json_escape(v));
            s.push('"');
        }
        s.push_str("}}");
    }
    s.push(']');
    s
}

/// Run Nexus source, push its output into the global buffer, and reply as JSON.
fn run_source(body: &str) -> String {
    match nexus_app::run_mixed(body) {
        Ok(lines) => {
            for line in &lines {
                output::push_output(line.clone());
            }
            output_json(&lines)
        }
        Err(e) => error_json(&e),
    }
}

/// The whole dashboard HTTP surface as a pure function.
pub fn route(method: &str, path: &str, body: &str) -> HttpReply {
    match (method, path) {
        ("GET", "/") => HttpReply::new(200, "text/html; charset=utf-8", DASHBOARD_HTML.to_string()),
        ("GET", "/api/output") => HttpReply::new(200, "application/json", output_json(&output::output())),
        ("GET", "/api/trace") => HttpReply::new(200, "application/json", trace_json()),
        ("POST", "/api/run") => HttpReply::new(200, "application/json", run_source(body)),
        _ => HttpReply::new(404, "text/plain; charset=utf-8", "not found".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{clear_output, set_output};

    #[test]
    fn index_is_the_dashboard() {
        let r = route("GET", "/", "");
        assert_eq!(r.status, 200);
        assert!(r.content_type.contains("html"));
        assert!(r.body.contains("Nexus"));
        assert!(r.body.contains("/api/trace"));
        assert!(r.body.contains("/api/run"));
        assert!(r.body.contains("/api/output"));
    }

    #[test]
    fn output_endpoint_reflects_buffer() {
        set_output(vec!["hello".into()]);
        let r = route("GET", "/api/output", "");
        assert_eq!(r.status, 200);
        assert!(r.content_type.contains("json"));
        assert!(r.body.contains("hello"));
        assert!(r.body.contains("\"output\""));
        clear_output();
    }

    #[test]
    fn run_ok_returns_output() {
        let r = route("POST", "/api/run", "print 2 + 3");
        assert_eq!(r.status, 200);
        assert!(r.body.contains("\"output\""));
        assert!(r.body.contains('5'));
    }

    #[test]
    fn run_pushes_into_buffer() {
        clear_output();
        let _ = route("POST", "/api/run", "print 41 + 1");
        assert!(output::output().iter().any(|l| l.contains("42")));
        clear_output();
    }

    #[test]
    fn run_error_is_flagged() {
        let r = route("POST", "/api/run", "print 1 / 0");
        assert_eq!(r.status, 200);
        assert!(r.body.contains("\"error\""));
    }

    #[test]
    fn unknown_is_404() {
        let r = route("GET", "/nope", "");
        assert_eq!(r.status, 404);
        assert!(r.content_type.contains("text/plain"));
    }

    #[test]
    fn trace_endpoint_is_json_array() {
        nexus_core::trace::info("dashboard-test", "marker");
        let r = route("GET", "/api/trace", "");
        assert_eq!(r.status, 200);
        assert!(r.body.starts_with('['));
        assert!(r.body.ends_with(']'));
        assert!(r.body.contains("\"seq\""));
        assert!(r.body.contains("\"level\""));
    }

    #[test]
    fn escapes_specials() {
        let s = json_escape("a\"b\\c\nd");
        assert_eq!(s, "a\\\"b\\\\c\\nd");
        assert!(json_escape("\t").contains("\\t"));
        assert!(json_escape("\u{01}").contains("\\u0001"));
    }
}
