# WeftScript — the meta-language

> *One fabric, woven from intent.*
>
> Made by **[Cognitive Industries / CogForgeLabs](https://github.com/CogForgeLabs)**
> · cognitive-industries.org · contact@cognitive-industries.org

---

## A language above high-level languages

Every language you know — Python, TypeScript, Go, Rust — is *high-level*. You
still write *how*: explicit loops, manual thread management, hand-wired error
handling, boilerplate security checks, and verbose plumbing between
abstractions. High-level means you skip machine code. WeftScript means you skip
all of that.

**WeftScript is a meta-language.** You declare *what* you intend. The runtime
handles *everything else* — automatically and verifiably:

| What you used to write manually | What WeftScript does for you |
| :------------------------------ | :--------------------------- |
| Thread pools, locks, `async/await` | Auto-parallelism across cores, processes, and hardware |
| GPU kernels, SIMD intrinsics | Transparent SIMD/GPU dispatch by problem size |
| `try/catch` + retry loops | Hard resource limits, rollback to last checkpoint, breach containment |
| Input sanitization, secret scrubbing | Built-in PII/secret redaction on every log line by default |
| Firewall rules, command allow-lists | Auto-sandbox: destructive commands denied even with no config |
| Unit tests for invariants | Formal proofs at compile time — counterexamples are concrete |
| Streaming + backpressure | Lazy generators and `take`/`collect` with bounded memory |
| Observability plumbing | Structured, span-aware trace built into every subsystem |
| Dashboard scaffolding | Optional zero-dep web UI — headless by default, one flag to enable |

One `.nx` file declares a schema, proves a safety invariant, runs general code,
fans work across CPU cores *and* separate OS processes, streams lazily, drives
shell tools, talks over the network, redacts sensitive data, and protects itself
from runaway allocation — with **235 passing tests** and no runtime surprises.

```
# declare intent
thing Invoice
    id: UUID
    amount: Decimal

guarantee NoOverdraft
    assume balance >= amount
    ensure balance - amount >= 0

# run it (same file)
scores = [risk(x) for x in customers]   # auto-parallelized if pure + large
safe   = redact(read_file("report.txt"))      # built-in file I/O, auto-redacted
slow   = filter("is_slow", scores)            # higher-order builtins by fn name
report = json_encode({
    "scores": scores,                         # multi-line literals just work
    "slow": len(slow),
})
```

```
cargo build --release -p nexus-cli      # builds the `weft` binary
target/release/weft app apps/showcase/credit_risk.nx      # mixed: proof + parallel scoring
target/release/weft app apps/showcase/build_pipeline.nx --trace  # tool automation + live logs
target/release/weft run-guarded apps/algorithms/dijkstra.nx      # hard limits + rollback
target/release/weft bench                                         # benchmarks vs traditional stacks
```

> The compiled binary is `weft`. The Cargo crates internally use the `nexus-`
> prefix (internal naming; not user-facing). Environment variables use `NEXUS_`
> prefix (see CLI section).

**Documentation:** full end-user guide → **[docs/GUIDE.md](docs/GUIDE.md)**.
To have an AI agent write WeftScript, give it **[docs/AI_AGENT_PROMPT.md](docs/AI_AGENT_PROMPT.md)**.
See **[APP_GALLERY.md](APP_GALLERY.md)** for 30 runnable programs, and
**[COMPARISON.md](COMPARISON.md)** for measured head-to-head benchmarks against
Python, TypeScript, and Rust (same programs, identical outputs, reproducible
via `bench/cross/run.sh`).

---

## What it measures (and proves)

The benchmark harness (`weft bench`) measures WeftScript against a traditional
stack implementing the *same* enterprise billing system:

| Dimension | Result |
| :-------- | :----- |
| **Head-to-head runtime** (same program, [COMPARISON.md](COMPARISON.md)) | full pipeline **4.1× faster than Python, 2.3× faster than TypeScript**; edit-to-result **4.5× faster than Rust** (no compile step); `vdot` beats numpy |
| **Token usage** (LLM/human authoring) | **3.6× fewer tokens, 72% saved** vs equivalent TypeScript; capability-parity pipeline: 1.8×/2.8×/3.1× fewer than Python/TS/Rust |
| **Compile speed** | full parse+lower in **~130 µs** (vs seconds for `tsc`) |
| **Resource use** | content addressing dedups **98.8%** of redundant inserts |
| **Time-travel cost** | 50 version snapshots share **98%** of nodes |
| **Auto-parallelism** | **3.8× speedup** from graph structure, identical output CIDs |
| **Safety** | no-overdraft invariant **formally proven** in ~8 µs; unsafe code rejected with a concrete counterexample |

These map directly onto the goal: *faster, safer, leaner, smarter, and easier
for both humans and AI to author, use, and debug.*

---

## Architecture

Fifteen crates, each independently tested:

| Crate | Responsibility |
| :---- | :------------- |
| **nexus-core** | 512-bit BLAKE3 CIDs · the seven primitives · content-addressed `GraphStore` with automatic dedup |
| **nexus-dsl** | indentation-aware lexer + parser → unified graph; parses the full reference program |
| **nexus-graph** | triple-graph partitioning (Knowledge/Plan/Execution) · Semantic Git (structural diff/merge) · time-travel |
| **nexus-verify** | exact-rational SMT solver (Fourier–Motzkin + model extraction) · symbolic proof of safety invariants · **sound nonlinear/bilinear reasoning** via sign case-split |
| **nexus-planner** | task-DAG auto-parallelism · HEFT scheduling · multi-objective Pareto routing · budget-aware fallback |
| **nexus-capability** | semantic capability discovery · MCP over JSON-RPC 2.0 · AI model router · universal data routing |
| **nexus-runtime** | dependency-driven parallel execution · cryptographic provenance · retry/backoff · knowledge-loop feedback |
| **nexus-optimize** | Triple Graph Grammar view sync · DSL projection · TuRBO Bayesian prompt optimization · digital-twin simulation |
| **nexus-exec** | general-purpose interpreter — Pratt parser, classes, exceptions, generators, comprehensions, interpolation, **language-level parallelism** (`pmap`/`parallel`), **hard resource limits + rollback**, 90+ builtins; Python-parity, token-competitive |
| **nexus-accel** | SIMD vectorization (`portable_simd`) · multithreading · cost-based CPU/GPU dispatch |
| **nexus-gpu** | real `wgpu`/WGSL compute backend (106× on compute-bound kernels) |
| **nexus-tool** | terminal/CLI/software automation — shell exec with timeouts, declarative command templates, command gen + mutation, auto-correction retry, parallel offload, **real networking** (TCP/HTTP sockets, proxy/anon routing) |
| **nexus-secure** | built-in **security/privacy/anonymity** — PII/secret redaction (Luhn-checked cards, SSN, email, IP, keys), capability sandbox (command + host allow/deny), stable pseudonyms + ephemeral tokens |
| **nexus-dashboard** | optional zero-dep web dashboard — view outputs/traces, run programs from the browser; **off by default (headless)** |
| **nexus-app** | mixed declarative+executable loader; host builtins `validate`/`check`/`prove`/`things`/`fields` |
| **nexus-cli** | the `weft` binary, benchmarks, token proof, MCP server (stdio **and TCP**), `run-guarded`, `--trace` |

Cross-cutting: **`nexus-core::trace`** — a zero-dependency structured tracing
sink (leveled, span-aware, thread-safe) that every subsystem feeds, so any run
can be followed end-to-end with `--trace`.

### The seven true primitives

`Thing` · `Intent` · `Constraint` · `Policy` · `Event` · `Capability` ·
`Contract` (plus composite `Workflow` and `Agent`). Every declaration — text,
diagram, or AI-emitted — lowers into the *same* directed graph, where each node
is identified by the BLAKE3 hash of its canonical encoding.

---

## The DSL

A clean, indentation-structured, context-free language designed for minimal
tokens. The complete reference program lives in
[`examples/enterprise_billing.nx`](examples/enterprise_billing.nx).

```
thing Invoice
    id: UUID
    amount: Decimal
    status: String

guarantee NoOverdraft
    assume balance == start - amount
    assume start - amount - minimum_reserve >= 0
    assume minimum_reserve >= 0
    ensure balance >= minimum_reserve
```

`weft check` proves that guarantee for **all** execution paths. Remove the
guard assumption and the compiler rejects the build with a concrete
counterexample (`balance = -1/2 < 0`). The prover handles **nonlinear**
guarantees soundly — e.g. `tax >= 0` given `tax == income*rate`, `income >= 0`,
`rate >= 0` is *proven* via a sign-aware case-split, while never reporting a
spurious counterexample.

---

## Beyond declarations: a full runnable language

The executable layer is Python-parity and then some.

**Parallelism, multiprocessing & hardware dispatch.** Transparent by default —
just write ordinary code and WeftScript auto-threads large pure comprehensions.
Manual control is also available.

```
fn score(x)
    return x * x
results = pmap("score", rows)          # one thread per chunk, order preserved
auto    = amap("score", rows)          # runtime auto-picks serial vs parallel
procs   = mp_map("print input * 2", rows)  # each item in its own OS process
totals  = parallel(["job_a", "job_b"]) # run independent functions concurrently
print vdot(a, b)                        # auto-dispatched: Cpu/scalar … Gpu by size
big    = [score(x) for x in rows]      # auto-parallelized if rows >= 256 + pure body
```

**Built-in security, privacy & anonymity.** Always on, zero configuration.

```
print sh("rm -rf /")                    # blocked by the auto-sandbox (error)
print redact("card 4111 1111 1111 1111")  # -> card [REDACTED:credit_card]
print anonymize("alice@corp.com")       # -> anon_9b617b2a7f7c  (stable pseudonym)
# env: NEXUS_ALLOW_CMDS / NEXUS_DENY_HOSTS (sandbox), NEXUS_PROXY (anonymity)
```

**Optional dashboard (headless by default).**

```
weft dashboard --port 8787            # serve the control/observability UI
weft app pipeline.nx --dashboard      # run, then surface output/traces on the web
```

**Terminal / tool automation.**

```
r = sh("echo build")                   # -> {code, stdout, stderr, ok, timed_out}
outs = psh(["echo a", "echo b"])       # parallel offload across processes
fixed = auto_fix("ecHo deploy")        # command gen + mutation + retry until it works
```

**Real networking.**

```
print tcp_line("127.0.0.1:8765", "{\"method\":\"ping\",\"id\":1}")
weft mcp-serve --tcp 127.0.0.1:8765   # MCP/JSON-RPC over the network
```

**Hard resource protection + rollback/recovery.**

```
weft run-guarded job.nx               # exit 0 = clean, 7 = breach contained & rolled back
NEXUS_ALLOC_CELLS=5000 NEXUS_WALL_MS=200 weft run-guarded job.nx
```

**Trace everything.**

```
weft app pipeline.nx --trace
# [    0] +     0us DEBUG exec.parallel: fan out {jobs=4, threads=4}
# [    1] +   905us INFO  app: computed squares
# [    2] +  1009us DEBUG exec.accel: dispatch vdot {n=3, device=Cpu, backend=scalar}
```

---

## CLI

```
weft parse  <file.nx>   Parse and summarize
weft build  <file.nx>   Full compiler pipeline (parse→plan→optimize→synthesize→prove→bind)
weft check  <file.nx>   Formally verify safety guarantees
weft fmt    <file.nx>   Re-project the graph to canonical DSL (TGG round-trip)
weft graph  <file.nx>   List content-addressed nodes and CIDs
weft plan   <file.nx>   Schedule the workflow (auto-parallel levels + HEFT + Pareto)
weft run    <file.nx>   Execute the workflow DAG with provenance
weft tokens <file.nx>   Compare token usage vs a traditional baseline
weft mcp                Capability discovery over JSON-RPC 2.0
weft demo               End-to-end demonstration of every subsystem
weft bench  [--write]   Run the benchmark suite (writes BENCHMARKS.md)
```

---

## Design choices

* **Exact arithmetic everywhere** (rationals, not floats) — content addressing
  is deterministic and SMT proofs are real proofs, not numerical approximations.
* **In-house SMT engine** — no Z3 dependency; the verifier is exact, embeddable,
  and adds zero external footprint.
* **Structural sharing** — a single content-addressed pool backs all versions,
  making time-travel and re-builds near-free.
* **Lean dependency tree** — only `blake3`, `serde`, `serde_json`. LTO +
  `panic=abort` + stripped for minimal binaries.
* **Parallelism from structure** — concurrency is derived from the DAG, never
  hand-written, and is proven to produce bit-identical results to serial runs.

---

## License

WeftScript is released under the **WeftScript Permissive License** (see
[LICENSE](LICENSE)). You may use, modify, and distribute it freely. The only
requirement is that you include a brief acknowledgement somewhere in your
project that you used WeftScript by Cognitive Industries / CogForgeLabs.

---

## Made by

**Cognitive Industries / CogForgeLabs**
<https://github.com/CogForgeLabs> · cognitive-industries.org · contact@cognitive-industries.org
