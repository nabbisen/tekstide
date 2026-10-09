---
title: "RFC-058 handoff: a project held by one process"
status: "**Accepted 2026-10-10**, D1–D7 as written plus D8–D10. `0.34.0`, M13. Three slices; A is reproduce-only and gates the other two."
rfc_file: "../../accepted/058-a-project-held-by-one-process.md"
target_milestone: "M13"
created: "2026-10-10"
---

# RFC-058 handoff

## Read first

1. The RFC, especially *Decided on acceptance* — **D8–D10 replaced the design**, and D9 rules out
   the thing most people would reach for.
2. [`what-the-lock-must-not-do.md`](./what-the-lock-must-not-do.md) — seven rows; two of them are
   about not making the editor unusable, which is the likelier failure here than the data loss.
3. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).
4. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md).

## The one-paragraph version

`REQ-PROJ-009` is a **must**, and nothing satisfies it for file-based project state. Two Tekstides
opening the same root get the **same project id** — reused from `recent-projects.json`, which is
what lets recovery survive a restart — so they compute the **same recovery-record paths** and write
the same files. Last tick wins, silently. A second instance should not open a project another
instance holds; it should send the user to the holder.

## Start by reproducing it

**This has not been watched happen.** The chain was read in the code during `0.34.0` planning:
`open_project` reuses the id by canonical root (`app.rs`), `records_dir(state_root, project_id)`
and `record_file_name(relative_path)` carry nothing about the process (`recovery/record.rs`).
PR-058-A's whole job is to make it happen with two real processes. **If it does not reproduce, that
finding outranks this RFC** — say so instead of building a lock for a hazard that is not there.

## The two decisions people get wrong

**D5 and D8 are not in tension, and the difference is the whole design.**

| Situation | What happens |
| --- | --- |
| The mechanism **works** and says another live instance holds this project | **Do not open it.** Name the holder, ask its window for attention (D8). |
| The mechanism **cannot decide** — a stale lock from a dead process, unreadable, unwritable, ambiguous | **Open the project anyway, and do not claim protection** (D5). |

A text editor that will not open a project because of a lock file it cannot reason about has chosen
the wrong failure. Refusing is only correct when we *know* someone else has it.

**D9: the window cannot be raised, and nothing may say it was.** `iced 0.14` exposes both
`window::gain_focus` and `window::request_user_attention`, but `gain_focus` reaches winit's
`focus_window`, documented **"Wayland: Unsupported"** — a Wayland client cannot raise itself. Only
the attention request works here, and the compositor decides whether it shows. `0.23.0` was *What
The Window Says Is True*; wording that implies a switch happened would be that defect returning.

## Where the pieces are

| What | Where |
| --- | --- |
| Project id reused by canonical root | `core/src/app.rs`, `open_project` → `recent_project_id_by_canonical_root` |
| The record paths two instances collide on | `core/src/recovery/record.rs`, `records_dir` + `record_file_name` |
| The state root both resolve | `core/src/project/recent/store.rs`, `AppStatePathProvider` |
| Prior art: a lock outliving its holder | `transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped` (fork-duplicated descriptor kept an `flock` alive past the drop) |
| Prior art: a per-pid marker, and its disclosed hole | RFC-027 D12, `core/src/recovery/instance.rs` |
| Prior art: a local `AF_UNIX` channel, and its path-length limit | the approval adapter channel; `SocketPathTooLong` in `test-process-leak.md` |

## What already satisfies the requirement, and must be left alone

**The audit store.** SQLite's own file locking with a `busy_timeout` (`journal_mode = DELETE`)
serialises writers across processes — *"or equivalent conflict-prevention mechanism"*, already met.
**Confirm it; do not build a second mechanism beside it.** A requirement satisfied twice in two ways
is a requirement nobody can reason about.
