# Workflow: AuditSignupFlow

**Audit an existing application's signup and onboarding flow against SaaS standards.**

---

## Trigger

User wants to check if their existing app meets SaaS signup/onboarding standards.

Examples:
- "Check my signup flow"
- "Audit my auth"
- "Is my onboarding complete?"
- "Review my user registration"

---

## Execution Steps

### Step 1: Discover Auth Routes

Search the codebase for:
- `/signup`, `/register`, `/sign-up` routes
- `/login`, `/signin`, `/sign-in` routes
- `/onboarding`, `/setup`, `/welcome` routes
- Authentication middleware
- Session/JWT management

```
Glob: **/signup/**
Glob: **/auth/**
Glob: **/onboarding/**
Grep: "onboarding" across codebase
Grep: "signup" across codebase
```

### Step 2: Audit Signup Form

Check the signup form for:

| Check | Pass | Fail |
|-------|------|------|
| Email field exists | Has email input with validation | Missing or no validation |
| Password field exists | Has password with strength check | Missing or no strength check |
| Confirm password | Has confirm field with match validation | Missing |
| ToS/Privacy checkbox | Has checkbox, links to docs | Missing |
| Error handling | Inline validation, duplicate email check | Generic errors or none |
| Post-signup redirect | Goes to /onboarding | Goes to /dashboard (CRITICAL FAILURE) |

### Step 3: Audit Onboarding Flow

Check for onboarding page/wizard:

| Check | Pass | Fail |
|-------|------|------|
| Onboarding page exists | `/onboarding` route exists | No onboarding route (CRITICAL FAILURE) |
| first_name collected | Required field present | Missing |
| last_name collected | Required field present | Missing |
| role/position collected | Required field present | Missing |
| organization_name collected | Required field present | Missing |
| organization_phone collected | Required field present | Missing |
| organization_location collected | Required field present | Missing |
| team_size collected | Required field present | Missing |
| Completion gate | `onboarding_completed` flag exists | No completion tracking |

### Step 4: Audit Route Guards

Check middleware/guards:

| Check | Pass | Fail |
|-------|------|------|
| Auth middleware exists | Middleware checks session on protected routes | No middleware |
| Onboarding guard exists | Middleware checks `onboarding_completed` | No onboarding check (CRITICAL FAILURE) |
| Dashboard protected | Cannot access without auth + onboarding | Accessible without both |
| API routes protected | Return 401/403 for unauthorized | No auth check on API |
| Redirect logic correct | Proper redirect chain (login → onboarding → dashboard) | Broken redirects |

### Step 5: Audit Database Schema

Check schema for:

| Check | Pass | Fail |
|-------|------|------|
| `onboarding_completed` field | Exists on user/profile table | Missing (CRITICAL FAILURE) |
| Profile table separate from auth | Profile data in separate table | All in users table |
| Organization table exists | B2B org data captured | No org table |
| Required fields NOT NULL | first_name, last_name, role are NOT NULL | Nullable required fields |

### Step 6: Audit E2E Tests

Check for test coverage:

| Check | Pass | Fail |
|-------|------|------|
| Signup test exists | Tests happy path signup | No signup test |
| Onboarding test exists | Tests onboarding flow | No onboarding test |
| Route guard test exists | Tests unauthorized access | No guard test |
| Edge case tests | Duplicate email, weak password, partial onboarding | No edge cases |

---

## Output Format

```markdown
## SaaS Standards Audit Report

### Overall Score: X/6 phases passing

### Phase 1: Signup Form — [PASS/FAIL]
- [details]

### Phase 2: Onboarding — [PASS/FAIL]
- [details]

### Phase 3: Route Guards — [PASS/FAIL]
- [details]

### Phase 4: Database Schema — [PASS/FAIL]
- [details]

### Phase 5: E2E Tests — [PASS/FAIL]
- [details]

### Critical Failures (MUST FIX)
1. [failure]
2. [failure]

### Recommendations
1. [recommendation]
2. [recommendation]
```
