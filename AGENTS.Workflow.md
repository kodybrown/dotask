# Shared Agent Module: Workflow

<!-- powercode-agent-module:workflow:v2026-08-24 -->

This file is an exact PowerCode-managed module. The repository's root
`AGENTS.md` defines project facts and takes precedence over this module.

## Task Startup

- At the start of a task, verify the exact Git checkout, current branch,
  integration branch, worktree location, and status before editing.
- Perform the full startup check once per task. Refresh only the state needed
  after a checkout, branch, or worktree change, an interrupted check, or an
  explicit user request.
- Read only the files needed for the task. Do not preload repository
  documentation or follow every Markdown link.
- If repository metadata, required tools, or the expected task base is
  missing, report the problem instead of creating or repairing repository
  state without approval.

## Checkout And Worktree Safety

- Treat the exact integration checkout and task worktree root declared in the
  root `AGENTS.md` as authoritative. Do not derive them from placeholders.
- Follow the root file's declared worktree mode. Do not create, switch, stash,
  repurpose, integrate, or clean up worktrees or branches unless that mode and
  the user's request authorize it.
- Never repurpose the user-owned integration checkout as a disposable task
  worktree. Preserve unrelated user changes and stop for direction if they
  overlap the task or make the task base ambiguous.
- When an isolated task worktree is used, create it beneath the declared task
  worktree root, base it on the declared integration branch, and use the
  declared task branch prefix.
- Keep follow-up work for the same task in the same checkout or task worktree.

## Scope And Handoff

- Prefer the smallest complete change that satisfies the current request.
- Do not commit known-broken work unless the user explicitly requests a
  checkpoint.
- Run the project-specific verification declared by the root `AGENTS.md` and
  report exactly what passed and what remains unverified.
- After source or tooling changes, provide copy-pasteable `cd`, verification,
  and run commands using the actual current checkout. Omit application run
  commands for documentation-only and other non-runtime changes.
