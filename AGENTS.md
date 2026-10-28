# OSC 7501 project guidance

On October 27, 2026 the user authorized autonomous work within this directory: "yolo just be reasonable." Proceed with reasonable implementation and testing choices, preserve existing work, and document material behavior/API decisions. Ask only for genuinely missing requirements or actions outside the authorized scope. Existing accepted decisions still apply.

Before designing or implementing this feature, read:

- `DECISIONS.md` for the user's accepted choices and pending decisions.
- `IMPLEMENTATION_PLAN.md` for scope, working arrangement and actual completion status.
- `WEZTERM_DESIGN_GUIDE.md` for source-backed architecture principles and their limits.

The user reviews significant architecture choices. Present viable options, reasons and consequences; record accepted choices. Follow already accepted decisions without asking again. Protocol requirements are requirements, not optional design choices.

The source reference clone is `wezterm/`, at revision e2a98e1b8af156988ad0ac778658ac27b60a55fb when the guide was written. The unfinished implementation currently lives in `.research/wezterm` on branch `osc7501-local-status`. Inspect both checkouts before choosing where to resume; preserve the existing changes. Do not treat our implementation as evidence of upstream conventions.

Distinguish stated author rationale, observed repository behavior and our inference. Historical precedent does not override the protocol or the user's accepted choices. D003 was accepted on October 27, 2026: expose a list of records sorted by task ID.

Keep parsing, terminal semantics, pane access and presentation in their established layers. Audit serialization and all alert consumers before describing an alert change as local-only. Verify complete integration before calling the feature usable.
