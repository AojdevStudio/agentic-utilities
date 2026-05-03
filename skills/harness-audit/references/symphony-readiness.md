# Symphony Readiness Overlay

Use this overlay when the user asks for Symphony readiness, ticket-level agent automation, unattended agent execution, or preparing a repo for an orchestrator like Symphony.

Baseline harness readiness asks: **can agents understand and edit this repo?**

Symphony readiness asks: **can an unattended agent pick up a ticket, work in isolation, validate the change, produce reviewable evidence, and hand off through the tracker/PR workflow without babysitting?**

Score each item `✅ Present / ⚠️ Partial / ❌ Missing` with concrete path evidence and one-line gap.

## 1. `WORKFLOW.md` contract

Look for a repo-local `WORKFLOW.md` or equivalent orchestrator contract.

Must include:
- tracker config placeholders, not hardcoded personal secrets
- active, review, rework, merge, and terminal state semantics
- workspace bootstrap hooks or pointers
- agent command / sandbox / approval posture
- prompt body with ticket lifecycle rules
- handoff definition: what counts as ready for human review

Strong signal: workflow is versioned with the repo and written as a durable operating contract, not a one-off launch command.

## 2. Disposable workspace bootstrap

Verify a fresh workspace can be prepared without hidden local state.

Look for:
- clone/setup command
- dependency install command
- environment template (`.env.example`, `mise.toml`, `.tool-versions`, `devcontainer.json`)
- database/service bootstrap command when applicable
- cleanup command

Audit question: from an empty directory, can an agent create the same working environment Symphony will create per ticket?

## 3. One-command validate loop

Look for one stable command that runs the meaningful quality gate:
- tests
- typecheck/compile
- lint/format check
- app smoke check where applicable

Good names: `make all`, `make verify`, `./scripts/verify.sh`, `pnpm verify`, `uv run pytest`, `cargo test`.

Flag commands that only work after undocumented setup, require interactive input, or run only a narrow subset while claiming full validation.

Check this even when baseline artifact #5 passes. Baseline tests can exist while the unattended validate loop is still incomplete or too narrow for Symphony.

## 4. Agent-visible app validation

For UI/service repos, check whether an agent can launch and inspect the app.

Look for:
- `scripts/launch-app.*`, `make dev`, `docker compose up`, or equivalent
- Playwright/Cypress/agent-browser/Chrome DevTools instructions
- deterministic test user/seed data
- console/server log capture
- screenshot/video capture
- shutdown/cleanup instructions

If the repo has user-facing behavior but no agent-visible app path, score this low even if unit tests exist.

## 5. Evidence protocol

Check for instructions that tell the agent what proof to produce.

Evidence may include:
- reproduction signal before changes
- test command and output summary
- screenshot/video for UI changes
- logs/metrics/traces for runtime changes
- PR comment or issue workpad update
- CI/check URL after push

Strong signal: evidence is compressed for humans. Raw logs alone are weak.

## 6. Ticket/PR lifecycle skill

Look for a repo-scoped skill/prompt that covers:
- read ticket and acceptance criteria
- maintain one persistent workpad/comment
- create branch/worktree
- open/update PR
- gather and resolve review comments
- push back or defer out-of-scope feedback
- move to human review
- handle rework by starting clean when needed
- land/merge only through approved flow

This can live under `.codex/skills/`, `.agents/skills/`, `.pi/skills/`, `.claude/skills/`, or `docs/agent-workflows/`.

## 7. Agent-readable observability

Check whether agents can query the signals needed to debug without human screenshots.

Look for:
- local app logs with clear paths
- structured logs
- summarized CI failure extraction
- metrics/traces query instructions
- browser console capture
- production log access docs, with safe scoping

Raw vendor dashboards with no CLI/API path are partial at best.

## 8. Safety, secrets, and workspace policy

Check that unattended execution has explicit boundaries:
- secrets loaded via env vars or secret manager, never pasted into docs
- `.env.example` names required vars without values
- destructive commands documented and guarded
- workspace path isolation expected
- generated artifacts ignored or scoped
- external posting/publishing requires configured credentials and clear policy
- sandbox/approval posture documented

## 9. Smoke-ticket eval

Best readiness proof: a tiny issue can be completed end-to-end by the workflow.

Look for a documented or runnable eval that:
1. creates or simulates a small ticket
2. launches the orchestrated agent flow in a disposable workspace
3. requires a branch/PR or local patch
4. requires validation evidence
5. verifies the handoff state/comment/PR body
6. cleans up disposable state

If no external tracker is available, a local fixture issue file is acceptable for a dry-run eval.

## Output addendum

After the 8 baseline harness artifacts, add:

```markdown
## Symphony readiness overlay

### 1. WORKFLOW.md contract
**Status:** ✅ / ⚠️ / ❌
**Evidence:** ...
**Gap:** ...

[... items 2-9 ...]

## Symphony readiness verdict
Ready / Close / Not ready

## Minimum blockers before unattended orchestration
1. ...
2. ...
3. ...
```

Verdict guide:
- **Ready:** no ❌ items, and a smoke-ticket eval is runnable end-to-end against a fixture. Placeholder docs or non-functional templates do not qualify.
- **Close:** no launch-blocking ❌ items in workflow contract, disposable bootstrap, validate loop, evidence protocol, ticket lifecycle, or safety policy; and app validation/observability/smoke eval gaps are documented as remaining work. Also use Close when all items are ✅/⚠️ but the smoke-ticket eval is not yet runnable.
- **Not ready:** no disposable bootstrap, no workflow contract, no validation/evidence path, unsafe secrets/workspace policy, or any other gap that would make unattended execution fail before meaningful work starts.
