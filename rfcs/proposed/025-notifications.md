# RFC-025: Notifications

Status: **Proposed 2026-09-17.** Reserved for M12 since the roadmap was written; scoped at the owner's
word as the headline of `0.21.0`, with RFC-030 scoped alongside and shipping separately.
Target milestone: **M12**
Date: 2026-09-17

Related RFCs:

- [RFC-023](../done/023-configuration-system.md) — the dependency the reservation named; shipped.
- [RFC-047](../done/047-audit-store-corruption-recovery.md) — D3's rule that a surface shows a state
  **only when degraded**, which every notice here inherits.
- [RFC-049](../done/049-transcript-retention-enforcement.md), [RFC-050](../done/050-transcripts-from-earlier-runs.md),
  [RFC-051](../done/051-recovering-the-recent-project-list.md) — each added a board notice of its own.
- RFC-030 (proposed alongside) — REQ-NOTIFY-002 names Git state, which only RFC-030 can produce.

## Summary

The requirements define a `Notification` — *"a project-scoped or global event requiring awareness or
action"*, visible from the Project Board and the active project's status bar. **No such type exists.**
What exists instead is four separate notice mechanisms on the board, each written by a different RFC
with its own idea of how long a notice lives. This RFC gives them one model before a fifth arrives.

## What is true today, measured

- **The requirements** (`tekstide-requirements-v0.md` §4.7, §6.11): REQ-NOTIFY-001 the board shows
  project task state and pending actions; **002** status bars show trust, Git state, running and
  failed sessions, and pending approvals; **003** background jobs get *actionable* labels — `2
  running`, `1 awaiting approval` — not bare counts; **004** notifications are keyboard-accessible;
  **005** no status relies on colour alone. REQ-PROJ-006 puts notifications in per-project session
  state.
- **There is no notification domain type.** The only type named for notifying is the terminal
  reader's `WakeNotifier`, which is unrelated.
- **Four notice producers feed the project board**, concatenated in one place: audit-store health
  (RFC-047), configuration diagnostics (RFC-045), the recent-project list's reset or recovery
  (RFC-050/051), and retention cleanup (RFC-049).
- **Each has its own lifetime, decided locally**: audit health shows *while degraded*; the reset and
  recovery lines show *on the start it happened*; the retention line shows *until the next cleanup*.
  All four are **absent when their fact is false**, which is the one rule they already share.

## Decisions required

**D1 — One `Notification` model, project-scoped or global.** Recommended: a notification carries its
scope, its kind, the text it renders, and **an explicit lifetime** from a closed set — *while the
condition holds*, *for the start it happened*, or *until acknowledged*. **A notice may not invent a
fourth.** The four existing lifetimes map onto the first two exactly.

**D2 — The four existing notices migrate into it, in this RFC.** Recommended, and the reason the RFC
exists now: a model beside four parallel mechanisms is a fifth mechanism. Each migration keeps its
text and its absent-when-false test unchanged, so the move is provable by the tests that already hold
the behaviour.

**D3 — Actionable labels (REQ-NOTIFY-003).** Recommended: the board and status bar render counts as
states — `2 running`, `1 awaiting approval`, `1 failed` — and never a bare number. The counts already
exist in `ProjectRuntimeSummary`; this is rendering, not measurement.

**D4 — Keyboard access and not colour alone (004, 005).** Recommended: notifications are reachable
through the existing keybinding registry and read as text; any colour is additional to a word. **Open:**
whether acknowledging a notification is a keyboard action in this RFC, or whether *until acknowledged*
waits for a notice that needs it.

**D5 — The status bar shows REQ-NOTIFY-002's fields, and Git state says "not available" until RFC-030
produces it.** Recommended: this RFC does not wait on Git, and does not fake it.

**D6 — In-app only.** Recommended: **no desktop or operating-system notifications, and no sound.** A
notification that leaves the window is a different privacy and consent question.

## Non-goals

- OS-level notifications, sound, or notification history beyond what D1's lifetimes express.
- Producing Git state (RFC-030).
- New notices for their own sake: this RFC moves what exists and adds the labels the requirements ask
  for.

## Risks

- **A migration that silently changes a lifetime** — a notice that used to disappear stays up, or the
  reverse. The existing absent-when-false tests are the guard, and they must pass unmodified.
- **Density.** RFC-034 §4's problem: a surface that always says something stops being read. D1's
  closed lifetimes are the mitigation.

## Acceptance criteria

- One `Notification` type; the four existing notices produced through it, **with their existing tests
  passing unchanged**.
- A new notice cannot be given a lifetime outside D1's set — held by the type.
- Status bar and board render actionable labels; no bare counts.
- Every notification is reachable by keyboard and readable without colour.
- Git state reads "not available", truthfully, until RFC-030 lands.
