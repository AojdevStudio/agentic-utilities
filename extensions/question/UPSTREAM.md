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
- keep `extensions/question.ts` as the single Pi-loaded question extension entrypoint;
- register upstream-compatible `ask_user_question` alongside legacy `AskUserQuestion` and `AskBatchQuestions`;
- expose the upstream TUI executor for `AskBatchQuestions({ presentation: "tui" })`;
- keep localization optional through runtime dynamic import shims.

## Compatibility notes

If a user also installs `@juicesharp/rpiv-ask-user-question`, both packages try to register `ask_user_question`. Pi tool registration is last-registration-wins, so extension load order can decide which implementation is active.
