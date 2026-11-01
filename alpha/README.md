# Testing the OSC 7501 alpha

This is a separate Windows development build with native local task records and a Lua tab indicator. Your regular WezTerm installation and configuration are not replaced.

For a step-by-step manual checklist with commands, expected results and a downloadable test record, open [TESTING_GUIDE.html](TESTING_GUIDE.html) in your browser. It works offline. Start with the automated smoke check, then use the visual demo and manual cases A through J.

## Start the guided demo

From PowerShell:

```powershell
cd C:\Users\omotu\projects\osc7501
.\alpha\Start-Alpha.ps1
```

The new window is titled `WezTerm OSC 7501 ALPHA`. The script pauses at each step. Check the tab, then press Enter to continue. It demonstrates working, permission blocking, child-task updates, subtree clearing, done surviving a shell prompt, error and clearing all records.

Press **Ctrl+Shift+U** in that window to log the active pane's snapshot and open the debug overlay. The message starts with `OSC 7501 records:`. Escape returns to the terminal.

## Run the native smoke checks

```powershell
.\alpha\Start-Alpha.ps1 -Mode Smoke -Wait
```

A fresh alpha window runs twelve checks. The launcher prints `PASS: 12 native GUI/Lua checks` when they pass. Most cases check both `pane:get_program_status()` and the data observed by the tab formatter. One case arms an expectation before emitting status-only output, then checks cached formatter data without the user-variable event that could otherwise cause an unrelated refresh. Results are saved under `alpha/test-results/<run-id>/` with actual record values for diagnosis. The window remains open for inspection. For an unattended run, add `-CloseAfterTest`; in Smoke mode this also implies `-Wait`.

The checks cover empty state, working progress, nested records and inherited app, invalid-report rejection, complete replacement, BEL framing, soft reset, alternate-screen preservation, clear-prefix boundaries, prompt cleanup and full reset. The final check actually exits the attached PowerShell process and verifies that working/blocked records disappear while done remains. The twelve checks group some related cases together.

This test-only handshake uses WezTerm's existing user-variable event and local JSON result files. It adds no OSC 7501 response command. All protocol replies remain limited to capability detection.

## Try your own reports

Start a shell with the test helpers loaded:

```powershell
.\alpha\Start-Alpha.ps1 -Mode Shell
```

In the new alpha window:

```powershell
Send-Status 'state=working:app=cargo:progress=42'
Send-Status 'state=blocked:id=build/tests:kind=permission'
Send-Status 'state=done:id=build/tests'
Send-Status 'state=clear:id=build'
Send-Status 'state=clear'
```

To include text, use base64 UTF-8:

```powershell
Send-Status ("state=blocked:kind=question:msg=" + (Encode-Text 'Which environment should I deploy to?'))
```

The formatter uses the most urgent record across all splits in a tab. Its priority is blocked, error, working, done, idle, then the first encountered record breaks ties. It shows the pane number so you can find the source. This is example presentation policy, not a protocol rule. It displays a static percentage or state label; there is no spinner.

## What to check manually

| Action | Expected result |
| --- | --- |
| Send a working report, then a blocked child | Both records appear in the snapshot. The tab highlights blocked. |
| Change the child to done without repeating its title or kind | The old title/kind disappear. Reports replace whole records. |
| Send `state=error:id=bad//id` | Existing records remain unchanged. |
| Send the same report repeatedly | Snapshot contents stay the same. Internal eviction recency still refreshes. |
| Add `build`, `build/tests` and `builder`, then clear `build` | Only `builder` remains from those three IDs. |
| Send working and done records, then OSC 133 A | Working/blocked disappear. Done/error remain. |
| Enter copy mode or quick select | The task indicator remains available through the overlay. |
| Put a blocked task in one split and focus the other | The tab still identifies the blocked task's pane. |
| Leave a task on another tab | Its indicator remains visible on that inactive tab. |

Use the debug overlay's Lua prompt to inspect the active pane directly:

```lua
wezterm.serde.json_encode(window:active_pane():get_program_status())
```

Changing a returned Lua table affects that snapshot only. It does not remove stored records.

## Lua contract

`pane:get_program_status()` returns a new list sorted by bytewise task ID. The same shape is available as `pane_info.program_status` in title formatting. Each record has `id` and `state`; absent optional fields are omitted. Optional fields are `kind`, `progress`, `app`, `title`, `msg`, and `effective_app`.

The root ID is the empty string. `app` is the explicit report value. `effective_app` includes nearest-ancestor inheritance. Task IDs are identities; numeric list positions are not stable identities. State/kind values are lowercase strings. Text fields are decoded UTF-8 plain text.

## Limits of this alpha

- Local panes are the supported presentation path. Remote WezTerm mux clients do not synchronize these records. A headless terminal can parse reports and answer queries, which does not imply remote GUI indicators.
- Ordinary SSH output may carry reports into a local pane, but this alpha does not claim an SSH test matrix.
- Finished/error records remain until replaced or cleared, including after prompt and attached-process exit. There is no dismissal UI. Clear with `Send-Status 'state=clear'`.
- Prompts only trigger cleanup when the shell emits OSC 133 A. A plain shell prompt that sends no marker cannot supply that event. The demo emits it explicitly when testing cleanup.
- There are no desktop notifications, OS taskbar indicators, OSC 9;4 mapping or support guarantees for other operating systems.
- This debug build is larger and slower than a release build. Use a separate window for testing.

## Rebuild after source changes

This branch contains the modified source at the repository root. Follow the root README to restore the pinned Windows runtime assets and initialize native submodules before building.

```powershell
.\alpha\Package-Alpha.ps1
```

Close alpha windows before replacing their binaries. The package script copies the built runtime into the ignored alpha/bin directory. Run .\alpha\Check-Source.ps1 for focused checks. These offline commands require dependencies to have been fetched first.