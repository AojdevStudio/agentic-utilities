# Story-mode markdown skeleton

Full markdown templates for the 10-section Story-mode README. Loaded only when the workflow has classified the project as Story mode (via Step 0 of Create.md or Step 5 of Improve.md). Skip this file entirely for Utility-mode projects — `references/utility-skeleton.md` is what you want.

The order is load-bearing: Hero → Problem → Insight → Solution → Demo → Features → Quick Start → How It Works → Story → Footer. Hook before installation, story before technical detail. Don't reorder without a strong reason.

## Section 1: Hero

```markdown
<div align="center">

# Project Name

### **[Provocative tagline]**

[![License](badge)](link)
[![PRs Welcome](badge)](link)

*One sentence that captures the breakthrough.*

[**Demo**](#demo) · [**Quick Start**](#quick-start) · [**Docs**](link)

</div>
```

## Section 2: The Problem

```markdown
## The Problem Everyone Ignores

[Paint the painful scenario in 3-4 sentences]

- Bullet point of failure mode 1
- Bullet point of failure mode 2
- Bullet point of failure mode 3

**Sound familiar?**

> *"Personal quote or scenario that's deeply relatable"*
```

## Section 3: The Insight

```markdown
## The Insight That Changed Everything

[Build up to the breakthrough]

<div align="center">

### **[First half of insight]**
### **[Second half that completes it]**

</div>

[Explain why this insight matters in 2-3 sentences]

<div align="center">

## **[The core principle/rule]**

</div>
```

## Section 4: The Solution

```markdown
## Introducing [Project Name]

[Brief acronym expansion if applicable]

[One sentence describing what it is]

| Component | Role |
|:----------|:-----|
| **Part A** | What it does |
| **Part B** | What it does |
| **Part C** | What it does |

[ASCII diagram or image reference]
```

## Section 5: Demo

```markdown
## See It In Action

<details>
<summary><b>Demo: [What the demo shows]</b></summary>

[GIF or code example]

</details>
```

## Section 6: Features / Defense

```markdown
## [Feature Header — Make it about benefit]

| Feature | What It Does | Why It Matters |
|:--------|:-------------|:---------------|
| **1** | Mechanism | Prevents/Enables X |
| **2** | Mechanism | Prevents/Enables Y |
```

## Section 7: Quick Start

```markdown
## Quick Start

### Prerequisites
[Minimal list]

### Install
[3-5 lines max]

### Run
[Show it working]
```

## Section 8: How It Works

```markdown
## How It Works

[Architecture diagram]

### The Core Loop/Pattern
[Code or pseudocode showing the mechanism]
```

## Section 9: The Story

```markdown
## The Story Behind [Project]

[Personal narrative — 3-4 paragraphs]
[Why you built this]
[What frustrated you]
[What you discovered]

<div align="center">

### [Memorable closing principle]

</div>
```

## Section 10: Footer

```markdown
## Roadmap
- [x] Completed feature
- [ ] Planned feature

## Contributing
[Brief guidelines]

## Acknowledgments
[Credits with links]

## License
[License type]

---

<div align="center">

**[Memorable tagline]**

If this helped you, [star the repo](link)

[![Star History](chart)](link)

</div>
```

## Story Mode Quality Checklist

- [ ] Hook comes before installation
- [ ] Story is not buried at the bottom
- [ ] Tables used for visual hierarchy
- [ ] Centered headings for emphasis
- [ ] Badges in hero section
- [ ] Clear CTA for stars
- [ ] GitHub username is correct (resolve via `gh api user --jq .login`)
- [ ] All images referenced exist
- [ ] Links are valid
