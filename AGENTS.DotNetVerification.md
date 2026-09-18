# Shared Agent Module: .NET Verification

<!-- Adapted from PowerCode dotnet-verification v2026-08-24 for dotask. -->

- Treat the verification commands declared in the root `AGENTS.md` as the
  authoritative repository interface. Use `./build.sh help` or `build.cmd help`
  for the repository's tasks.
- Run focused tests first when they provide faster feedback, then run the
  applicable documentation-only or source/tooling gate declared by the root
  file.
- Do not bypass a repository wrapper by invoking `dotnet test` directly when
  the project requires the complete bootstrap/verification workflow.
- Run `git diff --check` for every change unless the root file declares a
  stricter repository gate that already includes it.
- After build-tooling changes, verify evaluated output paths as required by
  `AGENTS.BuildArtifacts.md`.
- Report the exact commands that ran, what passed, and anything that remains
  unverified. Do not represent interactive, native, deployment, or publishing
  acceptance as automated verification.
