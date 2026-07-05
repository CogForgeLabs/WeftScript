# Live Cross-Domain Test — Weft vs. Python

The original benchmarks only tested Weft on its *home turf*: a declarative
billing/orchestration system. To find out whether Weft is **actually general
and actually better**, an executable layer (`nexus-exec`) was added so a Weft
program computes and prints real results, and four real apps were built across
four domains — each with an idiomatic Python equivalent producing the same
output.

Run any of them:

```
weft app apps/loan.nx          # finance
weft app apps/wordfreq.nx      # text/data
weft app apps/projectile.nx    # science/physics
weft app apps/todo.nx          # productivity
```

## Correctness — outputs vs. Python (line endings normalized)

| App | Domain | Output match |
| :-- | :----- | :----------- |
| loan | finance (amortization schedule) | identical except 1 line: `226` vs Python `226.0` |
| wordfreq | text (frequency analysis) | **byte-identical** |
| projectile | physics (trajectory) | identical except 2 lines: `0` vs `0.0` |
| todo | productivity (task manager) | **byte-identical** |

All four are **numerically identical**. The only differences are whole-number
display: Weft prints `226`, Python prints `226.0` (because Python's `round()`
returns a float). Weft's rendering is arguably cleaner; neither is wrong.

## Token economy — Weft vs. Python (the honest result)

| App | Weft tokens | Python tokens | Result |
| :-- | -----------: | ------------: | :----- |
| loan | 213 | 188 | **+13% more** |
| wordfreq | 204 | 175 | **+17% more** |
| projectile | 158 | 140 | **+13% more** |
| todo | 202 | 164 | **+23% more** |

**For general imperative code, Weft costs *more* tokens than Python**, not
fewer. Python's `{}` map literals, `m[k]`, `.lower()`, `.split()`, `math.cos`,
and list concatenation are simply more compact than Weft's function-call style
(`set(m,k,v)`, `get(m,k,0)`, `cos(x)`).

The 3.6× token *savings* measured earlier are **real but domain-specific**: they
come from the declarative layer (policies, contracts, workflows, multi-agent
flows, capability binding, formal guarantees), where you declare intent and the
framework supplies the machinery. They do **not** generalize to ordinary
scripting.

## Speed — wall-clock, avg of 10 runs (startup + execution)

| App | Weft | Python |
| :-- | ----: | -----: |
| loan | 24 ms | 71 ms |
| wordfreq | 22 ms | 67 ms |
| projectile | 24 ms | 70 ms |
| todo | 21 ms | 66 ms |

Weft is **~3× faster end-to-end** on these small programs — a compiled binary
with near-zero startup vs. CPython's interpreter startup. For heavy numeric
loops a tree-walking interpreter would narrow or lose this gap against CPython's
optimized C builtins; this advantage is honest but is mostly *startup*, not
*throughput*.

## Authoring friction — what it was like for an LLM to write these

Notes from building the apps:

* **The base system could not do this at all.** Intents carried no executable
  logic; I had to add a whole interpreter (`nexus-exec`) and extend the lexer
  with `=`, `(`, `)`, `%` — tokens the original DSL rejected. That is direct
  evidence the original system was a spec/verification substrate, not a
  general language.
* **Maps and strings are verbose.** `set(t, "name", name)` / `get(counts, w, 0)`
  instead of `t["name"] = name` / `counts.get(w, 0)`; `split(s, " ")` instead of
  `s.split(" ")`. This is the main reason Weft loses on tokens.
* **No tuples or comprehensions.** Sorting map entries by value (for a real
  "top-N words") is awkward; I sidestepped it by iterating sorted keys.
* **Formatting quirks I had to keep in mind:** whole-number floats print without
  `.0`; booleans print lowercase (`true`/`false`). Neither bit these apps but
  both would surface in others.
* **What was genuinely nice:** the indentation grammar reused from the DSL "just
  worked"; the Pratt parser made expressions clean; errors are clear
  (`undefined variable`, `division by zero`, step-limit guard against infinite
  loops). I made fewer silent mistakes than I'd expect in untyped Python because
  the value model is small and explicit.

## Verdict

Weft is now **genuinely general** — it runs real programs across finance, text,
science, and productivity, matching Python's outputs. But "better than normal
code" is **not universal**:

* **Better for declarative, verifiable, orchestrated systems** (its design
  center): far fewer tokens, formal safety proofs, content addressing,
  auto-parallelism, capability routing. *This is where Weft wins decisively.*
* **Not better — slightly worse on tokens — for general imperative scripting**
  vs. Python, though it is faster to start and produces correct results.

The right framing: Weft is a **verifiable intent-and-orchestration fabric with
a general-purpose escape hatch**, not a Python replacement. Use the declarative
layer where it shines; the executable layer makes it complete and adaptable
rather than locked to one example.
