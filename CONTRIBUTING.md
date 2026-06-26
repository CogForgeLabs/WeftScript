# Contributing to WeftScript

WeftScript is an open project and welcomes contributions from everyone —
humans, AI assistants, teams, and individuals at any experience level.

## Who can contribute

**Anyone.** That includes you, your AI pair-programmer, a bot you wrote, or a
pull request generated entirely by an LLM. We evaluate contributions on their
merit, not their origin. If the code is correct, safe, well-tested, and
improves the project — it ships.

If an AI assisted in writing your contribution, you don't need to disclose it,
but you're welcome to. What matters is that a human reviewed and stands behind
the change.

## Getting started

```sh
rustup toolchain install nightly      # the repo pins nightly (portable_simd)
cargo build --workspace
cargo test  --workspace
```

The `weft` binary lands at `target/debug/weft` (or `target/release/weft` with
`--release`). Try `weft demo` for an end-to-end tour, or
`weft app apps/showcase/credit_risk.nx` to run a sample program.

## What to work on

- **Language builtins** — new builtins belong in `crates/nexus-exec/src/builtins.rs`
  with a test in `crates/nexus-exec/tests/apps.rs`.
- **DSL / parser** — `crates/nexus-dsl/`.
- **Verification / SMT** — `crates/nexus-verify/`.
- **Hardware dispatch / GPU** — `crates/nexus-accel/` and `crates/nexus-gpu/`.
- **Security / privacy** — `crates/nexus-secure/`.
- **Dashboard / tooling** — `crates/nexus-dashboard/` and `crates/nexus-tool/`.
- **Apps and examples** — add `.nx` programs to `apps/` or `apps/showcase/`.
  Runnable examples are the best documentation.
- **Docs** — `docs/GUIDE.md` and `docs/AI_AGENT_PROMPT.md` are the primary
  end-user references; keep them in sync with code changes.
- **Benchmarks** — add cases to `crates/nexus-cli/src/bench.rs`. Be honest:
  report both benefits and drawbacks.

## Before opening a PR

1. `cargo fmt --all` — formatting is checked in CI (advisory, not a blocker).
2. `cargo test --workspace` — all tests must pass.
3. `cargo build --workspace` — must be warning-free.
4. Add tests for new behavior. Interpreter/language changes go in
   `crates/nexus-exec/tests/apps.rs`; cross-cutting features get an integration
   test under the relevant crate's `tests/`.
5. Keep one concern per PR. Large PRs are fine if they have a clear theme.

## Style and design rules

- **Soundness over speed in the verifier.** `nexus-verify` must never report a
  proven or violated result it cannot justify with exact rational arithmetic.
  `ProofResult::Unknown` is always a valid answer.
- **Standard library should be composable, not one-off.** A new builtin should
  work with `pmap`, streaming, and resource limits without special-casing.
- **Benchmark honestly.** Quote ratios, not raw timings. Surface drawbacks.
- **No silent truncation.** If a builtin bounds work (e.g. `range` cap), emit a
  trace event or error — never silently drop items.
- **Absolute numbers are machine-dependent** — write tests that check
  *correctness* (output values, ordering, rollback behaviour), not timing.

## Community rules

1. Be direct and constructive. Critique the code, not the contributor.
2. AI-generated PRs are welcome; the submitter is responsible for correctness.
3. Breaking changes to the DSL or builtins need a migration note in the PR
   description and an update to `docs/GUIDE.md`.
4. The attribution requirement in `LICENSE` applies to derivative works —
   include the one-line credit somewhere in your project.

## Attribution

WeftScript is made by **Cognitive Industries / CogForgeLabs**.
<https://github.com/CogForgeLabs> | cognitive-industries.org | contact@cognitive-industries.org
