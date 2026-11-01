# OSC 7501 in WezTerm

Target presentation: November 2, 2026, updated by the user on October 28.

## Working arrangement

The user owns the design decisions and learns the architecture through reviewing the feature. Codex implements, tests, and explains the code. Learning Rust syntax is not a prerequisite for this project.

For each significant choice, Codex presents the problem, viable options, a recommendation, and the consequence for the implementation. Accepted choices are recorded in DECISIONS.md. Routine implementation details follow the chosen architecture. Protocol requirements are identified as requirements rather than presented as discretionary choices.

Use [WEZTERM_DESIGN_GUIDE.md](WEZTERM_DESIGN_GUIDE.md) when preparing those proposals. It distinguishes current source patterns, explicit Wez-authored rationale, and our recommendations. The companion [architecture lesson](wezterm-architecture-guide.html) is for learning and review. Project-level agent guidance in AGENTS.md points future implementation work to these documents.

## Presentation scope

Accepted on October 15: native local support with Lua tab indicators. Remote mux support follows.

The presentation build should demonstrate:

- Native parsing of OSC 7501 reports and capability queries.
- Per-pane status records, including simultaneous parent and child tasks.
- Protocol validation, size limits, replacement, clearing, inheritance, and record lifetime behavior.
- A Lua interface and example tab formatting that distinguishes working, blocked, completed, and failed tasks.
- Repeatable demo commands and regression tests.
- An architecture explanation, decision history, and an honest account of completed and deferred work.

Deferred from the presentation target: remote WezTerm mux client synchronization, remote reconnect snapshots, desktop notification policy, OS taskbar integration, and legacy OSC 9;4 mapping. Ordinary SSH carries the byte stream but is not the same feature as remote WezTerm mux replication; include it only if it is specifically tested.

This is a local-support milestone, not a claim of complete remote support or an upstream release. Upstream review and acceptance are outside the presentation schedule.

## Implementation units

| Unit | What will be made | Evidence of completion |
| --- | --- | --- |
| Baseline | Reproducible parser tests and a Windows development build | Existing tests pass; development GUI can launch |
| Parser | Typed query/report commands in wezterm-escape-parser | Focused tests for valid, invalid, bounded, and fragmented input |
| Record model | Owned records, bounded storage, replacement, clearing, inheritance, and lifetime cleanup | Model tests, including clear-prefix boundaries and stable behavior across screen switches |
| Local integration | Capability reply, pane getter, change delivery, prompt/reset/process-exit wiring | Terminal and pane integration tests; reports update native state |
| Lua presentation | Structured pane data and a sample tab formatter | Running demo shows working and attention states |
| Presentation package | Demo script, architecture notes, test report, and known limitations | Demo can be repeated from written instructions |

## Order of work

1. Establish baseline tests and build prerequisites.
2. Settle record ownership, then implement the parser and record model with tests.
3. Connect local terminal/pane behavior and settle the Lua data contract.
4. Settle tab presentation and demonstrate it in a development build.
5. Finish regression checks, demo scripts, and presentation notes.

Reserve the last several days for integration failures, demo rehearsal, and packaging. A working local build is a milestone to verify early; the two-week date is a target, not an unverified delivery guarantee.

## First architecture choice

Status: Option A accepted by the user on October 15.

Option A, recommended: TerminalState owns the records, following existing OSC 9;4 progress. The parser produces typed commands; the terminal model applies them. The local pane reads model state and forwards internal change alerts. This keeps the record rules in a GUI-independent component with existing terminal test helpers.

Option B: LocalPane owns the records, following libghostty-vt's host-owned model. The terminal emits validated report and lifecycle callbacks. The pane wrapper applies the record rules. This isolates persistent records from the terminal core, but requires additional callbacks and coordination for prompts, resets, and consumers that embed the core without LocalPane.

Both preserve native parsing and can support the chosen local presentation scope. Neither implies storing authoritative state in Lua.

## Source baseline

- Local source checkout: .research/wezterm.
- The user's clean full-history clone at wezterm/ is now the architecture research reference at the same revision. Existing implementation changes remain in .research/wezterm; no patch was copied between the checkouts for the guide.
- WezTerm revision: e2a98e1b8af156988ad0ac778658ac27b60a55fb.
- Protocol reference: https://www.superlogical.com/rex/docs/build/program-status, revision 0.4, rechecked October 27.
- Rust and Cargo are installed. Visual Studio 2022 Community includes the C++ toolchain.
- Initial offline parser baseline could not run because anes 0.1.6 was not cached. Dependencies were downloaded, and `cargo test -p wezterm-escape-parser --lib conemu` passed: 1 passed, 0 failed.
- The Windows GUI and CLI development build now succeeds and the native alpha has launched. Its pinned native dependency submodules are initialized. A separate unmodified GUI binary was not tested.

## Current status

The local alpha is implemented on branch `osc7501-local-status` in .research/wezterm. D003 was accepted on October 27: expose a list sorted by task ID. The user also authorized reasonable autonomous implementation and testing within this directory.

Implemented:

- Typed OSC dispatch and owned reports, with validation, complete framing and a specific bound for incoming OSC 7501.
- Terminal-owned records, replacement, subtree clearing, app inheritance, eviction and change suppression.
- Prompt, reset and attached-process exit behavior, including protection against late buffered transient reports after exit.
- Local pane getter, separate nonserialized change callback/mux marker, Lua method and title-information field.
- A Lua tab formatter, separate Windows alpha package, guided demo and native smoke checks in `alpha/`.
- API documentation and a patch that applies to the untouched reference clone.

Verification completed:

- Framing parser: 31 tests passed; heapless no_std configuration compiles.
- Escape parser: 31 default no_std/alloc tests and 71 std tests passed.
- Terminal model and integration: 75 tests passed.
- Focused Lua contract: 1 test passed.
- Native alpha: all 12 GUI/Lua checks passed, including actual attached-process exit and a status-only refresh interval.
- Final Windows GUI and CLI build succeeded; the packaged binary passed a fresh native run after the framing changes were frozen.
- Source patch application check, script parsing and diff whitespace checks passed.

The October 28 review fixed malformed-byte and incomplete-UTF-8 framing, a late-output process-exit replacement race, and automatic-close launcher behavior. The rebuilt package passed the checks above. The findings and scope are in [CODE_REVIEW.md](CODE_REVIEW.md); the learning document is [OSC7501_IMPLEMENTATION_GUIDE.md](OSC7501_IMPLEMENTATION_GUIDE.md), with an [offline interactive version](osc7501-implementation-guide.html).

The local alpha is ready for user testing. Launch the guided demo with `alpha/Start-Alpha.ps1`, or run `alpha/Start-Alpha.ps1 -Mode Smoke -Wait` for the automated native checks. The verification report is [alpha/TEST_RESULTS.md](alpha/TEST_RESULTS.md).

Remote mux synchronization remains deferred. Manual presentation rehearsal and layout checks follow the alpha. Use [alpha/README.md](alpha/README.md) to test the build yourself.
