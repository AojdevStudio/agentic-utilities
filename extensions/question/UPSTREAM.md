# Upstream attribution

This extension vendors and adapts code from:

- Package: `@juicesharp/rpiv-ask-user-question`
- Version: `1.15.0`
- Package page: https://pi.dev/packages/@juicesharp/rpiv-ask-user-question
- Source: https://github.com/juicesharp/rpiv-mono/tree/main/packages/rpiv-ask-user-question
- License: MIT

Verification performed with `npm view @juicesharp/rpiv-ask-user-question@1.15.0` and `npm pack @juicesharp/rpiv-ask-user-question@1.15.0`.

## Adapted files

The upstream package contents are vendored under `extensions/question/rpiv/`, excluding package metadata but preserving the upstream MIT license at `extensions/question/rpiv/LICENSE`. The vendored source was adapted to:

- use this repo's `@mariozechner/*` Pi package imports instead of upstream `@earendil-works/*` imports;
- use `extensions/question/index.ts` as the single Pi-loaded question extension entrypoint;
- retain the upstream parameter contract under the repository-compliant `agentic_utilities_ask_user_question` name alongside legacy `AskUserQuestion` and `AskBatchQuestions`;
- expose the upstream TUI executor for `AskBatchQuestions({ presentation: "tui" })`;
- keep localization optional through runtime dynamic import shims.

## Compatibility notes

The vendored tool is deliberately namespaced. Installing `@juicesharp/rpiv-ask-user-question` can therefore expose both tools without last-registration-wins behavior; callers must use the appropriate tool name.
