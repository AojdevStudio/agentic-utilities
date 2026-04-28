# Finance Workflow

Specialized analysis workflow for investment, market, and financial content.

## Input

Same as GeneralWorkflow, plus:
- `config.focusArea` — one of: "strategy-breakdown", "action-items", "risk-analysis", "portfolio-ideas"

## Analysis Protocol

1. Read transcript chunk from file path
2. Identify financial strategies, theories, and recommendations
3. Categorize content: macro analysis, individual stocks, portfolio strategy, income/dividends, options, crypto, real estate
4. Extract specific numbers: returns, yields, allocations, price targets
5. Note disclaimers, risk warnings, and qualification statements
6. Identify the speaker's investment philosophy and biases

## Output Modes

### Strategy Breakdown Mode (config.focusArea = "strategy-breakdown")
- Each strategy named and explained
- Required capital and experience level
- Historical performance if mentioned
- Comparison to conventional approaches
- Risk/reward profile

### Action Items Mode (config.focusArea = "action-items")
- Specific actionable steps
- Tools and platforms needed
- Account types and minimums
- Timeline and milestones
- Prerequisites and assumptions

### Risk Analysis Mode (config.focusArea = "risk-analysis")
- Risk factors identified per strategy
- Market conditions required
- Worst-case scenarios mentioned
- Hedging strategies discussed
- Regulatory and tax considerations

### Portfolio Ideas Mode (config.focusArea = "portfolio-ideas")
- Specific holdings mentioned
- Allocation percentages
- Sector/asset class distribution
- Income projections
- Rebalancing triggers

## Output Structure

```markdown
## Executive Summary
[Core thesis and investment approach]

## Content Category
[Macro | Individual Stock | Portfolio Strategy | Income/Dividend | Options | Crypto | Real Estate | Mixed]

## Investment Strategies

### [Strategy Name]
- **Thesis:** [core argument]
- **Required Capital:** [amount/range]
- **Risk Level:** [Low/Medium/High/Very High]
- **Time Horizon:** [Short/Medium/Long]
- **Key Metrics:** [specific numbers mentioned]

## Actionable Takeaways
1. [Specific action with context]
2. [Specific action with context]

## Risk Assessment
- **Market Risks:** [identified risks]
- **Timing Sensitivity:** [how time-sensitive is this advice]
- **Assumptions:** [what must be true for this to work]

## Numbers & Data Points
| Metric | Value | Context |
|--------|-------|---------|
| [metric] | [value] | [what it means] |

## Speaker Context
- **Investment Philosophy:** [identified approach]
- **Potential Biases:** [conflicts of interest, promotion, etc.]
- **Track Record:** [if mentioned]

## Quality Assessment
- Content depth: [DEEP/SURFACE/MIXED]
- Actionability: [HIGH/MEDIUM/LOW]
- Disclaimer quality: [THOROUGH/ADEQUATE/MISSING]
```

## Chunk-Specific Instructions

Same as GeneralWorkflow — analyze only your assigned chunk. Extract ALL specific numbers, tickers, and strategies mentioned in your chunk.
