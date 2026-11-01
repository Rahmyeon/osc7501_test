# `pane:get_program_status()`

Local OSC 7501 alpha API. This is not an upstream release guarantee.

Returns an owned list of the pane's current Program Status Protocol records,
sorted by task ID. The root ID is the empty string. An empty list means no
records are currently stored, or this pane implementation does not expose them.

Each record has `id` and `state`. State is one of `idle`, `working`, `done`,
`blocked` or `error`. `clear` is a command and is never stored.

Optional fields are omitted when absent:

| Field | Meaning |
| --- | --- |
| `kind` | `permission`, `question` or `auth`, only with `blocked`. |
| `progress` | Integer from 0 through 100 with `working` or `blocked`. Absence means no percentage was supplied. |
| `app` | The report's explicit application identifier. |
| `effective_app` | The explicit application identifier or its nearest-ancestor inherited value. |
| `title` | Decoded UTF-8 label. |
| `msg` | Decoded UTF-8 single-line message. |

Every report replaces its entire record. Inherited application values are
computed when the snapshot is read, so ancestor replacement and eviction take
effect without rewriting children. Returned tables are copies; mutating them
does not change the terminal's records. Numeric positions are not task IDs.

```lua
for _, record in ipairs(pane:get_program_status()) do
  if record.state == 'blocked' then
    wezterm.log_info(record.id, record.kind, record.msg)
  end
end
```

For synchronous title callbacks, use the same-shaped `program_status` field on
[PaneInformation](../PaneInformation.md). The terminal notifies local GUI
consumers when record content changes; an identical report still refreshes
eviction recency without a separate status-change notification.

Records survive alternate-screen switches and soft reset. Full reset removes
all records. OSC 133 A and attached-process exit remove working and blocked
records, retaining idle, done and error. This alpha retains finished records
until replacement or clearing. It has no dismissal API.

The current implementation exposes local panes. Remote WezTerm mux clients
do not replicate these records. The headless terminal model can still parse
reports and answer feature detection, which does not imply remote indicators.

Program-supplied text is plain text. Do not interpret it as markup or infer
state from message wording. A formatter should disarm invisible direction
formatting before displaying text outside the terminal grid.

Protocol reference: [Program Status Protocol](https://www.superlogical.com/rex/docs/build/program-status).
