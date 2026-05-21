# Evidence Protocol

Use this reference when a repo needs human-reviewable proof for unattended agent work.

## Evidence goals

Evidence should answer three questions quickly:

1. What behavior was reproduced or inspected before the change?
2. What changed?
3. What proves the acceptance criteria now pass?

Raw logs are not enough. Agents should compress proof into a short workpad/PR section and attach durable artifacts only when they add signal.

## Required evidence by change type

| Change type | Minimum evidence |
| --- | --- |
| Pure docs | rendered/readback check or link check when available |
| Library/API logic | targeted tests + relevant full suite/verify command |
| CLI behavior | command transcript before/after |
| UI behavior | screenshot or video + console error check + test command |
| Runtime/perf | benchmark or trace/log query with threshold |
| Bug fix | reproduction signal before fix + passing proof after fix |
| Infra/CI | dry-run or CI link + failure-mode explanation |
| Security/auth | negative test + positive test; never paste secrets |

## Workpad / PR evidence section

Agents should maintain a compact section like:

```markdown
## Evidence

### Reproduction / baseline
- `command`: outcome summary
- artifact: `path-or-url`

### Validation
- `command`: pass/fail summary
- `command`: pass/fail summary

### UI/runtime proof
- screenshot/video/log query: `path-or-url`
- console/log errors checked: yes/no

### Known limits
- ...
```

## Artifact storage

Prefer durable, reviewable locations:
- PR comment attachments
- issue/workpad attachments
- `tmp/evidence/` ignored by git for local-only proof
- CI artifact uploads

Do not commit bulky generated evidence unless the repo already has that convention.

## Agent instructions to add to repo skills

- Capture evidence immediately after validation while context is fresh.
- Quote only the useful tail/summary of long output.
- Include exact commands, not paraphrases.
- Include failing evidence when blocked.
- Never claim UI behavior was validated without browser/app evidence.
- Never claim CI is green without checking the current commit's checks.
