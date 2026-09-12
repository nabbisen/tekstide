---
title: "RFC-050 — task breakdown and PR plan"
rfc: "RFC-050"
rfc_file: "../../accepted/050-transcripts-from-earlier-runs.md"
source_rfc_status: "Accepted 2026-09-13 — M12"
target_milestone: "M12"
created: "2026-09-13"
---

# Task breakdown and PR plan

**A → B → C. Nothing new becomes deletable until B.**

## PR-050-A — the lock, the id, the origin (core only, no loading)

- `BoundedTranscriptWriter` takes an exclusive `try_lock` on create and holds it for its lifetime.
  **If the lock fails, the writer is not created.** The launch degrades to capture disabled, with a
  reason on the run distinct from opt-out (RFC-049 D4′'s shape).
- `AgentRunId` gains a UUID-validating constructor, matching `ProjectId::from_persisted`.
- `Transcript` carries its **origin** as an exhaustive type: launched by this process, or found on
  disk. A found record holds no terminal or run the session lacks. Propose the type change in the
  request; do not invent ids to satisfy the ownership checks.
- RFC-049's liveness predicate matches on origin first. Launched records keep D8′ unchanged.

**Required tests:**

- While a writer is alive, a second handle's `try_lock` returns `WouldBlock`; after the writer drops,
  it succeeds. **Ablation:** remove the writer's lock; the first assertion fails alone.
- A writer whose lock cannot be taken is not created, and the launch proceeds without capture.
- `AgentRunId` rejects a non-UUID string.
- The predicate is exhaustive over origin. **Ablation:** add an origin variant; the crate fails to
  compile, in the predicate only.

## PR-050-B — load, count, purge

- At project open, enumerate `<state>/transcripts/<project_id>/` under the rules in the risk
  document's §1. Add a found record per accepted file, with `last_write_at` from mtime, and probe its
  lock once.
- The loader takes the state root as a parameter. Tests reach it only through the `cfg(test)` split
  (§5).
- **App-wide bytes** come from a read of all of `transcripts/`, taken at the same moments, so closed
  projects count. **Unclaimed bytes** (directories no open or recent project owns, plus skipped
  entries) are computed separately.
- **Purge honours the lock.** A live found file is skipped, and the purge summary reports it.
- Verify the dialog's count against tombstones (risk document §4) and fix it if the reading holds.

**Required tests:** each **drives a real restart**, meaning a fresh `AppState` restored from the
first one's saved recent-project state, never a session that simply kept its records.

- A real launch writes a transcript. After the restart and reopen, the summary counts it, and purge
  deletes **the file on disk**. Assert the path is gone. **Ablation:** skip loading; the test fails
  alone.
- A found file whose lock another handle holds is loaded as live and survives purge, and the summary
  names it. **Ablation:** remove the probe; the test fails alone.
- Each is skipped and **still exists afterwards**: a symlinked run directory pointing at a file
  outside the state root, a non-UUID directory name, a stray file, and a `transcript.log` three levels
  deep.
- The app-wide figure includes a project that is closed.
- A directory whose project id is not in the recent list is not loaded, not deleted, and counted as
  unclaimed.

## PR-050-C — say it, and stop saying the old thing

**The rule this slice serves (owner, 2026-09-13):** whenever transcripts stop belonging to any
project, the user is told what happened and where the files are, at the next moment they can see it.

- **The reset notice.** When `recent-projects.json` could not be read at startup, the project board
  says so **once**, on that start: the recent list was reset, transcripts from before remain on disk
  (bytes and location), and where the unreadable file was moved. Today the error reaches only stderr
  in `main.rs`; carry it to the board with the same pattern as RFC-045's configuration line. Absent
  on every other start.
- `remove_recent_project`'s doc comment names the rule, so any future removal control carries it in
  its confirmation. **Add no caller and no new API**; nothing removes a project today (RFC-036).
- Trust Settings shows **unclaimed bytes and where they are** when there are any, and nothing when
  there are none. After a recent-project reset this is every transcript the user has (D6′), so word it
  for that case: state the fact, do not imply the user did something, do not imply it is dangerous.
- The purge dialog names files still being written and will not be removed; absent when none.
- **Remove the request-387 disclosure** from the book, `README.md` and the changelog, in this commit.
- **Changelog defect entry**: `0.12.0` through `0.18.0`, what a user saw, and that deleting
  `transcripts/` was the only complete removal.

**Required tests:** each notice is present when true and absent when false, as separate assertions,
each ablated alone. **The reset notice also appears on the reset start only:** a second start with a
readable file shows nothing.

**Evidence:** a live walkthrough against a `mktemp -d` state root and config. Launch, quit the app,
start it again, open Trust Settings and see the earlier transcript counted, purge it, and show the
file gone. Try `wtype` first, per `ARCHITECTURE.md`.

## After this

RFC-049 PR-049-C's paused parts resume on loaded records. Its changelog sentence (*"the first run
after upgrading will delete every transcript already older than the configured age"*) becomes true
only now. Check it against a real restart before writing it.
