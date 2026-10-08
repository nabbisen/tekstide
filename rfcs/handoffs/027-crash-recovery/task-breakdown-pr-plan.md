# RFC-027 task breakdown and PR plan

Three slices, strictly ordered. **A before B because nothing should write content until a crash can
be detected; B before C because nothing should be offered back until it is being stored honestly.**

## PR-027-A — the marker

**Crash detection, and nothing else.** No buffer content is written in this slice.

- The per-instance marker (D12): written at startup under the state directory, named by pid,
  removed on clean shutdown.
- "The previous session crashed" = a marker exists whose pid is not alive. The liveness check is
  **production code**; `test_support`'s version is test code and must not be imported.
- Pid reuse is disclosed in the slice's own write-up, with its direction of failure named: recovery
  not offered, never falsely offered.
- Detection reported internally only. Nothing user-facing yet — there is nothing to recover.

**Proves:** a real `SIGKILL` leaves a marker; a clean exit does not; a second concurrent instance
does not make the first look crashed.

## PR-027-B — the record, with its purge

**The slice that creates user content, and therefore the slice that must make it findable.**

- Persist dirty documents only (D2): text, cursor, viewport, and the `FileSnapshot` the buffer was
  based on. No undo or redo stacks (D3).
- One file per record, own directory, `0600`/`0700` (D13, §2 row 6).
- The debounce cadence, chosen against the paired harness (D7, measurement 3).
- Per-document cost at one and ten dirty documents, twenty extrapolated and labelled as an
  extrapolation (D8, measurement 4).
- Per-buffer and total byte bounds, with the refusal naming the buffer and the limit (D9,
  measurement 5).
- Records deleted on save and on close (D11, measurement 6).
- **In this slice, not a later one** (D10, §2 row 9): the per-project purge covers recovery records,
  Trust Settings' *Retained locally* figure counts them, the setting exists and defaults on (D15),
  and `local-data-and-privacy.md` gains its section — including correcting the sentence that says
  the retained figure counts transcripts only.

**Proves:** a dirty buffer has a record and a clean one does not; the bound refuses and says so;
saving removes the record; a purge removes the records; the figure counts them.

## PR-027-C — the offer

- On restart with a crashed marker, the user is shown what can be recovered, per project and path,
  and chooses (D4, measurement 1).
- The three disk states (D5): unchanged restores; changed is offered through the existing
  `ExternalChanged`/conflict path; gone is offered with the deleted state. **No new conflict
  vocabulary** — if one seems necessary, that is a finding to report, not a word to mint.
- The offer says recovered buffers come back without undo history (D3, §3 row 10).
- Declining leaves every file on disk untouched (measurement 1).
- The real-kill round trip end to end (D6, measurement 2): edit without saving, `SIGKILL`, restart,
  recover the edit, verified on disk.

**Proves:** all six measurements complete; the round trip works against a real kill.

## Live capture

**PR-027-C needs one**, for the same reason PR-065-D did: it ships a visible control the product has
never shown. Capture the offer itself, and capture the *changed-on-disk* case rather than the easy
one — the screen where the user is told their edits were made against a file that has since moved on
is the screen worth proving.

## Not in scope

Restoring terminals or AgentRun processes (`REQ-RECOVER-003` requires the opposite), autosave, and
any new `REQ-`.
