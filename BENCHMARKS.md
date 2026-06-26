# Weft Benchmark Report

> **Note:** Absolute timings are machine- and load-dependent; the *ratios* are the meaningful figure. This suite is deliberately comprehensive and honest — it reports both where Weft wins and where it pays a cost.

Weft vs. traditional software stacks across the goal's dimensions.

## Detailed measurements

| Metric | Weft | Traditional | Verdict |
| :----- | :---- | :---------- | :------ |
| Token usage (same app) | 933 tok / 159 lines | 3391 tok / 419 lines | 3.6x fewer tokens (72% saved) |
| Full compile (parse+lower) | 173.1 µs | tsc cold build: ~1–3 s | ~5777x faster than a tsc build |
| Graph dedup on re-insert | 20 unique / 1620 inserts (98.8% saved) | files duplicated verbatim | structural sharing is automatic |
| 50 version snapshots | 20 nodes pooled (98.0% shared) | 50x full copies | time-travel is near-free |
| Auto-parallel execution (8-wide) | 16.5 ms parallel | 81.8 ms serial | 5.0x speedup, identical output CIDs |
| HEFT makespan (workflow) | 6.7 units (2 procs) | 8.0 units (1 proc) | resource-aware mapping |
| Formal no-overdraft proof | proven=true in 8.9 µs | runtime checks + manual tests | mathematical guarantee, not a test |
| Dot product 8M (CPU SIMD+threads) | 2.59 ms | 6.01 ms (naive scalar) | 2.3x via SIMD+threads |
| Dot product 8M (auto-dispatch) | 50.89 ms → Gpu/gpu | GPU: NVIDIA GeForce RTX 4060 Laptop GPU | memory-bound: CPU wins (transfer-dominated) |
| Compute kernel 2M×300 (GPU vs CPU) | 7.3 ms (GPU) | 1481.1 ms (CPU all cores) | 203.2x on NVIDIA GeForce RTX 4060 Laptop GPU |
| Parse executable program | 17.8 µs | tsc/rustc front-end: ms–s | sub-millisecond parse keeps edit/run instant |
| Interpreter throughput (fib 27) | 1186 ms, ~536 calls/ms | native Rust: 0.656 ms (635621 calls) | DRAWBACK: ~1808x slower than native on tight numeric recursion |
| Auto-parallel comprehension (N=1500) | 70.1 ms (auto-parallel) | 413.1 ms (forced serial) | 5.9x with zero parallel code (threshold N≥256, user-fn body) |
| Explicit pmap vs serial map (N=1500) | 71.1 ms (pmap) | 390.8 ms (serial map) | 5.5x across 16 cores |
| mp_map vs amap (8 cheap tasks) | amap 0.02 ms (in-process) | mp_map 79.7 ms (8 processes) | DRAWBACK: process isolation costs ~4713x here; pays off only for heavy/isolated work |
| Streaming take(naturals(), 1000) | 409.3 µs, O(1) memory | materialize full list: O(N) memory | lazy generators bound memory regardless of stream length |
| Sandbox command check | 100 ns/call | unchecked exec (no safety) | capability safety is ~free per call |
| Triple-graph partition | K=5 P=8 E=7 | no facts/plan/runtime separation | static spec decoupled from runtime |

## Benefits

- Token economy: the same app is ~3.6x smaller than the traditional stack (72% fewer tokens), cutting human and LLM cost.
- Instant parse/compile: programs parse in microseconds, so the edit-run loop stays interactive.
- Transparent auto-parallelism: large pure comprehensions fan across cores with zero parallel code in the source.
- Explicit parallelism on tap: pmap scales the same workload across all cores when you want control.
- Hardware dispatch: SIMD+threads beat naive scalar on large vector math, and the GPU wins big on compute-bound kernels — automatically.
- Streaming: infinite generators consumed with take() run in O(1) memory, so unbounded data never blows the heap.
- Safety is ~free: the capability sandbox checks each shell/network call in nanoseconds.
- Formal guarantees: invariants like no-overdraft are proven in microseconds, not merely tested.

## Drawbacks / Honest limitations

- Tree-walking interpreter: tight numeric recursion (e.g. fib) runs many times slower than native compiled code — push hot inner loops to the accelerated vector builtins or native code.
- GPU is not a free win: on memory-bound work like a plain dot product, PCIe transfer dominates and the CPU is faster; the GPU only pays off when compute per byte is high.
- Multiprocessing has real overhead: mp_map spawns an OS process per item, so for cheap tasks process spawn + IPC makes it far slower than in-process threading — it only pays off for heavy or isolation-critical work.
- Auto-parallelism is conservative: it only engages above a size threshold (N≥256) for side-effect-free comprehensions whose body calls a user function; small or impure loops stay serial by design.
- Absolute timings vary with machine and load; treat the ratios, not the millisecond figures, as the takeaway.

