<!-- RULEBOOK:START v5.3.0 — DO NOT EDIT BY HAND. Regenerated on `rulebook update`.
     Put project-specific content in AGENTS.override.md or CLAUDE.local.md.
     Anything outside the RULEBOOK:START/END sentinels is preserved across updates. -->

# CLAUDE.md

This project is managed by [@hivehub/rulebook](https://github.com/hivellm/rulebook).
The authoritative rules come from the imports below. Claude Code loads all of them
automatically at session start (see [Anthropic memory docs](https://code.claude.com/docs/en/memory#claude-md-imports)).


## Critical rules (highest precedence — apply on every turn)

1. **Read `AGENTS.md` and `AGENTS.override.md`** before making changes. These contain project-specific conventions that override generic guidance.
2. **Never revert or discard uncommitted work** — fix forward. Treat the working tree as sacred; investigate before destructive operations.
3. **Edit files sequentially**, not in parallel. When a task touches 3+ files, decompose into 1–2 file sub-tasks.
4. **Run `check`/type-check before `test`** — diagnostic-first. Cheap diagnostics catch issues that expensive test suites miss or take longer to surface.
5. **If a fix fails twice, escalate** — stop, research, or open a team. Do not retry the same approach a third time.
6. **Prefer MCP tools** (`mcp__rulebook__*` and project-specific MCP servers) over shell commands when the equivalent tool exists.
7. **Capture learnings**: at the end of significant work, save patterns and anti-patterns to `.rulebook/knowledge/` and insights to `.rulebook/learnings/`.
8. **Never archive a task** without docs updated, tests written, and tests passing — the task tail enforces this structurally.

## Persistent memory

This project uses the Rulebook MCP server for persistent memory across sessions.

- **Start of session**: `rulebook_memory_search` for relevant prior context.
- **During work**: `rulebook_memory_save` for decisions, bugs, discoveries, user preferences.
- **End of session**: `rulebook_session_end` to write a session summary.

Memory is auto-captured for tool interactions (task create/update/archive, skill enable/disable). Manual saves are required for everything else worth remembering.

## Knowledge base

Before implementing anything non-trivial:

- `rulebook_knowledge_list` — check existing patterns and anti-patterns.
- `rulebook_learn_list` — review past learnings.
- `rulebook_decision_list` — review architectural decisions.

After implementing, capture at least one entry per task:

- `rulebook_knowledge_add` for reusable patterns or anti-patterns to avoid.
- `rulebook_learn_capture` for implementation insights that don't belong in code comments.
- `rulebook_decision_create` for significant architectural choices.

## Task workflow

**MANDATORY: ALWAYS use the Rulebook MCP tools for task management.** Never create task directories or files manually — use `rulebook_task_create`, `rulebook_task_update`, `rulebook_task_archive`, `rulebook_task_list`, `rulebook_task_show`, `rulebook_task_validate`. These tools enforce naming conventions, mandatory tail items, phase structure, and metadata that manual file creation skips.

1. `rulebook_task_list` to see pending work.
2. `rulebook_task_create` to create new tasks — **never `mkdir` + `Write` manually**.
3. Pick the **first unchecked item from the lowest-numbered phase** — never reorder.
4. Read the task's `proposal.md` and `tasks.md` before touching code.
5. Implement step by step. Run lint + type-check after each significant change.
6. `rulebook_task_update` to change task status as you progress.
7. Mark items `[x]` in `tasks.md` as you finish them.
8. The mandatory tail (docs + tests + verify) is **not optional** — `rulebook_task_archive` will refuse to close the task otherwise.

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
