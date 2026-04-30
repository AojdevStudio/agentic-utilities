# Identity Verification

## Rules

- Verify identity-bearing values before writing them into manifests, READMEs, marketplace files, package metadata, or install commands.
- Use source-of-truth commands such as `gh api user --jq .login`, `git remote -v`, `npm whoami`, `git config user.email`, or registry queries.
- Do not infer GitHub user/org slugs, repo names, package names, versions, or marketplace identifiers from patterns.
- Leave placeholders when the source of truth is unavailable.

## Rationale

Plausible identifiers are not facts. A wrong slug or package name can break package installation and marketplace publication.
