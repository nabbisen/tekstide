# RFC-027: Crash Recovery and Unsaved Buffer Persistence

Status: **Proposed 2026-10-08.** `0.31.0`, M13. Queued behind RFC-065 since the split of
2026-10-07: you recover *buffers*, plural, so it always followed the document model. Requirements:
`REQ-RECOVER-002`, `REQ-RECOVER-005`.

## Summary

`0.30.0` gave the editor an open set of up to twenty documents, each able to hold unsaved edits.
Everything in that set is in memory only. If Tekstide dies — a crash, an OOM kill, a power loss —
every unsaved edit in every open document is gone, with nothing on disk to recover from and no
record that there was anything to lose.

The two requirements are deliberately hedged. `REQ-RECOVER-002` says restore buffers "**where
safe**"; `REQ-RECOVER-005` says recover unsaved buffers "**where technically feasible**". Both are
*should*, not *must*. **This RFC's job is to decide what those two hedges mean here, rather than
inherit them.** An RFC that ships the feature and leaves "where safe" undefined has not met the
requirement; it has copied it.

## What makes this different from everything shipped so far

**Unsaved buffer text is file content, and it leaves the project directory.** Every other thing
Tekstide writes to `~/.local/state/tekstide/` is metadata: `recent-projects.json` holds paths, the
audit store records that a paste was refused and never the pasted bytes, and a purge records its
scope and never a path. The one existing exception — AgentRun transcripts — is content, and it is
the precedent: it has a capture control, a *Retained locally* figure, and a purge.

So this RFC is not primarily an editor feature. It is the second thing in the product that writes
the user's own content outside their project, and the design is mostly about that.

## Decisions

**D1 — A crash is detected, not guessed.** A session marker is written under the state directory at
startup and removed on clean shutdown. A marker present at startup means the previous session did
not exit cleanly. Nothing infers a crash from the mere presence of buffer data: data with no marker
is a bug in our own cleanup and must be reported as one, not silently consumed.

**D2 — Only dirty documents are persisted.** A clean document's content is already on disk; writing
a second copy of it buys nothing and doubles what we retain. This is the direct lesson of RFC-065
PR-065-D, where `save_all` was described as writing N documents when it writes only the dirty ones:
the count that matters is the dirty count, and it should be the only count here too.

**D3 — The undo and redo histories are not persisted.** Neither requirement asks for them. They are
bounded at 500 entries *per document*, and twenty documents' worth of edit operations is a large
multiple of the text itself for a capability nothing names. **Disclosed, not silently dropped**: the
recovery offer must say that recovered buffers come back without their undo history, because a user
who presses `Ctrl+Z` expecting the pre-crash stack and gets nothing has been surprised by us.

**D4 — Recovery is offered, never applied silently.** On restart with a marker present, the user is
shown what can be recovered and chooses. Silent restoration would resurrect text the user may have
deliberately abandoned, and would make the editor's contents differ from the file on disk with no
event the user remembers causing. *Measurement 1: the offer lists each recoverable buffer with its
project and path, and declining leaves every file on disk untouched — proven on real files.*

**D5 — "Where safe" means the disk file is checked, not assumed (`REQ-RECOVER-002`).** The
persisted record carries the `FileSnapshot` the buffer was based on. At recovery the file on disk is
re-snapshotted and compared:

- **unchanged** — the buffer is restored as a dirty document, exactly as it was;
- **changed since the crash** — the buffer is offered through the external-change machinery that
  already exists (`ExternalChanged` / conflict), never silently applied over the newer file;
- **gone** — the text is offered, and the document carries its own deleted-on-disk state.

No new conflict vocabulary is invented. If this needs one, that is a finding about the existing
states, not a licence to mint a third one.

**D6 — "Technically feasible" means durable, and durability has to be proven against a real kill
(`REQ-RECOVER-005`).** A recovery file that is still in the page cache when the machine loses power
is not a recovery file. The write path must fsync, and the proof must be a **real `SIGKILL` of a
real process, followed by a real restart** — not a `simulate_crash()` helper, which would prove only
that our own abstraction round-trips. This project tests real PTYs, real sockets and real files; a
crash test that never crashes anything would be the one exception, and it would be the one place it
mattered most. *Measurement 2: kill -9 during an unsaved edit, restart, recover the edit.*

**D7 — The write cadence is measured, not chosen by taste.** Persisting on every keystroke is a disk
write per keystroke. The editor has a measured typing-latency baseline and RFC-026's paired-control
harness, reused by RFC-065 D7/D13. The debounce window must be picked against a measurement of what
persistence costs *between* keystrokes, with the control carried inside the same run.
*Measurement 3: typing latency and delivery work, with persistence on and off, paired.*

**D8 — The cost is per dirty document and is reported per unit.** RFC-065 D7 measured one document
and ten and published the ratio; the same shape applies. A figure for "a save" with the document
count unstated is the §4.1 pattern — a number describing something adjacent to what was measured.
*Measurement 4: per-document persistence cost at one and at ten dirty documents, with the open set's
bound of twenty extrapolated and labelled as an extrapolation.*

**D9 — Retention is bounded, and exceeding the bound is said out loud.** A dirty 100,000-line
document is megabytes; twenty of them is not a state directory, it is a second copy of the user's
work. There is a per-buffer byte bound and a total bound. **A buffer too large to persist is named
to the user at the moment it stops being protected** — silently not protecting something the user
believes is protected is worse than not having the feature. *Measurement 5: the bound is enforced
and the refusal names the buffer and the limit.*

**D10 — It is content, so it gets the content treatment, and that ships with the writing, not
after.** Recovery data is covered by a purge, counted in Trust Settings' *Retained locally* figure,
written user-only (`0600`), and documented in `local-data-and-privacy.md`. That page currently says
the retained figure "counts transcripts only, and says so" — a sentence this RFC makes false, and it
must change in the same slice that makes it false. **No slice may write user content before the
purge that removes it exists.**

**D11 — Recovery data is deleted the moment its reason ends.** When a buffer is saved, or closed, or
recovered, its record goes. Recovery data outliving the buffer it protects is not a stale cache; it
is the user's unsaved text persisting after they believe they are done with it. *Measurement 6: save
a recovered buffer and the record is gone from disk.*

## Non-goals

- **This is not autosave.** Autosave writes the user's own file; this writes a side record and never
  touches the file the user is editing. Conflating them would turn a recovery feature into a
  data-loss one. Nothing in this RFC may write to a path inside the project.
- **Terminal and AgentRun processes are not restored.** `REQ-RECOVER-003` already requires the
  opposite: we must not pretend killed processes are still running.
- **No new `REQ-`.** `REQ-RECOVER-002` and `005` cover this; unlike multi-document at RFC-065 D8,
  the requirements exist and name it.

## Slices

- **PR-027-A — the marker.** Crash detected and reported; nothing persisted and nothing restored.
  D1, with the real-kill test from D6 proving detection before anything depends on it.
- **PR-027-B — the record, with its purge.** D2, D3, D9, D10, D11 and measurements 3, 4, 5. Content
  retention and the purge that removes it land together, per D10.
- **PR-027-C — the offer.** D4, D5 and measurements 1, 2, 6.

## Open question for the owner

**Should unsaved-buffer persistence be on by default?** Transcript capture is a Trust Settings
control. The argument for default-on is that a recovery feature nobody enables protects nobody. The
argument for default-off is that it writes the user's content to disk without them asking, and this
product's existing habit is to ask. **My recommendation: default-on, with the retained figure and
the purge visible from the first release, and a setting to turn it off.** Losing unsaved work is the
failure this exists to prevent, and a protection the user must first discover does not prevent it.
