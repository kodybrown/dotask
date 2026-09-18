# Shared Agent Module: Terminal Completion

<!-- powercode-agent-module:terminal-completion:v2026-08-24 -->

This file is an exact PowerCode-managed module. It applies only when the root
`AGENTS.md` selects it and defines the project's changelog and integration
facts.

## Approval Triggers

- Treat either `Shobu ari!` or `Kachidoki!` as a terminal approval trigger when
  it is an unquoted standalone sentence accepting the implemented task. It may
  follow feedback, as in `Works great! Shobu ari!`.
- Quoted, backticked, hypothetical, or design-discussion mentions do not
  trigger completion. A traditional explicit request to commit, update the
  integration branch, and clean up remains valid.
- Terminal approval authorizes the safe local completion workflow for the
  current scoped task. It does not authorize pushing, deployment, publishing,
  remote mutation, or cleanup of user-owned worktrees.

## Changelog Policy

The root `AGENTS.md` must declare the changelog path, trigger, and date basis.

- With `approved-completions-only`, document only completed work that the user
  explicitly approved, and add the entry during terminal completion.
- With `every-agent-commit`, every agent-created commit, including
  documentation and metadata commits, must contain a new or materially updated
  changelog entry.
- Use the declared date basis, append entries to the end of `## Unreleased`,
  and format each entry as `### YYYY-MM-DD Title` without bold text. Keep
  entries in ascending date order and preserve completion or commit order within
  one date.
- Summarize completed outcomes, durable behavior, and verification. Do not add
  deferred, unimplemented, or unrelated work.
- Omit the current commit hash when the entry is included in that commit. Amend
  an unshared local task commit when appropriate; do not rewrite shared history.
- Do not edit a user-maintained notes file during completion unless the user
  explicitly requests that separate edit.

## Completion Workflow

1. Recheck repository status and accepted scope. Stop if unrelated changes
   overlap the task or make the commit ambiguous.
2. Add or update the required changelog entry before the final task commit.
3. Run applicable verification and `git diff --check`, then commit only the
   scoped task.
4. When using a disposable task worktree, follow the root file's declared
   integration procedure: update from the current integration branch, resolve
   and reverify as needed, integrate locally, prove ancestry, and rerun the
   required gate on the integrated branch.
5. Only after successful integration and a clean status, remove the disposable
   worktree without force, prune worktree metadata, and delete its task branch
   and directory. For direct work on the integration checkout, skip worktree
   integration and cleanup.
6. Report the commit, verification, integration proof, cleanup result, and that
   no push occurred unless the user separately requested one.

If any completion step fails, stop, preserve recoverable state, and report the
exact remaining work. Never force worktree removal or branch deletion.
