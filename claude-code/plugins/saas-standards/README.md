# saas-standards

Enforce non-negotiable SaaS application standards — signup, mandatory onboarding, route guards, database schema, backend/ORM selection, and state-management architecture. Prevents the #1 failure mode: signup flows with only email/password and zero onboarding.

## What it does

The skill routes a request to one of 4 workflow playbooks:

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| AuditSignupFlow | "audit my SaaS app", "review my onboarding flow" | Checks for the onboarding gate, required fields, and route guards; reports violations + fixes. |
| ScaffoldAuth | "set up auth", "build signup flow" | Scaffolds signup, mandatory onboarding wizard, route guards, and the user → profile → organization schema. |
| AuditStateManagement | "state management best practices", "audit state management" | Diagnoses which of the 5 state layers need tooling; recommends per layer. |
| SelectDataLayer | "pick a backend", "which ORM should I use" | Interviews on deployment target and needs; recommends backend (Supabase / Neon / Convex) and ORM. |

The skill enforces the non-negotiable standard: signup never redirects straight to a dashboard — onboarding is a mandatory gate.

## Bundled content

```
skills/saas-standards/
├── SKILL.md                          # the non-negotiable standard + workflow routing
├── signup-onboarding.md              # signup & onboarding deep dive
├── route-guards.md                   # route guard patterns
├── healthcare-saas.md                # healthcare/dental SaaS specifics
├── data-layer.md                     # backend selection, ORM choice, schema implementation
├── state-management.md               # state-management architecture
└── workflows/                        # 4 workflow playbooks (see table above)
```

## License

MIT — see repository LICENSE.
