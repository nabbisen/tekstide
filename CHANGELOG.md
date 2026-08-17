# Changelog

## 0.10.0 - Trust, and the First Reachable Agent Run

Status: released on 2026-08-17.

**The release where the product's premise becomes reachable.** Every previous release
shipped an application in which no project could *ever* leave Restricted Mode — there
was no code path anywhere that granted trust — so AgentRun launch, the thing Tekstide
exists to do, was blocked for every user, permanently. This release grants trust,
launches the run, and sizes the terminal it runs in.

### Added

- **Workspace trust granting and revocation** (RFC-032). `Ctrl+Alt+U` opens a Workspace
  Trust surface showing the project's real state. Granting opens a confirmation dialog
  whose focus defaults to **Cancel** — granting takes two deliberate acts, revoking
  takes one, because revoking is the safe direction. The path shown is the **canonical**
  path, which is what trust binds to; a symlinked project also shows the path you opened
  it by, so a redirected symlink cannot quietly bind trust somewhere else. Trust
  persists across sessions, and the **audit store** — not the user-writable
  recent-projects cache — is what restores it, queried for an *applied* grant
  specifically so an interrupted grant does not come back as trust.

  The dialog says three things it would be easy to leave implied: that the grant covers
  files not yet written, including anything an AI agent run writes there; that it lasts
  for this session and every session after; and that revoking stops future loading but
  does **not** undo anything that has already run. It does not claim that trusting is
  safe, or that Tekstide polices what runs.

- **AgentRun launch, reachable for the first time.** With trust granted, `Ctrl+Alt+A`
  launches a real Claude Code session in a project-owned terminal. Proven end to end
  from a real key press: a profile that honestly declares it may discover workspace
  files is refused in a fresh Restricted project, and launches for real once trust is
  granted through the GUI route.

- **The adapter-spawn pathway and a rendered command-approval dialog** (RFC-022):
  per-run Unix domain socket, capability-token delivery, structural risk classification,
  promotion re-evaluated rather than decided once, a bounded approval queue with expiry
  tracking, and an approval-history surface. All built and proven against production
  code. See *Known limitations* for why no user can reach it.

### Fixed

- **Terminals were permanently 24×80.** `ROWS`/`COLS` were fixed constants shared by the
  spawned PTY and the rendered grid, and nothing in the application called terminal
  resize at all — so every terminal ignored the window regardless of size. Terminals now
  follow a live window drag, and a pane launched before you ever resize the window gets
  the real size immediately rather than starting wrong. One computed size is applied to
  the PTY, the emulator grid, and the render path together.

- **A completed trust grant could be undone by a later interrupted one**, and trust was
  restored from the user-writable recent-projects cache rather than the audit store.

### Known limitations

- **You still cannot see what an agent run changed.** There is no diff review or
  AgentRun report surface, and the reason is structural rather than scheduling: nothing
  in the shipped application runs change detection, so no change set can exist for
  either surface to render. This is the next theme.
- **The real Claude Code CLI has never been exercised by this project's tests.** Every
  automated proof uses a controlled test executable, because the live product needs
  interactive authentication and makes real network calls. The launch pathway is proven;
  the real binary's behaviour under it is not.
- **Command approval remains unreachable, for a new reason.** It is no longer missing
  machinery — it is missing an ecosystem. No shipping AI CLI speaks RFC-021's protocol,
  so `Managed` mode can only ever be exercised by this project's own reference adapter,
  a test artifact. Approval also remains **cooperative, not enforced**: Tekstide does not
  intercept process execution and cannot withhold it from an adapter that ignores a
  rejection.
- **The approval-history surface cannot be opened.** It is implemented and tested, but no
  key is bound to it — a defect found while reviewing RFC-032 and recorded rather than
  quietly fixed. The underlying cause is now named: a navigation action marked
  *configurable* with no default binding is **dead**, not pending, because there is no
  configuration system yet to bind it with.
- **`NFR-PERF-004` (terminal input latency, 16 ms p95) remains unverified**, unchanged
  from `0.8.0`. The structural cause is gone and proven gone; bounding the true
  end-to-end path needs presentation timing this project has no non-perturbing way to
  measure.
- No screen-reader support, no cross-platform evidence beyond Linux, no safe-close
  dialog, no file watcher, no editor undo.

### Also in this cycle

- **A reachability audit** across 132 candidate capabilities, using compiler-enforced
  deprecation markers rather than grep. **104 were dormant** — correct, reviewed, tested
  code with no route from the GUI — of which 30 have no caller anywhere, a floor rather
  than a count. Two of its priority items are discharged in this release (terminal
  resize; trust granting). The rest is recorded in `rfcs/future-work.md`.
- `ARCHITECTURE.md` gained two conventions learned the hard way this cycle:
  **reachability comes before correctness** (name the user's path and the production
  producer before scheduling a surface), and **latency criteria stop the clock at state
  change, not pixels**.


## 0.9.0 - Transcript Capture, Re-homed

Status: released on 2026-08-16.

**A correctness release with nothing a user can see.** `0.8.0` replaced the terminal's read
path; this repairs a capability that replacement silently removed. It is published because
the fix should not sit unreleased while the work that depends on it is designed — not
because it adds anything reachable.

### Fixed

- **Transcript capture, which `0.8.0` had silently stopped performing.** The old read path
  wrote every byte to the transcript as a side effect of a function named for something
  else, and it was the only transcript-writing code in the workspace. When `0.8.0` moved the
  terminal's ingress to a dedicated reader thread, capture went with the old path — nothing
  failed, because nothing in this release or any before it creates an AgentRun, so no
  transcript writer is ever configured.

  Capture now lives in the reader thread, and writes **before** the bytes reach the display,
  so the durable record is a superset of what was shown rather than the reverse. Mid-stream
  write failure has a real policy for the first time: best-effort capture marks itself
  failed and keeps the terminal usable, while required capture stops reading — so the
  process stalls on its own `write()` rather than making progress that is not being
  recorded, and is not killed.

### Breaking

- **`TranscriptWriterConfig` gained a public `mode` field**, and its `new` constructor a
  third parameter. Callers constructing it either way must be updated. This is what makes
  the release `0.9.0` rather than `0.8.1`.

### Not in this release

Nothing user-visible, deliberately. Transcript retention is still **not wired into the
desktop application** — nothing creates an AgentRun, so no transcript is ever written in
practice. This release makes the capability correct *before* the work that will depend on
it, rather than after.

Command approval, diff content, the transcript reader, and the diff/AgentRun surfaces all
remain implemented, reviewed and unreachable, waiting on an adapter-spawn pathway that does
not exist. `NFR-PERF-004` remains not met.

### Also in this cycle

- A test-concurrency flake in the terminal reader suite, which made the workspace gate fail
  roughly one run in five, is fixed. Three separate bugs were behind it, two of them found
  only because a full-serialisation experiment failed to resolve the flake and the theory
  was re-examined rather than the fix tuned.


## 0.8.0 - Readiness-Driven Terminal I/O

Status: released on 2026-08-15.

One theme: the terminal stopped waiting on a timer. RFC-017 Amendment 1 replaced the 50 ms
poll tick with a dedicated reader thread that blocks on PTY readiness and wakes the UI when
bytes actually arrive.

### Implemented

- **Terminal output throughput rose from roughly 374 KB/s to 17-18 MB/s.** The old ceiling
  was not a property of the hardware or the emulator: a hardcoded 10 ms sleep ran on the UI
  thread every time a read found nothing, so the reader spent about 0.5% of each tick
  actually reading. Output now keeps pace with what a producing process can write, measured
  against the same flood script's own standalone rate.

- **The concurrent-terminal limit rose from 3 to 6**, re-derived from a fresh measurement
  rather than carried forward. The old `3` was never a product judgement — it was a
  consequence of each pane's poll costing ~10.1 ms against a 50 ms tick, which saturated at
  5. The new number comes from an N-pane benchmark that stays clean through 6 and first
  degrades at 8.

- **Terminal output can no longer be silently dropped.** The old read path truncated at
  64 KiB mid-read, discarded the remainder, and threw away the event that recorded it —
  feeding the emulator a byte stream with a hole in it. The replacement applies
  backpressure instead: when the buffer fills, the reader stops reading and the producing
  process blocks on `write()`, the way a real terminal behaves. Dropping is not handled
  better; it is structurally impossible on this path.

### Not fixed, and stated rather than implied

- **`NFR-PERF-004` (terminal input latency, p95 ≤ 16 ms) is still not met.** This release
  removes the structural cause of the previous failure — the 50 ms interval put the floor
  near 47.5 ms by arithmetic alone — but removing a known cause does not measure the
  result. Proving failure needed only a lower bound; proving success needs an upper bound,
  and three attempts to obtain one were confounded by unrelated load on the measuring
  machine and discarded rather than reported.

- **The criterion's own wording was corrected in this cycle**, after it had been evaluated
  twice against a boundary it never stated. It now says where the measurement stops —
  application state change, excluding compositor and GPU present time — consistent with the
  two neighbouring latency criteria, which had always been measured that way without saying
  so. Restating the boundary does not discharge the criterion.

- **Nothing here makes AgentRun or diff-review surfaces reachable.** They remain
  implemented at the model level with no route to them, along with command approval. The
  release notes have said this since `0.5.0` and it is still true.

- **A defect this work uncovered is recorded, not fixed**: the new reader has no transcript
  capture, and the code path that had it is no longer on the terminal's ingress. This is
  invisible today because nothing in production creates an AgentRun, and is recorded as a
  blocking prerequisite on the work that would.


## 0.7.0 - A Content-Independent Trusted-UI Tell, and Diff Preview Policy

Status: released on 2026-08-12.

A small release with one user-visible change and one library-level policy. **It contains a
breaking change to `tekstide-core`**, which is what makes it `0.7.0` rather than `0.6.1`.

### Implemented

- **The window dims behind the paste-confirmation dialog** (RFC-018 PR-018-G). RFC-018
  shipped the dialog claiming a *spatial* tell — a real dialog occludes trusted chrome,
  and terminal output cannot draw outside its own pane. Measuring that claim showed it was
  content-dependent: the dialog's size follows the pasted content, so an attacker who
  keeps a paste short keeps the dialog inside the terminal's own pane, where imitation is
  possible. The scrim replaces that tell with one the attacker does not control — its
  extent is fixed by the window, so it dims chrome no terminal pane can draw into whether
  the paste is one byte or one megabyte. It is translucent deliberately: an opaque overlay
  would be indistinguishable from any solid rectangle a spoof could also draw.

  This does **not** make the dialog unspoofable, and it does not repair the spatial claim —
  that claim was replaced, not fixed, and RFC-018's disclosed limitation stands. Keystroke
  suppression remains the load-bearing defence; the scrim is an additional check a user can
  make, not a guarantee.

- **Diff preview policy** (RFC-024, closed). `tekstide-core` can now read the content
  behind a detected change under an explicit policy: refuse rather than truncate, classify
  binary before reading text, bound the read against file metadata before any content is
  loaded, and hold content in a type that cannot outlive the request — `DiffContent`
  derives neither `Clone` nor `Serialize`, so storing it in session state or handing it to
  the audit store is a compile error rather than a review comment.

  **No surface renders any of this.** It is a library capability with no UI in this
  release; the diff review surface is RFC-020.

  One limitation worth stating plainly, because it constrains what a diff can ever be
  here: for a **modified** file there is no two-sided diff. The before-bytes were never
  captured — review baselines are metadata-only by deliberate design — so they are gone,
  not merely unretained, by the time a diff is requested. What is available is the current
  content, and the API says so in its own type rather than in a doc comment.

### Breaking

- **`ChangePathKind` no longer has a `Deleted` variant, and `DetectedChangedPath` carries a
  new `ChangeLifecycle { Added, Modified, Deleted }`** (RFC-012 Amendment 1). The old enum
  conflated *what a path is* (file, directory, symlink) with *what happened to it*, so a
  deleted directory could not be represented at all, and the Added-vs-Modified distinction
  was computed during change detection and then discarded. Callers matching on
  `ChangePathKind::Deleted` should read `ChangeLifecycle` instead.

### Not in this release

No diff or AgentRun report surface (RFC-020), no configuration system, no Git integration,
no file watcher, and no cross-platform evidence beyond Linux. The project board still
reports `terminals: not implemented` for a project with no open terminal, which is false —
it is a known defect, recorded in `rfcs/future-work.md`, not a statement about the feature.

## 0.6.0 - Editor and File Explorer

Status: released on 2026-08-11.

Tekstide `0.6.0` opens milestone M10 with RFC-019: Content mode stops being a placeholder
and becomes a real file explorer and a real text editor. **RFC-019 is closed**
(`rfcs/done/`). Diff review and the AgentRun report are RFC-020, M10's second half, and
are not in this release.

### Implemented

- **A file explorer tree.** Renders the project's directory scan, with keyboard
  navigation; Enter on a directory rescans, Enter on a file opens it. Read-only: no
  rename, delete, or create.
- **A text editor with a real cursor.** Open a file, move with the arrow keys, insert and
  delete at the cursor position across multiple lines, and save with `Ctrl+S`. The cursor
  position is shown.
- **External-change handling that asks rather than assumes.** If a file changes on disk
  while you have it open, saving does not overwrite it — a dialog offers to reload,
  and every way of dismissing that dialog leaves the disk file untouched. The dialog
  distinguishes a genuine conflict from a clean file that merely changed underneath, and
  only claims local changes will be discarded when there are some.

### Text safety — an asymmetry worth understanding

A file's **name** and a file's **contents** are treated oppositely, deliberately.

Names in the explorer and in the editor's header are **escaped**: a file called
`proj<U+202E>gpj.exe` renders with that override character visible as `<U+202E>` rather
than silently displaying as `projexe.jpg`. A repository can contain such a name and
nobody typed it.

File **contents** in the editor are **not** escaped, and bidirectional text reorders
normally. An editor that rewrote what it displayed would be broken — you would edit
around a character that is not really there and save something you did not intend.

The consequence is that source containing a bidi override still *reads* differently from
how it compiles — the Trojan Source class. Tekstide shows you the file as it is; it does
not currently mark such characters. See `rfcs/done/016-internationalization-and-localization.md`.

### Deferred

- **No undo.** A mid-buffer edit is unrecoverable within the session past what Backspace
  can still reach.
- **No syntax highlighting, language server, multi-cursor, or search.**
- **Files larger than 4 MiB are not editable** — the existing open policy refuses them,
  and the refusal is shown rather than failing silently.
- **The explorer never modifies the filesystem** — no rename, delete, or create.
- **Symlinks show status, not their target.** Whether an entry is a symlink, broken, or
  points outside the project is shown; the target path is not.
- **Reaching Content mode needs a mode toggle.** No keybinding opens the project
  workspace directly; `Ctrl+Alt+M` gets you there as a side effect.
- **A known `tekstide-core` inaccuracy**: after a blocked save, the project content status
  reports a conflict even when the open buffer had no local edits. The dialog no longer
  relies on this, but the status itself is imprecise. Recorded in `rfcs/future-work.md`.
- **Nothing here changes the terminal.** `NFR-PERF-004` remains not met, the
  three-terminal limit and the ~374 KB/s output ceiling are unchanged.
- **No screen-reader support.** Checked again this release
  (`cargo tree -p tekstide | grep -i accesskit`, empty).
- Linux only.

### Dependencies

No new dependencies.

## 0.5.1 - Paste Protection and Trusted-UI Evidence

Status: released on 2026-08-10.

Tekstide `0.5.1` completes milestone M9 with RFC-018, the second half of the terminal
work `0.5.0` began. **RFC-018 is closed** (`rfcs/done/`). The `0.5.0`/`0.5.1` split
follows the same shape as `0.4.0`/`0.4.1`: one milestone, two releases, because the
scope was too large for one.

Press **`Ctrl+Shift+V`** to paste into a focused terminal. What happens next is decided
by RFC-009's policy, not by the paste widget.

### Implemented

- **Real clipboard paste, classified before it reaches the shell (RFC-018).** Pasted
  bytes go through `TerminalInputPolicy::evaluate` before any PTY write. A single-line
  paste is allowed; a multi-line paste opens a confirmation dialog; a paste containing
  control characters is refused outright. Paste reaches the PTY through the same single,
  modal-gated ingress keystrokes already used — it did not get its own.
- **A real confirmation dialog**, built on the existing modal layer. Every dismissal path
  defaults to **not** pasting: Escape cancels regardless of which button is focused, and
  only an explicit accept writes anything.
- **The pasted content is shown, escaped.** The preview runs through the same
  untrusted-text path the Project Board uses, so a paste containing bidi-override or
  control characters renders as `<U+XXXX>` markers rather than reordering the dialog's
  own text. Newlines are escaped too, so pasted content cannot fabricate extra rows that
  imitate the dialog's controls.
- **Audit: `paste_blocked` has a producer.** A policy-refused paste is recorded. A
  sentinel test proves no pasted content, clipboard text, or command text reaches the
  durable store, checked against raw on-disk bytes.
- **Trusted-UI evidence**, nine screenshots against a real terminal running live output.

### What the trusted-UI evidence shows, and does not

One property distinguishes the genuine paste dialog from terminal output imitating it:
**while the dialog is open, keystrokes never reach the terminal.** That was demonstrated
live, with a positive control proving the keystrokes were reaching the application at the
time — so their absence is suppression, not non-delivery. An imitation drawn by terminal
output cannot suppress input.

The terminal grid can never render outside its own pane, so chrome is always authentic.
But whether the genuine dialog *visibly* uses that headroom depends on how wide its
preview is — which depends on the pasted content, which an attacker may influence. It is
therefore recorded as an architectural fact and **not** offered as something a user can
rely on seeing.

**This evidence shows an imitation cannot occupy chrome and cannot suppress input. It
does not show that a user would notice one that tries.**

### Dependencies

No new dependencies.

### Deferred

- **Pastes larger than 256 KiB are refused whole**, not truncated. Truncating before
  classification would let truncation change the classification and would silently write
  a prefix of what was copied.
- **The audit family records paste refusals only.** A paste the user *approves* has no
  valid encoding in the frozen v1 schema, and an over-cap refusal has none either. Both
  are recorded as known limitations rather than fixed by amending a frozen schema.
- **No semantic detection of dangerous pasted commands.** RFC-009 excludes it by design;
  a classifier that catches some dangerous pastes invites the belief that it catches all.
- **Nothing here improves terminal performance.** `NFR-PERF-004` remains not met, the
  three-terminal limit and the ~374 KB/s output ceiling are unchanged. All are downstream
  of the same poll defect and owned by readiness-driven terminal I/O
  (`rfcs/future-work.md`).
- **No screen-reader support.** Checked again this release
  (`cargo tree -p tekstide | grep -i accesskit`, empty).
- Linux only. No macOS or Windows terminal runtime evidence exists.

## 0.5.0 - Terminal Renderer, and a Terminal You Can Open

Status: released on 2026-08-08.

Tekstide `0.5.0` delivers the first half of milestone M9: RFC-017's terminal renderer,
plus the launch UX that makes it reachable. **RFC-017 is closed** (`rfcs/done/`), accepted
with `NFR-PERF-004` recorded as **not met** — see Deferred. RFC-018 (rendered paste
protection and adversarial spoofing evidence) is M9's second half and is not in this
release.

Press **`Ctrl+Alt+T`** in an open project and you get a real, PTY-backed terminal with
RFC-009's accepted-sequence policy enforced in front of the emulator.

### Implemented

- **Terminal surface (RFC-017).** A real `alacritty_terminal` grid behind RFC-009's
  security filter, rendered as a surface under RFC-015's contract. The filter's four
  properties — single ingress, no side channels, classification parity, and
  stream-position independence under adversarially chunked input — were re-proven
  against product code rather than inherited from the RFC-014 spike, each independently
  ablated.
- **Terminal launch UX.** `Ctrl+Alt+T` launches a terminal in the active project and
  switches to Terminal Mode. Typing `exit` really closes it: exit detection transitions
  the session, frees its visible slot, and makes the slot reusable.
- **Immersion mode, split, and session bar.** At most two visible panes, with the split
  decided from real measured font metrics — a split that cannot give each pane a full
  grid width is refused rather than rendered clipped. Session state is distinguishable
  without colour (`NFR-UX-002`).
- **Audit: `plain_terminal_observation` has a producer.** The first audit write the
  desktop application has ever performed. Opening a terminal records that a session
  started; exiting records that it terminated. A sentinel test proves no command text,
  output, or path reaches the durable store, checked against raw on-disk bytes.
- **Bounded scrollback** at 2,000 lines, ablation-verified under sustained output.
- The RFC-014 and RFC-007 spike crates were deleted, their properties having product-code
  equivalents with their own tests.

### Local data

**Opening a terminal now creates an audit database** at
`$XDG_STATE_HOME/tekstide/audit/audit.sqlite3`. This is a behaviour change from `0.4.1`,
which created no such file. It records that a terminal session started and stopped, and
nothing else — the schema has no field for command text, output, or paths. Delete the
`audit/` directory to reset it; there is no in-app purge command yet.

### Dependencies

No new dependencies. Two workspace crates were removed (`tekstide-gui-spike`,
`tekstide-pty-spike`), both `publish = false` and neither reachable from a shipped crate.

### Deferred

- **`NFR-PERF-004` (terminal input latency p95 ≤ 16 ms) is NOT met**, and is recorded as
  such rather than redefined until it passed. PTY bytes reach the grid only on a 50 ms
  poll tick, so poll-wait alone contributes a p95 near 47.5 ms. The fix is readiness-driven
  terminal I/O, scheduled as follow-up (`rfcs/future-work.md`).
- **At most three concurrent terminals per project.** This is a consequence of the same
  poll defect, not a product decision: every live pane is polled sequentially each tick at
  roughly 10 ms per pane, and five panes would saturate the tick. The limit is expected to
  rise once readiness-driven I/O lands.
- **Terminal output throughput is capped near 374 KB/s**, again by the same defect.
- **No trusted-UI separation or spoofing-resistance claim.** Nothing in this release
  demonstrates that terminal content cannot imitate Tekstide's own chrome. That is RFC-018.
- **No paste path exists** — the terminal accepts keystrokes only, so RFC-009's paste
  policy has nothing to protect yet. Rendered paste protection is RFC-018.
- **No terminate-from-UI and no pane selection.** Close a terminal by typing `exit`; input
  goes to the `Primary` pane.
- `TextStream::to_pty_bytes` is a defined subset, not a complete VT100/xterm encoder.
- **No screen-reader support.** `iced` offers no accessibility bridge; checked again this
  release (`cargo tree -p tekstide | grep -i accesskit`, empty).
- Linux only. No macOS or Windows terminal runtime evidence exists.

## 0.4.1 - Mode Switching, Focus Indicator, and RFC-015 Closure

Status: released on 2026-08-01.

Tekstide `0.4.1` completes milestone M8 (GUI Foundation) with RFC-015 PR-015-E: the
`0.4.0`/`0.4.1` split deferred mode switching and its latency measurement here because
M8 had no second mode to switch into until this slice built it. **RFC-015 is closed**
(`rfcs/done/`) as of this release — both risks the RFC-014 substrate decision carried
unverified are now discharged: **R1** (input latency) by `0.4.0`'s C2/C5 measurements
and this release's C4, and **R6** (the focus-trap property not transferring from the
spike) by PR-015-C's real test. The RFC-014 substrate decision record has no open
items remaining.

### Implemented

- RFC-015 PR-015-E mode switching and Content-mode scaffolding:
  - Content ↔ Terminal route switching, dispatched through a real `Ctrl+Alt+M`
    keybinding (`NavigationAction::ToggleProjectMode`, previously unbound) to the
    pre-existing `AppCommand::ToggleActiveProjectMode`; no animation or interpolation
    in the switch path;
  - sidebar and main-area scaffolding (`FocusZone::Sidebar`, still `#[non_exhaustive]`
    for RFC-017/019/020) that required no changes to the input-routing structure
    PR-015-C established;
  - a visible, non-colour-only focus indicator (`NFR-UX-002`): border colour, border
    width, and a textual `"> "` marker all change together with `state.focus` —
    `0.4.0` shipped without one, defensibly, because the shell had only one focus
    zone; this release adds the second zone the indicator was always meant for.
- RFC-014 R1 discharge, completed: C4 (`NFR-PERF-002`, mode-switch latency, budget
  p95 ≤ 32ms), reusing `0.4.0`'s measurement harness rather than a new mechanism.
  Decomposed input-to-state-change (p95 29µs) and view-build cost (p95 39µs) sum to
  68µs, met by roughly 470× — **measured against the Content/Terminal-mode
  placeholders this release ships** (single-line catalog text each), not against the
  real editor (RFC-019) or terminal grid (RFC-017) those placeholders stand in for.
  RFC-017's handoff carries the obligation to re-check `NFR-PERF-002` once Terminal
  Mode renders a real grid.

### Dependencies

No new dependencies; this release is entirely `crates/tekstide-core`/`crates/tekstide`
source changes (one new default keybinding, no new crates).

### Deferred

- Terminal rendering, editor, file explorer, and diff/review surfaces — M9/M10, RFC-017/019/020.
- Rendered security dialogs and an adapter-spawn pathway that would make command
  approval reachable — M11. Command approval remains implemented but unreachable.
- Screen-reader support — out of scope for the life of the `iced` substrate decision
  (RFC-014 R2, owner-accepted), unchanged.
- `NFR-PERF-002`'s re-check against real Content/Terminal-mode content once RFC-017
  and RFC-019 render it — the placeholder boundary above, not a new finding.

## 0.4.0 - Application Shell and Project Board

Status: released on 2026-08-01.

Tekstide `0.4.0` covers milestone M8 (GUI Foundation): RFC-014's substrate decision,
RFC-016 PR-016-B/C/D's i18n and text-safety foundations, and RFC-015 PR-015-B/C/D/F/G's
application shell and Project Board. Owner-approved `0.4.0`/`0.4.1` split (2026-07-30):
mode switching and its latency measurement move to `0.4.1` because M8 has no second
mode to switch into that isn't the Project Board against an empty placeholder. It
remains a GUI shell over the headless core, not the full AI CLI workbench.

### Implemented

- RFC-014 desktop GUI substrate decision:
  - `iced` approved as the substrate, with Option A terminal filtering;
  - spike evidence and findings R1-R9 recorded; R1 (latency unverified) and R6
    (focus-trap property) discharged by RFC-015; R2 (no screen-reader support) and R9
    (survivorship bias in confirmed-only percentiles) owner-accepted and carried
    forward as standing findings, not defects.
- RFC-016 i18n, locale, and text-safety foundations (PR-016-B/C/D):
  - string catalog, locale selection with fallback, and the discipline that no
    user-facing string is hardcoded;
  - a canonical shared text-safety primitive (escaping and bidi isolation for
    untrusted text) adopted by both the shell and `approval::coordinator::display_argv`,
    retiring the duplicate-escaping debt recorded in `rfcs/delivery-plan.md`;
  - `CatalogArgs`' typed `number`/`untrusted`/`trusted_symbol` interpolation API,
    closing the untrusted-text interpolation bypass structurally rather than by
    convention, plus pluralization support.
- RFC-015 application shell and rendered surface model (PR-015-B/C/D/F/G):
  - a real `iced` desktop application replacing the headless text harness: window,
    chrome/content/modal layer composition via `stack`/`opaque`, with surface code
    structurally unable to open, populate, or dismiss a modal or render trusted chrome;
  - a keyboard-driven focus and input-routing model (`ShellInput`/`SurfaceInput`/
    `TextStream` as distinct, module-private types) with modal exclusivity and
    input-class privacy enforced by the compiler, not a runtime check;
  - a Project Board surface rendering live `ApplicationShell` state, with untrusted
    project names and paths escaped and honest `CountDisplay` fidelity
    (`Unavailable`/`NotImplemented` never render as `0`);
  - app-internal latency measurement (behind an opt-in flag, proven non-contaminating
    by idle-CPU comparison) discharging RFC-014 R1: typing latency
    (`NFR-PERF-003`, an upper-bound proxy from the sum of two measured streams' p95s)
    clears its budget by roughly two orders of magnitude, and warm start
    (`NFR-PERF-001`) clears its budget comfortably, at about a fifth of it.

### Dependencies

- Added to `tekstide` only (`tekstide-core` gains no GUI dependency, mechanically
  checked via `cargo tree -p tekstide-core --edges normal | grep -i iced`): `iced 0.14`
  (`tokio`, `advanced` features), `fluent-bundle 0.16`, `unic-langid 0.9`,
  `sys-locale 0.3`.

### Deferred

- Mode switching between Content and Terminal views, and the `NFR-PERF-002`
  mode-switch latency measurement that depends on it — `0.4.1`, RFC-015 PR-015-E.
- Visible focus indicators at the shell-chrome level. Low-stakes today because the
  shell has a single focus zone, but required before PR-015-E adds a second one —
  tracked for `0.4.1`.
- Terminal rendering, editor, file explorer, and diff/review surfaces — M9/M10.
- Rendered security dialogs (trust, safe-close, destructive, configuration change) and
  an adapter-spawn pathway that would make command approval reachable — M11. Command
  approval remains implemented but unreachable, as in `0.3.0`.
- Screen-reader support — out of scope for the life of the `iced` substrate decision
  (RFC-014 R2, owner-accepted).
- Cross-platform terminal, storage, and GUI evidence beyond
  `x86_64-unknown-linux-gnu`.
- **RFC-015 is not closed by this release.** Per RFC-000, it stays in
  `rfcs/proposed/` until PR-015-E and `NFR-PERF-002` land in `0.4.1`.

## 0.3.0 - AgentRun, Transcript, Review, and Durable Audit

Status: released on 2026-07-28. **Git tag re-pointed 2026-08-17** — the original `0.3.0`
tag pointed at commit `1f5100b5`, which a later rewrite (stripping `Co-Authored-By`
trailers) left on an orphaned line of history that no branch contains. The tag now points
at `de40d648`, that rewrite's content-identical replacement: same tree `5291a6b1`, same
message, same author date, reachable from `main`. **What `0.3.0` marks is unchanged**;
only the pointer was repaired. The `0.3.0` package on crates.io still records the old
hash in its own `.cargo_vcs_info.json`, which is not editable after publication.

Tekstide `0.3.0` consolidates three milestones — M5 AgentRun launch, M6 transcript
and review foundations, and M7 durable audit — covering RFC-010 through RFC-013.
These milestones were developed sequentially but never separately released, so they
ship together here. It remains a headless core, not the full AI CLI workbench.

### Implemented

- RFC-010 AgentRun launch model and AI CLI profiles:
  - AI CLI profiles as reviewed launch contracts covering executable provenance,
    argv shape, compatibility level, cwd, environment, prompt, and transcript policy;
  - Restricted Mode rejection of workspace-local executables, wrappers, shims,
    symlink targets resolving into the project root, and project-local `PATH` entries;
  - implicit CLI workspace-config/tool/prompt/plugin discovery blocked or rejected
    before process start;
  - launch validation for project, root, cwd, profile source, environment, and
    compatibility before any process is created;
  - AgentRun launch through project-owned TerminalSessions, with lifecycle derived
    from runtime observation;
  - honest Plain/Supervised/Managed labels; Managed requires adapter capability evidence;
  - active-document dirty, external-change, conflict, and save-error states block
    launch before process start, and safe-save conflict blocking is preserved while
    AgentRuns are active.
- RFC-011 transcript retention and local data policy:
  - capture modes Disabled, LocalBounded, and RequiredLocalBounded;
  - default retention of 32 MiB per transcript, 256 MiB per project, 1 GiB app-wide,
    and 30 days, with aggregate accounting;
  - transcript paths resolved under Tekstide state, outside project roots, with
    symlinked state-root rejection;
  - bounded append-only writer with truncation state;
  - per-run opt-out before process start;
  - purge by transcript, AgentRun, and ProjectSession scope with content-free tombstones.
- RFC-012 generated change review foundations:
  - ChangeSet review model with detection source/status, association confidence,
    bounded content-free summaries, and validated review-state transitions;
  - filesystem baseline capture and metadata-only changed-path detection;
  - project-relative path validation rejecting absolute escapes, `..` traversal, and
    escaping symlinks; symlink entries recorded without following targets;
  - conservative AgentRun association — strong linkage requires a same-run baseline,
    a closed target run, and no overlapping run; ambiguous cases stay unlinked.
- RFC-013 durable audit store and local data policy:
  - versioned durable record with stable string codes and an exhaustive
    family/field validation matrix;
  - local SQLite store with CHECK constraints mirroring that matrix independently of
    Rust validation, transactional append, exact-retry idempotency, operation
    correlation, and phase cardinality;
  - bounded descending cursor queries;
  - schema identity, read-only probe before write-capable open, and a sequential
    migration harness with a statement allowlist;
  - explicit comprehensive diagnostics separate from the bounded startup probe;
  - corruption classification, exact-artifact quarantine with content-free manifests,
    atomic fresh-store installation, and restart-safe resume;
  - project and global purge with ephemeral receipts and local-data accounting;
  - security-event integration for trust grant/revoke, managed AgentRun lifecycle,
    and blocked root/symlink access.

Only three of the twelve audit-schema event families have a wired runtime producer
(trust decisions, managed AgentRun lifecycle, blocked root/symlink access). See
Deferred below and the security threat model's T-035 for why this distinction matters.

### Dependencies

- Added `rusqlite 0.39.0` with `default-features = false` and only `bundled` enabled,
  resolving `libsqlite3-sys 0.37.0` and bundled SQLite `3.51.3`. This is the first
  third-party native dependency; it compiles the SQLite C amalgamation during build.
  Third-party notices are recorded in `NOTICE`.

### Deferred

- Desktop GUI runtime, rendered terminal surface, and rendered paste/approval/trust
  dialogs.
- App/UI commands for launching, selecting, and closing terminals and AgentRuns.
- Command approval.
- Audit producers for command approval, terminal paste, restricted-feature blocks,
  safe-close and destructive decisions, sensitive configuration changes, transcript
  purge, project added, and plain-terminal lifecycle. These families exist in the
  audit schema but have no runtime producer. Wiring `paste_blocked` headlessly was
  considered for this release (RFC-009 already classifies paste without a GUI) and
  deferred to keep 0.3.0 reconciliation-only; it remains available for a future
  release alongside `project_added`, `plain_terminal_observation`, and
  `transcript_purge`.
- Git-based change detection; the RFC-012 detector reports Git as unavailable.
- File watcher, overwrite-confirmation UI, and multi-document conflict workflow.
- Cross-platform terminal, storage, and native build evidence beyond
  `x86_64-unknown-linux-gnu`.
- Encryption at rest, tamper-evident audit, secure deletion, and automatic retention.

### Release Gate Status

Completed on a clean, committed tree:

- `git status --short` clean; `git diff --check`;
- `cargo fmt --all --check`;
- `cargo test --workspace --all-targets --all-features` — 375 `tekstide-core` tests, 0 elsewhere, 0 failures;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked` (113 files, 872.3 KiB / 143.7 KiB compressed);
- `cargo publish --dry-run -p tekstide-core --locked`;
- `cargo package -p tekstide --locked` (6 files, 12.8 KiB / 4.7 KiB compressed);
- `cargo publish --dry-run -p tekstide --locked`;
- `cargo publish --workspace --dry-run --locked` (authoritative same-workspace pairing check; verifies `tekstide` against the local `tekstide-core 0.3.0`, not a stale registry version);
- package smoke test: `cargo build`/`cargo test` from the unpacked `tekstide-core-0.3.0` package artifact (not the working tree) — 375 tests passed;
- release tarball built via `git archive` at `tekstide-v0.3.0.tar`: no intermediate parent directory, `NOTICE` and `LICENSE` both at archive root, no `.git`/`.git-exclude`/`target`/local-agent-config paths present (249 entries).

Build-cost baseline (first captured; RFC-013 retained none, so there is no prior figure to compare against):

- Clean `cargo build --release --locked` on `x86_64-unknown-linux-gnu`, Rust 1.97.1: 27.3s wall-clock.
- `target/release/tekstide` binary size: 790,560 bytes unstripped, 605,368 bytes stripped.

Release-candidate review (request 104) found that both published READMEs undercounted wired audit producers (three instead of four — audit-store recovery was omitted) and that the ROADMAP M7 table row still listed producers the reconciled scope section had already moved to M8. Both were corrected in commit `1f5100b` before publishing; the gates above were re-run against the corrected tree and the release tarball and crate packages were rebuilt from that commit. The threat model's matching corrections live in `.git-exclude/specs/`, which is gitignored and carries no commit.

Post-publish verification on 2026-07-28:

- `cargo publish -p tekstide-core --locked` — published `tekstide-core 0.3.0` to crates.io.
- `cargo publish -p tekstide --locked` — published `tekstide 0.3.0` to crates.io, correctly resolved against the just-published `tekstide-core 0.3.0`.
- Tag `0.3.0` (signed) created at commit `1f5100b`, matching the `0.1.0`/`0.2.0` tagging convention.
- `crates.io` API confirms both `tekstide-core 0.3.0` and `tekstide 0.3.0` exist and are not yanked.

## 0.2.0 - Terminal Runtime Foundation

Status: released on 2026-07-17.

Tekstide `0.2.0` is scoped as an M4 terminal/runtime/security foundation release through RFC-009. It is not the full AI CLI workbench.

### Implemented

- RFC-007 Linux PTY feasibility evidence:
  - PTY-backed shell startup;
  - output capture/rendering in the spike harness;
  - scripted input;
  - resize observation;
  - foreground-child termination, timeout, and SIGKILL fallback observations;
  - output flood and latency evidence.
- RFC-008 TerminalSession/process lifecycle foundation:
  - project-owned Linux plain shell launch;
  - runtime boundary that keeps PTY/process handles out of persisted domain metadata;
  - bounded PTY output reads and dropped-byte accounting;
  - project-addressed input and resize routing;
  - process-group termination with SIGTERM, timeout, SIGKILL fallback, and honest unresolved cleanup outcomes;
  - ProjectSession terminal collection integration and visible-slot policy;
  - project close assessment for real running terminals.
- RFC-009 terminal security boundary:
  - conservative ANSI/VT/OSC parser/security boundary;
  - exact accepted and inert sequence-family policy;
  - inert/diagnostic OSC clipboard, title, hyperlink, host-integration, private-mode, query, reply, unsupported control, and invalid-byte behavior;
  - bounded diagnostics without raw private terminal output, OSC payloads, pasted text, shell output, or environment-like values;
  - typed-input vs paste-input classification before PTY write;
  - multiline paste confirmation decision before PTY write;
  - C0, DEL, and C1 control-containing paste blocking;
  - model-level trusted UI / terminal spoofing boundary;
  - honest Plain/Supervised/Managed labels without command-approval overclaim.

### Deferred

- Desktop GUI runtime and final terminal renderer.
- App/UI commands for launching, selecting, and closing terminals.
- App/UI paste-event wiring, rendered paste confirmation, paste queue, and replay behavior.
- Rendered trusted dialogs and screenshot-backed visual spoofing evidence.
- App-wide close aggregation.
- Cross-platform terminal runtime and GUI security evidence beyond Linux.
- AI CLI profile execution and AgentRun launch.
- Transcript capture, retention, purge, and review workflow.
- Durable audit storage.
- Command approval.
- File watcher and overwrite-confirmation UI.

### Release Gate Status

Completed before release:

- clean working tree;
- `git diff --check`;
- `cargo fmt --all --check`;
- `cargo test --workspace --all-targets`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked`;
- `cargo publish --dry-run -p tekstide-core --locked`;
- `cargo package -p tekstide --locked`;
- `cargo publish --dry-run -p tekstide --locked`;
- `cargo publish --workspace --dry-run --locked`;
- package smoke test from generated package artifacts;
- release-candidate review package accepted.

## 0.1.0 - Foundation Release

Status: released on 2026-07-06.

Tekstide `0.1.0` is scoped as a core/shell foundation release through RFC-006. It is not the full AI CLI workbench.

### Implemented

- Project Board and ProjectSession state.
- Core domain vocabulary for ProjectSession, TerminalSession, AgentRun, approvals, transcripts, change sets, and audit events.
- Navigation/mode policy for Project Board, Content Mode, and Terminal / Agent Immersion Mode.
- Restricted Mode policy/read-model baseline.
- Root-bound project file access policy.
- Bounded explorer read model.
- UTF-8 text document buffer.
- Safe save and external-change detection.
- Dirty-state propagation to project/runtime summaries.
- Shell-visible Content Mode evidence.

### Deferred

- Desktop GUI runtime.
- PTY terminal runtime.
- AI CLI profile execution and AgentRun launch.
- Transcript capture and review workflow.
- Generated diff/artifact review.
- Running-process safe close.
- Paste protection for real terminal input.
- File watcher.
- Overwrite-confirmation UI.
- Durable audit storage.
- Plugin marketplace, remote/container projects, debugger, cloud sync, and collaboration.

### Release Gate Status

Completed before release:

- clean working tree;
- `git diff --check`;
- `cargo fmt --check`;
- `cargo test --all-targets`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked`;
- `cargo package -p tekstide --locked`;
- package smoke test from generated package artifacts;
- release-candidate review package accepted;
- `tekstide-core` and `tekstide` published to crates.io.

### Future Work Themes

- Terminal/PTY runtime and process lifecycle.
- AgentRun launch and AI CLI profile execution.
- Transcript retention, review, and generated-change workflow.
- Durable audit storage and security evidence.
- Desktop GUI runtime and final Content Mode widgets.
- Release automation/checklist hardening.

See [`rfcs/future-work.md`](rfcs/future-work.md) for the durable deferred-theme index.
