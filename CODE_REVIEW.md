# OSC 7501 alpha code review

Review date: October 28, 2026. Presentation target: November 2, 2026.

Scope: the complete local feature patch across the framing parser, report parser, terminal model, lifecycle hooks, local pane, mux notifications, Lua API, GUI consumers, overlays and Windows alpha scripts. The clean upstream reference is WezTerm `e2a98e1b8af156988ad0ac778658ac27b60a55fb`. The feature lives in `.research/wezterm`; it is not evidence of upstream conventions.

Three actionable P2 issues were found and fixed. P2 here means a real correctness bug under a specific trigger, rather than a claim that normal demo usage always fails. A separate smoke-test coverage gap was improved. No broader refactor was needed.

## Fixed: malformed bytes could change a clear command's target

Affected code: [framing parser](vtparse/src/lib.rs), with regressions in the [report parser](wezterm-escape-parser/src/program_status.rs) and [terminal integration tests](term/src/test/program_status.rs).

The original alpha silently dropped some invalid raw bytes. An ID spelled `bad<0xff>id` became `badid`, so an invalid clear could delete a valid record. A standalone reproduction linked to the original compiled parser confirmed the same problem with raw `0xa0` and replacement OSC `0x9d`.

A second path involved incomplete UTF-8. In `state=clear:future=x<0xc2>:id=target`, decoding could swallow the colon. The unknown pair was then skipped, leaving a root clear. This could remove unrelated tasks.

Recognized OSC 7501 bodies now preserve invalid high bytes for validation without passing them through the generic Unicode decoder. The wire fields are ASCII; human text travels as base64. Delimiters and supported terminators therefore remain independent bytes. Replacement controls cancel the report. The unsupported raw C1 terminator is rejected for this protocol instead of applying a truncated prefix. Other OSC handling retains its existing path.

Tests cover completed BEL and ESC-backslash sequences, malformed IDs, malformed unknown pairs, incomplete UTF-8 before separators and terminators, cancellation and recovery. Terminal tests verify that existing records are not accidentally cleared. These tests matter because body-only tests never exercised the normalization that happened before the body parser.

## Fixed: buffered output could preserve an obsolete completed result

Affected code: [record store](term/src/program_status.rs), with store and terminal regressions.

The process waiter can observe exit before all PTY output is applied. If `build` was done, then buffered output reported a new working state for `build`, the exit guard ignored that late report and retained the obsolete done result.

The fix combines replacement and exit cleanup. A late working or blocked report removes an existing record with exactly the same ID. Finished descendants survive. If no record existed, there is no visible change and no extra notification. Tests cover previous done, error and idle states, both transient states, recency removal, inheritance changes and repeated no-ops.

## Fixed: an automatic-close request could leave a hidden GUI running

Affected code: [launcher](alpha/Start-Alpha.ps1).

`-Mode Smoke -CloseAfterTest` selected hidden startup, but the cleanup path required a separate `-Wait`. Without it, the held GUI remained invisible after the smoke shell exited. In Smoke mode, CloseAfterTest now implies Wait, so completion and cleanup run together.

## Improved: smoke tests now include a status-only refresh interval

Affected code: [exercise script](alpha/Exercise-Alpha.ps1) and [test configuration](alpha/alpha.lua).

The existing assertions sent a user-variable event after each report. WezTerm independently refreshes titles for that event, so a passing formatter snapshot could hide a broken status-change notification.

The new case arms an expectation before sending the status report, waits for the existing title refresh to settle, then emits only the status bytes and silently waits. Lua checks the cached formatter data. It does not request a live snapshot or inject another user-variable event during that interval. This removes the deliberate masking refresh. Unrelated window events can still refresh titles, so it is not exclusive proof of the notification route.

## Reviewed boundaries that did not need changes

- The model owns records and computes inherited app values when taking snapshots.
- Lua receives owned records sorted by ID. Numeric positions are not identities.
- The terminal lock is released before Lua conversion. Change delivery is scheduled to avoid immediate reentry while that lock is held.
- The serializable Alert enum and mux codec are unchanged. The local notification has explicit GUI and server handling; remote snapshots remain deferred.
- Copy and quick-select overlays expose the underlying records.
- The example formatter keeps tab priority and styling out of protocol semantics.

## Validation and limits

All six focused source checks passed after the final fixes: vtparse 31 tests, heapless compilation, escape parser 31 default tests and 71 std tests, terminal 75 tests, and one focused Lua contract test. Configurations overlap, so these counts are not a unique-test total. The GUI/CLI rebuilt successfully and the packaged alpha passed all 12 native checks, including the launcher invocation without an explicit Wait switch. Patch application, source/binary hashes, script parsing and whitespace checks passed.

Detailed execution results are recorded in [alpha/TEST_RESULTS.md](alpha/TEST_RESULTS.md). The original working package and design records were copied to `.recovery/2026-10-28-code-review/` before edits. That local archive is ignored by Git. The source comparison used the [official protocol reference](https://www.superlogical.com/rex/docs/build/program-status), revision 0.4 as checked on October 28.

This review is focused on the new feature and its integration, not every WezTerm component. Native tests inspect application state; they do not replace manual layout and real-application checks. Remote mux, other operating systems and an SSH compatibility matrix remain outside the verified milestone. The OpenCode idle observation remains application behavior to investigate using snapshots or captured reports; no evidence justified rewriting idle to done.

For the learning document, see [the implementation guide](OSC7501_IMPLEMENTATION_GUIDE.md). It separates your accepted decisions, implementation choices and protocol requirements, and includes a presentation walkthrough.
