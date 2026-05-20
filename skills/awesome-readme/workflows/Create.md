# Create Workflow

Create a compelling, story-driven README from scratch.

## Step 1: Gather Context

Before writing anything:

1. Read existing documentation: `README.md`, `CLAUDE.md`, `package.json`, and relevant docs.
2. Identify the tech stack.
3. Find the core problem the project solves.
4. Discover the unique insight or mechanism.
5. Inspect the repo structure if needed to confirm key features.

Recommended inspection order:
- `README.md` if it exists
- `package.json` or equivalent project metadata
- `CLAUDE.md` or other agent/docs files
- `docs/` contents
- Key source entry points

## Step 2: Ask About Visual Content

Ask the user plainly:

> Would you like visual content in the README? Options: create new visuals, use existing images, or keep it text-only.

If the user wants visuals:
- Decide which visuals are needed: architecture, workflow, hero graphic, feature comparison.
- Prefer existing images if they already exist.
- If no art/image tool is available, generate Mermaid or ASCII diagrams, or insert clear placeholders for later assets.

## Step 3: Identify the Emotional Hook

Find the pain point that resonates.

### Hook Formula

```text
[Target audience] experiences [painful problem].
Current solutions fail because [reason].
This project solves it by [unique approach].
```

### Example Hooks

| Project Type | Hook |
|--------------|------|
| Testing tool | Stop trusting. Start verifying. |
| DevOps automation | Your CI pipeline should not wake you at 3am. |
| Database tool | Migrations that do not break production. |
| API framework | APIs that write their own documentation. |

## Step 4: Structure the README

Follow this order.

### 1. Hero

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

### 2. The Problem

```markdown
## The Problem Everyone Ignores

[Paint the painful scenario in 3-4 sentences]

- Failure mode 1
- Failure mode 2
- Failure mode 3

**Sound familiar?**

> *"Relatable scenario or quote"*
```

### 3. The Insight

```markdown
## The Insight That Changed Everything

[Build up to the breakthrough]

<div align="center">

### **[First half of insight]**
### **[Second half of insight]**

</div>

[Explain why this matters]
```

### 4. The Solution

```markdown
## Introducing [Project Name]

[One sentence describing what it is]

| Component | Role |
|:----------|:-----|
| **Part A** | What it does |
| **Part B** | What it does |
| **Part C** | What it does |

[Diagram, image, Mermaid, or ASCII block]
```

### 5. Demo

```markdown
## See It In Action

<details>
<summary><b>Demo: [What the demo shows]</b></summary>

[GIF, screenshot, or code example]

</details>
```

### 6. Features / Defense

```markdown
## Why It Wins

| Feature | What It Does | Why It Matters |
|:--------|:-------------|:---------------|
| **1** | Mechanism | Prevents or enables X |
| **2** | Mechanism | Prevents or enables Y |
```

### 7. Quick Start

```markdown
## Quick Start

### Prerequisites
[Minimal list]

### Install
[Short install sequence]

### Run
[Show the happy path]
```

### 8. How It Works

```markdown
## How It Works

[Architecture explanation]

### The Core Loop
[Code or pseudocode]
```

### 9. The Story

```markdown
## The Story Behind [Project]

[Why it was built]
[What frustrated the author]
[What was discovered]
```

### 10. Footer

```markdown
## Roadmap
- [x] Completed feature
- [ ] Planned feature

## Contributing
[Brief guidelines]

## License
[License type]

---

<div align="center">

**[Memorable closing line]**

If this helped you, star the repo.

</div>
```

## Step 5: Create Visuals If Requested

If visuals are needed:
- Use existing assets first.
- Otherwise create Mermaid/ASCII diagrams or use an available art/image skill.
- Save generated files in `docs/` when creating new assets.
- Reference every asset with clear alt text.

## Step 6: Write and Deliver

1. Write the complete README.
2. Keep all technical details accurate.
3. Replace placeholders with real project details.
4. Infer GitHub links from the repo when possible.
5. Validate that headings, links, and file references are coherent.

## Quality Checklist

- [ ] Hook comes before installation
- [ ] Story is not buried
- [ ] Visual hierarchy is strong
- [ ] README includes a CTA
- [ ] Technical instructions are still correct
- [ ] Images or diagrams are referenced correctly
- [ ] Links and paths look valid
