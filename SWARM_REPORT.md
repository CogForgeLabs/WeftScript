# Production-hardening report — WeftScript

Branch: `swarm/harden-20260705` (all work here; nothing pushed, no crates published).
Date: 2026-07-05.

## Summary

The workspace was already in good shape: it built clean, all tests passed, and the
error handling in the interpreter's hot paths was genuinely defensive (division by
zero, index bounds, and NaN comparisons already return catchable errors rather than
panicking). The work here closes two real crash vectors reachable from a source
program, fixes a sandbox-escape in the shell builtins, clears every clippy lint, and
corrects a handful of documentation mismatches.

## Build and test status

| | Before | After |
|---|---|---|
| `cargo check --all-targets` | clean | clean |
| `cargo clippy --all-targets` | ~30 warnings | 0 warnings |
| `cargo test` (workspace) | all pass (~250 tests, 41 suites) | all pass, +11 new tests |
| Reachable-from-input aborts | 2 (stack overflow, unbounded alloc) | 0 |
| Shell-allowlist bypass | yes | closed |

## Issues found and fixed

### 1. Stack overflow from deeply nested input (crash, reachable from a program)
A source file with a deeply nested expression (thousands of parentheses, or a long
`1+1+1+...` chain) overflowed the native stack and aborted the process with exit 127.
This was uncatchable: `run-guarded` could not contain it. The recursive-descent
expression parser had no depth limit.

Fix: a depth guard in `parse_bp`/`parse_prefix` (limit 128, safe on a default ~1 MiB
stack) returns a clean parse error past the limit. The CLI now also runs its command
dispatch on a 64 MiB worker thread, so the deepest program the parser accepts always
evaluates without risking overflow in the recursive evaluator and its analysis passes.
Commit `93ac214`.

### 2. Unbounded string allocation (crash, reachable from a program)
`"x" * n`, `repeat`, `rjust`, and `ljust` had no ceiling on the produced string. A
large multiplier (`"x" * 1e15`) requested a petabyte-scale allocation and aborted the
process instead of returning an error. Fix: all four route through `checked_repeat`,
which uses checked multiplication and a 64 MiB cap and returns a catchable error.
Commit `93ac214`.

### 3. Sandbox escape via command chaining (security)
The shell builtins (`sh`, `sh_timeout`, `tool_run`, `psh`, `auto_fix`) run the whole
command string through `sh -c` / `cmd /C`, but the sandbox only vetted the first token.
An allowlist could be bypassed by chaining a second program — `echo x; curl evil`,
`echo x && rm file`, `a | sh` — and a denylisted program hidden after a separator
slipped through. Fix: `check_command` now splits on `;`, `&&`, `||`, `|`, `&`, and
newlines and vets every segment. When an allowlist is active it also refuses command
substitution and process substitution (`$(...)`, backticks, `>(...)`, `<(...)`), which
would otherwise hide the spawned program from the lexical check. The always-on
destructive denylist is unchanged. Commit `308c2af`.

### 4. Clippy warnings (quality)
Cleared all ~30 lints so `cargo clippy --all-targets` is warning-free: `map_or` to
`is_none_or`, `needless_range_loop` to enumerate/zip, `while_let_loop`, and a dozen
others. The `Rational` arithmetic methods in nexus-core keep their names (deliberate
public API) with a scoped `allow` and a comment. No behavior change. Commit `34e8a6d`.

### 5. Documentation drift (quality)
- README said "Fifteen crates"; the table lists 16. Corrected to "Sixteen".
- The CLI reference table was missing `app`, `run-guarded`, `tokencmp`, `dashboard`,
  and `mcp-serve` — all live subcommands, and `tokencmp` was referenced by
  COMPARISON.md. The table now lists every command.
- The release-profile bullet claimed `panic=abort`; the workspace deliberately keeps
  `panic=unwind` so the parallel builtins can recover a worker-thread panic. Text now
  matches Cargo.toml. Commit `650df7e`.

### 6. `weft bench --help` footgun (quality)
`weft bench --help` (or any unrecognized flag) ignored the flag and ran the whole slow
benchmark suite. It now prints usage. Commit `e498e54`.

## Verification

- `cargo clippy --all-targets`: 0 warnings.
- `cargo test`: 41 suites, 0 failures, including 11 new regression tests (string-cap,
  parser depth limit, chained-command bypass, denylist-after-separator, substitution
  refusal).
- End-to-end smoke test of the real programs and commands still passes:
  `weft app apps/showcase/credit_risk.nx`, `weft run-guarded apps/algorithms/dijkstra.nx`,
  `weft check examples/enterprise_billing.nx`, `weft tokens ...`, `weft demo`. All example
  apps in `apps/` run and produce the documented output.
- Confirmed the wall-clock guard actually stops an infinite loop under `run-guarded`
  (breach contained, state rolled back, exit 0).

## Secrets and risk flags

- `.env` is present at the repo root. It is listed in `.gitignore`, is not tracked, and
  was not read, moved, rotated, or deleted. `git check-ignore` confirms it is ignored.
- No real secrets are committed. The only matches for key-shaped patterns in tracked
  files are test fixtures in `nexus-secure/src/redact.rs` (AWS's documented dummy key
  and a fake `sk-` token used to exercise the redactor).

## Remaining gaps

None of these block a release; they are the next round of polish.

- The sandbox is a lexical, best-effort shell filter by design (its own doc comment
  calls it "lightweight"). It now resists the common chaining and substitution
  bypasses, but a determined program with an allowlisted interpreter (`python`, `sh`)
  can still run arbitrary code inside that interpreter. True confinement would need OS
  sandboxing (seccomp, job objects), which is a larger piece of work.
- The DSL `run` path and the `nexus-exec` `app` path are two different parsers; a `.nx`
  file that runs under `app` is rejected by `parse`/`run` (different language surface).
  This is by design but is a source of confusion; documenting which commands accept
  which surface would help.
- Verifier internals (`nexus-verify`) have a couple of non-test `unwrap`s in
  `symbolic.rs` on the bilinear-reasoning path. They are guarded by prior checks in
  practice, but hardening them into typed errors would remove the last panic paths
  outside the interpreter. Lower priority since the verifier runs on declared
  constraints, not raw attacker input.

## Production-readiness

Roughly 95%. The end-to-end language, verifier, planner, runtime, examples, and
benchmarks all work, the two hard-crash vectors and the sandbox bypass are closed,
lints are clean, and the docs match the CLI. The remaining 5% is the deeper sandbox
confinement and the verifier `unwrap` cleanup noted above — neither blocks selling or
shipping, both are worth doing before positioning the sandbox as a hard security
boundary rather than a guardrail.
