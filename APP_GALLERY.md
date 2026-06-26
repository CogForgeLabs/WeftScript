# Weft App Gallery — 29 runnable programs across many domains

Every program here runs with `weft app <file>` and produces real output. They
span everyday apps, classic algorithms, vectorized numerics, parallel pipelines,
CI/tool automation, and **mixed programs that combine a declarative verified
schema with executable logic**. A single test (`crates/nexus-app/tests/apps_gallery.rs`)
runs every one of them end-to-end on each build.

## Everyday apps (`apps/`)

| App | Domain | Output vs Python | Tokens vs Python |
| :-- | :----- | :--------------- | :--------------- |
| `loan.nx` | finance — amortization schedule | identical¹ | 160 vs 164 (−2%) |
| `wordfreq.nx` | text — frequency analysis | identical | 142 vs 150 (−5%) |
| `projectile.nx` | physics — trajectory | identical¹ | 114 vs 124 (−8%) |
| `todo.nx` | productivity — task manager | identical | 122 vs 129 (−5%) |

## Algorithms (`apps/algorithms/`)

| Program | What it shows | Verified output |
| :------ | :------------ | :-------------- |
| `quicksort.nx` | recursion + comprehensions | `[0,1,2,3,5,5,6,7,8,9]` |
| `binary_search.nx` | iterative search | indices 3 / -1 / 7 |
| `sieve.nx` | Sieve of Eratosthenes | primes ≤ 60 |
| `fib_dp.nx` | dynamic programming | fib 0..14 |
| `gcd_lcm.nx` | Euclid's algorithm | gcd(1071,462)=21 |
| `matrix.nx` | **SIMD `vdot` per cell** | `[[19,22],[43,50]]` |
| `knapsack.nx` | 0/1 knapsack, 2D DP | 9 |
| `edit_distance.nx` | Levenshtein, 2D DP | kitten→sitting = 3 |
| `dijkstra.nx` | shortest paths, graphs | `[0,3,1,4]` |
| `caesar.nx` | cipher via `ord`/`chr` | round-trips |
| `rle.nx` | run-length encoding | `a3b3c3d1` |
| `stats.nx` | **SIMD mean/sum/min/max** | mean 15.75, σ 11.399 |
| `mandelbrot.nx` | ASCII fractal | renders set |
| `rpn_calc.nx` | stack RPN evaluator | 35, 2 |
| `collatz.nx` | sequence lengths | longest <30 |
| `roman.nx` | integer → Roman | 2024 = MMXXIV |
| `bank.nx` | **mixed: schema + SMT proof** | proves SafeWithdraw |
| `inventory.nx` | **mixed: schema + checks** | validates stock |

¹ Identical except whole-number floats: Weft prints `226`, Python `226.0`.

## Showcase — mixing declaration, proof, parallelism, tools (`apps/showcase/`)

These combine the declarative core with the new runtime capabilities in one file.

| Program | What it shows | Verified output |
| :------ | :------------ | :-------------- |
| `credit_risk.nx` | schema `validate` + constraint `check` + SMT `prove` + **`pmap` parallel scoring** | per-applicant scores, `SafeApproval proven … true` |
| `data_pipeline.nx` | ETL: validate → **parallel transform** → group-by aggregate | grand total 797.5, per-region totals |
| `sensor_grid.nx` | declared safety band + runtime `check` + **SIMD `vmean`/`vmax`/`vmin`** | flags 2 out-of-band, mean 22.2 |
| `build_pipeline.nx` | `policy` + **tool automation**: `sh` gates, `psh` parallel matrix, `auto_fix` typo-correction | gates passed, 3/3 built, deploy auto-fixed |
| `secure_pipeline.nx` | schema `validate` + **auto multithreading (`amap`)** + **privacy** (`redact`/`anonymize`) | pseudonymized ids, PII scrubbed, no leak |

## Aggregate token economy (8 programs with Python baselines)

Weft **907** tokens vs idiomatic Python **941** — **3% fewer**, with identical
outputs. On the *declarative* billing system the gap is far larger (3.6× fewer).
So: token-competitive-to-better on ordinary code, and dramatically better where
the declarative layer earns its keep.

## Mixed declarative + executable

`bank.nx` declares a `thing Account`, a `constraint Solvent`, and a `guarantee
SafeWithdraw`, then *executable code* calls back into the declarative graph:

```
print "SafeWithdraw invariant proven by SMT: {prove(\"SafeWithdraw\")}"   # -> true
a = account("acc-1", 100)
print "valid against schema: {validate(a, \"Account\")}"                   # -> true
a = withdraw(a, 30)
print "solvent: {check(a, \"Solvent\")}"                                   # -> true
```

`prove`, `validate`, and `check` are host builtins backed by the SMT solver and
the content-addressed schema graph. This is the bridge between Weft's verifiable
core and its general-purpose runtime.

## Hardware acceleration

Numeric vector ops (`vadd`/`vmul`/`vdot`/`vsum`/`vmean`/`vscale`) auto-dispatch
by workload size:

```
accel_backend(10)      -> Cpu/scalar
accel_backend(5000)    -> Cpu/simd
accel_backend(500000)  -> Cpu/simd+threads
```

Measured: a 4M-element dot product runs **2.3× faster** via SIMD+threads than a
naive scalar loop. `hardware()` reports cores, SIMD lanes, and devices.

## MCP server

`weft mcp-serve` is a JSON-RPC 2.0 server over stdio. Any MCP client can
discover/bind capabilities **and execute Weft programs**:

```
$ echo '{"jsonrpc":"2.0","method":"run_app","params":{"source":"print 2+3*4"},"id":1}' | weft mcp-serve
{"id":1,"jsonrpc":"2.0","result":{"output":["14"]}}
```

Proven by an integration test that spawns the server as a subprocess and drives
it over real pipes — including running a mixed program that proves an SMT
invariant on the client's behalf.
