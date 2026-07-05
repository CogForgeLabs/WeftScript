//! The `nexus` command-line driver: parse, build, verify, format, plan,
//! discover capabilities, execute, and benchmark Nexus programs.

mod bench;
mod pipeline;
mod tokens;

use std::process::ExitCode;

/// Stack for the worker thread that runs the whole command. The interpreter and
/// its analysis passes walk the AST recursively, so a deeply nested program can
/// use far more stack than the ~1 MiB the OS gives the main thread by default.
/// The parser caps nesting depth, but running on a generous stack means the
/// deepest program the parser accepts always evaluates without risking an
/// uncatchable stack-overflow abort.
const WORKER_STACK_BYTES: usize = 64 * 1024 * 1024;

fn main() -> ExitCode {
    // Run everything on a worker thread with a large stack. Recursion depth in
    // the parser is already bounded; this guards the recursive evaluator and
    // analysis passes for any program within that bound.
    match std::thread::Builder::new()
        .name("weft-main".into())
        .stack_size(WORKER_STACK_BYTES)
        .spawn(run)
    {
        Ok(handle) => match handle.join() {
            Ok(code) => code,
            Err(_) => {
                eprintln!("error: fatal internal error");
                ExitCode::FAILURE
            }
        },
        // If the OS refuses the thread, fall back to running inline.
        Err(_) => run(),
    }
}

fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        usage();
        return ExitCode::from(2);
    }
    let cmd = args[1].as_str();
    let rest = &args[2..];

    // Built-in auto privacy: install the PII/secret redactor so nothing
    // sensitive ever lands in a trace/log line (unless explicitly disabled).
    if std::env::var("NEXUS_NO_REDACT").as_deref() != Ok("1") {
        nexus_core::trace::set_redactor(nexus_secure::redact);
    }

    // Opt-in live tracing for any command: `--trace` (or NEXUS_LOG=<level>)
    // streams structured records to stderr so a whole run can be followed.
    configure_tracing(rest);

    // `run-guarded` reports containment with a distinct exit code (7), so it
    // returns its own ExitCode rather than going through the Ok/Err mapping.
    if cmd == "run-guarded" {
        return run_guarded_main(rest);
    }

    let result = match cmd {
        "parse" => with_file(rest, pipeline::cmd_parse),
        "build" => with_file(rest, pipeline::cmd_build),
        "check" => with_file(rest, pipeline::cmd_check),
        "fmt" => with_file(rest, pipeline::cmd_fmt),
        "graph" => with_file(rest, pipeline::cmd_graph),
        "plan" => with_file(rest, pipeline::cmd_plan),
        "run" => with_file(rest, pipeline::cmd_run),
        "app" => {
            // Optional dashboard: `nexus app file --dashboard [--port N]`.
            if rest.iter().any(|a| a == "--dashboard") {
                let addr = dashboard_addr(rest);
                with_file(rest, |p, s| pipeline::cmd_app_dashboard(p, s, &addr))
            } else {
                with_file(rest, pipeline::cmd_app)
            }
        }
        "dashboard" => pipeline::cmd_dashboard(&dashboard_addr(rest)),
        "tokens" => with_file(rest, pipeline::cmd_tokens),
        "tokencmp" => pipeline::cmd_tokencmp(rest),
        "mcp" => pipeline::cmd_mcp(),
        "mcp-serve" => match tcp_addr(rest) {
            Some(addr) => pipeline::cmd_mcp_serve_tcp(&addr),
            None => pipeline::cmd_mcp_serve(),
        },
        "bench" => pipeline::cmd_bench(rest),
        "demo" => pipeline::cmd_demo(),
        "help" | "--help" | "-h" => {
            usage();
            Ok(())
        }
        other => Err(format!("unknown command '{other}'")),
    };

    let code = match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    };
    maybe_dump_trace(rest);
    code
}

/// Enable structured tracing to stderr when `--trace` is passed or `NEXUS_LOG`
/// is set. `NEXUS_LOG` names the minimum level (trace/debug/info/warn/error).
fn configure_tracing(args: &[String]) {
    use nexus_core::trace::{self, Level};
    let level = std::env::var("NEXUS_LOG").ok().map(|s| Level::parse(&s));
    let want = args.iter().any(|a| a == "--trace") || level.is_some();
    if want {
        trace::set_level(level.unwrap_or(Level::Debug));
        // With --trace, stream live; with only NEXUS_LOG, buffer for a dump.
        if args.iter().any(|a| a == "--trace") {
            trace::set_stderr(true);
        }
    }
}

/// After a run, if `NEXUS_TRACE_DUMP=1`, print the buffered trace to stderr.
fn maybe_dump_trace(_args: &[String]) {
    if std::env::var("NEXUS_TRACE_DUMP").as_deref() == Ok("1") {
        eprint!("{}", nexus_core::trace::render());
    }
}

/// Extract the address following a `--tcp` flag, if present.
fn tcp_addr(args: &[String]) -> Option<String> {
    let i = args.iter().position(|a| a == "--tcp")?;
    Some(args.get(i + 1).cloned().unwrap_or_else(|| "127.0.0.1:8765".to_string()))
}

/// Dashboard bind address from `--port N` (or `--tcp addr`), default localhost:8787.
fn dashboard_addr(args: &[String]) -> String {
    if let Some(i) = args.iter().position(|a| a == "--port") {
        if let Some(p) = args.get(i + 1) {
            return format!("127.0.0.1:{p}");
        }
    }
    if let Some(a) = tcp_addr(args) {
        return a;
    }
    "127.0.0.1:8787".to_string()
}

/// Drive `run-guarded`, mapping the outcome to an exit code: 0 clean, 7 breach
/// contained (recovered), 2 usage/IO error, 1 hard error.
fn run_guarded_main(args: &[String]) -> ExitCode {
    let path = match args.iter().find(|a| !a.starts_with("--")) {
        Some(p) => p,
        None => {
            eprintln!("error: expected a .nx file path");
            return ExitCode::from(2);
        }
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let code = match pipeline::cmd_run_guarded(path, &src) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(7), // breach contained and rolled back
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    };
    maybe_dump_trace(args);
    code
}

fn with_file(
    args: &[String],
    f: impl Fn(&str, &str) -> Result<(), String>,
) -> Result<(), String> {
    let path = args.iter().find(|a| !a.starts_with("--")).ok_or("expected a .nx file path")?;
    // `-` reads the program from stdin (used by multiprocessing child runs).
    let src = if path == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).map_err(|e| format!("cannot read stdin: {e}"))?;
        buf
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?
    };
    f(path, &src)
}

fn usage() {
    println!(
        "weft — the verifiable software fabric\n\
\n\
USAGE:\n\
  weft <command> [args]\n\
\n\
COMMANDS:\n\
  parse  <file.nx>   Parse and summarize a program\n\
  build  <file.nx>   Run the full compiler pipeline (parse→plan→verify→bind)\n\
  check  <file.nx>   Formally verify safety invariants\n\
  fmt    <file.nx>   Re-project the graph to canonical DSL (TGG)\n\
  graph  <file.nx>   List content-addressed nodes and CIDs\n\
  plan   <file.nx>   Schedule the workflow (HEFT + Pareto)\n\
  run    <file.nx>   Execute the workflow DAG with provenance\n\
  app    <file.nx>   Run an executable Nexus app (general-purpose programs)\n\
  run-guarded <file> Run under hard resource limits with rollback/recovery\n\
  tokens <file.nx>   Compare token usage vs a traditional baseline\n\
  mcp                Demo capability discovery over JSON-RPC 2.0\n\
  mcp-serve [--tcp A] Run an MCP server over stdio, or TCP if --tcp <addr>\n\
  dashboard [--port N] Serve the optional web dashboard (headless by default)\n\
  demo               End-to-end demonstration of every subsystem\n\
  bench  [--write]   Run the benchmark suite (optionally write BENCHMARKS.md)\n\
\n\
GLOBAL FLAGS:\n\
  --trace            Stream structured logs to stderr during the run\n\
  --dashboard        (with `app`) serve outputs/traces on the web dashboard\n\
\n\
ENV:\n\
  NEXUS_LOG=<level>  NEXUS_STEPS  NEXUS_ALLOC_CELLS  NEXUS_WALL_MS  (limits/logging)\n\
  NEXUS_ALLOW_CMDS  NEXUS_DENY_CMDS  NEXUS_ALLOW_HOSTS  NEXUS_DENY_HOSTS  (sandbox)\n\
  NEXUS_PROXY=<host:port>  (anonymity)   NEXUS_NO_REDACT=1  (disable log redaction)\n"
    );
}
