---
name: deep-dive
description: "USE WHEN: deep dive into X, break this down for me, audit my approach, give me a thorough breakdown, how should I manage X, create a policy for X, expert rundown on X. Delivers structured opinionated analysis with one clear path forward."
---

# Deep Dive Skill

Deliver comprehensive, opinionated, actionable analysis on any topic. The core value
proposition: the user gets ONE clear path forward, grounded in their actual context and
current documentation — not a menu of options to choose from.

---

## Step 1: Detect Context and Fetch Live Docs

### 1a: Scan project context (Claude Code only)

When running in Claude Code with file system access, scan for project context before starting:

```
Check for:
- package.json / bun.lock / Cargo.toml / pyproject.toml -> stack & package manager
- monorepo indicators (workspaces field, pnpm-workspace.yaml, turbo.json)
- .env files -> infrastructure hints (DB, cloud provider, etc.)
- README.md -> project description and architecture notes
- Directory structure (src/, apps/, packages/) -> project shape
```

Use what you find to ground the deep dive in the user's actual setup. Reference specific
files, configs, and dependency versions when relevant. If no file system is available
(Claude.ai), skip this step and work from what the user provides conversationally.

### 1b: Fetch current documentation (Claude Code only)

When the topic involves specific tools, frameworks, or services, use the `find-docs` skill
(Context7 CLI) to pull up-to-date documentation BEFORE writing the analysis. This prevents
stale advice from training data. For example:

- Topic mentions Supabase -> fetch current Supabase docs for the relevant feature
- Topic mentions Bun workspaces -> fetch current Bun workspace docs
- Topic mentions Vercel env vars -> fetch current Vercel environment variable docs

This is important because tools evolve fast. The user deserves advice grounded in the
current API, not last year's. If `find-docs` is not available, proceed with training
knowledge but note where documentation should be verified.

---

## Step 2: Challenge the Premise

Before diving into "how to do X with Y," evaluate whether Y is the right tool. The user
may have inherited a stack choice, copied a tutorial, or picked something without comparing
alternatives. A deep dive that optimizes the wrong approach is worse than useless.

**What this looks like in practice:**

- User says "manage lockfiles in my Bun monorepo" -> First ask: is Bun actually the best
  monorepo tool for your situation? Compare Bun vs pnpm vs yarn workspaces. Pick the winner
  based on the user's actual constraints (team size, ecosystem needs, build speed). THEN
  dive into how to set it up correctly.

- User says "secrets management with .env files" -> First ask: what's the right secrets
  architecture for your deployment target? Don't just fix the .env approach — recommend
  the best approach from scratch.

- User says "should we use Git Flow" -> Pick the best workflow outright. Don't present
  a balanced comparison — make a call.

**The pattern:** Zoom out before zooming in. Recommend the best tool/approach, justify it
briefly (2-3 sentences on why alternatives lose), then spend the rest of the analysis on
how to implement the winner correctly.

If the user's current choice IS the best option, say so — "You're already on the right
tool. Here's how to use it properly." Don't manufacture doubt.

---

## Step 3: Classify the Topic

Auto-detect which category the topic falls into. This determines which output sections to
include. The user can override by saying something like "treat this as an ops topic."

### Categories and their signature sections:

**Operational / Infrastructure**
Topics: dependency management, CI/CD, deployment, monitoring, lockfiles, migrations, DevOps.
Sections: Placement & Structure, Current State Audit, Cleanup Plan, Maintenance Cadence,
Commands & Conventions, Do/Don't Table.

**Architecture / Design**
Topics: system design, data modeling, API design, monorepo structure, framework selection.
Sections: Trade-off Analysis, Decision Matrix, Recommended Approach, Migration Path,
Risk Assessment.

**Strategy / Business**
Topics: pricing, go-to-market, team structure, vendor selection, build-vs-buy.
Sections: Framework & Mental Model, Options Analysis, Recommendation, Implementation
Roadmap, Risk Assessment.

**Security / Compliance**
Topics: auth, permissions, secrets management, audit logging, data privacy.
Sections: Threat Model, Current Gaps, Hardening Plan, Policy Recommendations,
Verification Checklist.

**Workflow / Process**
Topics: team conventions, code review, git workflow, release process, onboarding.
Sections: Current Pain Points, Proposed Workflow, Team Conventions, Automation
Opportunities, Rollout Plan.

If the topic spans multiple categories, blend the relevant sections. Don't force-fit.

---

## Step 4: Select Depth Level

Detect depth from the user's phrasing, or default to **Standard**.

| Level        | Trigger Phrases                                   | Scope                                                  |
|-------------|---------------------------------------------------|--------------------------------------------------------|
| **Quick**    | "quick take", "80/20", "high-level", "tl;dr"     | ONE clear recommendation with 2-3 supporting reasons. ~400-500 words. No tables, no sub-sections, no action plan — just the answer and why. |
| **Standard** | (default) "deep dive", "break down", "how should" | 5-8 structured questions answered. Full adaptive sections. Aim for ~2000 words (1500-2500 range). |
| **Exhaustive** | "exhaustive", "comprehensive", "leave nothing out", "full audit" | Every applicable section, edge cases, exception handling, team policy language, example commands. Aim for ~3500 words (3000+ minimum). |

### Quick depth is fundamentally different

At Quick depth, the user wants a decision, not an analysis. Give them:
- The recommendation (1 sentence)
- Why it wins (2-3 bullet points)
- The one exception where you'd change your mind (1 sentence)
- What to do next (1-2 sentences)

That's it. No Situation Assessment, no Do/Don't table, no Action Plan sections. Those
are Standard/Exhaustive features. Quick means the user is in a hurry — respect that by
being brief and decisive.

---

## Step 5: Deliver the Analysis

Structure the response using the applicable sections from the detected category.

### For Standard and Exhaustive depth:

Every deep dive follows this backbone:

1. **Situation Assessment** — What are we dealing with? Ground this in the user's actual
   context (repo structure, stack, symptoms they described). Include the premise challenge
   here — if you're recommending a different approach than what the user assumed, state it
   upfront with brief justification.

2. **Core Questions Answered** — The 3-8 most important questions for this topic, each
   answered with ONE clear recommendation. Not "Option A vs Option B" — pick A or B and
   explain why in 2-3 sentences. The user hired you to make the call.

3. **Category-Specific Sections** — Pull in the relevant sections from the topic category
   above. Not all sections are required every time — include only the ones that add value.

4. **Do This / Don't Do This** — A crisp reference table. Two columns. Short rows.
   Focused on the mistakes the user is most likely to make.

5. **Action Plan** — Numbered steps the user can execute immediately. If the topic has a
   cleanup phase and an ongoing maintenance phase, separate them clearly.

### For Quick depth:

Skip the backbone entirely. Write flowing prose — recommendation, reasoning, exception,
next step. See the Quick depth section above.

---

## Principles

1. **One answer, not a menu.** The user is asking for your expert opinion. Every question
   gets ONE recommendation. Briefly note the 1 condition where you'd deviate, then move on.
   If you find yourself writing "Option A... Option B... Option C..." you're doing it wrong.
   Pick the best one and own it.

2. **Challenge before optimizing.** Zoom out before zooming in. Make sure the user is
   solving the right problem with the right tool before spending 2000 words on implementation.

3. **Ground in live docs.** Use `find-docs` to fetch current documentation for any tool
   or framework mentioned. Training data gets stale; official docs don't.

4. **Be specific to their stack.** If you know they're using Bun, don't give npm advice.
   If you scanned their repo, reference their actual file names and config.

5. **Focus on what to avoid.** The most valuable part of expert advice is knowing what NOT
   to do. Every deep dive should surface the common mistakes, anti-patterns, and traps.

6. **Make it actionable.** Every section should leave the user knowing what to DO, not just
   what to think. Include commands, config snippets, and checklists where applicable.

7. **Respect the depth level.** Quick means ~400-500 words, no structure. Standard means
   ~2000 words with full structure. Exhaustive means ~3500 words with every section.
   Don't pad Quick or shortchange Exhaustive.

---

## Edge Cases

- **User provides a vague topic** ("deep dive into my project"): Ask 1-2 clarifying
  questions before proceeding. Don't guess.
- **Topic doesn't fit a category**: Use the Operational category as default and adapt.
- **User overrides category or depth**: Always respect explicit overrides.
- **find-docs unavailable**: Proceed with training knowledge, but flag areas where the
  user should verify against current docs.
