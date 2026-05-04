# deep-dive

Opinionated, actionable analysis on any topic — delivered as **one clear path forward**, not a menu of options.

## What it does

The skill auto-activates when you ask for a deep dive, breakdown, audit, policy, or expert rundown on any topic. It:

1. **Scans your project context** (package.json, monorepo files, README, dir structure) to ground advice in your actual stack.
2. **Fetches live documentation** via the `find-docs` skill (Context7) for any tool/framework mentioned, so advice doesn't go stale.
3. **Challenges the premise** before optimizing — if your assumed tool isn't the best fit, it says so first and recommends the winner.
4. **Auto-classifies the topic** into one of five categories (operational, architecture, strategy, security, workflow) and pulls the relevant section structure.
5. **Scales depth** to your phrasing: Quick (~500 words, decision only), Standard (~2000 words, structured), or Exhaustive (~3500 words, every angle).
6. **Returns one recommendation per question** with a brief justification — never "Option A vs B vs C."

## Trigger phrases

- "deep dive into X"
- "break this down for me"
- "audit my approach to X"
- "give me a thorough breakdown of X"
- "how should I manage X"
- "create a policy for X"
- "expert rundown on X"

## Optional companion

If you have a `find-docs` (Context7) skill installed, deep-dive will use it automatically to pull current docs before writing. Without it, the skill proceeds on training knowledge and flags areas to verify.

## License

MIT — see repository LICENSE.
