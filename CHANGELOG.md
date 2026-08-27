# Changelog

## 0.15.0 - What The Terminal Started, And Who Could See It

Status: released on 2026-08-27.

**Security release.** Every shell this application launched inherited the PTY master file
descriptor of every other terminal open at that moment. A PTY master is read/write access to that
terminal, so code running in one project's terminal — including an AI CLI agent, which is what
this application exists to run — could read what another project's terminal displayed and inject
input into it. Fixed. Details and what to check are under **Security** below.

Also: closing a terminal now actually ends what that terminal started, a previewed file is
readable rather than one escaped line, and you can record a decision about a change set.

### Security — cross-terminal descriptor access (all releases through `0.14.0`)

- **What was wrong.** `openpty()` does not set close-on-exec, and nothing added it. At `exec`,
  every child inherited **every PTY master descriptor open in the parent** — not only its own. A
  live process was measured holding **27** of them.

- **What that allowed.** Anything running inside a Tekstide terminal could read the output of, and
  write input to, every other Tekstide terminal open when it started. No capability check, no
  audit record, nothing on screen. This crosses the terminal security boundary RFC-009 defines.

- **What it did not allow.** Nothing outside Tekstide's own terminals, and nothing across user
  accounts — the descriptors were only ever those of terminals this application itself opened in
  the same session.

- **The fix.** `FD_CLOEXEC` on the PTY master and slave at creation, plus a `close_range` sweep of
  everything above the child's own stdio before `exec`. **Either alone closes the hole** — proven
  by ablating each independently — so the second is a real net, not decoration.

- **What to check.** There is nothing to clean up on disk and no record to inspect; the exposure
  was between live processes. **If you ran code you would not fully trust — an AI CLI agent, a
  build script from an unfamiliar project — in a Tekstide terminal while another terminal was
  open, treat what was on those other terminals as potentially read.** In practice that means
  anything typed into them, secrets included. If you only ever ran your own commands, the exposure
  is theoretical. If you are unsure, `0.15.0` removes the possibility going forward.

### Fixed — closing a terminal now ends what that terminal started

- **`0.14.0` shipped this as a known limitation** — *"a backgrounded job survives closing the
  terminal that started it."* **That is no longer true**, and this is the correction to it.

  Closing signalled one process group. An interactive shell puts every `&` job in a process group
  of its own, so those jobs were never in the group being signalled. Termination now hangs up and
  re-scans the whole **session**: close the PTY master, `SIGHUP` the session leader so the shell
  hangs up its own jobs, escalate to `SIGKILL` for anything left, then re-enumerate to confirm.

  **The close confirmation says so before you click**: *"Anything started from these terminals ends
  too, including a backgrounded job."*

- **A process you deliberately detached still survives, by design.** `nohup`, `disown` and `setsid`
  work by leaving the session, and the session is exactly the boundary this fix respects. That is
  your opt-out and it is deliberate, not an oversight — it is also why this kills by session rather
  than by container.

  **If you relied on the old behaviour** — backgrounding a long build with `&` and closing the
  project expecting it to continue — **that no longer works, and `nohup` or `disown` is how to ask
  for it now.**

- **The safe-close audit record can now support a stronger claim, and does.**
  `terminal_process_groups_confirmed_empty` is renamed `terminal_session_confirmed_empty`, and it
  is no longer inferred from the termination outcome — it is read from a real, session-wide re-scan
  performed after the kill. `0.14.0`'s entry told you an `applied` outcome meant less than it
  looked like; it now means what it says, for everything inside the session. A process that left
  the session remains outside the claim, deliberately, and the field's own documentation says so.

### Fixed — a previewed file is readable

- **`0.14.0` shipped the content preview escaping every line break**, so a real source file
  rendered as one continuous run of text with `<U+000A>` between every line. Lines are lines now.

  Every other control character is still escaped — tab, carriage return, ANSI sequences, bidi
  overrides — and file content renders inside its own bordered container, kept in a type the rest
  of the surface cannot draw. A file whose first line reads `Review state: Accepted` cannot
  impersonate the interface around it.

- **The "not a diff" label can no longer be scrolled away from what it labels.** The framing text
  and the file list sit outside the content's own scroll region.

- **Content over 4,000 lines is refused whole rather than truncated**, matching the existing byte
  bound. A partial preview you cannot tell is partial is the trap this surface exists to avoid.

### Added — record a decision about a change set

- **Mark accepted / Mark rejected**, reachable by mouse or by `a` / `r`. The first time anything in
  this application could change a change set's review state.

- **It is a note, and it says so**: *"Marking this here only records your own note about it: it
  changes no file, cannot be undone, and disappears when you close Tekstide."* All three are true
  and all three would otherwise surprise you. Nothing is reverted, staged, or applied — there is no
  before-side to revert to.

- **No audit record is written for a review decision.** Said plainly so the audit store's silence
  is never read as evidence that no decision was made.

### Fixed — a keyboard-only user can close a project, and every surface-local key is advertised

- **This release's own gate found that a keyboard-only user could not close a project** — the `×`
  on a project tab was the only thing that emitted a close, and no key reached it, so a
  keyboard-only user could not reach the termination behaviour this release is largely about.
  **That is no longer true.** `Delete`, with a project's own tab highlighted, closes it — the same
  `attempt_close_project_tab` the `×` button already reached, so both routes converge on the same
  confirmation dialog for a project with live work. RFC-044.

- **`a`/`r` here, and `Enter`/`Space`/`Delete`/arrows elsewhere, appeared in neither the Help modal
  nor `--help`.** They do now: a surface-grouped section, generated from the same registry that
  drives the fix above, lists every surface-local key by the surface it belongs to. RFC-044.

### Known limitations, unchanged

- **No screen-reader support.** `iced` has no accessibility bridge; out of scope for that reason
  and no other.
- **Still no two-sided diff.** The before-bytes were never captured; blocked on Git-backed
  detection.
- **A review decision does not survive closing Tekstide.**

## 0.14.0 - What It Changed, And What This Cannot Tell You

Status: released on 2026-08-26.

**`0.13.0` gave the product a change review surface that could not show you a single line of a
file.** It listed paths, counts and detection status and said so plainly. This release fills that
in — and, in doing so, found two places where change review had been quietly telling users less
than it knew, and one place where the audit trail claimed more than it could see.

### Added — see the content of a changed file

- **A change review file row is now a real button.** Selecting one renders that file's content
  according to what kind of change it was: the whole content for an added file, the current
  content for a modified one, and nothing at all for a deleted one.

  This machinery — gated, bounded, size-capped, reviewed — has existed in `tekstide-core` since
  `0.7.0` and had **zero production callers for six releases**. Nothing needed to be designed
  here; something needed to call it. What was actually missing was the detection result, which
  the code path that creates a change set discarded the moment it was done with it.

- **A modified file's content is labelled "not a diff", in the words the limitation deserves.**
  The surface says: *this is the file's current content, not a diff; it cannot show what changed,
  only what the file looks like now; whether the agent's own edit is still there, was reverted, or
  was overwritten by something else since, this screen cannot tell you.*

  There is no before/after comparison because the before-bytes were never captured. That is
  blocked on Git-backed detection, not cancelled, and the surface says which.

- **A stale baseline refuses, and names why.** If the file has moved on since the change was
  detected, you get a refusal that explains itself rather than content that silently describes a
  different moment.

- **Every line break in the previewed content is shown escaped, as `<U+000A>`, so a multi-line
  file renders as one continuous run of text rather than as lines.** File content is untrusted
  text drawn in trusted chrome — the same rule that escapes a project name or a path — and
  escaping control characters is what stops a file from forging the interface around it. The
  consequence was not designed, though: previewing a 300-line source file gives you a wrapped
  block with `<U+000A>` between every line, not something you would want to read. **The preview is
  honest but it is not yet readable for real source files**, and the surface itself does not say
  so. Found by running the release binary against a four-line file during this release's own
  gate. Preserving line structure while keeping every other character escaped is the obvious next
  step and has not been designed yet; it is a security decision, not a formatting one.

### Fixed — change detection was blind to git hooks

- **`.git/hooks/` and `.git/config` are now watched.** Change detection excludes `.git/`,
  `target/` and `node_modules/` — a deliberate default, and a necessary one: walking this repository in
  full reaches **64,415** entries and **1,506** after those three exclusions. But it meant an agent that installed a
  `.git/hooks/pre-commit`, or redirected `core.hooksPath`, changed something that would execute on
  your next commit and **never appeared in change review at all**. The file explorer collapses
  `.git/` too, so there was no second route by which you would have noticed.

  Everything else under `.git/` — `refs/`, `objects/`, `index` — is still skipped exactly as
  before. A `core.hooksPath` redirect is **not** followed to its target; watching `.git/config`
  reports that the hook location changed, which is the fact that matters, and resolving the
  redirect is separate work that has not been done.

  **If you used the change review surface on `0.13.0` — the only release where it was visible —
  and read it as what an agent run touched, that answer excluded git hooks.** Detection itself has
  had this blind spot since `0.11.0`, when it was first wired. Check `.git/hooks/` and
  `.git/config` in those projects directly: `ls -la .git/hooks/` and `git config --list --local`.
  Nothing on your disk was changed by this release; what changed is that these paths are now
  reported.

- **A large change set no longer reports as no change set at all.** Detection caps the changed-path
  list at 4,096 entries. Over that cap it used to discard the entire list and mark the scan
  incomplete — and an incomplete scan produces **no change set**, which on the surface is
  indistinguishable from "the agent changed nothing." An agent run that touched more than 4,096
  files therefore showed you nothing, silently. It now keeps the first 4,096 and reports the
  remainder as a count.

  **If an agent run showed no change review entry on `0.13.0`, that was not proof it changed
  nothing** — a run over the cap looked identical, and the missing change set was never built at
  all on any release from `0.11.0` on. There is no record to recover; re-check those projects with
  `git status` if it matters.

- **Two kinds of "not shown" are now two separate lines, never one number.** Files omitted because
  the surface showed a shorter list are **recoverable** — they are still in the change set. Files
  omitted because detection capped its own scan are **not** — they were dropped before the change
  set existed, and no larger limit brings them back. Summing them into a single count meant a
  reader could not tell which they were looking at, on a surface whose whole job is deciding
  whether to trust what an agent did.

### Fixed — an audit record that claimed more than it could see

- **`fully_confirmed` on a safe-close audit decision was renamed to
  `terminal_process_groups_confirmed_empty`, because that is all it ever knew.** When you close a
  project with live terminals, Tekstide signals the process group it launched and confirms that
  group is empty. A job you backgrounded inside that terminal (`sleep 30 &`, a build left running)
  gets its **own** process group from the shell, which that signal never reaches and that check
  never looks at. So a durable audit record could say a close was fully confirmed while a process
  that terminal started was still running.

  **No behaviour changed. The predicate is bit-for-bit identical**, and every `applied`/`failed`
  outcome this project has ever written is unaffected — the field was never itself persisted. What
  changed is that the name no longer asserts something the check cannot establish, and the doc
  comment now states both halves: what `true` means, and what it does not.

  **If you closed a project on `0.13.0` and read an `applied` safe-close outcome as "everything
  that terminal started is gone", that was more than the record could support.** Check with `ps`
  or `pgrep` for anything you backgrounded. The audit store is at
  `$XDG_STATE_HOME/tekstide/audit/audit.sqlite3` (`~/.local/state/tekstide/audit/audit.sqlite3` if
  unset); no record in it is rewritten or invalidated by this release, and nothing needs removing.

  Killing sibling process groups a terminal spawned is a **product decision that has not been
  made** — a real terminal emulator often leaves them running on purpose, which is `nohup`'s entire
  reason to exist. It is recorded as open work, not silently pending.

### Documentation

- `crates/tekstide-core/README.md` claimed safe-close audit producers were "defined in the audit
  schema but not yet wired." They were wired in `0.13.0`. Corrected, and split from
  configuration-change producers, which genuinely still have no caller. Found by the release
  gate's own instruction to check **every** README a published crate names, not only the workspace
  root — the third time that instruction has caught something on that specific page.

### Known limitations, unchanged by this release

- **Still no two-sided diff.** Blocked on Git-backed detection (RFC-030), which does not exist.
- **Absence of visible change is not absence of change.** The content preview shows a file as it
  is now, not as the agent left it.
- **Previewed content is not laid out as lines** — see the escaping note above.
- **`core.hooksPath` redirects are reported, not followed.**
- **A backgrounded job survives closing the terminal that started it**, and Tekstide does not yet
  say so on the close confirmation.
- **No screen-reader support.** `iced` has no accessibility bridge; this remains out of scope for
  that reason and no other.

## 0.13.0 - Something To Do, And A Way To Do It

Status: released on 2026-08-25.

**`0.12.1` made the product describe itself. It still could not be used.** A user starting
`tekstide` with no argument saw an honest empty board, a full keyboard reference, and no way to
open a project without leaving the window. Five keybindings were live and every one of them was
the *only* route to what it did.

This release is three RFCs' worth of answer to that: you can open a project, move between
projects, close one, see what an agent changed — and do nearly all of it with a mouse, because
eleven of the thirteen things this application can do now have a control you can see.

### Added — opening and moving between projects

- **A path field on the Project Board**, focused the moment the board is empty, and reachable on
  a populated board via `Ctrl+Alt+O`. A failed path renders a bounded, escaped diagnostic and
  leaves the field editable; it never exits.
- **A folder browser** (`Ctrl+Alt+B`, or the **Browse…** button) — choose a project by navigating
  the filesystem rather than typing a path. `Enter` moves into a folder, `Space` chooses the one
  shown.
- **Reopen a remembered project with one key.** The board already listed recent projects; now
  `Enter` on a highlighted row, or its own **Open** button, reopens it.
- **A project tab strip** in the top bar: which projects are open, which is active, a permanent
  **Projects** tab back to the board, and `×` to close one. `Ctrl+Alt+N` cycles.
- **Closing a project** — the first time this has been possible from the application at all. A
  project with nothing running closes immediately. One with live terminals or an agent run raises
  a confirmation naming **what will be lost** by count, and identifying the project by its
  **canonical path** rather than its display name, so two similarly-named projects cannot be
  confused at the moment it matters.

### Added — seeing what an agent run changed

- **A change review surface** (`Ctrl+Alt+D`, or **Change Review** in Trust Settings). Since
  `0.11.0` this product has detected what an agent run changed and been unable to show it.

  **It renders metadata, not a diff**: file paths, counts, detection status, review state. It
  says so on the surface itself, along with the limits that matter more than the feature —
  detection is conservative and excludes `.git/`, `target/` and `node_modules/`, so a change an
  agent makes in a git hook is not reported. A *truncated scan* and *nothing changed* are
  rendered as different facts, because they are.

### Added — you can use it with a mouse

- **Eleven of thirteen actions now have a visible control**, up from three. Launching a terminal,
  switching modes, saving a file, starting an AI CLI run, opening Trust Settings, Help, the
  AgentRun report, Approval History — all had keyboard-only routes and now have buttons where the
  action applies.
- **Every dialog can be finished or abandoned with a mouse.** Previously all nine were
  keyboard-only for their own decision — several of them *opened* by a button, so a user arrived
  with a mouse and was stranded.
- **A Help modal** (`Ctrl+Alt+K`) reachable from anywhere, including from inside Terminal
  Immersion, where no route to any help existed.

The two actions that remain keyboard-only are deliberate and recorded as such, not overlooked.

### Fixed (security)

- **A project's cached trust label could be restored without confirmation against the audit
  store.** A user who granted a project trust, closed it, and later reopened it by any route
  other than the original command line — retyping the path, browsing to it, or the new one-key
  reopen — got `Trusted` back on the strength of a **user-writable cache file alone**. Fixed at
  all three routes; each now re-confirms against the audit store the way a command-line-opened
  project already did.

### Fixed

- **Tekstide could leak shell processes.** A panicking terminal left its shell running with
  nothing owning it; at scale this exhausted the system's pseudo-terminal pool. Terminals now
  terminate their process group when dropped. **A related gap remains and is disclosed below.**
- **`close_project` could never report a project safe to close** — its resource assessment had no
  way to reach a "known" state, so the capability was unreachable even once wired.
- Two duplicated rows in the README's keyboard reference.

### Breaking changes

- **`ProjectBoardEmptyState`'s `primary_action` and `secondary_action` fields are removed** from
  `tekstide-core`'s public API. They held pre-baked English for two actions — "Add Project",
  "Open from path" — that were never reachable from anywhere. `0.12.1` stopped rendering them;
  this release removes the fields. Any external caller constructing or reading them will not
  compile.

### Known limitations

- **No diff content.** The change review surface shows which files changed, not what changed in
  them. Reading diff content is a separate, unbuilt piece of work.
- **A backgrounded job can outlive the terminal that launched it.** Closing a project terminates
  each terminal's process group — but a job the user backgrounded inside a shell gets its own
  group, so it survives, and the close is recorded as confirmed. Bounded and disclosed rather
  than fixed: what "closing a terminal" *should* mean for a job someone deliberately backgrounded
  is a product decision, not a mechanical one.
- **Two actions remain keyboard-only** by deliberate convention: pasting into a terminal, and the
  path field's own shortcut, whose workflow has a visible route through **Browse…**.
- **A change set produced by a real agent run has never been observed live** — only proven end to
  end in test, and seen on screen via a seeded change.
- Unchanged from `0.12.0`: the configuration system loads nothing, so writing a config file has
  no effect; no screen-reader support; no cross-platform evidence beyond Linux; the real Claude
  Code CLI still never exercised by the test suite; `NFR-PERF-004` still unverified.

## 0.12.1 - You Can See What It Does

Status: released on 2026-08-22.

**`0.12.0` shipped a window that told a new user nothing.** Started with no argument — which
this project's own Quick Start told people to do — it showed an empty Project Board naming two
actions that do not exist, while the nine keyboard shortcuts that do exist were named nowhere
in the running application. This release makes the product describe itself. It does not yet
add the missing action; see *Known limitations*.

### Fixed

- **The Project Board's empty state named two actions that do not exist.** It rendered
  **"Add Project"** and **"Open from path"** as plain text — no button, no handler, nothing
  behind either — from the day the surface landed. Both are gone. The empty state now says how
  a project is actually opened (`tekstide /path/to/project`), which is the truth, and the keys
  in the catalogue naming those two actions are deleted so no surface can render them again.

- **The running application named no keyboard shortcut anywhere.** The string `Ctrl` appeared
  **zero times** in the entire user-facing catalogue while nine bindings were live, so every
  capability the product had was reachable only by a user who had read `navigation.rs` or the
  repository README. The Project Board now lists every binding and what it needs — in both its
  empty and populated states, because help that disappears when you open your first project is
  not help. The status bar carries a standing pointer back to the board.

  The list is **derived from `KeybindingPolicy`**, not written: it cannot drift from the
  bindings the input layer dispatches on, and it cannot advertise the five actions that have
  no working binding. Four of those are `Configurable` with no key — dead rather than pending
  — and one is the reserved command-palette binding with nothing behind it.

- **`tekstide --help` printed `folder does not exist: --help`.** Every argument was treated as
  a project path, so the only documented entry point rejected the universal request for
  documentation. `-h`, `--help`, `-V` and `--version` now work, and `--help` prints usage plus
  the same derived keyboard list. An unrecognised `-`-prefixed argument is still treated as a
  path, deliberately: a real file can begin with a dash.

- **The published Quick Start led users into the dead end.** It said to run `tekstide` bare and
  mentioned passing a path second, as an option, when a path is the only way to put a project
  on the board. Corrected to lead with the path and to state the limitation.

### Known limitations

- **There is still no in-app way to add a project.** A path on the command line remains the
  only route; `tekstide` with no argument opens an empty board that now explains itself instead
  of misleading you. **RFC-038 is the fix** and is proposed, not scheduled — this release is a
  correction, and correcting a product that misrepresents itself came first.
- **A user inside Terminal Immersion still needs to know `Ctrl+Alt+P`** to reach the board
  where the keyboard list lives. A help surface independent of the board is RFC-038's Goal 4.
- **`--help`'s framing sentences are English only.** The shortcut descriptions are
  catalog-driven and localize; the surrounding usage text does not, because argument parsing
  happens before the catalog is resolved. Recorded in RFC-038 rather than left as a surprise.
- **`ProjectBoardEmptyState`'s `primary_action` / `secondary_action` fields still exist** in
  `tekstide-core`'s public API, still holding the pre-baked English for the two actions that do
  not exist. Nothing reads them. Removing them is a breaking change and belongs to RFC-038.
- Unchanged from `0.12.0`: the configuration system loads nothing, no screen-reader support, no
  cross-platform evidence beyond Linux, the real Claude Code CLI still never exercised by the
  test suite, and `NFR-PERF-004` still unverified.

## 0.12.0 - Your Transcripts, and What the Agent Did

Status: released on 2026-08-22.

`0.11.1` was a documentation-only release that corrected a false privacy claim and had to
publish an admission with it: transcripts are written for every AI CLI run, **and there was
no in-app way to stop it or delete them**. This release removes that sentence by fixing the
thing it described. It also makes an agent run's transcript readable inside the application
for the first time — until now it was recorded and never shown.

### Added

- **Per-project transcript capture opt-out, purge, and retained-size visibility**, all on the
  Trust Settings surface (`Ctrl+Alt+U`). You can decline capture for a project before running
  anything, see how many transcripts are retained and how many bytes they occupy, and purge
  them permanently through a confirmation dialog. The decision persists across sessions.
  Declining affects **future** runs; it does not delete what is already there, which is what
  purge is for.

  The retained-size figure is read from the files themselves at display time, not from a
  counter incremented as data is written. That is deliberate: a counter would read `0` for
  every transcript written before the counter existed, so the dialog would have understated
  exactly the data a user opting out most wants to find. The figure shown and the bytes
  deleted now come from the same source.

- **The AgentRun report surface**, with `Ctrl+Alt+R`: the transcript of the current project's
  most recent agent run, rendered in the application. It states whether the run is still
  active — in which case the transcript may still be growing — and it distinguishes two
  different kinds of "you are not seeing everything": the *window* (only the most recent
  bytes are shown, and it says which bytes) and *writer truncation* (the run produced more
  than storage kept, and that data is gone). Those are different facts and it does not
  collapse them. Transcript content is treated as untrusted and escaped.

- **Two more audit families have real producers**: `restricted_mode_blocked`, recorded when
  Restricted mode refuses a launch, and `project_added`, recorded when a project is added to
  the board. Purging transcripts records `transcript_purge`. The audit store still has **no
  surface** — nothing in the application renders it — so this improves the record on disk,
  not what you can see.

### Fixed

- **The modal scrim failed WCAG 2.1 SC 1.4.11 at its worst case.** `0.11.0`'s new contrast
  gate measured the pairs it was given; this cycle measured the *derived* pair nobody had —
  the modal card's own border against the scrim, with bright terminal content behind it —
  and found **2.40:1** where 3:1 is required. The scrim is now 0.75 alpha, measuring 3.62:1
  at that same worst case, and remains visibly translucent.

  The interesting part is why it was missed twice. Contrast of a translucent layer is not
  monotonic in the backdrop: the worst case sits at neither the darkest nor the brightest
  backdrop but at a crossing point in between, so **sampling a few plausible backdrops finds
  a passing ratio and stops.** It is now swept rather than sampled, and adding a new theme
  role that participates in a translucent pair cannot leave it unmeasured.

- **Restricted mode overstated what it blocks.** The board reported ten blocked automations
  when nine are actually enforced: the count was taken from the full `RestrictedModeFeature`
  vocabulary, which had grown a tenth variant reserved for a capability nothing enforces yet.
  A security surface that overstates its own protection is the wrong direction to be wrong
  in, so the count is now taken from what is enforced.

- **Purging transcripts no longer depends on the audit store opening.** Deletion was reached
  through the code path that records it, so a project whose audit store failed to open would
  have had its purge silently do nothing — failing closed on the audit record, but also on
  the deletion the user asked for. Deletion now happens on both paths; only the record is
  conditional.

- **Both published crates now ship `LICENSE` and `NOTICE`.** They live at the workspace root,
  which cargo does not include when packaging a crate in a subdirectory, so every release
  from `0.1.0` to `0.11.1` published without the Apache-2.0 licence text or the NOTICE —
  Apache-2.0 §4 requires both to travel with the distribution. The NOTICE also carries
  rusqlite's MIT terms and SQLite's public-domain notice, so third-party attribution was
  missing too. **If you are redistributing an earlier version, take the two files from the
  repository root**; the licensing itself is unchanged and always was Apache-2.0.

- A test-harness defect that leaked real child processes when a test panicked. No effect on
  the shipped application.

### Known limitations

- **Purge does not erase every trace.** The transcript files are deleted; a tombstone marking
  that the run had a transcript remains in project state, and the `transcript_purge` audit
  record remains in the audit store by design — an audit trail you can erase from inside the
  application is not an audit trail. Stated here rather than left for someone to discover.
- **The retained-bytes figure can understate**, never overstate: a file whose size cannot be
  read is counted as zero rather than guessed at.
- **The AgentRun report shows the most recent run only.** There is no run history, and no way
  to open a specific earlier run.
- **There is still no way to review generated changes.** `0.11.0` made change detection real;
  rendering it remains unbuilt (RFC-020 PR-020-C), so a detected change set is still
  something the product knows and cannot show you.
- **The audit store remains unviewable** from the application.
- Unchanged: no screen-reader support, no cross-platform evidence beyond Linux, the real
  Claude Code CLI still never exercised by the test suite, and `NFR-PERF-004` still
  unverified.

### Also in this cycle

- **A configuration system was implemented, and a configuration file still does nothing.**
  The file format, search precedence, atomic validation, bounded diagnostics, the rule that
  security-sensitive settings do not take effect on hot reload, and configuration-defined AI
  CLI profiles routed through the same launch validation as the built-in one are all built
  and tested (RFC-023). **Nothing in the application loads them** — no code constructs a
  configuration store, so writing a config file has no effect on the running product. This is
  named here explicitly because "configuration system" in a changelog reasonably reads as
  "you can configure it now," and you cannot. It is groundwork, and the slice that makes it
  reachable is not yet scheduled.
- The RFC process moved to a five-folder lifecycle, and RFC-023, RFC-031, RFC-033 and
  RFC-037 closed.

## 0.11.1 - Transcript Disclosure Correction

Status: released on 2026-08-18.

**A documentation-only release, published for one reason: the privacy claim it corrects is
wrong on the page people read.** crates.io serves the README of the published version, so
correcting it in the repository changes nothing for a user until a release carries it. No
code changed.

### Corrections

- **Tekstide records AI CLI session transcripts to disk, and `0.10.0` and `0.11.0` both said
  it did not.** Both releases' READMEs stated that transcript retention was "not wired into
  the desktop application" and that "no transcript is ever written." That was false from
  `0.10.0` onward, the release in which launching an agent run first became possible.

  What actually happens: pressing `Ctrl+Alt+A` in a trusted project writes that session's
  terminal output to
  `$XDG_STATE_HOME/tekstide/transcripts/<project>/<agent-run>/transcript.log`, bounded by
  RFC-011's policy — 32 MiB per transcript, 256 MiB per project, 1 GiB application-wide, 30
  days. There is no in-app way to disable capture or purge it; deleting the `transcripts/`
  directory is the only route today. Plain terminals (`Ctrl+Alt+T`) are not recorded.

  **The behaviour is intended** — RFC-011 designed capture, its bounds, and its purge policy
  deliberately. **The documentation was wrong**, and the reason is worth recording: the claim
  was verified by searching the `tekstide` crate only, where nothing configures a transcript
  writer. `tekstide-core` configures one on that crate's behalf, so a true premise carried a
  false conclusion into two releases. No test asserted transcript behaviour on the real launch
  path, so the suite could not contradict it.

  **What to do if you relied on the old claim** (added 2026-08-19; this entry originally
  described the correction without saying what a reader who had acted on it should check).
  **If you ran an AI CLI session on `0.10.0` or `0.11.0` believing nothing was recorded, those
  transcripts are on your disk now.** They are at
  `$XDG_STATE_HOME/tekstide/transcripts/` — `~/.local/state/tekstide/transcripts/` if
  `XDG_STATE_HOME` is unset. Delete that directory if you did not want them. Nothing was
  transmitted anywhere; this is local data only.

  `README.md`'s *Local Data and Privacy* section now describes what is written, where, under
  what bounds, and what has no user-facing control. Called out here rather than corrected
  quietly, because it is a privacy claim users may have relied on.


## 0.11.0 - What the Agent Changed

Status: released on 2026-08-18.

`0.10.0` made an agent run launchable. This release makes what it did **detectable** — and
fixes an accessibility defect found by measuring something nobody had measured before.

### Added

- **Change detection for agent runs.** Launching a run captures a filesystem baseline of the
  project **before the agent's process is spawned**, and when that run's terminal exits the
  two are compared, producing a real change set naming the files the run actually touched.
  Four capabilities that had been implemented, reviewed and dormant since RFC-012 —
  `capture_agent_run_filesystem_baseline`, `detect_filesystem_changes`,
  `add_detected_generated_change_set`, and `apply_agent_terminal_outcome` — have production
  callers for the first time.

  **This does not give you a way to look at the result.** There is still no diff review or
  AgentRun report surface. What changed is that the change set those surfaces need now
  exists rather than being structurally impossible: diff review is **buildable, not
  reachable**.

- **The approval-history surface opens**, with `Ctrl+Alt+H`. It was built and tested in
  `0.10.0` and had no key bound to it, so nothing could open it. This makes the *surface*
  reachable, not command approval itself — no shipping AI CLI speaks the protocol, so a real
  user will see the surface empty, which is correct rather than a bug.

- **A WCAG contrast gate over the theme**, asserting real ratios: 4.5:1 for text pairs, 3:1
  for non-text UI boundaries, and the translucent modal scrim composited over its backdrop
  before being measured. The suite it replaces checked that colour channels were within
  `0.0..=1.0` — a bound no plausible colour can fail.

### Fixed

- **Unfocused pane borders failed WCAG 2.1 SC 1.4.11.** They measured **2.63:1** against the
  background and 2.37:1 against elevated surfaces, below the 3:1 required for the visual
  boundary of a UI component; they are now 3.85:1 and 3.48:1. Text contrast was never the
  problem — it sits above 14:1 — and focus indication was unaffected, since the focused
  border is the accent at 6.38:1 and carries a second, non-colour channel. The gate above was
  written first and **observed failing at those exact ratios** before the colour changed.

### Known limitations

- **Change detection excludes `.git/`, `target/` and `node_modules/`** by design; build
  output and VCS metadata would otherwise drown the result and exceed the scan limit on any
  real project. The consequence is stated rather than left implied: **a change an agent makes
  inside those directories is not reported**, git hooks included.
- **Detection runs only when a run's terminal exits.** A long-lived interactive session
  reports nothing until it ends.
- **The baseline is held in memory.** If the application closes while a run is live, that
  run produces no change set — indistinguishable, from outside, from an agent that changed
  nothing.
- **A truncated scan is recorded as truncated, not as "no changes."** Those are different
  facts and the product does not collapse them — but no surface renders either yet.
- Unchanged from `0.10.0`: no screen-reader support, no cross-platform evidence beyond
  Linux, the real Claude Code CLI still never exercised by the test suite, and
  `NFR-PERF-004` still unverified.

### Also in this cycle

- An evaluation of the `snora` GUI framework at the owner's request, declined as a
  dependency with the reasoning recorded. It is what prompted measuring our own contrast in
  the first place, and the resulting exchange found defects in both projects.
- The `0.3.0` git tag, which had pointed into an orphaned line of history since a trailer
  rewrite, was re-pointed at its content-identical replacement; `0.4.0` and `0.4.1`, which
  had never been pushed, now exist on the remote.


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
