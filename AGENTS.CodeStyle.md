# Shared Agent Module: Code Style

<!-- powercode-agent-module:code-style:v2026-08-24 -->

Formatting is owned by repository tools such as `.editorconfig`, formatters,
and linters. Do not restate mechanically enforceable rules in prose.

- Match established nearby patterns before introducing a new abstraction.
- Prefer the smallest clear change that solves the current problem.
- Keep types and functions focused, with names that communicate intent.
- Use guard clauses when they reduce nesting and make failure paths clearer.
- Keep public signatures narrow and deliberate; pass only the data a routine
  needs.
- Prefer declarative collection operations such as LINQ when they improve
  clarity; use explicit loops when control flow, mutation, allocation, or
  performance is clearer.
- Preserve type and null safety; do not weaken or suppress it merely to silence
  a warning.
- Use asynchronous code for genuinely asynchronous work, propagate
  cancellation when the surrounding contract supports it, and avoid blocking
  asynchronous flows.
- Put values in configuration only when users or deployments genuinely need to
  change them; otherwise use a well-named constant or domain type.
- Explain non-obvious intent, constraints, and tradeoffs; do not narrate clear
  code.
- Avoid speculative layers, extension points, dependencies, and configuration.
- Keep public behavior and persisted formats deliberate, with focused tests
  and documentation updated when their contracts change.
