# OSC 7501 local alpha for WezTerm

A Windows development alpha that adds native OSC 7501 program status records to WezTerm, with a Lua API and example tab indicators. The terminal owns task state; Lua chooses how to display it.

The implementation is distributed here as [a complete source patch](alpha/osc7501-alpha.patch) against WezTerm revision `e2a98e1b8af156988ad0ac778658ac27b60a55fb`. Local source clones and compiled binaries are intentionally excluded from this repository. Cloning this repository alone does not install a runnable WezTerm executable.

## What works

- Parsing and validation of reports and capability queries.
- Per-pane records, replacement, subtree clearing, app inheritance and lifecycle cleanup.
- `pane:get_program_status()` and title-formatting snapshots, exposed as a list sorted by task ID.
- A Lua tab indicator for working, blocked, done, error and idle states.
- A separate alpha launcher, guided demo and native smoke checks.

Local panes are the supported presentation path. Remote WezTerm mux synchronization is deferred. This is an experimental development build, not an upstream WezTerm release.

## Read the project

- [Testing guide](alpha/TESTING_GUIDE.html), download and open locally for the interactive offline checklist.
- [Alpha commands and Lua contract](alpha/README.md).
- [Verification report](alpha/TEST_RESULTS.md), including the distinction between automated results and user feedback.
- [Accepted design decisions](DECISIONS.md).
- [Implementation scope and status](IMPLEMENTATION_PLAN.md).
- [Source-backed WezTerm architecture guide](WEZTERM_DESIGN_GUIDE.md).

The HTML guides work offline after downloading. GitHub's file viewer shows their source rather than running their interactive controls.

## Recreate the source checkouts

From the root of this repository, in PowerShell with Git installed:

```powershell
git clone https://github.com/wezterm/wezterm.git wezterm
git -C wezterm checkout e2a98e1b8af156988ad0ac778658ac27b60a55fb

New-Item -ItemType Directory -Path .research -Force | Out-Null
git clone https://github.com/wezterm/wezterm.git .research/wezterm
git -C .research/wezterm checkout -b osc7501-local-status e2a98e1b8af156988ad0ac778658ac27b60a55fb
git -C .research/wezterm apply --check ../../alpha/osc7501-alpha.patch
git -C .research/wezterm apply ../../alpha/osc7501-alpha.patch
git -C .research/wezterm submodule update --init --recursive
```

Run these clone commands only for a fresh setup. Existing local checkouts may already contain the feature changes; do not apply the patch twice. The `wezterm/` checkout stays clean as the reference for patch verification. Build and edit the feature in `.research/wezterm/`.

## Build and test on Windows

Install WezTerm's Windows build prerequisites, including Rust, the MSVC C++ toolchain and the required native build tools, following the contributing instructions in the cloned source. This repository does not install them automatically. Cargo's first build may need network access to fetch dependencies.

```powershell
Push-Location .research/wezterm
cargo build -p wezterm-gui -p wezterm
Pop-Location
.\alpha\Package-Alpha.ps1 -SkipBuild
.\alpha\Start-Alpha.ps1
```

The packaging script requires Python and checks the exported patch against the clean reference before copying native runtime files. It puts the executable and DLLs in the ignored `alpha/bin/` directory. Keep that local directory to run the alpha; it does not belong in Git commits.

```powershell
.\alpha\Start-Alpha.ps1 -Mode Smoke -Wait
.\alpha\Check-Source.ps1
```

The smoke test should print `PASS: 11 native GUI/Lua checks.` The source checks run offline and require their dependencies to have been fetched already. See the testing guide for manual checks and known limitations.
