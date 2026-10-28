# Design decisions

## D001: Presentation scope

Date: October 15, 2026.

Status: accepted by the user.

Decision: native local OSC 7501 support with Lua tab indicators for the October 29 presentation. Remote mux support follows.

Alternatives considered:

- Native local support with Lua tab indicators, followed by remote mux support.
- Native local and remote mux support in the same presentation milestone.
- A visual prototype first, with protocol completion afterward.

Reason: the selected option gives the presentation a real terminal feature while keeping remote wire changes and reconnect behavior out of its critical path.

Consequence: the presentation must clearly identify remote mux synchronization as deferred. Protocol behavior for supported local panes still needs tests and validation.

## D002: Ownership of task records

Date opened: October 15, 2026.

Status: accepted by the user.

Option A, recommended: TerminalState owns records, following WezTerm progress state.

Option B: LocalPane owns records and consumes callbacks from the terminal, following Ghostty's embedding model.

Decision: TerminalState owns the records.

Reason: follow WezTerm's existing progress architecture and keep the record rules in a GUI-independent component that can be tested directly.

Consequence: the parser produces owned reports, the terminal model stores them, and the pane and GUI layers consume model state. The attached-process exit signal still comes from the pane wrapper.

## D003: Shape of the Lua snapshot

Date opened: October 15, 2026.

Status: accepted by the user on October 27, 2026, after the design exercises.

Decision: a list of record tables sorted by task ID. Each record carries its ID. This shape is shared by the pane getter and title-formatting information.

Option A, recommended: a list of record tables sorted by task ID. Each entry includes its id. This makes iteration and deterministic display straightforward. Finding one named task requires a search or a helper, and numeric positions must not be treated as permanent identities.

Option B: a table keyed by task ID. This makes lookup by id and comparison between snapshots convenient. Callers need to sort keys for deterministic display. The root is addressed with the empty-string key, and ids containing slashes use bracket indexing.

This choice changes the Lua representation, not ownership of the Rust record store or the on-wire protocol. Both options expose the full record set and can represent a missing root.

Worked examples: osc7501-decisions.html#lua-shape.

## D004: Alpha change delivery

Date: October 27, 2026.

Status: implementation choice within D001's local scope.

Decision: use a dedicated default callback on AlertHandler and a local MuxNotification::ProgramStatusChanged marker. Leave the serializable Alert enum and mux codec unchanged. Pane consumers read a fresh snapshot after notification.

Reason: a local refresh should not force an incomplete remote wire contract. Existing alert delivery establishes the scheduling pattern; the separate marker avoids changing shared serialized variants. GUI front-end, term-window subscriber filters, overlays and mux server dispatch are audited explicitly.

Consequence: remote mux clients do not receive OSC 7501 records. The headless terminal model can still parse reports and answer capability queries; that does not imply remote GUI indicators. Finished records remain until replacement or clearing in this alpha. No new acknowledgement command is introduced.

## D005: Bound OSC 7501 during framing

Date: October 27, 2026.

Status: implementation choice under the user's authorization to proceed within this directory.

Decision: recognize OSC 7501 framing in vtparse to cap buffered input before a terminator arrives, retain its colon-separated body as one parameter, wait for a complete ST, and discard canceled/oversized reports. Other OSCs keep their existing handling.

Reason: checking the report body after parsing does not bound an unterminated input buffer. Generic semicolon parameter truncation and ignored controls could otherwise change a report before validation. Applying a global OSC limit would risk unrelated existing protocols.

Consequence: this alpha adds a focused protocol-aware exception to the low-level parser. It needs explicit review if proposed upstream. It does not claim a new bound for every OSC sequence. Explicit invalid IDs are rejected before generic malformed-pair skipping, so they cannot silently address the root record. Reports only affect records after complete framing and validation.
