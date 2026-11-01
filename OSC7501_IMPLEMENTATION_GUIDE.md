# How our OSC 7501 implementation works

Prepared for the November 2, 2026 presentation. This guide explains our implementation and its tradeoffs. It does not claim that Wez Furlong chose or endorsed our additions.

An agent knows that it is waiting for permission. The terminal sees only bytes unless the agent reports that fact. OSC 7501 gives those bytes a structured meaning. Our feature remembers the report and makes it available to Lua, which decides what the tab displays.

## Follow one task through the system

Suppose a build's tests need permission. It sends these reports. `ESC` and `ST` name control bytes:

```text
ESC ] 7501 ; state=working:app=cargo:progress=25 ST
ESC ] 7501 ; state=blocked:id=build/tests:kind=permission ST
```

The first report addresses the root because it omits `id`. The second addresses a child. Both records exist simultaneously.

| Layer | What it does with this example |
| --- | --- |
| `vtparse` | Recognizes a complete OSC sequence and retains its body. |
| Escape parser | Validates the body and produces a typed `Command::Report`. |
| `TerminalState` | Applies the report to its record store. |
| `LocalPane` and mux | Expose the records and schedule a change notification. |
| Lua API and GUI snapshot | Provide owned record tables to consumers. |
| Example tab formatter | Chooses blocked over working and displays the source pane. |

The resulting Lua data looks like this, with formatting simplified:

```lua
{
  { id = '', state = 'working', progress = 25,
    app = 'cargo', effective_app = 'cargo' },
  { id = 'build/tests', state = 'blocked',
    kind = 'permission', effective_app = 'cargo' },
}
```

The child inherits `effective_app` from the root despite the missing intermediate `build` record. The tab summarizes the records.

## Which decisions were ours?

Some behavior comes from the protocol and must be implemented correctly. Other behavior is a design choice we can evaluate.

| Decision | Authority and consequence |
| --- | --- |
| D001: local native support first | You accepted this scope. Remote mux synchronization follows later. |
| D002: `TerminalState` owns records | You accepted this owner. State remains testable without a GUI. |
| D003: Lua receives a list sorted by task ID | You accepted this API. Iteration is deterministic; finding one ID requires a search. |
| D004: separate local change notification | Our implementation choice. It avoids changing the serialized remote alert format for a local milestone. |
| D005: protocol-specific framing safeguards | Our implementation choice. It bounds incomplete OSC 7501 input while preserving existing handling of unrelated OSCs. |
| D006: late replacement after process exit | A review correction. Later transient reports supersede old results, then exit cleanup leaves that exact ID absent. |
| Whole-record replacement, subtree clear, lifetime rules | Protocol requirements, not optional preferences. |
| Blocked outranks error in a tab | Example Lua policy. A different formatter can choose differently. |

See [DECISIONS.md](DECISIONS.md) for the alternatives and [the WezTerm design guide](WEZTERM_DESIGN_GUIDE.md) for upstream evidence, with inference labeled separately from stated author rationale.

## Parsing makes a safe instruction

There are two parsing jobs. [vtparse](vtparse/src/lib.rs) recognizes framing across arbitrary input chunks. A read may end halfway through a sequence, including between the two bytes of `ESC \`. The [report parser](wezterm-escape-parser/src/program_status.rs) interprets the complete body.

Its `Command` enum has two alternatives: `Query` and `Report`. The report uses a `State` enum and an optional `Kind` enum. Rust must handle named cases instead of repeatedly comparing strings across the program.

Optional fields use `Option<T>`. For example, `Option<u8>` distinguishes a known percentage from no percentage. Zero means 0%; absence means indeterminate. `kind` applies only to blocked records, and `progress` applies only to working or blocked records.

The parser validates before returning a report. An invalid explicit ID rejects the report instead of accidentally targeting the root. Invalid base64 or decoded control characters also reject the whole report. Unknown keys and many malformed pairs are ignored according to the protocol. Ignoring a pair and rejecting a report are different operations.

Text is decoded into owned Rust strings. Those strings remain valid after the input buffer is reused. The parser crate retains its existing `no_std` configuration with allocation, so it does not require the full standard library merely to understand this protocol.

The sequence limit is 4096 bytes. The alpha conservatively reserves two terminator bytes even when BEL is used. We also bound input while it is still arriving. Waiting until the report is complete would allow an unterminated sequence to accumulate memory first.

The review found another boundary problem: low-level parsing could silently drop certain malformed bytes inside an ID. For example, removing a bad byte from `bad<byte>id` could manufacture the legitimate ID `badid`. The fix preserves malformed input for validation instead of repairing its meaning. This matters especially for a clear report, which could otherwise remove a real task. D005's exception remains a tradeoff that deserves upstream review.

Incomplete UTF-8 could also consume a following colon, hiding an ID field. Recognized OSC 7501 bodies now retain invalid high bytes directly, keeping separators independent. The wire body is ASCII; human text uses base64. Supported terminators are BEL and ESC-backslash. A raw C1 terminator cancels the report.

## Storage and the public API need different shapes

The [record store](term/src/program_status.rs) contains a `HashMap<String, Report>` and a `VecDeque<String>`.

The map finds a record by its ID. The queue records update order, with the oldest update first. We chose 256 records, the protocol's stated cap; implementations must support at least 64. Creating a record past the cap evicts the least recently updated record.

Updating an identical report still refreshes its recency. That does not mean the visible data changed. The store returns a boolean describing the content change, allowing the caller to avoid an unnecessary notification.

A queue update scans at most 256 IDs. A more elaborate linked structure could avoid that scan, but would add bookkeeping and more ways to break the map/queue relationship. The small bound makes the simple implementation reasonable. This is an engineering tradeoff, not a claim that linear scans are always best.

For Lua, we copy records into a vector and sort by ID. This gives stable ordering without requiring the internal map to preserve order. A keyed Lua table would make direct lookup easier; the accepted list is convenient for drawing tabs. Its numeric positions are not identities. Always identify a task by `record.id`.

The snapshot owns its data. Changing a returned Lua table does not modify Rust's records. The terminal lock is released before Lua conversion or rendering. This avoids keeping the model locked while consumer code runs.

## Replacement, clearing and inheritance

These three operations have different meanings:

```text
state=done:id=build/tests
```

This replaces only `build/tests`. Its previous `kind`, title and message disappear if omitted. It does not alter children. Retaining omitted fields would leave stale information, such as a permission reason on a completed task.

```text
state=clear:id=build
```

This removes `build` and descendants such as `build/tests`. It leaves `builder` alone because the slash is a path boundary. The parent need not exist. Clearing an absent subtree succeeds as a no-op. Omitting `id` clears every record.

App inheritance is computed when building a snapshot. A child first uses its own explicit app; otherwise we walk toward the root to find the nearest ancestor with one. We retain both `app` and `effective_app` so consumers can distinguish explicit input from derived data.

Copying the parent's app permanently into each child would become stale when that parent changes or is evicted. Computing it dynamically avoids rewriting all descendants. The short bounded IDs keep ancestor traversal small.

## State changes and notifications are separate

[TerminalState](term/src/terminalstate/mod.rs) owns the store. Its [performer](term/src/terminalstate/performer.rs) handles the typed command. After a visible mutation, it invokes `program_status_changed` on the notification handler.

[LocalPane](mux/src/localpane.rs) schedules delivery on the main thread. This is necessary because terminal actions can execute while the terminal mutex is held. Calling a consumer immediately could make it request a snapshot and try to acquire the same lock again.

The local `MuxNotification::ProgramStatusChanged` says that a pane's data changed. Consumers then read a fresh snapshot. The notification is not the only copy of the data, so a consumer does not need every past event to understand the present.

We deliberately did not add a variant to the serialized `Alert` enum. That type also participates in remote transport. A default empty pane getter and an ignored local notification do not implement remote synchronization. Remote support needs an initial snapshot, subsequent updates and reconnection behavior.

The capability query receives a fixed reply through the terminal's writer. That reply describes support; it contains no task IDs, messages or stored records. Reading snapshots through local Lua is separate from replying to programs over the terminal stream.

## Lifetimes require events from outside the store

Records belong to a terminal, not to its current screen. Entering a full-screen application or returning from it preserves them. A soft reset also preserves them. A full RIS reset clears them.

OSC 133 A announces a new shell prompt. The terminal then removes working and blocked records. Done and error survive. This implementation also retains idle records, which the protocol permits. Printing an ordinary prompt without that marker cannot trigger this cleanup.

The pane wrapper supplies attached-process exit because it owns the process. This means the process attached to the pane, often the shell. Returning from OpenCode to that still-running shell is not the shell's exit.

The review fixed a subtle ordering bug. Suppose the store remembers `done` for `build`, but buffered output says that `build` has started working again. The process-exit notification can arrive before those bytes are applied. Previously the sticky exit guard ignored the late working report, leaving the obsolete done record behind.

The corrected rule combines replacement with cleanup: a late working or blocked report removes any existing record with that exact ID. It never revives active work after exit, and it preserves finished descendants. Repeating the late report after removal produces no additional visible change. Late done or error reports remain useful and can still be stored.

## Lua chooses what deserves space in a tab

The [Lua pane method](lua-api-crates/mux/src/pane.rs) exposes `pane:get_program_status()`. The GUI separately adds the same record shape to pane information used during title formatting. Both connections are necessary.

Our [example formatter](alpha/alpha.lua) examines every split in a tab. It selects blocked, then error, working, done and idle. The first encountered record breaks a tie. It shows the source pane and record count, filters selected invisible formatting characters, and shortens the displayed label.

That priority is not a protocol rule. Nor should WezTerm invent done when an application sends idle. An idle report means the app is waiting for another instruction; done means a completed result is ready to inspect. Your OpenCode observation should be checked against a fresh snapshot before attributing it to the formatter.

The formatter must stay fast. It reads ready data rather than launching processes or making network requests. There is currently no dismissal interface, desktop notification policy or OS taskbar integration.

## How we test the boundaries

Parser tests check complete and fragmented sequences, invalid input, limits and recovery after rejection. Model tests check replacement, clearing, inheritance, recency and independent snapshots. Terminal integration tests send real bytes through the terminal and exercise lifecycle hooks. Lua tests check the public table shape.

Native smoke tests run the actual alpha and compare the live getter with data observed by the tab formatter. Manual tests cover layout, inactive tabs, splits, overlays and real applications. Passing data assertions cannot prove that text is readable or correctly positioned.

One new smoke case arms a check before sending status-only output. During the observation period, it reads only cached formatter data. Earlier checks sent a user-variable event after each report, which could itself refresh the title. Removing that deliberate refresh makes the new check more useful, though unrelated window events can still refresh the display.

The review added regression coverage for malformed-byte handling and late reports after exit. Consult [test results](alpha/TEST_RESULTS.md) for actual execution results. Writing a test does not establish that it passes. Windows local support is the target; remote mux and other platforms require additional work.

This branch contains the implementation directly at its root. Follow [README.md](README.md) to clone and build it; no patch application is needed. Source links in this guide point to the files in this branch.

## A three-minute presentation route

1. **First minute:** show a working task and a child awaiting permission. Explain that the program reports facts, TerminalState remembers them, and Lua chooses the summary.
2. **Second minute:** change the child to done, then clear its parent path. Explain whole-record replacement, simultaneous tasks and slash-boundary clearing.
3. **Third minute:** show the sorted Lua snapshot and describe one review fix. Explain why testing the parser alone would miss process-exit ordering. State that remote synchronization is deferred.

## Small glossary

- **Report:** one incoming instruction from a program.
- **Record:** the current stored status for one ID.
- **Enum:** a Rust type with named alternatives, such as working or blocked.
- **Snapshot:** an owned copy of the current records for a consumer.
- **Mutex:** a lock protecting shared state while a thread accesses it.
- **Notification:** a signal to reread current state after it changes.

## Check your understanding

1. Why does the parser not clear `build/tests` itself?
   **Answer:** it knows the instruction's syntax, but the terminal store owns the records and their relationships.
2. A child changes from blocked to done and omits kind. Should permission remain?
   **Answer:** no. Each report completely replaces that ID's record.
3. Why refresh recency without always redrawing?
   **Answer:** update order affects later eviction, while an identical record contains no new visible data.
4. Why can a done record disappear after a late working report following exit?
   **Answer:** the later report supersedes that old result. Exit cleanup then removes transient state, while finished descendants remain independent.
