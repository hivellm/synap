<!-- RULEBOOK:START v7.0.0 — generated; put custom content outside this block. -->

# CLAUDE.md

Managed by @hivehub/rulebook — few rules, all deliberate.

## Project overrides (user-owned, survive updates, win on conflict)
@AGENTS.override.md

## Commands
- Before each commit: type-check + lint + tests covering the change.
- Before push/PR/task archive: FULL quality gate (type-check → lint → full
  suite), all green. Never bypass hooks — pre-commit/pre-push wiring is the floor.
- Type-check before tests; it is the faster signal.
- No OS schedules (cron, systemd, launchd, schtasks): scheduling lives in the app.

## Values
1. Complete implementations — no stubs or TODO markers; finish, or say concretely why you can't.
2. Root causes, not workarounds — diagnose before changing code; never guess at bug causes.
3. Surgical diffs — touch only what the task needs; match existing style.
4. Simplicity first — the least code that solves the problem; nothing unrequested.
5. Fix forward — never discard uncommitted work.
6. State assumptions — if interpretations diverge, say so instead of picking silently.

## Communication
- Plain words over jargon; gloss a term when it isn't obvious from context.
- Answer first, reasoning after.
- Cut what the reader knows and options you didn't take. Length follows
  the result, not the effort behind it.

## Git safety (requires explicit user authorization)
`reset --hard` · `checkout -- .` / `restore .` · `clean -f` · `push --force` ·
`rebase` on shared branches · `stash` · `branch -D` · switching a shared checkout
with changes you did not author. Yours: status/diff/log/add/commit,
branches you create (create/switch/merge), `git worktree`, PRs via `gh`.
Worktrees live outside the repo tree and come out via `git worktree remove` —
never `rm -rf` a worktree, never delete or move a `.git`.

## Orchestration
Gate first: act on Jev `routing` from the prompt hook, else from
`rulebook_gate {prompt}`; unavailable or undecided → these rules. Main session never does the work itself — delegate each task to one subagent, model set
per call: Fable 5.1 architecture, hard bugs, review; Opus 5.5 edits, tests, docs,
refactors (never Fable for simple work); Haiku 4.5 research, summaries. Plan first;
parallelize independent agents; read reports, not files. Subagents archive their task.
Monitor; pause/restart stalled agents; review, then update CHANGELOG.
See `.rulebook/specs/orchestration.md`.

## Rulebook (on demand — no ceremony for small fixes)
- Multi-session or multi-phase work: track via the `rulebook` MCP (`rulebook_task`).
  Checklist order = dependencies; independent items may run in parallel.
- Undecidable choice? `rulebook_task {action:"ask"}`, show the form. Same request
  again? Capture a learning under one title; at 2+ promote it to a skill.
- Optional: `rulebook_session` (context), `rulebook_memory` (learnings).
- Read a `.rulebook/specs/` spec when work touches its area.
- Analyses: `docs/analysis/<slug>/`, numbered files, one theme each.
- `/compact <focus>` at a task boundary (~60% context); after an archive
  `/clear` is free — state lives in `.rulebook/`.

<!-- RULEBOOK:END -->

<!-- Project-specific reinforcement, preserved across `rulebook update`. -->

## Agents, teams, and parallelism (project preference)

**Use specialized agents and parallel execution aggressively.** A turn that funnels every search, review, and edit through the main thread is usually slower and produces shallower work than splitting it across the right specialists. The agent pool and Teams support are *load-bearing*, not optional polish — reach for them by default, not as a last resort.

### Delegate to the right specialist by default

| Situation | Spawn |
|---|---|
| Open-ended exploration ("how is X wired up?", "where do we call Y?", more than ~3 hops) | `Explore` or `researcher` |
| Code review of just-changed code | `code-reviewer` / `feature-dev:code-reviewer` |
| Implementation plan for a non-trivial feature | `Plan` first, then `feature-dev:code-architect` |
| Refactor / code-smell hunt | `refactoring-agent` |
| Build / CI / Docker / dependency breakage | `build-engineer` or `devops-engineer` |
| Performance / profiling / hot-path analysis | `performance-engineer` |
| Security audit of pending changes | `security-reviewer` |
| Database schema, migrations, query tuning | `database-architect` |
| REST / GraphQL surface design | `api-designer` |
| Accessibility / UX review of frontend changes | `accessibility-reviewer` / `ux-reviewer` |
| Test writing after implementation | `tester` |
| Documentation updates after code changes | `docs-writer` |

When two or more of these apply to one task, **spawn them in parallel from the same turn** (one assistant message, multiple `Agent` tool calls).

### Parallelism is the default execution mode

- Independent searches, reads, or edits → **one message with multiple tool calls**, never a sequential chain.
- Independent shell checks (`git status` + `git diff` + `git log`; `cargo check` + `npx tsc --noEmit`) → batched in a single `Bash` invocation or sent as parallel calls.
- Independent agent runs (research vs. review vs. test writing) → spawned together so they execute concurrently.
- Sequential is only correct when one call's output is required to shape the next.

### Multi-agent work goes through Teams

- Background `Agent` calls without `team_name` are **rejected by the rulebook hook** unless they are the `team-lead` bootstrap.
- Multi-agent parallel work flows: `TeamCreate` → spawn members with `team_name` → members coordinate via `SendMessage`. See `.claude/rules/multi-agent-teams.md`.
- When in doubt, spawn a foreground `team-lead` and let it shape the team — don't try to coordinate background agents from the main thread without a Team.

### Codify recurring patterns into skills and agents

- A recurring sub-task (3+ occurrences) with a clear, reusable shape → create a **skill** under `.claude/skills/` or a slash command under `.claude/commands/` so the next session can invoke it by name.
- A specialist role we keep ad-hoc spawning with similar prompts → register a custom agent under `.claude/agents/` with its system prompt, allowed tools, and trigger conditions baked in. Then call it by name instead of re-explaining the role each time.
- Capture the rationale via `rulebook_decision_create` or `rulebook_learn_capture` so the codification has institutional memory and future sessions know *why* the skill/agent exists.

### What does NOT need an agent

Single-file edits, lookups against a known path (`Read` / `Grep`), one-shot verifications (`cargo check` after a small edit), and tasks where the main thread already holds the relevant context. Delegating those just dilutes context and adds round-trip latency.

### Self-check before answering a non-trivial question

Before replying directly, ask:

1. Is this open-ended enough that an agent would do it deeper?
2. Are there independent sub-tasks I can fan out in parallel?
3. Is this the third time I've done a similar ad-hoc thing? → time to make a skill or agent for it.

If yes to any → delegate / parallelise / codify *before* answering.
