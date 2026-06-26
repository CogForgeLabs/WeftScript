# WeftScript — End-User Guide

WeftScript is a meta-language — a level above traditional high-level languages
like Python or TypeScript. You declare *intent*; the runtime handles
parallelism, security, hardware dispatch, resource limits, and privacy
automatically. One `.nx` file does two things:

1. **Declare** software as a verified graph of intent — schemas, constraints,
   and formally-proven safety guarantees.
2. **Run** ordinary, general-purpose code — with classes, exceptions,
   generators, automatic parallelism and multiprocessing, hardware dispatch,
   shell/tool automation, real networking, built-in security/privacy/anonymity,
   hard resource limits, full tracing, and an optional web dashboard.

The two halves talk to each other: running code can validate data, evaluate
declared constraints, and prove invariants from inside the program.

---

## 1. Install & build

```
# From the repository root
cargo build --release            # builds the `weft` binary at target/release/weft
cargo test  --workspace          # run the full test suite
```

Run programs:

```
weft app path/to/program.nx     # run an executable / mixed program
weft run-guarded program.nx     # run under hard resource limits + rollback
weft app program.nx --trace     # stream structured logs while running
```

---

## 2. CLI reference

| Command | Purpose |
| :------ | :------ |
| `weft parse <file>` | Parse and summarize a declarative program |
| `weft build <file>` | Full compiler pipeline (parse → plan → verify → bind) |
| `weft check <file>` | Formally verify declared safety guarantees |
| `weft fmt <file>` | Re-project the graph to canonical DSL |
| `weft graph <file>` | List content-addressed nodes + CIDs |
| `weft plan <file>` | Schedule a workflow (auto-parallel levels, HEFT, Pareto) |
| `weft run <file>` | Execute a workflow DAG with provenance |
| `weft app <file>` | Run an executable / mixed program (general code) |
| `weft run-guarded <file>` | Run under hard limits; exit 0 = clean, 7 = breach contained |
| `weft tokens <file>` | Compare token usage vs a traditional baseline |
| `weft tokencmp <a> <b>` | Compare token/char/line counts of two files |
| `weft mcp` | Demo capability discovery over JSON-RPC 2.0 |
| `weft mcp-serve [--tcp <addr>]` | Run an MCP server over stdio, or TCP |
| `weft dashboard [--port N]` | Serve the optional web dashboard (headless by default) |
| `weft demo` | End-to-end demonstration of every subsystem |
| `weft bench [--write]` | Run the benchmark suite |

**Global flags:** `--trace` (stream structured logs to stderr),
`--dashboard` (with `app`: serve outputs/traces on the web UI),
`--port N` / `--tcp <addr>` (bind address for servers).

`-` as a file path reads the program from **stdin**.

---

## 3. Language guide

Weft source is **indentation-structured** (like Python). A program is either a
plain script (statements at top level) or an `app` with `fn`/`main` blocks; both
work. Comments start with `#`.

### 3.1 Values & variables

```
x = 42                # number (all numbers are 64-bit floats)
name = "Ada"          # string
ok = true             # bool: true / false
nothing = nil         # nil
xs = [1, 2, 3]        # list
person = {"name": "Ada", "age": 36}   # map (string keys)
```

Assignment is `=`. Compound assignment: `+= -= *= /=`.
Integer-valued numbers print without a trailing `.0` (`42`, not `42.0`).

### 3.2 Operators

```
+  -  *  /  %          # arithmetic (+ also concatenates strings/lists)
== != < <= > >=        # comparison
and  or  not           # logical (short-circuit)
-x                     # unary minus
```

### 3.3 Strings & f-strings

Any string may interpolate expressions with `{ ... }`:

```
n = 5
print "n is {n}, squared is {n * n}"      # -> n is 5, squared is 25
```

Escapes: `\"  \n  \t  \r  \\`. Strings support methods:

```
print "A,B,C".lower().split(",")          # -> [a, b, c]
```

> Because `{ }` means interpolation, a **literal** brace in a string is not
> directly supported — use the `<key>` template form (`tool_run`) when you need
> deferred substitution.

### 3.4 Control flow

```
if score >= 90
    grade = "A"
elif score >= 80
    grade = "B"
else
    grade = "C"

for i in range(1, 5)         # 1,2,3,4
    total = total + i

while x > 0
    x = x - 1
```

`for ... in` iterates lists, string characters, or map keys.

### 3.5 Functions & recursion

```
fn fib(n)
    if n < 2
        return n
    return fib(n - 1) + fib(n - 2)

print fib(10)                # -> 55
```

A function with no `return` yields `nil`.

### 3.6 Classes (objects + methods + `self`)

```
class Point
    fn init(self, x, y)      # constructor
        self.x = x
        self.y = y
    fn dist(self)
        return sqrt(self.x * self.x + self.y * self.y)

p = Point(3, 4)
print p.dist()               # -> 5
print p.x                    # -> 3
```

### 3.7 Exceptions

```
try
    throw "boom"
catch e
    print e                  # -> boom
```

`try/catch` catches both explicit `throw` values and runtime errors (e.g. a
division by zero or an out-of-range index).

### 3.8 Generators & streaming

Generators are **lazy streams**: a `yield`ing function produces values on demand
through a bounded channel, so memory stays flat and *infinite* generators are
fine. Consume a stream with a `for` loop (pulls lazily), `take(stream, n)` (a
finite prefix), or `collect(stream)` (materialize to a list). Printing a finite
stream materializes it.

```
fn naturals()
    i = 0
    while true            # infinite — fine, because it's lazy
        yield i
        i += 1

print take(naturals(), 5)        # -> [0, 1, 2, 3, 4]

fn evens(n)
    i = 0
    while i < n
        yield i * 2
        i += 1

print evens(4)                   # -> [0, 2, 4, 6]   (finite stream, materialized)
for x in evens(3)                # pulls one item at a time
    print x
```

### 3.9 Comprehensions

```
squares = [i * i for i in range(1, 6)]            # [1, 4, 9, 16, 25]
evens   = [n for n in range(11) if n % 2 == 0]    # [0, 2, 4, 6, 8, 10]
```

### 3.10 Collections

Lists: `[...]`, index with `xs[i]` (negative indexes from the end), assign with
`xs[i] = v`. Maps: `{"k": v}`, access with `m.k` or `m["k"]` or `get(m, "k")`.

> **Limitation:** list/map literals must be written on a **single line**.

---

## 4. The declarative layer (verified intent)

A program can declare an intent graph. The key blocks:

```
project Bank                 # names the program

thing Account                # a schema / data type
    id: UUID
    balance: Decimal

constraint Solvent           # a runtime-checkable rule
    balance >= 0

guarantee NoOverdraft        # a formally-proven invariant
    assume balance == start - amount
    assume start - amount >= 0
    ensure balance >= 0
```

Field types: `UUID`, `String`, `Decimal`, `Bool`, `Timestamp`, `List`, or a
named `Thing`. `policy` blocks support `restrict <action> -> <target>`,
`encrypt`, `audit <bool>`, `trust`, `default_deny <bool>`.

### 4.1 Mixed programs — code that calls the verified graph

Inside the same file, executable code can use these **host builtins**:

| Builtin | Meaning |
| :------ | :------ |
| `validate(record, "Thing")` | Does the record match the declared schema? → bool |
| `check(record, "Constraint")` | Does the record satisfy a declared constraint? → bool |
| `prove("Guarantee")` | Formally prove a declared safety invariant via SMT → bool |
| `things()` | List declared thing names |
| `fields("Thing")` | List a thing's field names |

```
project Bank
thing Account
    id: UUID
    balance: Decimal
constraint Solvent
    balance >= 0
guarantee NoOverdraft
    assume balance == start - amount
    assume start - amount >= 0
    ensure balance >= 0

fn make(id, bal)
    return {"id": id, "balance": bal}

acct = make("a1", 100)
print validate(acct, "Account")    # -> true
print check(acct, "Solvent")       # -> true
print prove("NoOverdraft")         # -> true  (proven for ALL execution paths)
```

The SMT prover is exact (rational arithmetic) and **sound**, including a class of
nonlinear guarantees (e.g. `tax >= 0` given `tax == income * rate`,
`income >= 0`, `rate >= 0`). It never reports a spurious counterexample.

---

## 5. Parallelism, multiprocessing & hardware

| Builtin | Behavior |
| :------ | :------- |
| `pmap("fn", list)` | Apply a 1-arg function to each element across **threads**; order preserved |
| `amap("fn", list)` | **Automatic**: runtime picks serial (small) vs threaded (large) |
| `parallel(["f", "g", ...])` | Run several 0-arg functions concurrently |
| `mp_map(src, list)` | Run `src` once per element in a **separate OS process** (full isolation) |
| `proc_input()` | Inside an `mp_map` child: the element passed to this process |
| `take(stream, n)` / `collect(stream)` | Pull a prefix / materialize a lazy stream |
| `cores()` | Number of CPU cores |
| `hardware()` | Map describing cores, SIMD lanes, devices, GPU |

**Transparent auto-parallelization.** You usually don't need any of the above:
the interpreter *automatically* runs a large comprehension on multiple threads
when its body is side-effect-free and does real work (calls a function) — same
result, just faster. Plain code like `[score(x) for x in big_list]` parallelizes
itself. Anything with a side effect (printing, shell, I/O) stays serial so output
order is never scrambled. Disable with `NEXUS_AUTO_PAR=0`; reach for `pmap`/
`amap`/`mp_map` when you want to be explicit.

```
fn sq(x)
    return x * x

print pmap("sq", [1, 2, 3, 4])     # threads -> [1, 4, 9, 16]
print amap("sq", [1, 2, 3, 4])     # auto    -> [1, 4, 9, 16]
print mp_map("print input * 2", [10, 20, 30])   # processes -> [20, 40, 60]
```

**Vectorized numerics** auto-dispatch to scalar / SIMD / threads / GPU by size:
`vadd vsub vmul vscale vsum vdot vmean vmax vmin`. `accel_backend(n)` reports
which backend a workload of size `n` would use.

```
print vdot([1, 2, 3], [4, 5, 6])   # -> 32  (SIMD-accelerated dot product)
```

---

## 6. Tool & terminal automation

| Builtin | Behavior |
| :------ | :------- |
| `sh(cmd)` | Run a shell command → `{code, stdout, stderr, ok, timed_out}` |
| `sh_timeout(cmd, ms)` | Same, killed if it exceeds `ms` |
| `psh([cmd, ...])` | Run commands concurrently (parallel offload) |
| `which(name)` | Path to an executable, or `nil` |
| `tool_run(template, params)` | Render a `<key>` template from a map, then run |
| `auto_fix(cmd)` | Run; if it fails, generate corrected variants and retry → result + winning `cmd` |

```
r = sh("echo hello")
print r.ok                          # -> true
print contains(r.stdout, "hello")   # -> true

p = {"word": "build"}
print tool_run("echo <word>", p).ok # deferred template uses <key>, not {key}
```

> Native f-strings are the idiomatic command builder: `sh("echo {word}")`.
> Use the `<key>` form only for deferred templates passed as data.

---

## 7. Networking

| Builtin | Behavior |
| :------ | :------- |
| `tcp_request(addr, payload)` | Send to `host:port`, return the full response |
| `tcp_line(addr, line)` | Send one line, read one response line (JSON-RPC style) |
| `http_get(url)` | HTTP/1.0 GET → `{status, body}` (neutral headers; honors `NEXUS_PROXY`) |

```
print tcp_line("127.0.0.1:8765", "{\"method\":\"ping\",\"id\":1}")
print http_get("http://example.com/").status
```

You can also expose Weft itself over the network:
`weft mcp-serve --tcp 127.0.0.1:8765` runs an MCP/JSON-RPC server other tools
can call.

---

## 8. Security, privacy & anonymity (built-in, on by default)

**Security** — every `sh`/`psh`/`auto_fix`/`tool_run`/`tcp_*`/`http_get` call is
checked against a capability sandbox. Destructive commands (`rm -rf /`, `mkfs`,
`shutdown`, …) are denied even with no configuration.

| Builtin | Behavior |
| :------ | :------- |
| `sandbox_ok(cmd)` | Would the sandbox allow this command? → bool |
| `redact(text)` | Replace detected PII/secrets with `[REDACTED:kind]` |
| `contains_pii(text)` | Does the text contain PII? → bool |
| `pii_kinds(text)` | Which kinds of PII (email, credit_card, ssn, ipv4, phone, api_key) |
| `anonymize(s)` | Stable pseudonym (`anon_…`); same input → same id |
| `anon_token()` | Ephemeral one-off token |

```
print sh("rm -rf /")                 # blocked → runtime error
print redact("card 4111 1111 1111 1111")   # -> card [REDACTED:credit_card]
print anonymize("alice@corp.com")    # -> anon_9b617b2a7f7c
```

**Privacy** — structured logs are auto-redacted of PII/secrets by default, so
sensitive data never lands in a trace line. Disable with `NEXUS_NO_REDACT=1`.

**Anonymity** — `http_get` sends only neutral headers (no cookies/referer) and
routes through `NEXUS_PROXY` (`host:port`) when set; point it at a Tor/SOCKS
bridge to mask the origin.

Configure the sandbox with env vars: `NEXUS_ALLOW_CMDS`, `NEXUS_DENY_CMDS`,
`NEXUS_ALLOW_HOSTS`, `NEXUS_DENY_HOSTS` (comma-separated).

---

## 9. Resource limits, rollback & recovery

`weft run-guarded <file>` runs under hard limits. A breach is caught *before*
exhaustion, state rolls back to the last consistent checkpoint, and the run ends
cleanly — a runaway program never crashes the host.

```
weft run-guarded job.nx                       # exit 0 clean, 7 = breach contained
NEXUS_ALLOC_CELLS=5000 NEXUS_WALL_MS=200 weft run-guarded job.nx
```

Limits via env: `NEXUS_STEPS` (max interpreter steps), `NEXUS_ALLOC_CELLS`
(allocation budget), `NEXUS_WALL_MS` (wall-clock budget).

---

## 10. Logging & tracing

A structured, leveled, span-aware trace runs through every subsystem.

```
weft app pipeline.nx --trace
# [   0] +     0us DEBUG exec.parallel: fan out {jobs=4, threads=4}
# [   1] +   905us INFO  app: computed squares
# [   2] +  1009us DEBUG exec.accel: dispatch vdot {n=3, device=Cpu, backend=scalar}
```

From code: `trace(msg)` logs at info level, `trace(level, msg)` at a named level
(`trace`, `debug`, `info`, `warn`, `error`). `trace_count()` returns how many
records exist. (Note: `log` is the natural-logarithm math function, not logging.)

Control: `--trace` streams live; `NEXUS_LOG=<level>` sets the threshold;
`NEXUS_TRACE_DUMP=1` prints the buffered trace after a run.

---

## 11. The dashboard (optional, headless by default)

Nothing binds a socket unless you ask. Opt in to a web UI that shows program
output and traces and lets you run programs from the browser:

```
weft dashboard --port 8787          # then open http://127.0.0.1:8787
weft app pipeline.nx --dashboard    # run, then serve its output/traces
```

Endpoints: `GET /` (UI), `GET /api/output`, `GET /api/trace`,
`POST /api/run` (body = Weft source → JSON result).

---

## 12. MCP servers

Weft speaks MCP/JSON-RPC 2.0 so other agents/tools can drive it:

```
weft mcp-serve                      # over stdio
weft mcp-serve --tcp 127.0.0.1:8765 # over TCP
```

Methods include `ping`, `list_tools`, `discover`, `bind`, and `run_app`
(execute a Weft program and return its output).

---

## 13. Environment variables (reference)

| Var | Effect |
| :-- | :----- |
| `NEXUS_LOG=<level>` | Minimum trace level captured |
| `NEXUS_TRACE_DUMP=1` | Print buffered trace after a run |
| `NEXUS_STEPS` / `NEXUS_ALLOC_CELLS` / `NEXUS_WALL_MS` | Guarded-run limits |
| `NEXUS_ALLOW_CMDS` / `NEXUS_DENY_CMDS` | Command sandbox allow/deny lists |
| `NEXUS_ALLOW_HOSTS` / `NEXUS_DENY_HOSTS` | Network host sandbox lists |
| `NEXUS_NO_REDACT=1` | Disable automatic log redaction |
| `NEXUS_PROXY=<host:port>` | Route `http_get` through a proxy (anonymity) |
| `NEXUS_BIN` | Path to the `weft` binary used by `mp_map` children |

---

## 14. Gotchas & limitations

- **List/map literals must be on one line.** No multi-line `[ ... ]`.
- **`{ }` in strings is interpolation.** Use `<key>` for deferred tool templates.
- **`log` is natural log**, not logging — use `trace(...)` to log.
- **All numbers are 64-bit floats.** The declarative/verification layer uses
  exact rationals separately.
- **Generators are eager** — `yield` collects into a list rather than streaming.
- **`mp_map` requires the `weft` binary** (it spawns child processes); set
  `NEXUS_BIN` if it isn't on the default path.
- The SMT prover is sound but **incomplete on hard nonlinear goals** — it returns
  *unknown* rather than guessing.

---

## 15. A complete example

```
project Orders

thing Order
    id: UUID
    total: Decimal

constraint NonNegative
    total >= 0

fn order(id, total)
    return {"id": id, "total": total}

fn taxed(o)
    return round(o.total * 1.1, 2)

orders = [order("a", 100), order("b", 250), order("c", 75)]

# validate against the declared schema, then process in parallel
clean = [o for o in orders if validate(o, "Order")]
gross = amap("taxed", clean)

print "orders: {len(clean)}"
print "grand total: {round(vsum(gross), 2)}"
print "all non-negative: {check(order(\"x\", 0), \"NonNegative\")}"
```

Run it: `weft app orders.nx` (add `--trace` to watch it work, or
`--dashboard` to view it in the browser).
