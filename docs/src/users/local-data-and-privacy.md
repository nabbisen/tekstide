# Local data and privacy

Tekstide is local-first: **it does not send project data anywhere.** This chapter lists every
file it writes, what each one can contain, and how to remove it.

**On macOS, every *durable* path below is under `~/Library/Application Support/tekstide/`
instead** — there is no `XDG_STATE_HOME` there, and macOS draws no distinction between
configuration and state the way XDG does, so both share that one directory. **The two Unix-domain
sockets below are the one exception**: a socket path is capped at around 100 bytes, which
`~/Library/Application Support/tekstide/...` does not reliably fit, so on macOS those two live
in the per-user temporary directory instead — their own sections below say exactly where. Each
other path below names its macOS equivalent beside the Linux one.

## The recent-projects list

`$XDG_STATE_HOME/tekstide/recent-projects.json`
(`~/.local/state/tekstide/recent-projects.json` if `XDG_STATE_HOME` is unset; `~/Library/Application
Support/tekstide/recent-projects.json` on macOS), created on every launch.

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
(`~/.local/state/tekstide/audit/audit.sqlite3` if `XDG_STATE_HOME` is unset; `~/Library/Application
Support/tekstide/audit/audit.sqlite3` on macOS).

**Pressing `Ctrl+Alt+T` to open a terminal creates this database**, the first time you do it.

What it records, and what its schema makes impossible:

- **Projects added.** Adding a project records the project and that it was added — never a
  path outside the project root, and never anything about the files inside it.
- **Command approvals.** A command proposed in a Managed AI CLI session records that it was
  requested, and then approved, edited-and-approved, or rejected — never the command text, its
  arguments, or its output. Only a risk level and a reference bounded to a fixed symbol set are
  recorded alongside the decision. A proposal whose claimed working directory disagreed with the
  one Tekstide itself verified records that mismatch as its own distinct event — never which path
  was claimed, only that a disagreement happened.
- **Restricted-mode refusals.** A feature blocked because a project is in Restricted mode
  records that something was blocked, and that Restricted mode was the reason — never which
  feature. **This family's schema has no field for naming one.**
- **Root-access refusals.** A path operation blocked for escaping the project root, or following
  a symlink out of it, records which of those two reasons applied — never the path itself.
- **Safe-close decisions.** Closing a project with live work running records whether the user
  chose to terminate it or leave it running, or that this was itself treated as a destructive
  action — never which processes, their output, or their command lines.
- **Audit store recovery.** A session that repaired its own audit store after an interruption
  records that a recovery completed — never what was found broken or what was discarded.
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

**Correction, RFC-059 PR-059-C:** earlier editions of this page said there was no in-app command to
purge the audit store, and that deleting the `audit/` directory was the only way to reset it. That
is no longer true. The Audit view (`Ctrl+Alt+V`, or the "Audit" button in the top bar) can purge
either one project's own audit records or every project's, each behind a confirmation that names
its scope and says the act cannot be undone. **Purging a project's audit records also revokes its
trust**, because the trust record this page already describes above (*"the audit store is
authoritative"*) is itself an audit record: once it is gone, that project is no longer trusted, and
Tekstide asks for trust again the next time the project is opened. The already-open project keeps
working as it was until then — the revocation is not immediate, only unrecoverable. A global purge
does the same for every currently trusted project, open or closed. The number shown afterwards is
the real count of rows the deletion removed, not a count taken before it ran. Deleting the `audit/`
directory by hand still works too, and remains the only way to remove the store's own file
entirely rather than its rows — the in-app purge only ever deletes records, never the database
file itself.
`TEKSTIDE_TERMINAL_DEMO` still exists for diagnostic use, but `Ctrl+Alt+T` is what ordinary use
reaches, and it opens the same store.

## Transcripts

**Launching an AI CLI run records that session's transcript to disk.** Two early releases claimed
the opposite; the error is described under *Corrections* in the
[changelog](../record/changelog.md). What follows is what actually happens.

Pressing `Ctrl+Alt+A` in a trusted project starts a bounded transcript for that run at
`$XDG_STATE_HOME/tekstide/transcripts/<project>/<agent-run>/transcript.log`
(`~/Library/Application Support/tekstide/transcripts/<project>/<agent-run>/transcript.log` on
macOS). It contains **the
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
records; Trust Settings' *Retained locally* figure counts transcripts, and a second figure beside it
counts unsaved-buffer recovery data (below) — two different content types, two separate numbers, never
folded into one.

The run's folder is removed only when nothing else is left in it: **a file Tekstide did not write is never
deleted, and it keeps its folder.** A `run.json` Tekstide cannot read is renamed `run.json.corrupt` beside the
transcript rather than deleted, the Project Board says so, and a purge removes that too.

### The report you export

`e` on the AgentRun Report writes a Markdown file **at the path you type**. Tekstide does not keep a copy: the
report is put together from the run's record and whatever transcript and changed-file information still exists
at that moment, written, and forgotten. Before you save, the field says what the file will contain.

The file holds three kinds of text, and **keeps them apart in the file itself**, not only on screen:

- **Your words** — the classification and notes you wrote — on lines that begin with `| `.
- **What Tekstide recorded** — the run's id, profile, prompt summary, start and end, and the counts of its
  approvals, change sets and audit events — as plain `Name: value` lines. A run whose end was not seen says
  *unknown*, never a time it was not.
- **What came from the run** — the project paths it changed, and the last 16 KiB of its transcript — on lines
  that begin with `> `. This is text an AI CLI produced, so it is quoted.

Every untrusted value is escaped the way the rest of Tekstide escapes it: a newline, a control character or a
right-to-left override in a label, a note, a path or the transcript cannot start a line of its own or borrow
another kind's marker. A note line you typed that begins with `> ` is written `| > …`.

**Once written, the file is yours and outside Tekstide's reach.** Tekstide creates it as a new file readable
only by you, never overwrites an existing file, and refuses a path inside its own data folder. It cannot change
or remove the file afterwards, and **purging a run does not remove a report you exported** — the report names
project paths and quotes the run, so treat it as you would any file you made from them.

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

## Unsaved-buffer recovery data

**Opening a project offers back whatever it has unsaved records for — not only after a crash.**
Two parts: a per-instance marker, written at every launch and removed at every clean exit, so a
marker left behind at the next launch means the previous one did not exit cleanly; and, for every
**dirty** document, a small side record — its text, cursor and viewport, and the file it was opened
against — written periodically while you type, so there is something to offer back. **The offer
itself is driven by the record's presence alone, never by the marker**: quitting a window without
saving removes the marker along with everything else a clean exit removes, but leaves the records
behind, and that is the ordinary case this exists for — the marker only ever describes how the
previous session ended, never whether anything is offered.

**It never writes the file you are editing.** This is not autosave. The record lives entirely
outside your project, at `$XDG_STATE_HOME/tekstide/recovery/` (`~/.local/state/tekstide/recovery/`
if `XDG_STATE_HOME` is unset; `~/Library/Application Support/tekstide/recovery/` on macOS); the
file on disk is never touched by it.

**A clean document has no record.** Only a document with unsaved edits gets one — the same
reasoning transcripts and every other retained content in this product already follow: if it is
already on disk, writing a second copy protects nothing. The record is removed the moment the
document is saved, and every record a project holds goes when that project closes.

**Bounded, and a refusal is never silent.** A single document's own record, and the total across
every open project, are each capped. A buffer too large to fit is named on screen, at the moment
its own protection stops — not discovered only later, at a restart that has nothing to offer for
it.

**Counted and purged exactly like a transcript.** Trust Settings shows how much recovery data a
project is retaining, right beside its transcript figure, and the same **Purge** button removes
both — there is no separate control for it.

**On by default, with a setting to turn it off.** `[recovery] persist_unsaved_buffers = false` in
your configuration file stops new records from being written; it does not delete ones that already
exist, the same forward-only shape transcript capture's own decline already has. Losing unsaved
work is the failure this feature exists to prevent, and a protection you have to find and enable
first does not prevent it — which is why it ships on.

**To remove every recovery record, delete the `recovery/records/` directory** — the path is
above — preferably with Tekstide closed. Deleting `recovery/instances/` as well just means the
next launch cannot tell whether this one exited cleanly; harmless, and self-correcting at the
launch after that.

## The project lock and attention socket

**Opening a project root takes a real, process-visible lock — not an in-memory flag.** Each open
project gets a file at `$XDG_STATE_HOME/tekstide/project-locks/<project-id>.lock`
(`~/.local/state/tekstide/project-locks/` if `XDG_STATE_HOME` is unset;
`~/Library/Application Support/tekstide/project-locks/` on macOS), held by an OS-level
`flock` for as long as the window has that project open. This is what *A project can be held by
only one Tekstide window at a time* (see
[What works today](./what-works-today.md#projects-files-and-editing)) is actually made of: a
second window asking for the same project root finds the lock held and is refused, rather than
opening a second copy that could silently overwrite the first window's own recovery records above.

**It never outlives the process that took it.** The lock is released the moment every file handle
pointing at it closes — ordinary exit, or a crash, including one as abrupt as the process being
killed outright. There is nothing to clean up by hand, and nothing to get stuck: a lock file left
behind by a process that is no longer running does not block the next open.

**Its content is a process id, and that id is never shown to you.** It exists only so this
process's own diagnostic output can name which process is holding a project it was refused — a
pid means nothing to a person reading the screen, and the lock file's content plays no part in the
exclusivity decision itself, which belongs to the lock primitive alone.

**A second window's refusal asks the first window for your attention, over a small local socket**
at `<project-locks>/<short-id>.sock`
(`$TMPDIR/tekstide-attention/<short-id>.sock` on macOS — **not** under
`~/Library/Application Support/`, for the same socket-length reason [The approval
socket](#the-approval-socket) explains below), also removed when its project closes. `<short-id>`
is the last 12 characters of the project's id once its hyphens are removed, not the full
identifier — the filename is built the same way on both platforms, chosen for the macOS case
below where the full id does not reliably leave room under the length limit, and carried over to
Linux rather than giving the two platforms two different names for the same thing. This only ever
*asks*; a Wayland
window cannot bring itself to the front on its own, so nothing here claims the first window was
raised, switched to, or focused — only that it was asked.

**This socket is best-effort, by design, on every platform.** Binding it is attempted alongside
taking the lock above, but nothing about opening a project depends on it succeeding: a path too
long for the same length limit the approval socket names below, a bind error, or a permission
error all fail the same way — silently, with no message on screen. Losing it never loses the
lock itself; it only means a second instance's own "please look" has nowhere to land. Treated the
same as a compositor that declines to surface the request at all.

**If the mechanism cannot tell who, if anyone, holds a project** — an unreadable state directory,
for instance — **opening proceeds unprotected rather than refusing.** A text editor that will not
open because a lock file is confusing has chosen the wrong failure; the trade is a once-over
protection gap in an abnormal environment, not a product that stops working in one.

**To remove it, delete the `project-locks/` directory** — the path is above — preferably with
every Tekstide window closed. Nothing beyond the next open depends on it: a missing directory is
recreated the next time a project is opened.

## The approval socket

**A `Managed` AI CLI session (see [Configuration](./configuration.md#declaring-an-adapter)) talks
to Tekstide over a local socket** at
`$XDG_STATE_HOME/tekstide/approval/<run-id>.sock`
(`~/.local/state/tekstide/approval/` if `XDG_STATE_HOME` is unset), created when the session
starts and removed when it ends. `<run-id>` is the last 12 characters of the run's id, with its
`agent-run-` prefix stripped and its own hyphens removed first — a short fragment, not the full
`agent-run-...` form and not the full identifier either, on both platforms. Nothing is written to
disk through it; it only carries the proposal-and-decision exchange a command approval dialog is
built from.

**On macOS this socket is *not* under `~/Library/Application Support/`** — it lives at
`$TMPDIR/tekstide/<run-id>.sock` instead, in the per-user temporary directory macOS gives every
process. This is deliberate, not an oversight: `~/Library/Application Support/tekstide/approval/`
is long enough on its own that a real socket path under it would not reliably fit the length
limit below, which is why this socket moves off the durable state directory in the first place.

**Its path has a real length limit, inherited from the operating system, and it applies on
macOS too — to the per-user temporary directory above, not only to the durable paths this page
otherwise describes.** A Unix domain socket's path is capped at a small, fixed size (around 100
bytes on both Linux and macOS), shared with the rest of the path. **On Linux, if
`XDG_STATE_HOME` points somewhere with an unusually long or deeply nested path, a `Managed`
launch can fail to start, naming this directly rather than failing silently.** Pointing
`XDG_STATE_HOME` at a shorter path resolves it. **On macOS, where there is no equivalent setting
to point elsewhere, the same failure is possible if the per-user temporary directory macOS
itself chose is long enough** — and a `Managed` launch refuses to start by name
(`SocketPathTooLong`) rather than silently, exactly as on Linux, if it is.

## Where the full policies live

The retention and purge policies, and the audit store's own retention rules, are specified in the
RFCs that own them — RFC-011 for transcripts, RFC-013 for the audit store, RFC-027 for crash
detection, recovery data and the restart offer, and RFC-058 for the project lock and attention
socket, all under `rfcs/done/` in the repository. [Security
decisions](../contributors/security-decisions.md) is the canonical home for the reasoning behind
the behaviour described here.
