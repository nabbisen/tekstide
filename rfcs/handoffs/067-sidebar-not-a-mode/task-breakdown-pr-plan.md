# RFC-067 task breakdown and PR plan

## PR-067-A — the sidebar persists

- `sidebar_view` stops matching on `ProjectMode`; the explorer renders in both (D1).
- `sidebar-placeholder-title` is **deleted** from `en.ftl`, with its now-dead composition in
  `sidebar_label` (D1, §row 3).
- Activating a file in terminal mode switches to Content mode and shows it (D7).
- **Only that activation switches** (D8, §row 1): not a recovery restore, not a background refresh,
  not any open performed on the user's behalf. Prove it with a test that opens a document by the
  non-user path while in terminal mode and asserts the mode did not change.
- `FocusZone::Sidebar` keeps its place in the cycle; no zone added (D3, §row 2).
- Live capture: terminal mode with the file tree beside it, against the same throwaway fixture the
  placeholder appears in today (D2).

### `0.33.0` planning (2026-10-09): the three call sites D8 governs, named

Checked after `0.31.0` and `0.32.0` shipped, because this RFC was written before either. **Its
premises still hold**: `sidebar_view` still matches on `ProjectMode`, and
`sidebar-placeholder-title` still reads *"Files are listed here in Content mode."*

**D8 named three things that must not switch the mode — "an open performed on the user's behalf, a
refresh, a recovery restore" — and the code has exactly those three, no more:**

| Path | Site | Under D7/D8 |
| --- | --- | --- |
| The explorer's own activation | `shell.rs:4667`, `Action::Open(path)` | **This is the one that switches** (D7). It is also the call site whose behaviour this RFC changes, since the tree becomes reachable in terminal mode for the first time. |
| The recovery offer accepting a record | `shell.rs:11700` / `:11714` (RFC-027) | **Must not switch.** Shipped in `0.31.0`, after this RFC was written. |
| The watch notice / background refresh | `record_project_watch_notice`, `shell.rs:7606`+ | **Must not switch.** Opens nothing new; touches documents already in the set. |

So D8's list is complete and implementable as written — **drive the test through the recovery offer
specifically**, since it is the only one of the three that both opens a document and is reachable
while a terminal is on screen.

## PR-067-B — measure the switch

- Render cost of a mode switch, paired, control inside the same run, spread published beside the
  median (D4).
- Report per switch. Do not derive a rate by dividing one condition's figure by another's count —
  the correction RFC-027 needed four reviews to arrive at.

## PR-067-C — the decision

- D5. Read the number against the owner's own framing: *"at a time or a near real-time."*
- **If the switch is below perception, build nothing and record the number that made it
  unnecessary.** That closes the RFC.
- If it is not, this slice does **not** design the alternative: it records what the measurement
  showed and hands the question to a new RFC.

## Not in scope

Splits or panes; a third mode; changes to the session bar, the six-terminal bound or the two visible
slots; a new `REQ-`.
