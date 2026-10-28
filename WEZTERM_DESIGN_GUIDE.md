# A source-based guide to designing features in WezTerm

Prepared October 15, 2026, for the OSC 7501 project.

This is the engineering reference we will use before proposing or implementing a design. The companion [HTML lesson](wezterm-architecture-guide.html) explains the same ideas with a walkthrough and exercises.

## What this guide can tell us

The best question is "What fits WezTerm's existing architecture, and what evidence supports that choice?" We cannot predict what Wez would approve. We can study decisions he explained, recognize current patterns, and make proposals that a maintainer can assess.

The reference checkout is the user's clean, full-history clone at `wezterm/`, revision `e2a98e1b8af156988ad0ac778658ac27b60a55fb`. We examined source, crate documentation, contribution instructions, and selected historical commits. This is a focused review of the terminal feature path and related design decisions, not a review of every renderer and platform backend.

The separate `.research/wezterm` checkout contains our unfinished OSC 7501 implementation. Its additions are **not evidence of Wez's design style**. No upstream feature code was changed for this guide.

Use these evidence labels:

- **Stated** means a source comment, project document, or Wez-authored commit explicitly gives the reason.
- **Observed** means the current implementation demonstrably has the pattern.
- **Inferred** means we explain a likely benefit or recommend applying the pattern. That is our judgment.

WezTerm has many contributors. Current repository behavior is project evidence; personal attribution below is limited to commits whose author was verified as Wez Furlong. Historical reasons explain historical decisions. Current code still needs to be checked.

## Start with a complete path through the system

A program writes an escape sequence into its output stream. WezTerm turns those bytes into a command, applies the command to a terminal, makes the result accessible through a pane, and presents it in a window.

| Component | Responsibility | What belongs elsewhere |
| --- | --- | --- |
| `vtparse` and `wezterm-escape-parser` | Recognize byte framing and produce typed commands. The escape parser also encodes commands. | Persistent task records, tab selection, Lua styling. |
| `term`, especially `TerminalState` and its performer | Apply terminal semantics and own terminal state. | Window layout, platform event loops, direct PTY ownership. |
| `mux::Pane` | Give consumers a common interface to different pane implementations. | Assuming every pane is backed by a local `TerminalState`. |
| `mux::LocalPane` | Wrap the local terminal, process and PTY; expose core state and forward notifications. | Reimplementing the protocol parser. |
| Mux notifications, server, codec and client pane | Distribute changes and represent remote panes. | Choosing how a task should look in a tab. |
| `lua-api-crates/mux` | Expose pane capabilities to Lua through the shared pane interface. | Becoming the authoritative owner of protocol state. |
| `wezterm-gui` | Build title information, run formatting callbacks, render and schedule redraws. | Defining protocol replacement, clearing or inheritance rules. |

These are cooperating components, not one strict call chain. The GUI reads the pane interface to build `PaneInformation`; the live Lua pane methods are another access path. Notifications tell consumers when to refresh. `PaneInformation` contains owned presentation data, but some of its existing getters also resolve a pane dynamically, so do not assume every property is a frozen snapshot.

The parser explicitly promises encoding and decoding without terminal emulation [E01]. The terminal crate explicitly promises a GUI-independent model and accepts a caller-supplied writer instead of managing a PTY [E02]. Those are stated boundaries, not a diagram we invented.

For OSC 9;4, follow `OperatingSystemCommand::ConEmuProgress` to the performer, then `get_progress`, the pane interface, Lua and the tab renderer [E03 through E09]. This small feature is our strongest neighboring example. The parser can retain partial input; its separation from terminal state does not make it stateless.

## Principles to use when making a design decision

### 1. Place behavior where its required knowledge already lives

**Stated.** The title getter explains that `TerminalState` processes escape sequences without knowing about other tabs. It maintains terminal titles; the GUI decides the larger window presentation [E04].

**Our rule.** Ask what information the behavior needs. A report validator needs bytes and protocol rules. Subtree clearing needs existing records. Tab aggregation needs the panes in that tab. Put each operation in the component that already has that information.

**OSC 7501 application.** D002 already selects `TerminalState` as the record owner. Keep replacement, inheritance and cleanup there or in a helper it owns. Keep tab aggregation in Lua or the GUI. A process-exit event originates in the process wrapper and must be delivered to the owner.

**Limit.** This does not mean every feature needs a new crate. The existing Lua crates were split partly to improve compilation time, explicitly stated in their README and Wez's commit [E14, H04]. Component boundaries have practical costs as well as conceptual benefits.

### 2. Turn external syntax into Rust types before applying behavior

**Observed.** The escape parser uses enums for commands. The progress parser's `SetPercentage(u8)` is an instruction; the terminal's `Percentage(u8)` is stored state. They are separate types [E03, E05].

**Our inference.** This makes the handoff reviewable. The performer handles named cases instead of repeatedly interpreting raw strings throughout the application.

**OSC 7501 application.** Parse a query or validated report, then let the record owner apply it. State and kind should have typed alternatives. Owned text can outlive the parser call. Do not put Lua values in the terminal model.

**Limit.** Recognition and implementation are different. The progress parser recognizes `Paused`, but the performer maps it to `None` [E03, E05]. For our new feature, explicitly document which semantics and capability claims are implemented. Adding a parser variant alone is not feature support.

### 3. Separate authoritative state from messages that announce changes

**Observed.** Progress is stored in `TerminalState`. The performer compares the incoming value to the old one, stores a changed value, and emits an alert. A local pane getter reads the stored value [E04 through E06].

**Our inference.** Consumers can ask for the current fact without retaining every historical event. The event causes consumers to refresh; it is not a substitute for the stored fact.

**OSC 7501 application.** Keep one authoritative task store. An identical valid report may still affect eviction recency, while producing no visible content change. Treat those as different outcomes. Preserve information needed for later rendering rather than storing only a chosen tab label.

**Limit.** Change suppression is feature-specific. The current SetUserVar handler sends its alert after insertion without the progress equality check [E10]. "Never notify twice" is not a repository-wide law. Decide what a notification means for this feature and test it.

### 4. Reuse the existing route, but inspect all of its destinations

**Stated.** Wez's initial progress commit chose the existing `MuxNotification::Alert` route. It also acknowledged that a reconnecting mux client would see no progress until the next update because separate state transfer was missing [H01].

**Our rule.** Reuse an existing mechanism when it fits, after tracing subscribers, serialized forms and client caches. A familiar enum name does not make a change local.

**OSC 7501 application.** An `Alert` in this architecture is an internal message, not a request to show a desktop notification. A new serializable alert variant can affect the remote codec even if our intended milestone is local. Audit that impact before choosing the notification payload.

**Limit.** A change feed cannot reconstruct an arbitrary current state after attach or reconnect. Remote support later needs both current-state transfer and subsequent changes. We should record that obligation now, not claim that reusing progress transport solves it.

### 5. Let shared interfaces hide the implementation, not missing behavior

**Observed.** Local and remote panes implement `Pane`. Progress has a trait getter, a local implementation and a cached client implementation. The Lua getter resolves a pane and calls the shared interface [E06 through E08, E12].

**Stated history.** Wez merged a separate GUI pane Lua object into the mux object because it had no GUI-specific functionality [H08].

**OSC 7501 application.** Put ordinary Lua pane access in `lua-api-crates/mux`, through `Pane`. Add the title-formatting snapshot separately if the selected UI needs it. The model getter, `Pane` getter, Lua method and `PaneInformation` field are distinct connections to implement.

**Limit.** A default empty getter lets an implementation compile; it does not prove remote support. Document supported pane kinds and verify capability-query behavior rather than inferring support from a method's presence.

### 6. Expose useful data, then make presentation policy explicit

**Stated history.** The first progress commit exposed state to Lua and supplied a formatting example. A follow-up added default tab display. The later spinner commit explicitly preserves custom `format-tab-title` behavior [H01 through H03]. Current code tries the Lua result before the default [E09].

**Our inference.** A structured API permits different presentations without changing the protocol model. A built-in default can still be useful; Lua-first history is not a ban on defaults.

**OSC 7501 application.** Return records with stable identities. Decide ordering, omitted fields, root representation and effective app information deliberately. The user accepted D003 on October 27: a list sorted by task ID. The codebase contains both sequence-shaped API data and string-keyed user variables, so precedent alone did not settle that choice [E07, E08, E10].

For a beginner's worked comparison of direct lookup and list searching, see [Finding one task](wezterm-architecture-guide.html#lua-lookup). Both representations contain enough information. The difference is how a caller accesses it, not a need for extra task context. The Lua representation also does not determine the Rust store's internal container.

**Performance condition.** `format-tab-title` is synchronous and must return quickly [E11]. Its data should be ready to inspect. Avoid process spawning, network queries, waiting for a remote reply or taking unrelated locks while formatting.

**Compatibility condition.** Public configuration has deliberate compatibility handling. For example, Wez retained old boolean font settings when introducing a richer slant enum [H11]. That differs from the mux's explicit codec-version gate. Choose the Lua contract carefully before callers depend on it; neither example implies that every API is compatible forever.

### 7. Respect lifetimes, locks and the platform event loop

**Stated.** The PTY reader uses a separate thread because nonblocking PTY reads are not portable across all supported platforms [E13]. The scheduling helper explains that GUI event loops may need the main thread [E15]. `LocalPane` schedules alert delivery through that mechanism [E06].

**Stated history.** Wez described a deadlock path where a blocked output reader could prevent a large paste from completing. He favored a portable architecture and rate control over the old hard-blocking channel [H05].

**OSC 7501 application.** Follow the established scheduling path. Copy a snapshot while holding the terminal lock, then release the lock before conversion, callbacks or rendering. Check the actual call chain before notifying: an alert handler can run while terminal actions are being applied. Do not call back into the same locked terminal synchronously.

**Limit.** The historical channel fix does not prohibit bounded storage. A queue that blocks a reader and a record store that evicts old entries have different behavior. Our record cap is still appropriate.

### 8. Keep reusable crates reusable; optimize for a demonstrated cost

**Stated and observed.** The escape parser supports `no_std` with allocation. Wez explicitly accepted slower linear lookup fallbacks where the usual lookup tables were unavailable [H06]. Adjacent print actions are combined to reduce heap use, and a test checks action sizes [E01].

**Our rule.** Preserve feature configurations, keep large payloads from needlessly enlarging hot-path enums, and measure a cost before introducing complexity. Match nearby ownership and allocation conventions.

**OSC 7501 application.** The parser must not acquire a GUI, mux, Lua or unconditional `std` dependency. Keep bounded decoded data and parser framing limits distinct. Test both the default parser feature configuration and the `std` configuration. Check enum-size tests if a new variant changes representation.

**Limit.** `no_std` does not mean no heap. A map plus an explicit recency structure can be a reasonable first bounded store. This evidence does not select our internal container or justify a premature storage framework.

### 9. Choose error behavior for the boundary being crossed

**Observed.** Unknown OSC commands have a configurable logging path. Clipboard operations and a query reply use their own error handling; some replies intentionally discard writer errors [E05]. Lua formatting failures log and fall back to the default title [E09].

**Our inference.** Error handling depends on the contract. A malformed terminal command, a missing pane, a failing callback and a broken PTY writer are different situations.

**OSC 7501 application.** Apply the protocol's invalid-input and unknown-field rules precisely. Validate before mutating records so a rejected report cannot leave a half-update. Handle capability replies through the supplied writer. Follow local error conventions, and explain any new logging or propagation behavior.

**Limit.** The repository contains both strict and forgiving code. It supports no universal claim such as "always return an error," "always ignore input," or "never unwrap." Use the specification and the particular call contract.

### 10. Make behavior and intent reviewable with tests and documentation

**Stated.** CONTRIBUTING asks for tests, comments that clarify their intent, and documentation for changed behavior. It describes these contribution guidelines as suggestions, not hard-and-fast rules [E16]. The current HEAD adds `no_std` CI specifically to prevent silent regressions [H07].

**OSC 7501 application.** Test syntax in the parser, record rules in the model, and actual byte-to-state behavior in terminal integration tests. Then test the Lua shape and a repeatable GUI demonstration. An isolated store test cannot prove the OSC dispatcher reaches the store.

**Lifecycle evidence.** Wez's recent RIS fix explains that initialization and the existing reset path had left some keyboard and other modes uncleared after an unclean program exit [H12]. Specify full reset, soft reset, screen switch, prompt, exit and acknowledgement separately for each new state field.

**Practical detail.** Follow the crate's edition and neighboring code. The parser is Rust 2024 while `term` is Rust 2018 at this revision [E17]. Avoid broad reformatting and unrelated refactors in a feature patch. Use focused checks while iterating, then the required broader checks before delivery.

**Limit.** Documentation can lag. The current progress document still says WezTerm does not directly use the information, while the current tab renderer does [E09, E18]. Some contribution paths are historical too. Use source to verify behavior; update documentation instead of copying stale claims.

## What the history actually demonstrates

These selected decisions are stronger evidence than a general impression of coding style. Dates below are author dates.

| Wez-authored change | Explicit reason or decision | Useful lesson |
| --- | --- | --- |
| [H01, progress, 2025-02-09](https://github.com/wezterm/wezterm/commit/44866cc137e336d8c00b23f5e0d7d1f03983b591) | Reuse alerts, expose Lua data, acknowledge missing reconnect transfer. | Follow established integration paths and document what remains incomplete. |
| [H02, default progress UI, 2025-02-10](https://github.com/wezterm/wezterm/commit/52c1ca749552af4d8a2f18ec10fe8f14b6622519), [H03, spinner, 2026-07-15](https://github.com/wezterm/wezterm/commit/31c758f1cd7bb1b4c06a9d7b9e89f17ae007016c) | Add presentation incrementally; spinner is conditional on no custom title event. | State/API and display can evolve in separate changes. |
| [H04, Lua crate split, 2022-05-18](https://github.com/wezterm/wezterm/commit/2f14d640e8b0ff73141a4f50a9058f5f1dc7983c) | Reduce build time and permit more parallel compilation. | Account for developer iteration cost when choosing boundaries. |
| [H05, PTY channel fix, 2019-06-24](https://github.com/wezterm/wezterm/commit/bb04d911015a888c9c0e38238ec4bb41c8d2191f) | Prevent reader/writer blockage, use rate control, preserve Windows portability. | Trace blocking and platform behavior before changing concurrency. |
| [H06, parser no_std, 2025-04-20](https://github.com/wezterm/wezterm/commit/0cd3fe8f975515c28a44825dce7894da802296d9) | Support no_std with allocation, accepting some linear fallbacks. | Reuse and portability can justify a measured performance tradeoff. |
| [H07, parser CI repair, 2026-10-10](https://github.com/wezterm/wezterm/commit/e2a98e1b8af156988ad0ac778658ac27b60a55fb) | Add CI to prevent silent no_std regressions. | A supported configuration needs ongoing checks. |
| [H08, pane Lua object merge, 2022-09-11](https://github.com/wezterm/wezterm/commit/42e4a51a2825e1a4166d6bd2dc92571c9494550c) | Remove a GUI object without GUI-specific functionality. | Put shared pane functionality in the shared abstraction. |
| [H09, window overrides, 2021-02-27](https://github.com/wezterm/wezterm/commit/db08b8c1dcd81b5d9a54cf3341dc85159fa0163a) | Store data without Lua-context-bound closures or userdata; begin with whole top-level overrides. | Respect object lifetime and choose a small API with explicit limits. |
| [H10, SetUserVar, 2021-04-25](https://github.com/wezterm/wezterm/commit/87677a73bf8d1828b00e1e9c021dab712075f971) | State explicitly that the initial feature does not cross the mux protocol. | Local-first delivery has precedent; its limitations must remain visible. |
| [H11, font slant compatibility, 2022-03-12](https://github.com/wezterm/wezterm/commit/84d4187c9af93be5b7b41cd1cfcf96d316ee9c74) | Keep accepting boolean italic settings after introducing a slant enum. | Consider existing callers when improving a public representation. |
| [H12, RIS reset repair, 2026-08-12](https://github.com/wezterm/wezterm/commit/fe3006aefcdc4c22924e7bce966b2c430dade4f1) | Fix recovery after an unclean exit and clear other state omitted from full reset. | Initialization and lifecycle recovery each need explicit behavior. |

Do not turn these into a personality claim such as "Wez always prefers X." They show decisions made under particular constraints.

## Apply the guide to our OSC 7501 project

The authority order is the user's accepted decisions and the protocol contract, followed by current repository constraints and evidence-based recommendations. Historical precedent informs our choices; it does not override requirements.

| Question | Current position | Next implementation consequence |
| --- | --- | --- |
| What is the presentation scope? | D001 accepted: local native support and Lua indicators; remote mux follows. | Deliver a real local path and explicitly identify unsupported remote synchronization. |
| Who owns records? | D002 accepted: `TerminalState`. | Keep the store GUI-independent; wire prompt, reset and process-exit lifecycle events to it. |
| What does Lua return? | D003 accepted October 27: sorted list with an ID on each record. | Keep live pane and title information representations consistent; return owned copies. |
| How is a state change delivered? | D004: separate local callback and mux invalidation marker. | The serializable Alert enum and codec remain unchanged. Remote clients do not receive records. |
| How does title formatting see records? | Both `pane:get_program_status()` and `PaneInformation.program_status`. | The alpha formatter can summarize every split in a tab. |
| What happens on resets and process events? | Required integration, not completed by a store helper. | Specify each event separately and test at its actual entry point. Do not infer task lifetime from screen clearing or switching. |
| What gets shown or acknowledged? | Presentation and acknowledgement policy still needs a concrete design. | Keep the full records available, then explain which states the example highlights and how it satisfies the selected lifetime contract. |
| How will remote support work later? | Deferred and incomplete. | Design attach/reconnect snapshots, updates and codec compatibility when that milestone starts. |

The local alpha in `.research/wezterm` now connects typed OSC dispatch, a terminal-owned store, lifecycle behavior, pane/Lua access and example tab presentation. See [alpha/README.md](alpha/README.md) for running it and [alpha/TEST_RESULTS.md](alpha/TEST_RESULTS.md) for verification. Remote mux support remains deferred; this is not an upstream release.

The clean `wezterm/` clone is the research reference; `.research/wezterm` holds the alpha implementation. Preserve that distinction when continuing work. The architecture evidence above remains pinned to the original clean revision, excluding our feature code.

## A repeatable design and review procedure

Use this procedure for each substantial change. The user reviews architecture choices; the implementation handles routine details within the accepted design.

1. Write a concrete example: input, current behavior, intended behavior and affected pane kinds.
2. Read `DECISIONS.md` and the applicable protocol requirements. Separate fixed rules from choices.
3. Find the nearest existing feature. Trace it through parser, model, pane, notifications, Lua and display. Check history when the reason is unclear.
4. Name the authoritative owner and all copies. State when each copy becomes stale and how it refreshes.
5. List lifecycle events and external boundaries: reset, prompt, exit, screen switch, main-thread scheduling, serialization and reconnect where relevant.
6. For each real design choice, give viable options, source evidence, a recommendation and its cost. Mark conclusions that are our inference.
7. Implement one useful path with tests at the boundaries it crosses. Keep speculative frameworks and unrelated cleanup out of the patch.
8. Verify feature configurations and documentation. Record completed checks separately from checks not yet run.
9. Write a reviewer explanation of the final behavior, the reason for the design and remaining limitations. Add the accepted decision to `DECISIONS.md`.

A useful question for you as design reviewer is: **"Which component owns this fact, and what happens if the consumer was absent when it changed?"** That question catches both state-ownership mistakes and reconnect bugs without requiring advanced Rust syntax.

## Source index

All current-source links below are pinned to `e2a98e1b8af156988ad0ac778658ac27b60a55fb`. Local paths are relative to this document. The cited line is a reading entry point, not necessarily the whole supporting implementation.

| ID | Local source | Pinned reading entry |
| --- | --- | --- |
| E01 | [Parser crate contract, action batching and size test](wezterm/wezterm-escape-parser/src/lib.rs) | [lib.rs, line 1](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-escape-parser/src/lib.rs#L1), [batching, line 70](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-escape-parser/src/lib.rs#L70) |
| E02 | [Terminal crate contract](wezterm/term/src/lib.rs) | [lib.rs, line 1](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/term/src/lib.rs#L1) |
| E03 | [OSC parser and progress command enum](wezterm/wezterm-escape-parser/src/osc.rs) | [Parsing, line 320](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-escape-parser/src/osc.rs#L320), [enum, line 884](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-escape-parser/src/osc.rs#L884) |
| E04 | [State getters and title design rationale](wezterm/term/src/terminalstate/mod.rs) | [mod.rs, line 638](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/term/src/terminalstate/mod.rs#L638) |
| E05 | [Stored progress type](wezterm/term/src/terminal.rs), [performer](wezterm/term/src/terminalstate/performer.rs) | [Enum, line 37](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/term/src/terminal.rs#L37), [progress, line 1082](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/term/src/terminalstate/performer.rs#L1082), [unknown commands, line 776](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/term/src/terminalstate/performer.rs#L776) |
| E06 | [Shared Pane interface](wezterm/mux/src/pane.rs), [local pane](wezterm/mux/src/localpane.rs) | [Interface, line 240](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/mux/src/pane.rs#L240), [local getter, line 465](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/mux/src/localpane.rs#L465), [alert scheduling, line 943](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/mux/src/localpane.rs#L943) |
| E07 | [Lua pane API](wezterm/lua-api-crates/mux/src/pane.rs) | [Getter, line 149](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/lua-api-crates/mux/src/pane.rs#L149), [user vars, line 195](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/lua-api-crates/mux/src/pane.rs#L195) |
| E08 | [GUI pane information](wezterm/wezterm-gui/src/termwindow/mod.rs) | [Snapshot, line 259](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-gui/src/termwindow/mod.rs#L259) |
| E09 | [Title callback and default renderer](wezterm/wezterm-gui/src/tabbar.rs) | [Callback, line 54](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-gui/src/tabbar.rs#L54), [default, line 206](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-gui/src/tabbar.rs#L206) |
| E10 | [SetUserVar state update](wezterm/term/src/terminalstate/performer.rs) | [performer.rs, line 833](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/term/src/terminalstate/performer.rs#L833) |
| E11 | [Title callback contract](wezterm/docs/config/lua/window-events/format-tab-title.md) | [Synchronous constraint, line 8](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/docs/config/lua/window-events/format-tab-title.md#L8) |
| E12 | [Remote client pane](wezterm/wezterm-client/src/pane/clientpane.rs), [codec](wezterm/codec/src/lib.rs), [version check](wezterm/wezterm-client/src/client.rs) | [Progress cache, line 198](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-client/src/pane/clientpane.rs#L198), [codec version, line 445](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/codec/src/lib.rs#L445), [version check, line 1160](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-client/src/client.rs#L1160) |
| E13 | [PTY reader and mux scheduling](wezterm/mux/src/lib.rs) | [Reader, line 279](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/mux/src/lib.rs#L279), [notification scheduling, line 715](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/mux/src/lib.rs#L715) |
| E14 | [Lua crate split explanation](wezterm/lua-api-crates/README.md) | [README, line 1](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/lua-api-crates/README.md#L1) |
| E15 | [Scheduler rationale](wezterm/promise/src/spawn.rs) | [spawn.rs, line 39](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/promise/src/spawn.rs#L39) |
| E16 | [Contribution guidance](wezterm/CONTRIBUTING.md) | [Tests, line 98](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/CONTRIBUTING.md#L98), [documentation, line 157](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/CONTRIBUTING.md#L157) |
| E17 | [Parser manifest](wezterm/wezterm-escape-parser/Cargo.toml), [term manifest](wezterm/term/Cargo.toml) | [Parser features, line 39](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/wezterm-escape-parser/Cargo.toml#L39), [term edition, line 5](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/term/Cargo.toml#L5) |
| E18 | [Progress documentation](wezterm/docs/config/lua/pane/get_progress.md) | [Lua-only claim, line 30](https://github.com/wezterm/wezterm/blob/e2a98e1b8af156988ad0ac778658ac27b60a55fb/docs/config/lua/pane/get_progress.md#L30) |

## Reading route for the student reviewer

Read the first comments in E01 and E02. Then read E04's title explanation and the progress case in E05. Follow E06 into E07 and E08, and finally E09. Read H01 after tracing the current code. You will then recognize which parts were intentional reuse and which parts were acknowledged omissions.

You do not need to understand every Rust expression on this first pass. Be able to identify an input, an owner, a mutation, a notification and a consumer. Those are the pieces you will review when we add OSC 7501.
