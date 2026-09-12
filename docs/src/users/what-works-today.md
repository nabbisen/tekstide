# What works today

This chapter states what the product does and what it does not, as of the version you are
reading. It deliberately says nothing about **when** each part arrived — the
[changelog](../record/changelog.md) owns that, per release, and the
[delivery plan](../contributors/delivery-plan.md) owns what is scheduled next. A running
release-by-release narrative in a third place is a copy that drifts, and this project has had
documentation assert things the code had stopped doing more than once.

It is not yet the full AI CLI workbench. Read the limits as closely as the capabilities; they are
here because they are true, not as hedging.

## Projects, files, and editing

Several projects can be open at once, each with its own tab — a filled dot marks the active one,
and a permanent **Projects** tab returns to the board. Each project gets a bounded directory
scan, root-bound file access so nothing outside the project folder is reachable, UTF-8 text
buffers, and a save that will not silently overwrite a file that changed on disk: a dialog offers
to reload, every dismissal leaves the disk file untouched, and it only claims local changes will
be lost when there are some.

The explorer is **read-only** — no rename, delete, or create.

**The editor has no undo.** A mid-buffer edit is unrecoverable within the session past what
`Backspace` can still reach. There is no syntax highlighting, language server, multi-cursor, or
search, and files above 4 MiB are not editable.

File **names** in the explorer and the editor header are escaped, because they are untrusted,
attacker-influenced text. File **contents** are deliberately not: the editor shows a file as it
is, which means source containing a bidi-override character still *reads* differently from how it
compiles.

## Terminals

A project-owned Linux PTY terminal opens with `Ctrl+Alt+T`: a real session rendering real output,
through a conservative output-security policy, with bounded IO, resize that follows a live window
drag, and exit detection — so the session bar reflects what is actually running rather than what
was last launched. Up to **six** concurrent terminals per project.

Clipboard paste (`Ctrl+Shift+V`) goes through the same policy as everything else that reaches a
PTY. Single-line and empty pastes go through and control-containing pastes are blocked outright,
both without a dialog. A multi-line paste opens a real confirmation dialog showing an escaped
preview; accepting is the only thing that writes it, and every other way out leaves the terminal
untouched.

**Terminal input latency is not verified against its 16 ms p95 target.** The structural cause of
the previous failure — a fixed polling interval — is gone, and the terminal now wakes on PTY
readiness, but removing a known cause is not the same as measuring the result. The criterion is
recorded as still unmet rather than assumed fixed.

## AI CLI runs

With workspace trust granted, `Ctrl+Alt+A` launches an AI CLI run in a project-owned terminal,
with honest `Plain` / `Supervised` / `Managed` labels and active-file safety checked before the
process starts. AI CLI profiles are reviewed launch contracts; Restricted Mode blocks
workspace-local executables, wrappers, project-local `PATH`, and implicit CLI workspace-config
discovery.

**The real Claude Code CLI has never been exercised by this project's tests.** Every automated
proof uses a controlled test executable, because the live product needs interactive
authentication and makes real network calls. The launch pathway is proven end to end against
production code; the specific behaviour of the real binary under it is not.

### What a run said

`Ctrl+Alt+R` opens the AgentRun report for the most recently launched run: real transcript
content, escaped at render, so a directionality override in what the AI CLI printed shows as a
visible marker rather than an invisible reordering. Unlike change detection, this is reachable
**while a run is still active**, and the surface says so.

The window is a bounded tail (1 MiB) of the transcript, not the whole thing. If the transcript hit
its own retention limit, that is shown as a separate, independent fact from "this is only a
partial view" — conflating the two was the specific failure this surface was built not to repeat.

### What a run changed

Launching a run captures a filesystem baseline **before the agent's process starts**; when the
run's terminal exits, the two are compared into a change set naming the files the run actually
touched. `Ctrl+Alt+D` opens Change Review: the files, a count, a detection-status line, and a
review state.

Four limits are deliberate and disclosed:

- **No two-sided diff.** Clicking a file shows whole content for an added file, and **current
  content, explicitly labelled not a diff**, for a modified one. There is no before/after
  comparison, because the before-bytes for a modified file were never captured and are gone by
  request time. A real diff is blocked on a real before-source, which only Git integration could
  provide, and this surface will not approximate one.
- **`target/`, `node_modules/` and most of `.git/` are excluded**, so a change made inside them
  is never reported. The exceptions are `.git/hooks/` and `.git/config`, watched like any other
  path precisely because they are the two places a change could install or redirect code that
  runs on this machine. A `core.hooksPath` redirect is not followed to wherever it points;
  watching `.git/config` already reports that the hook location changed.
- **Detection runs only at exit**, so a long-lived interactive session reports nothing until it
  ends, and the baseline lives in memory, so it does not survive the application closing mid-run.
- A scan that hits its entry limit is recorded as **truncated**, never as "nothing changed" —
  those are different facts and the product does not collapse them.

A preview is bounded at 4 MiB and 4,000 lines, and refused whole above either, never truncated.
Previewed content renders with its real line structure — every character other than the line
break is still escaped — inside its own bordered container, so a file cannot forge a line reading
"Review state: Accepted". The frame, including the "not a diff" label, is pinned outside the
scroll region so the content cannot scroll it away.

## Command approval, and why you cannot use it

Tekstide implements a command-approval protocol a cooperating AI CLI adapter can use: a versioned
sideband channel over a per-run Unix domain socket, two-layer peer authentication, a structural
risk classifier, and single-use decisions recorded in the durable audit trail. It is built,
audited, and proven end to end against production code.

**No shipping AI CLI speaks this protocol.** `Managed` mode, and therefore command approval, can
only be exercised by this project's own reference adapter, which is a test artifact. The pathway
is proven; the ecosystem does not exist. Anyone reading "command approval shipped" into this is
reading more than the record says.

`Ctrl+Alt+H` opens the approval-history surface, and a real user opening it today sees it
**empty** — correctly, not as a bug.

**It is cooperative, not enforced.** Approval works only if the adapter asks. Tekstide does not
intercept process execution and has no execution path of its own to withhold, so an adapter that
ignores a rejection — or never submits a proposal — runs its command regardless. Tekstide does not
approve commands, and does not control what an AI CLI can run.

## Dialogs you can trust, and how far that goes

The modal layer is real for the paste-confirmation, file-changed-on-disk, workspace-trust,
transcript-purge, command-approval, and project-close dialogs. `Esc` dismisses, `Enter` activates
the focused choice, and every choice also has a real clickable button routed through the same code
the keyboard handling uses, not a second parallel path.

Two checkable properties distinguish the real paste dialog from terminal output imitating it.
Keystrokes typed while it is open never reach the terminal, verified with a positive control
proving they were reaching the app. And the window dims behind the dialog, covering chrome the
terminal grid can never draw into — unlike the dialog's own size, which depends on the pasted
content and is therefore attacker-influenced, the dimmed area is fixed by the window, so the same
tell holds for a one-byte paste and a large one alike.

**Neither property makes the dialog unspoofable**; they raise the cost of a convincing imitation.
Nothing here claims an untrained user would notice either property unprompted.

## Accessibility

The shell is keyboard-navigable by design, with a visible focus indicator that is not
colour-only, and the theme passes a WCAG contrast gate whose checked pairs are derived from an
exhaustive destructure of the theme rather than hand-written — a new colour role cannot be added
without being classified, or the crate fails to compile.

**There is no screen-reader support.** Not limited, not planned: absent for the life of the
`iced` substrate decision.

## Audit

A durable local SQLite audit store records trust decisions, managed AgentRun lifecycle, blocked
root/symlink access, audit-store recovery outcomes, plain-terminal session starts and
terminations, paste refusals, command-approval decisions, restricted-feature refusals,
project-added opens, safe-close decisions, and configuration changes. **All twelve event families
have a real producer.**

What each family can and cannot contain is in
[Local data and privacy](./local-data-and-privacy.md), which is the chapter to read if the
question is what ends up on your disk.

## Not built

There is no Git integration, Git-based change detection, file watcher, or command palette
(`Ctrl+Shift+P` is reserved and currently does nothing). For a consolidated list of what else is
missing or deferred, see [Deferred work](../contributors/future-work.md), which is a live index
rather than a wish list — items leave it only when they are done or explicitly rejected.
