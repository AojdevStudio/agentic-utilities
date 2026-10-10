# Analyze Workflow

Review a README and provide actionable feedback without making changes. Score against the *right* rubric — story-driven projects and pure utilities are judged differently.

## Step 1: Read the README

```bash
cat README.md
```

## Step 1.5: Determine Project Mode

Before scoring, classify the README's intended mode (see "Project Mode (Story vs Utility)" in `SKILL.md`). Use the README's own framing as the primary signal — does it currently tell a story, or is it a utility with no narrative? If unclear, ask the user.

- **Story mode** → use the full rubric below; "Story Presence" is a real criterion.
- **Utility mode** → swap "Story Presence" for **Visuals / Diagrams** (described in the rubric). Do not penalize a utility README for missing a personal narrative — it is correctly omitting one.
- **Hybrid mode** → score "Story Presence" leniently (a 2–3 sentence "Why" callout earns a 7+; missing one earns ~5).

Surface the chosen mode at the top of the analysis report so the user knows which lens you applied.

## Step 2: Score on Key Criteria

Rate each criterion from 1-10:

### Scoring Rubric

| Criterion | 1-3 (Poor) | 4-6 (Average) | 7-10 (Excellent) |
|-----------|------------|---------------|------------------|
| **Hook Strength** | No hook, starts with install | Generic tagline | Provocative (Story) or sharp literal one-liner (Utility) |
| **Problem Clarity** | Problem not stated | Problem mentioned vaguely | Vivid pain point (Story) or precise scope statement (Utility) |
| **Unique Insight** | No differentiation | Lists features | Clear "aha" moment (Story) or concrete capability differentiator (Utility) |
| **Visual Hierarchy** | Wall of text | Some headers | Tables, images, centered text |
| **Story Presence** *(Story / Hybrid only)* | No personal narrative | Brief mention | Compelling origin story |
| **Visuals / Diagrams** *(Utility / Hybrid)* | No images at all | One screenshot | Diagram + screenshot/GIF + annotated examples |
| **Technical Clarity** | Confusing or missing | Adequate docs | Clear, well-structured, copy-pasteable |
| **CTA Effectiveness** | No call to action | Weak "please star" | Compelling reason to star (Story) or clear "next step" link (Utility) |

### Generate Score Card

```markdown
## README Analysis: [Project Name]

### Scores

| Criterion | Score | Max |
|-----------|-------|-----|
| Hook Strength | X | 10 |
| Problem Clarity | X | 10 |
| Unique Insight | X | 10 |
| Visual Hierarchy | X | 10 |
| Story Presence | X | 10 |
| Technical Clarity | X | 10 |
| CTA Effectiveness | X | 10 |
| **Total** | **XX** | **70** |

### Rating
- 60-70: Star-worthy
- 45-59: Good, needs polish
- 30-44: Average, significant improvements needed
- Below 30: Major rewrite recommended
```

## Step 3: Identify Specific Issues

### Structure Analysis
- [ ] What section comes first? (Should be hook)
- [ ] Where is installation? (Should be after hook)
- [ ] Is there a story? Where? (Should be prominent)
- [ ] Are there visuals? (Should have at least 1)

### Content Analysis
- [ ] Is the problem clearly stated?
- [ ] Is there a unique insight/breakthrough?
- [ ] Are features benefit-focused or feature-focused?
- [ ] Is there a clear CTA?

### Technical Analysis
- [ ] Are prerequisites clear?
- [ ] Is installation copy-pasteable?
- [ ] Are examples working?
- [ ] Are links valid?

## Step 4: Generate Recommendations

For each issue found, provide:

```markdown
### Issue: [Name]

**Current:** [What the README does now]

**Problem:** [Why this hurts stars/engagement]

**Fix:** [Specific action to take]

**Example:**
[Show before/after snippet if applicable]
```

### Priority Levels

| Priority | Impact | Effort | Action |
|----------|--------|--------|--------|
| P0 | High | Low | Do immediately |
| P1 | High | Medium | Do soon |
| P2 | Medium | Medium | Do when possible |
| P3 | Low | Any | Nice to have |

## Step 5: Deliver Analysis Report

```markdown
# README Analysis Report

## Summary
[2-3 sentence overview of the README's current state]

## Score: XX/70 ([Rating])

## Top 3 Issues to Fix

### 1. [Highest impact issue]
[Details]

### 2. [Second issue]
[Details]

### 3. [Third issue]
[Details]

## Quick Wins (P0)
- [ ] [Quick fix 1]
- [ ] [Quick fix 2]

## Medium-Term Improvements (P1)
- [ ] [Improvement 1]
- [ ] [Improvement 2]

## Nice-to-Haves (P2-P3)
- [ ] [Optional improvement]

## What's Working Well
- [Positive 1]
- [Positive 2]

---

**Next Steps:**
Run `/awesome-readme:improve` to automatically implement these recommendations.
```

## Important Notes

- This workflow is **analysis only** - do not modify the README
- Be specific with examples, not generic advice
- Focus on star-getting improvements, not just technical docs
- Always highlight what's working well, not just problems
