# WeftScript vs Python vs TypeScript vs Rust — measured head-to-head

Same programs, written idiomatically in each language, run on the same machine.
Everything here is reproducible: the sources live in [`bench/cross/`](bench/cross/)
and [`bench/cross/run.sh`](bench/cross/run.sh) re-runs the whole suite.

**Machine:** Windows 11, 16 logical cores, NVIDIA RTX 4060 Laptop GPU.
**Toolchains:** `weft` release build · Python 3.14.5 · Node 24.15 (native
TypeScript type-stripping) · rustc 1.98 nightly (`-O`).
**Method:** best of 3 full-process runs (includes each runtime's startup).
Outputs were verified **identical across all four languages** before timing.

## 1. Runtime (total wall time, ms — lower is better)

| Benchmark | WeftScript | Python | TypeScript (Node) | Rust (run only) |
| :-- | --: | --: | --: | --: |
| `pipeline` — validate → **parallel** score → redact → file → invariant | **145** | 593 | 335 | 41 |
| `parscore` — CPU-bound scoring of 20k items | **162** | 191 | 109 | 36 |
| `vdot` — dot product of 2M-element vectors | **145** | 511 (numpy: 182) | 107 | 41 |
| `accumulate` — 300k map updates over 3000 keys | 369 | 195 | 127 | 63 |

Process-startup floors under this harness (`print 1`): weft 90 ms, python 74 ms,
node 70 ms, rust 32 ms — subtract those to compare pure compute.

What the table says:

- **Weft beats Python on every workload with real work in it** — 4.1× on the
  full pipeline, 3.5× on vector math (still 1.25× faster than *numpy*), and
  1.2× on parallel scoring — because parallelism and SIMD are automatic. Python
  only wins the pure-interpreter-overhead microbenchmark (`accumulate`).
- **Weft beats TypeScript 2.3× on the pipeline**, the only benchmark where more
  than one core matters and where Node's `worker_threads` boilerplate is the
  idiomatic path. V8's JIT wins tight single-threaded loops.
- **Rust wins raw runtime everywhere** — it's a native AOT compiler; that is
  exactly what it's for. But see the next section for what each Rust run costs
  *before* it can run.

## 2. Edit-run latency (change one line → see the result)

| | WeftScript | Python | TypeScript | Rust |
| :-- | --: | --: | --: | --: |
| Compile step | none (parses in ~20 µs) | none | `tsc` type-check (not measured here; Node strip-types skips checking) | `rustc -O`: **266–616 ms** per file measured |
| `pipeline` edit-to-result | **145 ms** | 593 ms | 335 ms | 616 + 41 = **657 ms** |

On the develop-run loop that LLM agents actually live in — write, run, read
error, repeat — Weft's total time-to-result on the pipeline beats even Rust by
**4.5×**, and each retry costs no build step. (These are single-file `rustc`
numbers; a real Cargo project with dependencies compiles slower.)

## 3. Cost for LLMs (tokens to author the same capability)

The parity benchmark implements the identical spec in all four languages:
schema validation + constraint check + **parallel** CPU-bound scoring + PII
redaction + report file + invariant verification. Weft ships all of that
built-in; the others assemble it by hand (`ProcessPoolExecutor` + `__main__`
guard, `worker_threads` plumbing, hand-rolled thread pool + email masker).

| Implementation | Tokens | vs Weft |
| :-- | --: | --: |
| `pipeline.nx` (WeftScript) | **223** | — |
| `pipeline.py` (Python, stdlib) | 398 | 1.8× more |
| `pipeline.ts` (TypeScript, no deps) | 617 | 2.8× more |
| `pipeline.rs` (Rust, std only) | 695 | 3.1× more |

Counted with `weft tokencmp`. On the declarative billing reference (schema +
invariants + workflow), the gap is wider still: **3.6× fewer tokens than the
equivalent TypeScript stack** (see `weft bench` / [BENCHMARKS.md](BENCHMARKS.md)).
And tokens are only the linear part of the cost: every capability an LLM does
not have to hand-assemble is a class of bugs it cannot introduce, and every
retry loop is 145 ms instead of a build.

## 4. What is built in vs assembled by hand

| Capability | WeftScript | Python | TypeScript | Rust |
| :-- | :-- | :-- | :-- | :-- |
| Parallel map across cores | `amap("f", xs)` or automatic | `ProcessPoolExecutor` + guard | `worker_threads` plumbing | threads/rayon crate |
| Transparent auto-parallelism (pure code) | **yes** (proven side-effect-free) | no | no | no |
| SIMD vector math | automatic (`vdot`, …) | numpy (dependency) | no | manual/nightly |
| Formal proof of invariants | `prove("G")` — SMT, µs | no | no | no (external tools) |
| Schema validation | declarative `thing` + `validate` | manual / pydantic | manual / zod | serde + validator crates |
| PII redaction (also auto-applied to logs) | `redact(s)` built-in | regex by hand | regex by hand | regex crate by hand |
| Command sandbox (destructive-command denial) | on by default | no | no | no |
| Resource limits + rollback recovery | `weft run-guarded` | no | no | no |
| Lazy infinite generators, O(1) memory | `yield` streams | generators | generators | iterators |
| Build configuration required | none — one binary | none | tsconfig/node_modules for real projects | Cargo project |

## 5. Where the others win (measured, not hidden)

- **Rust** wins every pure-runtime benchmark (4–35× faster execution than
  Weft's interpreter) — the price is a compile step per iteration, a project
  scaffold, and 3.1× the tokens. Push hot numeric loops into Weft's vector
  builtins (which *are* native SIMD+threads — see `vdot` beating numpy) or keep
  them in Rust.
- **Node/V8** wins tight single-threaded scalar loops (`accumulate`, serial
  `parscore`) thanks to a JIT; Weft's tree-walking interpreter does not JIT.
- **Python** wins on ecosystem breadth (numpy/pandas/etc.) and the
  interpreter-overhead microbenchmark.

The claim this file supports is therefore precise: **for complete tasks —
validated, parallel, redacted, verified — WeftScript is the fastest
time-to-correct-result of the four for both humans and LLMs, at a fraction of
the authoring cost, while staying within 2–4× of scripting-language peers on
worst-case micro-loops.**
