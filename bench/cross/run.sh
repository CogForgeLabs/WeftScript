#!/usr/bin/env bash
# Cross-language benchmark runner. Reproduces the numbers in ../../COMPARISON.md.
# Requires: the release `weft` binary (cargo build --release -p nexus-cli),
# python 3.x, node >= 22.18 (built-in TypeScript type stripping), rustc.
set -u
cd "$(dirname "$0")"
W="../../target/release/weft.exe"
[ -f "$W" ] || W="../../target/release/weft"

best() { # best-of-3 wall time in ms for a full process run
    local min=999999999
    for _ in 1 2 3; do
        local s e ms
        s=$(date +%s%N)
        "$@" > /dev/null 2>&1
        e=$(date +%s%N)
        ms=$(( (e - s) / 1000000 ))
        [ "$ms" -lt "$min" ] && min=$ms
    done
    echo "$min"
}

echo "== compiling Rust versions (timed: this is Rust's edit-run cost) =="
for f in accumulate parscore vdot pipeline; do
    s=$(date +%s%N)
    rustc -O "$f.rs" -o "$f.exe"
    e=$(date +%s%N)
    echo "rustc -O $f.rs: $(( (e - s) / 1000000 ))ms"
done

echo
echo "== correctness: outputs must match across languages =="
for b in accumulate parscore vdot pipeline; do
    echo "-- $b"
    "$W" app "$b.nx"
    python "$b.py"
    node "$b.ts" 2>/dev/null
    "./$b.exe"
done
python vdot_numpy.py

echo
echo "== timings (best of 3, total process wall time, ms) =="
echo "bench,weft,python,ts_node,rust_run_only"
echo "accumulate,$(best "$W" app accumulate.nx),$(best python accumulate.py),$(best node accumulate.ts),$(best ./accumulate.exe)"
echo "parscore,$(best "$W" app parscore.nx),$(best python parscore.py),$(best node parscore.ts),$(best ./parscore.exe)"
echo "vdot,$(best "$W" app vdot.nx),$(best python vdot.py),$(best node vdot.ts),$(best ./vdot.exe)"
echo "vdot_numpy,-,$(best python vdot_numpy.py),-,-"
echo "pipeline,$(best "$W" app pipeline.nx),$(best python pipeline.py),$(best node pipeline.ts),$(best ./pipeline.exe)"

echo
echo "== LLM token cost (capability-parity pipeline) =="
for f in pipeline.py pipeline.ts pipeline.rs; do
    "$W" tokencmp pipeline.nx "$f" | tail -4
done
