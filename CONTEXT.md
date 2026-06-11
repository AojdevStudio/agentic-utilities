# Agentic Utilities

This context describes the language for reusable Pi extensions, agent skills, prompt templates, and related agent-facing utilities maintained in this package.

## Language

**Pi Extension**:
A TypeScript module loaded by Pi to add tools, commands, UI, providers, or event interception.
_Avoid_: Plugin, script, hook when referring specifically to Pi-loaded extension modules.

**Conditional Hook**:
A configured Pi Extension behavior that runs a side-effect command only when a Pi event and its payload match explicit filters.
_Avoid_: Lifecycle hook when payload filtering is the important distinction.

**Search Backend**:
A web search provider used behind an agent-facing search tool, such as Brave Search, Tavily, or Google Custom Search.
_Avoid_: Search tool when discussing the provider rather than the Pi tool surface.

**Google CSE Credentials**:
The Google Custom Search API key plus Programmable Search Engine ID (`cx`) required to query Google Custom Search JSON API.
_Avoid_: Google key alone, CSE token.

**Conditional Hook**:
A user-configured policy that watches Pi events and may run a side-effect command after a matching event.
_Avoid_: Built-in hook when the behavior comes from JSON config.

**Smart Fetch**:
A browser-fingerprinted URL fetch and extraction flow that returns agent-usable content rather than just raw HTTP bytes.
_Avoid_: Web search, crawl.

**PDF Extraction**:
Converting a fetched PDF document into text or markdown content for agent use.
_Avoid_: PDF download when the primary result is extracted content.

**Canonical Skill**:
An Agent Skill whose source of truth lives in this package for sharing, validation, and distribution.
_Avoid_: Global skill, daily skill when referring to the repo-owned source.

**Harness Inventory**:
The set of skills installed for one agent harness, intentionally allowed to differ from other harnesses.
_Avoid_: Global skills, shared skills when isolation is intended.

**Coupled Inventory**:
A Harness Inventory deliberately linked to a Canonical Skill so edits affect both places.
_Avoid_: Symlinked skill when discussing the policy decision rather than the mechanism.

**Distribution Lane**:
A packaging path that exposes the same agent-facing resource to a specific ecosystem or installer.
_Avoid_: Duplicate copy when the duplication is intentional packaging.

**CodeGraph Snapshot**:
A versioned repository graph used by agents for structural code discovery.
_Avoid_: Cache when the graph is intentionally tracked with the repo.

## Relationships

- A **Pi Extension** can expose one or more agent-facing tools.
- A **Conditional Hook** is implemented by a **Pi Extension** but is configured separately from the extension code.
- A search **Pi Extension** can route one tool through multiple **Search Backends**.
- **Google CSE Credentials** enable Google Custom Search as an optional **Search Backend**.
- **Smart Fetch** may use **PDF Extraction** when the fetched URL resolves to a PDF.
- A **Canonical Skill** can be copied into one or more **Harness Inventories**.
- A **Coupled Inventory** is exceptional and should be intentional, not the default.
- A **Canonical Skill** may appear in multiple **Distribution Lanes** when each lane serves a different installer or runtime.
- A **CodeGraph Snapshot** should reflect the source tree that agents are expected to explore.

## Example dialogue

> **Dev:** "Should Google be our default web search?"
> **Domain expert:** "No — keep Brave as the default **Search Backend**, and use Google only when **Google CSE Credentials** are configured or explicitly requested."

## Flagged ambiguities

- "Google credentials" was clarified to mean **Google CSE Credentials**: both `GOOGLE_SEARCH_API_KEY` and `GOOGLE_CSE_ID` / `GOOGLE_CUSTOM_SEARCH_ENGINE_ID` are required.
- "Fetch a PDF" was clarified to mean **PDF Extraction** by default, not only downloading the PDF file.
- "Global skills" was clarified to mean either **Canonical Skill** or **Harness Inventory** depending on context; default policy is harness isolation, not shared global coupling.
