# Smoke-Ticket Eval

A smoke-ticket eval proves the repo is ready for ticket-level unattended orchestration.

This can be implemented against a real tracker or as a local dry-run fixture. Prefer local first; it is cheaper and safer.

## Minimal fixture

Create a tiny issue fixture such as `docs/agent-evals/smoke-ticket.md`:

```markdown
# SMOKE-001: Update smoke-test marker

## Goal
Make the smallest safe change that proves the agent can edit, validate, and hand off.

## Acceptance criteria
- Change the configured marker text from `before` to `after` in the documented fixture file.
- Run the repo validation command.
- Record evidence in the workpad/PR section.
- Do not touch unrelated files.

## Validation
- Run `./scripts/verify.sh` or the repo's equivalent validation command.
```

## Eval harness expectations

A runnable eval should:

1. create a disposable workspace or git worktree
2. render the repo's workflow prompt with the fixture issue
3. launch the selected agent runner or dry-run substitute
4. require the expected file change
5. require validation evidence
6. verify no unrelated tracked files changed
7. clean up workspace state unless debugging is requested

## Pass/fail criteria

Pass only when:
- expected change exists
- validation command was run and recorded
- evidence section exists
- no unrelated files changed
- run completes without human prompt/input

Fail when:
- the agent asks the human what to do despite clear fixture instructions
- it cannot bootstrap the workspace
- it edits outside the workspace
- it skips validation
- it claims evidence without artifact/command proof

## Practical script shape

A repo can expose this as:

```bash
./scripts/agent-smoke-eval.sh
```

or:

```bash
make agent-smoke-eval
```

The script may use a cheap stub agent first. The important part is that the repo has an executable definition of what "ready for unattended ticket work" means.
