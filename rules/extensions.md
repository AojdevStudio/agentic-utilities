# Extensions

## Rules

- Use kebab-case extension directories: `extensions/<name>/index.ts`.
- Export `default function (pi: ExtensionAPI)` from every extension entry point.
- Use globally specific snake_case tool names, for example `agentic_utilities_ping`.
- For string enum parameters, use `StringEnum` from `@mariozechner/pi-ai`; do not hand-roll `Type.Union([Type.Literal(...)])`.
- Wrap custom tools that mutate files with `withFileMutationQueue()` across the full read-modify-write window.
- Truncate large tool output and report where the full output is stored.
- Check `ctx.hasUI` before depending on interactive UI behavior.

## Rationale

Extensions run with broad local access. Naming, schema, mutation, and output rules reduce collisions, race conditions, and unusable tool results.
