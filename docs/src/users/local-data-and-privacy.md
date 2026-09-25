# Local data and privacy

Tekstide is local-first: **it does not send project data anywhere.** This chapter lists every
file it writes, what each one can contain, and how to remove it.

## The recent-projects list

`$XDG_STATE_HOME/tekstide/recent-projects.json`
(`~/.local/state/tekstide/recent-projects.json` if `XDG_STATE_HOME` is unset), created on every
launch.

It holds the paths of projects you have opened, used to restore the Project Board across
sessions, plus each project's last-known trust state so the board can label rows without opening
every project.

**That trust cache is a display hint only.** The file is user-writable, so the audit store is
what actually restores trust, and editing this file cannot grant a project anything.

There is no in-app command to clear it. Delete the file to reset it.

**Tekstide keeps one previous-good copy of this file**, `recent-projects.json.bak`, written whenever
it saves a list it really loaded. If the live file is ever damaged or unreadable, it is moved aside
and the list is restored from that copy **with its project identities intact** — which is what keeps
your trust decisions re-verifiable and your transcripts attached to their projects. The project board
tells you which happened: restored from the last saved copy, or reset with nothing to recover.

**Restoring the list never restores trust.** Every restored project is re-checked against the audit
store, and one with no recorded grant is Restricted again.

## The audit store

`$XDG_STATE_HOME/tekstide/audit/audit.sqlite3`
(`~/.local/state/tekstide/audit/audit.sqlite3` if `XDG_STATE_HOME` is unset).

**Pressing `Ctrl+Alt+T` to open a terminal creates this database**, the first time you do it.

What it records, and what its schema makes impossible:

- **Terminal sessions.** Each launch records a `Started` event; if the session later exits —
  typing `exit`, or the shell dying — a matching `Terminated` event names only whether the
  process exited or was signalled. Never a command, its output, or a path. **This family's
  schema has no field for any of those**, so none can ever be recorded in it.
- **Refused pastes.** A blocked paste records only that a paste was blocked and which
  project and terminal it was aimed at — never the pasted content, the clipboard text, or the
  command it would have produced. This family's schema has no field for those either. **A paste
  the policy allows is not audited at all**; only refusals are, which is a disclosed limitation
  of the schema rather than an oversight.
- **Trust changes.** Granting or revoking workspace trust records the project and the canonical
  path the grant binds to. This is what makes trust survive a restart: the store is
  authoritative, and it is queried for an *applied* grant specifically, so an interrupted or
  authorized-but-not-applied attempt does not restore as trust.
- **AI CLI runs.** Launching one records that it was authorized and started, and a matching
  `Terminated` event when it ends, naming only *how* it ended: the process exited, it was
  terminated, or the runtime failed. Never an exit code, a signal number, or any runtime text.
  **A run Tekstide stopped supervising has no recorded ending**: if it loses track of the process,
  the trail stops at `Started` and stays there, because nobody observed an ending and a record
  claiming one would be worse than the silence. The trail still does **not** answer whether a run is
  currently going — only what has already happened to it.
- **Transcript purges.** A purge records that it happened and its scope — never a path or a byte
  count.
- **Configuration changes.** Confirming a configuration-defined AI CLI profile's first use, or a
  reload that weakens a security-relevant setting, records that a sensitive setting changed and
  in which direction — **never which setting or what value, and there is no field that could.**

There is no in-app command to purge the audit store. Delete the `audit/` directory to reset it.
`TEKSTIDE_TERMINAL_DEMO` still exists for diagnostic use, but `Ctrl+Alt+T` is what ordinary use
reaches, and it opens the same store.

## Transcripts

**Launching an AI CLI run records that session's transcript to disk.** Two early releases claimed
the opposite; the error is described under *Corrections* in the
[changelog](../record/changelog.md). What follows is what actually happens.

Pressing `Ctrl+Alt+A` in a trusted project starts a bounded transcript for that run at
`$XDG_STATE_HOME/tekstide/transcripts/<project>/<agent-run>/transcript.log`. It contains **the
terminal output of that AI session as it was produced** — which means whatever the AI CLI
printed, **including anything it quoted from your files.**

Capture is bounded by policy, not by chance: at most **32 MiB per transcript**, **256 MiB per
project**, and **1 GiB across the application**. Capture is best-effort — if writing fails
mid-session the run marks capture failed and the terminal stays usable, rather than silently
continuing unrecorded.

**Transcripts past the configured retention age are removed without being asked.** The age is
`transcript_retention_days` in [Configuration](./configuration.md), 30 days by default, measured
from a transcript's last write.

This happens at exactly two moments, both of which you cause: **opening a project**, and
**launching an AI CLI run** in it. There is no timer, no watcher, and no background sweep — if
Tekstide is not doing one of those two things, nothing is being deleted. **The project board says
what was removed**, and says separately when a transcript could not be deleted.

A transcript a run may still be writing is **never** removed, at any pressure. When that means the
byte budgets cannot be brought under their limits, the next run starts **without a transcript**
rather than deleting a live one — the launch confirmation says so before you start it, and the
run's own AgentRun Report says why it has none.

**Each run also leaves a small record, `run.json`, in the same folder as its transcript.** It holds
references and no output: the run's id, its profile, its prompt summary, timestamps and the ids of the
approvals, change sets and audit events it touched — at most 200 of each, and the record says when it left
some out. It is what lets a project you reopen list its earlier runs with their transcripts attached.
A run that was still going when Tekstide closed says it does not know when it ended.

**Purging removes a run's record along with its transcript. The retention age does not.**
`transcript_retention_days` and the size limits expire *transcripts*; a run's record — and, once you write
them, its classification and notes, which are your own words — stays, and the run is still listed. Purging
afterwards removes it, whether or not the transcript is still there: **Purge** in Trust Settings is what
asks for a run's stored data to go. The purge dialog counts runs and the bytes of their transcripts *and*
records; Trust Settings' *Retained locally* figure counts transcripts only, and says so.

The run's folder is removed only when nothing else is left in it: **a file Tekstide did not write is never
deleted, and it keeps its folder.** A `run.json` Tekstide cannot read is renamed `run.json.corrupt` beside the
transcript rather than deleted, the Project Board says so, and a purge removes that too.

### The two controls, on Trust Settings (`Ctrl+Alt+U`)

- **Decline capture for future runs, per project.** `Space` toggles it. This is **forward-only**
  — declining does not delete any transcript that already exists — and the setting persists
  across a restart.
- **Purge this project's transcripts**, including those from earlier runs. **Delete** opens a
  confirmation naming the scope (this project; other projects are unaffected) and stating that it
  cannot be undone. A content-free tombstone record remains after a purge, as does the audit entry
  above.

**Purge leaves two kinds of file in place.** A transcript that was still being written when the
project opened — by a second Tekstide on the same state directory, for example — is never deleted
from under its writer. And transcripts whose project is no longer in the recent list, for example
after `recent-projects.json` could not be read, belong to no project, so no project's purge reaches
them. **Both are shown, not hidden.** The purge dialog names files it will leave, Trust Settings
says how many bytes no purge will delete — those belonging to no project, and files Tekstide does
not recognise — and where they are, and on a start where
`recent-projects.json` could not be read, the project board says the list was reset and where the
earlier transcripts are.

**To remove every transcript, delete the `transcripts/` directory** — the path is above — preferably
with Tekstide closed.

**One limitation remains: a plain terminal (`Ctrl+Alt+T`) is not recorded.** Only AI CLI runs
are.

## Where the full policies live

The retention and purge policies, and the audit store's own retention rules, are specified in the
RFCs that own them — RFC-011 for transcripts and RFC-013 for the audit store, both under
`rfcs/done/` in the repository. [Security decisions](../contributors/security-decisions.md) is
the canonical home for the reasoning behind the behaviour described here.
