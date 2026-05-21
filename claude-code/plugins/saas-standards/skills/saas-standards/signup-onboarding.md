# Signup & Onboarding — Deep Dive

Reference document for SaaSStandards skill. Covers the complete signup-to-dashboard flow.

---

## The Fundamental Rule

**Signup creates the account. Onboarding creates the user.**

An account with just email + password is useless to the application. The app needs to know WHO the person is, WHERE they work, WHAT their role is, and HOW BIG their team is. Without this, the app cannot:
- Personalize the experience
- Set up proper permissions
- Configure the workspace
- Route notifications correctly
- Bill appropriately (seat-based, practice-based, etc.)

---

## Signup Form UX Standards

### Layout
- Single column, centered, max-width 400-480px
- Social login buttons at top (if applicable), then "or" divider, then email form
- Show/hide password toggle
- Password strength indicator (real-time)
- Inline validation on blur (not on every keystroke)
- Submit button disabled until all required fields valid
- Terms of Service + Privacy Policy links with checkbox

### Validation Rules

| Field | Rule | Error Message |
|-------|------|---------------|
| Email | Valid format + unique check (debounced) | "Please enter a valid email" / "This email is already registered" |
| Password | Min 8 chars, 1 upper, 1 lower, 1 number | "Password must be at least 8 characters with mixed case and a number" |
| Confirm Password | Must match password | "Passwords don't match" |
| ToS Checkbox | Must be checked | "You must accept the Terms of Service" |

### Post-Signup Flow

```
User submits signup form
  → API creates user record (onboarding_completed: false)
  → API creates session/JWT
  → API sends verification email (if enabled)
  → Client redirects to /onboarding
  → /onboarding renders Step 1
```

---

## Onboarding Wizard UX Standards

### Layout
- Full-page wizard (not a modal)
- Progress indicator at top (Step 1 of 3, Step 2 of 3, etc.)
- One logical group per step
- Back button on steps 2+
- "Continue" button (not "Submit" until final step)
- Data persists on each step (partial save)
- No skip on required steps

### Recommended Step Structure

**Step 1: Personal Info**
- first_name (required)
- last_name (required)
- role / job_title (required)
- phone (recommended)

**Step 2: Organization**
- organization_name (required)
- organization_phone (required)
- organization_address (required — at minimum city + state)
- team_size (required — dropdown: 1-5, 6-20, 21-50, 51-200, 200+)

**Step 3: Preferences (Optional)**
- use_case / industry (recommended)
- how_did_you_hear (recommended)
- notification preferences
- timezone (auto-detect with override)

### Completion Gate Logic

```typescript
function isOnboardingComplete(user: User, profile: Profile, org: Organization): boolean {
  return Boolean(
    profile.first_name &&
    profile.last_name &&
    profile.role &&
    org.name &&
    org.phone &&
    org.location &&
    org.team_size
  )
}
```

### What Happens After Completion

1. `user.onboarding_completed = true` saved to database
2. Redirect to `/dashboard`
3. Dashboard can now assume all required fields exist
4. No null checks needed for name, org, role, etc.
5. Welcome message uses first_name: "Welcome, {first_name}!"

---

## Multi-Tenant Considerations

### Owner vs Member Onboarding

**Office Owner (first user to create org):**
- Full onboarding: personal info + create organization + preferences
- Gets `role: 'office_owner'` in memberships table

**Invited Member (joins existing org):**
- Partial onboarding: personal info only (org already exists)
- Gets `role: 'office_staff'` (or whatever the inviter specified — `office_manager` or `office_staff`)
- Onboarding skips the "Organization" step

**Note:** These are tenant-scoped office roles, separate from `users.platform_role` which is reserved for app operators (`platform_admin`). See `data-layer.md` for the two-layer role model.

### Team Invitation Flow

```
Owner completes onboarding
  → Dashboard has "Invite Team" prompt
  → Owner enters team member emails
  → System sends invitation emails
  → Invited user clicks link → /signup?invite=TOKEN
  → Signup pre-fills org, skips org step in onboarding
  → Invited user completes personal info only
  → Invited user lands on dashboard
```

### Multi-Tenant Onboarding with an Auth Provider

**Owner Flow (creates organization):**
1. Signup (email + password)
2. Email verification
3. Auth provider creates org → receives `orgId`
4. Onboarding wizard collects: first_name, last_name, role, org_name, org_phone, org_address, team_size
5. `modules/offices/ensureOfficeProfile(orgId, data)` → creates DB-backed profile
6. `modules/offices/setOnboardingComplete(orgId)` → marks complete
7. Redirect to dashboard

**Member Flow (joins existing organization):**
1. Signup (email + password)
2. Email verification
3. Invitation acceptance → joins existing org, receives `orgId`
4. Onboarding wizard collects: first_name, last_name, role (org info already exists)
5. Profile created, linked to existing `orgId`
6. Redirect to dashboard

**Key:** Both flows use `orgId` as canonical identifier. The onboarding wizard adapts based on whether the user is creating or joining an organization.

**Multi-Office:** A single user CAN belong to multiple organizations. The active organization context determines which `orgId` is used for queries.

---

## Email Verification Strategy

### Option A: Verify Before Onboarding (Strict)
```
Signup → Verify Email → Onboarding → Dashboard
```
Pro: Ensures valid email before any data collection
Con: Higher friction, higher drop-off

### Option B: Verify After Onboarding (Recommended)
```
Signup → Onboarding → Dashboard (with banner: "Verify your email")
```
Pro: Lower friction, user is invested by the time they see banner
Con: Some unverified accounts may accumulate

### Option C: Verify in Background (Lax)
```
Signup → Onboarding → Dashboard (verify silently, remind after 7 days)
```
Pro: Minimal friction
Con: Risk of fake accounts

**Recommendation:** Option B for most SaaS. Option A for healthcare/financial (compliance).

---

## Error States & Edge Cases

| Scenario | Handling |
|----------|----------|
| User refreshes during onboarding | Resume from last completed step (data saved per-step) |
| User closes browser during onboarding | Next login → redirect to onboarding (incomplete) |
| User tries to navigate to /dashboard directly | Middleware redirects to /onboarding |
| User modifies URL to skip onboarding step | Server-side validation catches missing required fields |
| Session expires during onboarding | Re-auth → resume onboarding from last saved step |
| User changes email after signup | Allow in settings (after onboarding), re-verify |
| User wants to change organization | Allow in settings (after onboarding), admin approval |
