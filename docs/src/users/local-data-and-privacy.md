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

**A configured retention age is recorded but not yet enforced.** See the caveat in
[Configuration](./configuration.md#caveat-on-transcript_retention_days): nothing removes a
transcript because of its age.

### The two controls, on Trust Settings (`Ctrl+Alt+U`)

- **Decline capture for future runs, per project.** `Space` toggles it. This is **forward-only**
  — declining does not delete any transcript that already exists — and the setting persists
  across a restart.
- **Purge every transcript retained for this project.** **Delete** opens a confirmation naming
  the scope (this project; other projects are unaffected) and stating that it cannot be undone. A
  content-free tombstone record remains after a purge, as does the audit entry above.

To remove transcripts without using the app, delete the `transcripts/` directory.

**One limitation remains: a plain terminal (`Ctrl+Alt+T`) is not recorded.** Only AI CLI runs
are.

## Where the full policies live

The retention and purge policies, and the audit store's own retention rules, are specified in the
RFCs that own them — RFC-011 for transcripts and RFC-013 for the audit store, both under
`rfcs/done/` in the repository. [Security decisions](../contributors/security-decisions.md) is
the canonical home for the reasoning behind the behaviour described here.
