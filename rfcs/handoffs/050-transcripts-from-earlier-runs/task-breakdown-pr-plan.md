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

## PR-050-A follow-up (response 388) — before B

- **`RequiredLocalBounded` refuses on a lock failure**, in `launch_project_shell` **and**
  `launch_project_adapter`; the two branches are duplicates. Only `LocalBounded` degrades.
- **Lock regular files only.** Take one `fstat` on the opened handle; if it is a regular file, lock
  it, then truncate it; otherwise do neither. **Remove the `/dev/full` test mutex** and update the
  register entry that called it the fix.

**Required tests:**

- A `RequiredLocalBounded` launch against a locked file is refused: no process starts, and the held
  bytes are untouched. **Ablation:** degrade regardless of mode; the test fails alone.
- Two writers on the same FIFO both create. A second writer on a locked regular file is still
  refused. **Ablation:** lock every file type; the FIFO test fails alone.

### Second follow-up (response 390) — before PR-DOC-C

- **Move the transcript-writer preparation into one helper that both launch sites call.** The block
  that creates the writer and decides between degrading and refusing is duplicated in
  `launch_project_shell` and `launch_project_adapter`, and it has needed the same fix twice. The
  required-mode test reaches only the shell site. **Move that block and nothing else**: RFC-022's
  reason for duplicating the rest of the orchestration still holds.
- **No new test.** The existing required-mode test holds the helper, and so both sites.
  **Grep:** neither launch site calls `BoundedTranscriptWriter::create` directly. **Ablation:**
  degrade every mode inside the helper; the required-mode test fails alone.

## PR-050-B — load, count, purge

**First commit, carried from other reviews:** the wake-notifier test's ordering fix (response 391),
and PR-DOC-C's link-test follow-up (response 392, in the documentation handoff).

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
- **Run-directory names must be the product's own spelling** (risk document §1): `agent-run-` and a
  lowercase hyphenated UUID, as a round trip. `from_persisted` stays unchanged.
- **The dialog counts only what purge will delete**, so a live found file is never promised.
- **In the commit that makes purge true (D8):** remove the request-387 disclosure from the book,
  `README.md` and the changelog, and add the changelog defect entry: `0.12.0` through `0.18.0`, what a
  user saw, and that deleting `transcripts/` was the only complete removal. *(Moved here from C at
  response 388.)*

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
- Run directories spelled uppercase, without hyphens, braced, and `urn:uuid:` are each skipped and
  still present. **Ablation:** accept whatever `from_persisted` accepts; the test fails alone.

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
- **A run still in progress** (D3′, response 393): when a purgeable transcript belongs to a run this
  session launched that is still running, the dialog says the run keeps running and its transcript
  is deleted too.
- **Two figure tests missing at review** (response 393): a closed recent project's bytes are claimed,
  not unclaimed; and a symlink under `transcripts/` adds no bytes to either figure.
- *(The disclosure's removal and the changelog defect entry moved to PR-050-B at response 388: D8
  puts them in the commit that makes purge true.)*

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
