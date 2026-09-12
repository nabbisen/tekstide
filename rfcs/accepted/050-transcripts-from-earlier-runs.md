# RFC-050: Transcripts From Earlier Runs

Status: **Accepted by the human owner 2026-09-13.** **D1–D9 decided by the architect on acceptance** — see the end; **D6 is restated on a measurement**, which dissolves its open question. Proposed the same day, scoped at the owner's word after RFC-049 PR-049-C stopped on request
387, which found that a project knows only the transcripts launched since it was opened in the
current process. That makes RFC-033's purge, shipped in `0.12.0`, remove nothing from earlier runs,
while its confirmation says it removes everything retained for the project.
Target milestone: **M12**
Date: 2026-09-13

Related RFCs:

- [RFC-011](../done/011-transcript-retention-and-local-data-policy.md) — owns the storage layout and
  the retention figures.
- [RFC-033](../done/033-transcript-lifecycle-controls.md) — owns the purge this makes true. Its
  record carries the correction.
- [RFC-049](../accepted/049-transcript-retention-enforcement.md) — waits on this. Retention cannot
  remove what the session cannot see.

## Summary

Load transcript records from disk when a project opens, so the purge, the retained-size figures, and
RFC-049's retention all act on the transcripts that exist rather than on the ones this process
launched. Make "is anything still writing this file?" answerable across processes with a lock the
writer holds. Say plainly, in the release that fixes it, that purge did not do what it said from
`0.12.0` through `0.18.0`.

## What is true today, measured

Every item below was checked in the code by the architect, not taken from the request.

- `ProjectSession::new` starts with no transcripts. The only production code that adds one is the
  launch's `attach_agent_run_transcript`. `add_transcript` is compiled for tests only.
- `close_project` removes the session. Opening a project restores trust and the capture opt-out and
  nothing else. **Nothing reads `transcripts/` back from disk**, and nothing about a transcript is
  persisted anywhere except its bytes.
- Files live at `<state>/transcripts/<project_id>/<agent_run_id>/transcript.log`. A project's id is
  reused on reopen only while it is in `recent-projects.json`. `ProjectId::from_persisted` requires a
  UUID; `AgentRunId` has no validating constructor.
- The Trust Settings figure and the purge dialog both come from the session's records
  (`transcript_local_data_summary_for`). `app_wide_retained_transcript_bytes` sums open sessions only,
  and its doc comment says so.
- **The writer is a thread inside the Tekstide process**, and it opens its file with `truncate`. A
  file left by an earlier process has no writer in this one, whatever became of the agent's child
  process.
- **Nothing prevents a second Tekstide on the same state root.** Its live files look exactly like
  leftovers.
- **Today's purge does not check liveness either.** It unlinks a file under a running writer, which
  keeps writing to an inode nobody can see, while Trust Settings reports zero. Found while scoping
  this; it has the same shape as the defect above and the same fix.
- **`std::fs::File::try_lock` works on this toolchain** (1.98.1, edition 2024), with no new
  dependency. Probed: a second handle in the same process gets `WouldBlock` while the writer holds
  the lock, and succeeds once the writer's handle drops. So the rule below can be tested in-process.

## Decisions required

Each has a recommendation. The owner decides on acceptance.

### D1 — Load records; do not make purge walk the directory

**Recommended.** At project open, enumerate `transcripts/<project_id>/` and add a record per file.
The existing purge and figures then consume records as they do today. A purge that walked the
directory itself would be a second route to transcript files: a second deletion path (RFC-049's risk
document, §3) and a second source for every figure. One loader, one deletion path.

### D2 — A loaded record claims only what is known

A `Transcript` today requires a terminal and a run the session holds, and a loaded file has neither.
**Recommended:** the record carries its origin as an exhaustive type, *launched by this process* or
*found on disk*, and a found record names no terminal and no run the session does not have. Inventing
ids to satisfy the ownership checks would make the record lie (RFC-047 §4.1's shape). How the type
changes is the implementer's to propose.

### D3 — Liveness: a lock the writer holds

**Recommended.** The writer takes an exclusive `try_lock` on its file for as long as it writes. The
loader probes each file **once, at load**:

- **Lock held:** live for this session. Never marked, selected, or purged. That covers a second
  Tekstide, and a run of this process whose project was closed and reopened while it wrote.
- **Lock free:** not live. No process holds it, and a new run never reopens an old file, because it
  writes under its own run id with `truncate`.

`Detached` is not the hard case for a found file: its writer died with the process that owned it. For
launched records, RFC-049's D8′ stands unchanged; the predicate matches on origin first.

**User purge honours the same rule**, which fixes the finding above. The purge dialog names files
that are still being written and will not be removed, and says nothing when there are none.

### D4 — Age: the file's mtime, read once at load

**Recommended.** A found file's mtime becomes its `last_write_at`. For a file with no writer, that is
exactly its last write, which is the age RFC-049 uses. It is read at load and never inside selection,
so RFC-049's D7 holds. Nothing records a creation time. A found record must not display its
`created_at` as a creation time; the field holds a value no later than the true one.

### D5 — The app-wide figure covers all of `transcripts/`

**Recommended.** The app-wide bytes come from a read of the whole directory at the same moments
records load, counting closed projects too. Otherwise RFC-049's app-wide budget resets whenever a
project closes. This reads sizes only; it creates no records for projects that are not open.

### D6 — Directories no project can claim

A project removed from the recent list reopens under a new id, and its old directory matches nothing.

**Recommended:** such directories are **never loaded and never deleted by any in-app path in this
RFC**. Their bytes count in the app-wide figure, and Trust Settings says how many bytes belong to
projects no longer in the recent list and where they are. An in-app way to delete them is its own
decision. **Open for the owner:** should removing a project from the recent list say that its
transcripts stay on disk?

### D7 — Enumeration refuses anything it did not write

- Symlinks are refused at every level (`symlink_metadata`, never `metadata`).
- Only a regular file named `transcript.log`, two levels down, is a transcript.
- Both directory names must parse as UUIDs. `AgentRunId` gains the validating constructor
  `ProjectId` already has.
- The existing containment checks and purge's project-local refusal stay as they are.
- **Anything unrecognized is skipped, never deleted**, and is counted in D6's figure.

### D8 — The disclosure leaves in the commit that makes it false

The disclosure added after request 387 (book, `README.md`, changelog) is removed in the same commit
that makes purge true: not before, not after. The release notes state the defect plainly: `0.12.0`
through `0.18.0`, what a user saw, and that deleting `transcripts/` was the only complete removal.

### D9 — Sequencing

RFC-050 lands before RFC-049 PR-049-C resumes. PR-049-C then runs on loaded records, and its
changelog sentence (*"the first run after upgrading will delete every transcript already older than
the configured age"*) becomes true.

## Non-goals

- **A persisted transcript index.** The directory is the source of truth. An index is a second copy
  that can disagree with the disk, which is the defect class this RFC exists to remove.
- **Restoring agent runs or terminals** from earlier processes.
- **Deleting unclaimed directories in-app** (D6).
- **Coordination between instances** beyond the writer's lock.
- **Platforms other than Linux.** `try_lock` is portable, but only Linux is tested, per the MVP.

## Risks

- **A writer that fails to take its lock looks dead.** §2 would then delete under it. The writer
  must refuse to capture rather than write unlocked, and a test must hold that.
- **Enumeration is a new filesystem read at project open**, on a directory that grows. It is bounded
  by what RFC-049 lets exist, and it runs at a moment the user caused (RFC-049 D2).
- **The honest changelog entry is uncomfortable.** Seven releases told users a purge removed what it
  did not. The project's rule is to say so rather than let it be found.

## Acceptance criteria

- After a real launch, a real restart (a fresh `AppState`), and a reopen, Trust Settings counts the
  earlier transcript, and purge deletes the file from disk.
- A file whose lock another handle holds is loaded as live, excluded from purge, and named in the
  dialog. Removing the lock probe makes a test fail on its own.
- A symlinked run directory, a non-UUID name, and a stray file are each skipped and not deleted.
- The app-wide figure includes a closed project's transcripts.
- The request-387 disclosure is gone, and the changelog carries the defect entry.

## Decided on acceptance (2026-09-13)

**D1–D5 and D7–D9 as recommended.** D6 restated below, plus four details the recommendations left
open.

### D6′ — the unclaimable directory comes from a reset, not a removal

**The proposal's premise was wrong, and the owner's open question dissolves with it.**
`remove_recent_project` has **no caller in the GUI**, so no user act removes a project from the list.
The real source was measured in `main.rs`: a corrupt `recent-projects.json` is renamed aside, the
error goes to stderr only, the app starts with an empty list, and it **saves that empty list**.
Every project then reopens under a new id, and **every** transcript directory becomes unclaimable at
once, with nothing on screen. A deleted state file does the same.

**Decided:**

- Unclaimable directories are never loaded and never deleted in-app, as D6 said. Trust Settings shows
  their bytes and where they are. **After a reset that is every transcript the user has**, so this
  line is the main surface, not a corner case.
- **Decided by the owner, 2026-09-13: yes, tell the user.** Removing a project from the recent list
  says that its transcripts stay on disk, and where. The architect had recorded the question as
  dissolved, because no removal act exists. That was too narrow: the owner's reason is to keep users
  from being confused about their own data, and it applies wherever transcripts lose their project.
  **The rule:** *whenever transcripts stop belonging to any project, the user is told what happened
  and where the files are, at the next moment they can see it.* It covers two cases:
  - **A removal act.** None exists today, so there is nothing to build now. Any removal added later
    carries this in its confirmation; `remove_recent_project`'s doc comment names the rule, so the
    next caller meets it in the code. No unused API is added for it, per RFC-036.
  - **The silent reset, which does happen today.** On the first start after `recent-projects.json`
    could not be read, the project board says so once. The notice states that the list was reset,
    that transcripts from before remain on disk (how many bytes, and where), and where the unreadable
    file was moved. It is absent on every other start. It uses the same board-line pattern as RFC-045's
    configuration notice. **Moved into PR-050-C** from `future-work.md`.
- **Repairing the store is still not done here.** The reset also drops every restored trust decision,
  and recovering the recent-project store is RFC-047's problem shape. It stays reserved in
  `future-work.md`. This RFC makes the reset visible; it does not undo it.

### Details decided

- **A writer that cannot take its lock does not capture.** The run starts with capture disabled and
  its detail says why, as in RFC-049 D4′. It never writes unlocked.
- **The load-time probe takes the lock and drops it at once.** It never holds a lock past the probe.
- **The loader takes the state root as a parameter**, as the launch does, and resolves it through the
  same split as `resolve_agent_run_state_dir`. **No test may enumerate the real state root.**
- **Slices:** A (writer lock, `AgentRunId` validation, liveness by origin; core, no loading) →
  B (loader, enumeration safety, figures, purge honouring the lock) → C (the unclaimed-bytes line,
  the disclosure removed, the changelog defect entry, and a restart walkthrough).
