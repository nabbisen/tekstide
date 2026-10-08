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
