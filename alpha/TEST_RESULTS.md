# Alpha verification

Date: October 27, 2026. Platform: Windows x64, MSVC development build.

Baseline source revision: `e2a98e1b8af156988ad0ac778658ac27b60a55fb`, with the local changes in `osc7501-alpha.patch`. The clean `wezterm/` reference remains unchanged. `source-manifest.json` records changed-file hashes; `build-info.json` records packaged binary hashes.

## Completed source checks

| Check | Result |
| --- | --- |
| `vtparse` library tests | 27 passed |
| `vtparse` heapless `no_std` build | Passed |
| Escape parser with default no_std/alloc configuration | 29 passed |
| Escape parser with std | 69 passed |
| Terminal model and integration tests | 71 passed |
| Focused Lua serialization contract | 1 passed |
| Final Windows GUI and CLI build | Passed |
| Patch against the clean reference, `git apply --check` | Passed, 22 changed/new source files |
| PowerShell script parsing | Passed |
| Interactive demo launcher visibility | Passed after fixing startup window style; visible main window and title verified |
| Focused `git diff --check` | Passed |

Feature configurations overlap, so these counts should not be added and described as unique tests. The source logs and exact commands are in `source-checks/`, with a machine-readable `results.json`.

The terminal integration tests exercise real incoming bytes, complete and fragmented terminators, invalid-input atomicity, replacement, subtree clearing, dynamic app inheritance, full/soft reset, screen switches, OSC 133 A cleanup, duplicate notification suppression, process exit and late buffered reports, snapshot independence, and fixed capability replies.

Framing tests cover incomplete and canceled reports, raw control bytes, oversized unterminated input, malformed semicolons and parser recovery. The new buffer bound is specific to recognized OSC 7501 input; unrelated OSCs retain their existing policy.

## Native GUI and Lua

The final packaged alpha passed all eleven native checks in:

`test-results/ecf0282e79ac47f4ae361f29592c1ab4/`

Each check reads the real pane Lua getter and the record snapshot observed by the tab formatter. The final check exits the attached PowerShell process and verifies held-pane cleanup while preserving a done record.

This run used the rebuilt binary with the frozen framing changes and the startup-safe test handshake. `build-info.json` records the binary hashes. The source checks and native run are complete; further layout and presentation checks are listed below.

## Fixes found during verification

- Explicitly imported the terminal test suite's assertion macro to resolve a compilation ambiguity.
- Delayed OSC 7501 dispatch until the complete ST instead of accepting an ESC prefix.
- Canceled reports do not apply a valid-looking prefix; otherwise ignored controls reach validation rather than repairing malformed values.
- Removed `-NoExit` from smoke-mode PowerShell so its final process-exit check exercises a real exit. Demo and shell modes still keep the shell available.
- The final cold launch exposed a test-handshake race: a user-variable event could precede GUI subscription. The harness now retries the check request and deduplicates received checks. It never resends task reports to keep records alive.
- User testing exposed that the launcher requested a hidden window for Demo and Shell too. Interactive modes now use Normal window style; only unattended Smoke with CloseAfterTest uses Hidden. A new demo process exposed the expected window title and a nonzero visible main-window handle. No Rust rebuild was needed for this launcher-only fix.
- Native compilation of the isolated Lua contract needed execution outside the filesystem sandbox. The initial sandbox attempt failed to update its native static library; that environment failure is not a passing test result.

The final build emits existing deprecation/lifetime warnings in font and window code. It exits successfully. No unrelated warning cleanup was included in this feature.

## Limits of verification

Native checks inspect actual application state through Lua; no screenshot-based visual review was performed. Manual checks still recommended: inactive tabs, multiple splits, copy/quick-select overlays, resizing and font/layout appearance. No remote mux synchronization, SSH matrix, desktop notification, OS taskbar or other-operating-system tests were performed.

## User testing feedback, October 27, 2026

The user reports finishing their checks and that behavior seems good overall. They observed OpenCode appearing to return to idle after completing a task, rather than retaining done. No per-case checklist, captured OSC byte stream or corresponding Lua snapshot was supplied, so this is qualitative user feedback rather than a separately verified pass for each manual case. OpenCode's emitted state and the formatter's selected record should be distinguished before attributing that behavior to either application or terminal. No terminal behavior was changed in response.

## Package recovery verification, October 28, 2026

After the user removed the packaged executables while trying to publish the project, all seven runtime files were restored from the existing build outputs. The executable hashes still match build-info.json, and all 22 feature source files still match source-manifest.json. The source patch still applies to the unchanged reference checkout. A fresh `Start-Alpha.ps1 -Mode Smoke -Wait -CloseAfterTest` run passed all 11 native GUI/Lua checks. Its locally retained result directory is `test-results/25dd2ada7d1f4190bd16412bf920c6f2/`. New generated smoke results are ignored by Git; previously committed verification examples remain in the repository.

See `README.md` for the guided demo, automated smoke run, exact Lua contract and known alpha limitations.
