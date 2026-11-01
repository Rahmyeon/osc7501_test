# WezTerm with OSC 7501

This branch contains the complete modified WezTerm source, with native local OSC 7501 program status support. Start with [term/src/program_status.rs](term/src/program_status.rs), [the parser](wezterm-escape-parser/src/program_status.rs), and [the Lua tab example](alpha/alpha.lua).

Based on upstream WezTerm revision `e2a98e1b8af156988ad0ac778658ac27b60a55fb`. The source includes the October 28 review fixes. Local panes are supported; remote mux record synchronization is deferred. This is an experimental alpha, not an upstream release.

## Clone and build

```powershell
git clone --branch osc7501-source --recurse-submodules https://github.com/Rahmyeon/osc7501_test.git
cd osc7501_test
.\alpha\Restore-WindowsAssets.ps1
cargo build -p wezterm-gui -p wezterm
.\alpha\Package-Alpha.ps1 -SkipBuild
.\alpha\Start-Alpha.ps1
```

Install the upstream Windows build prerequisites first, including Rust, Visual Studio's MSVC C++ tools and the native build tools. See [the upstream contributing guide](CONTRIBUTING.md). No patch application is required.

Executables, DLLs and build output are excluded. The restore script downloads the five pinned upstream Windows runtime assets to your own checkout and verifies their SHA-256 hashes; these are needed by the existing Windows build and package scripts. No compiled alpha is included in this branch. The native dependency submodules remain pinned upstream submodules.

## Learn and test

- [Implementation and design decisions](OSC7501_IMPLEMENTATION_GUIDE.md).
- [Offline interactive learning guide](osc7501-implementation-guide.html), download and open locally.
- [Code review and fixes](CODE_REVIEW.md).
- [Accepted decisions](DECISIONS.md).
- [Alpha commands and tests](alpha/README.md).
- [Verification history](alpha/TEST_RESULTS.md).

The reviewed Windows build passed all six source checks and 12 native GUI/Lua checks on October 28. These are recorded results, not a claim that this publication ran a fresh full build. After building locally, run:

```powershell
.\alpha\Start-Alpha.ps1 -Mode Smoke -CloseAfterTest
.\alpha\Check-Source.ps1
```

The original project's documentation branch is `main`. This `osc7501-source` branch exposes the source files directly. Earlier documents may describe the historical two-checkout/patch workflow; building this branch uses the commands above. Upstream license files are retained.
