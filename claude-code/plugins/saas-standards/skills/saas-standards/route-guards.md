# Route Guards — Implementation Patterns

Reference document for SaaSStandards skill. Covers route protection across frameworks.

> **Module-first:** All auth imports in this document use `@/modules/auth` orchestrators. Never import auth provider SDKs (Clerk, Supabase Auth, etc.) directly in route files. Keep auth logic behind the module boundary so providers can be swapped without touching route code.

---

## The Principle

**Every protected route must verify TWO things:**
1. User is authenticated (has valid session/token)
2. User has completed onboarding (profile is complete)

If either check fails, the user is redirected — never shown a broken page with null data.

---

## Authentication State Machine

```
UNAUTHENTICATED
  ├── Can access: /signup, /login, /forgot-password, /reset-password
  └── All other routes → redirect to /login

AUTHENTICATED_UNVERIFIED (email not verified, if strict mode)
  ├── Can access: /verify-email, /resend-verification
  └── All other routes → redirect to /verify-email

AUTHENTICATED_UNONBOARDED (email verified OR lax mode, but onboarding incomplete)
  ├── Can access: /onboarding, /logout
  └── All other routes → redirect to /onboarding

AUTHENTICATED_ONBOARDED (fully set up)
  ├── Can access: /dashboard, /settings, /api/*, all app routes
  ├── /signup, /login → redirect to /dashboard
  └── /onboarding → redirect to /dashboard
```

---

## Next.js App Router Pattern

### middleware.ts (Root Level)

```typescript
import { NextRequest, NextResponse } from 'next/server'
import { getAuthSession } from '@/modules/auth'

const PUBLIC_ROUTES = ['/signup', '/login', '/forgot-password', '/reset-password']
const ONBOARDING_ROUTE = '/onboarding'
const VERIFY_EMAIL_ROUTE = '/verify-email'

export async function middleware(request: NextRequest) {
  const session = await getAuthSession()
  const { pathname } = request.nextUrl

  // Static assets and API auth routes — skip
  if (pathname.startsWith('/_next') || pathname.startsWith('/api/auth')) {
    return NextResponse.next()
  }

  // Unauthenticated user
  if (!session) {
    if (PUBLIC_ROUTES.some(route => pathname.startsWith(route))) {
      return NextResponse.next() // Allow public routes
    }
    return NextResponse.redirect(new URL('/login', request.url))
  }

  // Authenticated but email not verified (strict mode)
  if (!session.emailVerified && pathname !== VERIFY_EMAIL_ROUTE) {
    return NextResponse.redirect(new URL(VERIFY_EMAIL_ROUTE, request.url))
  }

  // Authenticated but onboarding incomplete
  if (!session.onboardingCompleted) {
    if (pathname === ONBOARDING_ROUTE) {
      return NextResponse.next() // Allow onboarding page
    }
    if (PUBLIC_ROUTES.some(route => pathname.startsWith(route))) {
      return NextResponse.redirect(new URL(ONBOARDING_ROUTE, request.url))
    }
    return NextResponse.redirect(new URL(ONBOARDING_ROUTE, request.url))
  }

  // Fully onboarded — redirect away from auth pages
  if (PUBLIC_ROUTES.some(route => pathname.startsWith(route)) || pathname === ONBOARDING_ROUTE) {
    return NextResponse.redirect(new URL('/dashboard', request.url))
  }

  return NextResponse.next()
}

export const config = {
  matcher: ['/((?!_next/static|_next/image|favicon.ico|public).*)'],
}
```

### Server Component Guard (Fallback)

```typescript
// Usage in page.tsx with @/modules/auth orchestrators:
import { requireAuth, requireOnboardedOrg } from '@/modules/auth'

// Protected page (auth required, onboarding not required):
export default async function SettingsPage() {
  const session = await requireAuth()
  // session is AuthSession — guaranteed authenticated
}

// Dashboard page (auth + onboarding required):
export default async function DashboardPage() {
  const { session, orgId } = await requireOnboardedOrg()
  // session is AuthSession, orgId is string — guaranteed authenticated + onboarded
  // Safe to use session data — no null fallbacks needed
}
```

---

## Express.js Pattern

```typescript
// middleware/auth.ts
export function requireAuth(req: Request, res: Response, next: NextFunction) {
  if (!req.session?.userId) {
    return res.redirect('/login')
  }
  next()
}

export function requireOnboarding(req: Request, res: Response, next: NextFunction) {
  if (!req.session?.userId) {
    return res.redirect('/login')
  }
  if (!req.session?.onboardingCompleted) {
    return res.redirect('/onboarding')
  }
  next()
}

// Usage:
app.get('/dashboard', requireOnboarding, dashboardHandler)
app.get('/onboarding', requireAuth, onboardingHandler)
app.get('/signup', guestOnly, signupHandler)
```

---

## API Route Protection

```typescript
// For API routes that require full auth + onboarding:
import { requireOrg } from '@/modules/auth'

export async function GET() {
  const { session, orgId } = await requireOrg()
  // Use orgId for tenant-scoped queries
  // requireOrg() throws/redirects if unauthenticated or no org
}

// For API routes that need onboarded org:
import { requireOnboardedOrg } from '@/modules/auth'

export async function POST() {
  const { session, orgId } = await requireOnboardedOrg()
  // Guaranteed: authenticated + has org + onboarding complete
}
```

---

## Client-Side Guard (React)

```tsx
// components/AuthGuard.tsx
'use client'

import { useSession } from '@/hooks/useSession'
import { useRouter } from 'next/navigation'
import { useEffect } from 'react'

export function AuthGuard({ children }: { children: React.ReactNode }) {
  const { session, loading } = useSession()
  const router = useRouter()

  useEffect(() => {
    if (loading) return
    if (!session) router.replace('/login')
    else if (!session.onboardingCompleted) router.replace('/onboarding')
  }, [session, loading, router])

  if (loading) return <LoadingSpinner />
  if (!session || !session.onboardingCompleted) return null

  return <>{children}</>
}
```

---

## Testing Route Guards (Playwright)

```typescript
// tests/route-guards.spec.ts
import { test, expect } from '@playwright/test'

test.describe('Route Guards', () => {
  test('unauthenticated user cannot access dashboard', async ({ page }) => {
    await page.goto('/dashboard')
    await expect(page).toHaveURL(/\/login/)
  })

  test('un-onboarded user cannot access dashboard', async ({ page }) => {
    // Login as user who hasn't completed onboarding
    await loginAsUnOnboardedUser(page)
    await page.goto('/dashboard')
    await expect(page).toHaveURL(/\/onboarding/)
  })

  test('onboarded user redirected away from signup', async ({ page }) => {
    await loginAsOnboardedUser(page)
    await page.goto('/signup')
    await expect(page).toHaveURL(/\/dashboard/)
  })

  test('onboarded user can access dashboard', async ({ page }) => {
    await loginAsOnboardedUser(page)
    await page.goto('/dashboard')
    await expect(page).toHaveURL(/\/dashboard/)
    // Verify user data is present (not null)
    await expect(page.locator('[data-testid="user-name"]')).not.toBeEmpty()
  })
})
```

---

## Security Note: Defense in Depth

Next.js middleware can be bypassed via header manipulation in older versions (CVE-2025-29927 affected v11.1.4–13.5.6, 14.x before 14.2.25, and 15.x before 15.2.3). Always implement server-side session checks in addition to middleware — middleware is a UX optimization, not your sole security layer.

**Mitigation:** Keep Next.js updated to >= 14.2.25 or >= 15.2.3. Add server-side guards in server components and API handlers regardless.
