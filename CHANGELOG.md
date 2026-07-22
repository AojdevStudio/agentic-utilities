# Changelog

## 0.1.0

- Initial Pi package scaffold.
- Added sample `hello` extension.
- Added `scaffold-notes` maintenance skill.

## [Unreleased]

### Added

- global, guided, and worktree-safe (#41) [#41]
- rename command, fix review blockers, add launch README
- semantic colors in action menu
- restyle TUI + action-submenu UX
- restyle TUI — rounded panels, selection bars, human labels
- TUI/CLI wrapper around the bws CLI
- add pr-review-queue fleet reviewer skill (#35) [#35]
- add public Herdr fleet workflow (#32) [#32]
- add plugin packaging and question TUI (#26) [#26]
- share documentation workflows (#30) [#30]
- add skill-inspector plugin + verdict-aware renderer
- Add skill-inspector skill: skillspector wrapper with readable reports (#28) [#28]
- add ship-issue Claude Code plugin (#27) [#27]
- append output and dedupe events (#24) [#24]
- run matching bash hooks (#23) [#23]
- add config status command (#22) [#22]
- port dogfood skill to Claude Code plugin (#14) [#14]
- package shared Pi skills, extensions, and 9 Claude Code plugins (#11) [#11]
- Add public shared skill/plugin ports (#4) [#4]
- add Bambu Lab 3D-printing plugin and skill
- add slash argument completions
- add YouTube analyzer plugin marketplace
- report slice completion status
- add grill-first AFK workflow

### Changed

- bump brace-expansion from 5.0.5 to 5.0.7 in the npm_and_yarn group across 1 directory (#37) [#37]
- bump ws from 8.20.0 to 8.21.1 in the npm_and_yarn group across 1 directory (#31) [#31]
- bump undici in the npm_and_yarn group across 1 directory (#29) [#29]
- bump shell-quote (#16) [#16]
- bump the npm_and_yarn group across 2 directories with 1 update (#12) [#12]
- Port claude-md-improver skill to Claude Code plugin (#13) [#13]
- Package shared Pi skills and extensions (#7) [#7]
- bump the npm_and_yarn group across 1 directory with 2 updates (#9) [#9]
- bump the npm_and_yarn group across 1 directory with 2 updates (#5) [#5]
- Initial commit

### Fixed

- remediate merged-#35 CodeRabbit findings (#36) [#35]
- harden terminal and clipboard lifecycle
- aggregate every terminal cleanup failure
- make clipboard and empty-project failures visible
- post-merge remediation for #33 (validator hardening, packaging hygiene, ask-codex metadata) (#34) [#33]
- complete issue #10 port hardening (#33) [#10]

### Security

- bump protobufjs to 7.5.6 (#6) [#6]

