# Analyze Workflow

Review a README and provide actionable feedback without making changes.

## Step 1: Read the README

Read `README.md` or the user-specified README file.

## Step 2: Score Key Criteria

Rate each criterion from 1-10.

### Scoring Rubric

| Criterion | 1-3 | 4-6 | 7-10 |
|-----------|-----|-----|------|
| **Hook Strength** | No hook, generic opening | Some positioning | Provocative and memorable |
| **Problem Clarity** | Problem not stated | Problem implied | Pain is vivid and relatable |
| **Unique Insight** | Features only | Some differentiation | Clear breakthrough or aha |
| **Visual Hierarchy** | Wall of text | Basic sections | Strong tables, demos, diagrams |
| **Story Presence** | No narrative | Brief context | Strong origin story |
| **Technical Clarity** | Confusing or incomplete | Adequate | Clear and easy to follow |
| **CTA Effectiveness** | No CTA | Weak CTA | Clear reason to star, try, or contribute |

### Score Card Format

```markdown
## README Analysis: [Project Name]

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
```

### Rating Bands

- 60-70: Star-worthy
- 45-59: Good, needs polish
- 30-44: Average, significant improvements needed
- Below 30: Major rewrite recommended

## Step 3: Identify Specific Issues

### Structure Analysis

- What section appears first?
- Where is installation?
- Is there a story, and is it prominent?
- Are there visuals or diagrams?

### Content Analysis

- Is the problem clearly stated?
- Is there a unique insight?
- Are features benefit-focused?
- Is there a clear CTA?

### Technical Analysis

- Are prerequisites clear?
- Is installation copy-pasteable?
- Are examples coherent?
- Do links and references look valid?

## Step 4: Generate Recommendations

For each issue, use this format:

```markdown
### Issue: [Name]

**Current:** [What the README does now]

**Problem:** [Why this hurts engagement or clarity]

**Fix:** [Specific action to take]

**Example:**
[Before/after snippet when useful]
```

### Priority Levels

| Priority | Impact | Effort | Action |
|----------|--------|--------|--------|
| P0 | High | Low | Do immediately |
| P1 | High | Medium | Do soon |
| P2 | Medium | Medium | Do when possible |
| P3 | Low | Any | Nice to have |

## Step 5: Deliver Analysis Report

Use this structure:

```markdown
# README Analysis Report

## Summary
[2-3 sentence overview]

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

## What Is Working Well
- [Positive 1]
- [Positive 2]
```

## Important Notes

- This workflow is analysis only. Do not modify the README.
- Be specific. Avoid generic advice.
- Focus on improvements that increase clarity, differentiation, and engagement.
- If the user wants changes after analysis, switch to the Improve workflow.
