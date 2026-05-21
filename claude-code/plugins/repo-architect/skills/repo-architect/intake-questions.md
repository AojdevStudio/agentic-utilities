# Intake Questions — RepoArchitect

Structured questions for the AskUserQuestion tool. Used by the NewProject workflow to collect all inputs before framework selection.

Questions are organized into 4 groups, delivered as 3-4 AskUserQuestion calls (batched for efficiency).

---

## Group 1: App & Repo Context

### Question 1.1: Product Type

```
question: "What type of product are you building?"
header: "Product type"
options:
  - label: "Web Application (SPA/SSR)"
    description: "Frontend-heavy app with browser UI (React, Next.js, Vue, etc.)"
  - label: "Backend Service / API"
    description: "Server-side service, REST/GraphQL API, microservice"
  - label: "CLI Tool"
    description: "Command-line application or developer tool"
  - label: "Library / Package"
    description: "Reusable code published as npm/pip/crate package"
  - label: "Knowledge Base"
    description: "Documentation-heavy, topic-organized, minimal runtime code"
  - label: "Ops Hub"
    description: "Multi-workspace operational hub combining knowledge, planning, and automation"
multiSelect: false
```

### Question 1.2: Repo State

```
question: "Is this a new project or an existing repo that needs restructuring?"
header: "Repo state"
options:
  - label: "New project (Recommended)"
    description: "Starting fresh — will scaffold from scratch"
  - label: "Existing repo — needs audit"
    description: "Repo exists, want to evaluate current structure"
  - label: "Existing repo — needs refactor"
    description: "Repo exists, want to restructure it"
multiSelect: false
```

### Question 1.3: Primary Users

```
question: "Who are the primary users of this codebase?"
header: "Users"
options:
  - label: "Internal developers"
    description: "Your team writes and maintains this code"
  - label: "External developers / OSS"
    description: "Open source contributors or API consumers"
  - label: "Ops / DevOps"
    description: "Infrastructure and operations teams"
  - label: "Non-technical users"
    description: "Business analysts, content creators, end users"
multiSelect: true
```

### Question 1.4: Repository Visibility

```
question: "Is this repository public or private?"
header: "Visibility"
options:
  - label: "Public"
    description: "Open source or publicly visible — LICENSE required, CONTRIBUTING.md recommended"
  - label: "Private"
    description: "Internal/personal — no LICENSE needed, relaxed requirements"
  - label: "Not sure"
    description: "Will auto-detect from GitHub if possible"
multiSelect: false
```

**Impact:** Determines LICENSE requirement, .env.example requirement, CONTRIBUTING.md recommendation.

---

## Group 2: Tech & Deployment

### Question 2.1: Tech Stack

```
question: "What is your primary tech stack?"
header: "Stack"
options:
  - label: "TypeScript / Node.js"
    description: "JavaScript/TypeScript ecosystem (Bun, Node, Deno)"
  - label: "Python"
    description: "Python ecosystem (FastAPI, Django, Flask)"
  - label: "Go"
    description: "Go ecosystem"
  - label: "Rust"
    description: "Rust ecosystem"
multiSelect: false
```

### Question 2.2: Deployment Model

```
question: "How will this be deployed?"
header: "Deploy"
options:
  - label: "Docker / Containers"
    description: "Containerized deployment (Docker, k8s, ECS)"
  - label: "Serverless"
    description: "AWS Lambda, Vercel, Cloudflare Workers, etc."
  - label: "Platform-managed"
    description: "Vercel, Railway, Render, Heroku — platform handles infra"
  - label: "Bare metal / VPS"
    description: "Direct deployment to servers"
multiSelect: false
```

---

## Group 3: Scale & Constraints

### Question 3.1: Team & Scale

```
question: "What is your team size and expected project scale?"
header: "Scale"
options:
  - label: "Solo / Small (1-3 devs)"
    description: "Small project, minimal coordination overhead"
  - label: "Medium team (4-10 devs)"
    description: "Need clear conventions and some CI enforcement"
  - label: "Large team (10+ devs)"
    description: "Need strict conventions, CI gating, and documentation"
multiSelect: false
```

### Question 3.2: Compliance & Constraints

```
question: "Are there special compliance or constraint requirements?"
header: "Constraints"
options:
  - label: "None / Standard"
    description: "No special compliance requirements"
  - label: "HIPAA / Healthcare"
    description: "Health data, audit logging, BAA requirements"
  - label: "SOC2 / Enterprise"
    description: "Enterprise security controls, access logging"
  - label: "PCI-DSS / Financial"
    description: "Payment card data, encryption requirements"
multiSelect: true
```

---

## Group 4: Automation & Preferences

### Question 4.1: CI & Hooks

```
question: "What level of CI/automation enforcement do you want?"
header: "Strictness"
options:
  - label: "Standard (Recommended)"
    description: "Pre-push hooks + GitHub Actions CI on PRs"
  - label: "Strict"
    description: "Pre-commit + pre-push hooks + CI with blocking checks"
  - label: "Light"
    description: "CI only, no local hooks"
  - label: "None"
    description: "No automated enforcement (not recommended)"
multiSelect: false
```

### Question 4.2: Mono vs Poly

```
question: "Monorepo or separate repositories?"
header: "Repo model"
options:
  - label: "Single repo (Recommended)"
    description: "Everything in one repository — simpler to start"
  - label: "Monorepo with packages"
    description: "One repo with multiple packages (Turborepo, Nx)"
  - label: "Polyrepo"
    description: "Separate repos per service/package"
multiSelect: false
```

---

## Default Assumptions

If the user skips a question or doesn't provide a value, use these defaults:

| Input | Default |
|-------|---------|
| Product type | Web Application |
| Repo state | New project |
| Primary users | Internal developers |
| Tech stack | TypeScript / Node.js |
| Deployment | Platform-managed |
| Team size | Solo / Small |
| Compliance | None / Standard |
| CI strictness | Standard |
| Repo model | Single repo |
| Visibility | Private |

---

## Question Delivery Strategy

Deliver questions in 3 AskUserQuestion calls to minimize user friction:

1. **Call 1:** Questions 1.1 + 1.2 + 1.3 + 1.4 (App & Repo Context)
2. **Call 2:** Questions 2.1 + 2.2 (Tech & Deployment)
3. **Call 3:** Questions 3.1 + 3.2 + 4.1 + 4.2 (Scale + Automation)

AskUserQuestion supports up to 4 questions per call. Group related questions together.
