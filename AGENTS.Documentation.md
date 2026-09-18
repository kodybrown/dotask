# Shared Agent Module: Documentation

<!-- powercode-agent-module:documentation:v2026-08-24 -->

- Follow the root `AGENTS.md` project-documentation section for the repository's
  root-file allowlist and authoritative documentation locations.
- For repositories using the canonical restricted layout, keep only
  `README.md`, `AGENTS.md`, `AGENTS.*.md`, and `LICENSE.md` as root Markdown
  files. Place curated documentation under `docs/`, focused feature
  documentation under `docs/features/`, and temporary or historical material
  under `docs/scratch/`.
- Scratch material is not authoritative. Keep one authoritative source for
  each decision.
- Use relative links with exact filename casing, and update indexes and links
  whenever documentation moves.
- Do not link to ignored machine-local notes or commit credentials, customer
  data, or other secrets.
- Treat root `AGENTS.*.md` modules as synchronized policy files, not as general
  project documentation. Do not edit a synchronized copy independently when
  the intended change belongs in PowerCode.
