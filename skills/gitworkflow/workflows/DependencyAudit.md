# Dependency Audit Workflow

Surface open Dependabot alerts for the current repo, grouped by severity, with remediation offered. **Advisory only** — invoked by every Commit and runnable standalone, it never blocks, delays, or aborts anything: every missing precondition degrades to one clear line and the caller continues.

## Behavior contract

Query `gh api repos/{owner}/{repo}/dependabot/alerts -f state=open --paginate`, capturing the exit status separately so API errors degrade instead of aborting. Outcomes:

| Outcome | Line (then continue) |
|---------|----------------------|
| Alerts found | severity rollup + detail list (below) |
| None | `✅ No open Dependabot alerts.` |
| 403 | `ℹ️ … disabled or token lacks the security_events scope.` (fix: `gh auth refresh -s security_events`) |
| 404 | `ℹ️ … not available for this repo.` |
| gh missing / unauthenticated / non-GitHub remote / any other error | one-line reason, skip |

## Report (alerts present)

Headline the critical/high counts; list alerts most-severe first, one line each: `[SEVERITY] package — advisory summary (url)`. Then list open Dependabot security PRs (`gh pr list --author "app/dependabot"`) so the user can act immediately.

Remediation is **offered, never auto-acted**: enable/tune Dependabot via CISetup's dependency categories; merge listed security PRs only with explicit consent; re-run standalone via `/GitWorkflow audit`.

```
## Dependency Audit
**Repo:** owner/repo
**Open alerts:** N (🔴 critical: A · 🟠 high: B · 🟡 medium: C · ⚪ low: D)
**Alerts:** <one line each, most severe first>
**Open Dependabot PRs:** <#N title — url | none>
**Status:** ℹ️ Advisory only — the surrounding workflow proceeds regardless.
```

On any skip, the report collapses to the single skip line plus the Status line.
