# Package Database Schema

Documentation for the YouTubeAnalyzer tutorial package tracking system.

## Overview

The package database tracks software packages, frameworks, and tools mentioned in tutorial videos. It enables version comparison (what the tutorial used vs latest) and cross-video package discovery.

## Database Location

`~/.config/youtube-analyzer/package-db.json`

## Schema

### Root Object

| Field | Type | Description |
|-------|------|-------------|
| `lastUpdated` | ISO 8601 datetime | When the database was last modified |
| `packages` | PackageEntry[] | Array of tracked packages |

### PackageEntry

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | npm/pypi package name (lowercase, e.g., "next") |
| `displayName` | string | Yes | Human-readable name (e.g., "Next.js") |
| `versionMentioned` | string | Yes | Version used in the tutorial video |
| `latestVersion` | string | No | Latest known version (from web search) |
| `latestChecked` | string (ISO date) | No | When latestVersion was last verified |
| `category` | enum | Yes | One of: framework, library, tool, service, language, database, other |
| `sourceVideos` | string[] | Yes | YouTube URLs where this package was mentioned |
| `notes` | string | No | Free-form notes (e.g., "Major breaking changes in v15") |

### Category Enum Values

| Value | Description | Examples |
|-------|-------------|----------|
| `framework` | Full application framework | Next.js, Django, Rails, Spring |
| `library` | Reusable code package | React, lodash, axios, zod |
| `tool` | Developer tooling | ESLint, Prettier, Webpack, Vite |
| `service` | External service/API | Stripe, Auth0, Vercel, AWS |
| `language` | Programming language | TypeScript, Python, Rust, Go |
| `database` | Database system | PostgreSQL, MongoDB, Redis, SQLite |
| `other` | Anything else | VS Code extensions, OS tools |

## Example Database

```json
{
  "lastUpdated": "2026-01-30T14:00:00Z",
  "packages": [
    {
      "name": "next",
      "displayName": "Next.js",
      "versionMentioned": "14.2",
      "latestVersion": "15.1.0",
      "latestChecked": "2026-01-30",
      "category": "framework",
      "sourceVideos": [
        "https://youtube.com/watch?v=abc123",
        "https://youtube.com/watch?v=def456"
      ],
      "notes": "Major version behind, App Router changes in 15.x"
    },
    {
      "name": "prisma",
      "displayName": "Prisma",
      "versionMentioned": "5.0",
      "latestVersion": "5.22.0",
      "latestChecked": "2026-01-28",
      "category": "library",
      "sourceVideos": [
        "https://youtube.com/watch?v=abc123"
      ],
      "notes": ""
    }
  ]
}
```

## CLI Operations

### Add/Update Package
```bash
bun run PackageDb.ts add \
  --name "next" \
  --display-name "Next.js" \
  --version-mentioned "14.2" \
  --category "framework" \
  --source "https://youtube.com/watch?v=abc123" \
  --notes "Tutorial uses Pages Router"
```

### List All Packages
```bash
bun run PackageDb.ts list
```

### Query Specific Package
```bash
bun run PackageDb.ts query --name "next"
```

### Find Stale Entries
```bash
bun run PackageDb.ts refresh --stale-days 7
```

## Version Comparison Logic

When the synthesis agent runs, it should:
1. Run `refresh --stale-days 7` to find packages needing version updates
2. For each stale package, dispatch a sub-agent to check the latest version via web search
3. Update with `add --name X --latest-version Y`
4. Flag packages where `versionMentioned` is a major version behind `latestVersion`

## Integration Points

- **TutorialWorkflow.md** — Populates database after analyzing tutorial content
- **Synthesis agent** — Runs `add` for each package found across all chunks
- **User queries** — `list` and `query` for browsing tracked packages
- **Refresh cycle** — `refresh` identifies stale entries for web search updates

## Usage Examples

### Adding Packages During Tutorial Analysis

When the synthesis agent processes tutorial chunks:

```bash
# Framework detected in video
bun run PackageDb.ts add \
  --name "next" \
  --display-name "Next.js" \
  --version-mentioned "14.2.0" \
  --category "framework" \
  --source "https://youtube.com/watch?v=abc123"

# Library with notes
bun run PackageDb.ts add \
  --name "zod" \
  --display-name "Zod" \
  --version-mentioned "3.22.0" \
  --category "library" \
  --source "https://youtube.com/watch?v=abc123" \
  --notes "Used for form validation"
```

### Updating Latest Versions

After web search agent checks npm/pypi:

```bash
bun run PackageDb.ts add \
  --name "next" \
  --latest-version "15.1.0"
```

### Querying Package Info

```bash
# Get full details for a package
bun run PackageDb.ts query --name "next"

# Output:
{
  "name": "next",
  "displayName": "Next.js",
  "versionMentioned": "14.2.0",
  "latestVersion": "15.1.0",
  "latestChecked": "2026-01-30",
  "category": "framework",
  "sourceVideos": [
    "https://youtube.com/watch?v=abc123",
    "https://youtube.com/watch?v=def456"
  ],
  "notes": "Major version behind, App Router changes in 15.x"
}
```

### Finding Stale Packages

```bash
# Find packages not checked in 7+ days
bun run PackageDb.ts refresh --stale-days 7

# Output:
{
  "stalePackages": [
    {
      "name": "next",
      "displayName": "Next.js",
      "lastChecked": "2026-01-20",
      "daysStale": 10
    },
    {
      "name": "prisma",
      "displayName": "Prisma",
      "lastChecked": "",
      "daysStale": -1
    }
  ],
  "totalStale": 2,
  "totalPackages": 15,
  "staleDaysThreshold": 7
}
```

## Workflow Integration

### During Tutorial Analysis

1. Transcript chunks are analyzed for package mentions
2. Each package is added via `PackageDb.ts add`
3. Multiple videos mentioning the same package append to `sourceVideos`

### Version Staleness Check

1. Periodically run `refresh --stale-days 7`
2. For each stale package, dispatch web search agent
3. Agent queries npm/pypi/GitHub for latest version
4. Update via `add --name X --latest-version Y`

### User Queries

User asks: "What tutorials use Next.js?"

```bash
# Query the package
bun run PackageDb.ts query --name "next"

# Response includes all source videos
{
  "sourceVideos": [
    "https://youtube.com/watch?v=abc123",
    "https://youtube.com/watch?v=def456"
  ]
}
```

User asks: "Show me all tracked packages"

```bash
bun run PackageDb.ts list

# Outputs formatted table + JSON
```

## Error Handling

### Missing Database

If `package-db.json` doesn't exist, commands automatically create an empty database:

```json
{
  "lastUpdated": "2026-01-30T14:00:00Z",
  "packages": []
}
```

### Invalid Arguments

```bash
bun run PackageDb.ts add --name "next"

# Output:
{
  "error": "Missing required arguments",
  "required": ["name", "display-name", "version-mentioned", "category", "source"],
  "usage": "bun run PackageDb.ts add --name <name> --display-name <display> ..."
}
```

### Package Not Found

```bash
bun run PackageDb.ts query --name "nexxt"

# Output (with fuzzy match suggestion):
{
  "error": "Package 'nexxt' not found",
  "suggestion": "next"
}
```

### Invalid Category

```bash
bun run PackageDb.ts add --name "next" --category "invalid" ...

# Output:
{
  "error": "Invalid category: invalid",
  "validCategories": ["framework", "library", "tool", "service", "language", "database", "other"]
}
```

## Version Warning Specification

When the synthesis agent compares `versionMentioned` to `latestVersion`, generate warnings using this severity scale:

### Warning Levels

| Level | Condition | Display | Action |
|-------|-----------|---------|--------|
| **HIGH** | Major version difference (e.g., 14 -> 15) | `HIGH - Major version behind` | Include migration guide link if known |
| **MEDIUM** | Minor version difference (e.g., 14.2 -> 14.5) | `MEDIUM - Minor updates available` | Note notable changes if any |
| **LOW** | Patch version difference (e.g., 14.2.0 -> 14.2.3) | `LOW - Patch updates only` | Generally safe, note if security patches |
| **CURRENT** | Same version | `CURRENT` | No action needed |
| **UNKNOWN** | No `latestVersion` data | `UNKNOWN - Version not checked` | Queue for version lookup |

### Version Comparison Logic

```
Parse semver: MAJOR.MINOR.PATCH
Compare MAJOR first:
  If different -> HIGH
Compare MINOR:
  If different -> MEDIUM
Compare PATCH:
  If different -> LOW
If identical -> CURRENT
```

### Output Format in Analysis

```markdown
## Package Version Status

| Package | Tutorial Version | Latest Version | Status | Notes |
|---------|-----------------|----------------|--------|-------|
| Next.js | 14.2 | 15.1.0 | HIGH - Major version behind | App Router breaking changes |
| Prisma | 5.0 | 5.22.0 | LOW - Patch updates only | Compatible |
```

### Production Checklist Integration

For HIGH warnings, add to the Telos Entries section:

```markdown
- [ ] `[production]` Upgrade {package} from v{mentioned} to v{latest} -- {migration notes}
```

## Cross-Video Tracking

When the same package appears in multiple video analyses:

### sourceVideos Deduplication

The `sourceVideos` array tracks all YouTube URLs where a package was mentioned. `PackageDb.ts add` already handles deduplication -- if a source URL is already in the array, it won't be added again.

### Version Discrepancy Tracking

If a package is mentioned with different versions across videos, the `notes` field should capture this:

```json
{
  "name": "next",
  "versionMentioned": "14.2",
  "notes": "Also seen as v13.5 in video xyz123, v15.0 in video abc789"
}
```

The `versionMentioned` field always reflects the MOST RECENT video analysis. Previous versions are noted in `notes`.

### Cross-Video Query

To find all tutorials using a specific package:

```bash
bun run PackageDb.ts query --name "next"
# Returns sourceVideos array with all video URLs
```

## Staleness Rules

| Condition | Action |
|-----------|--------|
| `latestChecked` is empty | Always check (never been verified) |
| `latestChecked` is > 7 days ago | Check on next tutorial analysis |
| `latestChecked` is <= 7 days ago | Skip check, use cached `latestVersion` |

### Version Lookup Sources

| Package Type | Lookup URL | Parse Field |
|-------------|-----------|-------------|
| npm packages | `https://registry.npmjs.org/{name}/latest` | `version` |
| PyPI packages | `https://pypi.org/pypi/{name}/json` | `info.version` |
| Go modules | `https://proxy.golang.org/{module}/@latest` | `Version` |
| Rust crates | `https://crates.io/api/v1/crates/{name}` | `crate.max_version` |
| Other | WebSearch `"{name} latest version"` | Parse from results |

## Future Enhancements

- **Automatic version checking**: Scheduled cron job to refresh stale packages
- **Version comparison alerts**: Flag tutorials using deprecated/vulnerable versions
- **Package popularity ranking**: Track which packages appear most frequently
- **Cross-tutorial recommendations**: "If you learned X, you might like Y"
- **Export to CSV/JSON**: Generate package inventory reports
