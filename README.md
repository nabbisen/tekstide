# Tekst IDE

![Status](https://img.shields.io/badge/status-early--implementation-orange)
[![license](https://img.shields.io/crates/l/tekstide.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/tekstide.svg?label=tekstide)](https://crates.io/crates/tekstide)
[![docs.rs](https://img.shields.io/docsrs/tekstide?version=latest)](https://docs.rs/tekstide)
[![Dependency Status](https://deps.rs/crate/tekstide/latest/status.svg)](https://deps.rs/crate/tekstide)
[![crates.io](https://img.shields.io/crates/v/tekstide-core.svg?label=core)](https://crates.io/crates/tekstide-core)
[![docs.rs](https://img.shields.io/docsrs/tekstide-core?version=latest)](https://docs.rs/tekstide-core)
[![Dependency Status](https://deps.rs/crate/tekstide-core/latest/status.svg)](https://deps.rs/crate/tekstide-core)

Tekst IDE (`tekstide`) is a local-first, multi-project workbench for supervising terminal-based AI development workflows.

## Current Status

The current implementation is a headless core through RFC-013, the command-approval
model of RFC-021 (rendered as of `0.10.0`), and the shared text-safety primitive of
RFC-016 PR-016-C: terminal runtime, AgentRun launch, transcript retention,
generated-change review, and durable audit storage. As of RFC-015 (now closed,
`rfcs/done/`), there is also a real desktop GUI application shell with mode
switching. It includes:

- Project Board and ProjectSession state, root-bound file access, bounded explorer,
  UTF-8 text buffers, and safe save with external-change detection;
- project-owned Linux PTY terminal lifecycle with bounded IO, resize, and
  process-group termination;
- conservative terminal output security policy, paste classification, and
  model-level trusted UI boundaries;
- AI CLI profiles as reviewed launch contracts, with Restricted Mode blocking
  workspace-local executables, wrappers, project-local `PATH`, and implicit CLI
  workspace-config discovery;
- AgentRun launch through project-owned terminals, with honest Plain/Supervised/
  Managed labels and active-file safety before process start;
- bounded local transcript capture with retention limits and purge policy — **reached for real by AI CLI runs**, with an in-app per-project opt-out and purge on the Trust Settings surface, see *Local Data and Privacy*;
- metadata-only generated-change detection and review-state tracking;
- durable local SQLite audit storage with schema identity, migration harness,
  corruption diagnostics, restart-safe recovery, and explicit purge;
- a real `iced` desktop shell: window/chrome/content/modal layer composition, a
  keyboard-driven focus and input-routing model with a visible, non-colour-only focus
  indicator, i18n-backed text and a compiled theme, and a Project Board surface
  rendering live `ApplicationShell` state with untrusted project names and paths
  escaped, never trusted;
- Content ↔ Terminal mode switching for an active project, with a real,
  user-reachable terminal (`Ctrl+Alt+T`): a security-filtered PTY session
  rendering real output, with a bounded terminal-count limit and exit
  detection so the session bar reflects what is actually running rather than
  what was last launched. As of `0.8.0` the terminal wakes on PTY readiness
  rather than a fixed 50 ms timer: output throughput rose from roughly
  374 KB/s to 17-18 MB/s, and the concurrent-terminal limit from 3 to 6.
  As of `0.10.0` the terminal is sized to the window rather than fixed at
  24×80: it follows a live window drag, and a pane launched before you ever
  resize the window gets the real size immediately. Input latency is still
  **not** verified against its target — see below.
- A real file explorer and text editor in Content mode (RFC-019, closed as
  of `0.6.0`): a keyboard-navigable explorer tree over the project's
  directory scan (Enter on a directory rescans, Enter on a file opens it;
  read-only — no rename, delete, or create), and a cursor-aware editor —
  open a file, move the cursor with the arrow keys, insert and delete at
  the cursor position across multiple lines, save with `Ctrl+S`. Saving
  never silently overwrites a file that changed on disk: a dialog offers
  to reload, every dismissal leaves the disk file untouched, and the
  dialog only claims local changes will be lost when there are some. File
  **names** shown in the explorer and the editor's header are escaped
  (untrusted, attacker-influenced text); file **contents** in the editor
  are deliberately not — the editor shows a file as it is, which means
  source containing a bidi-override character still *reads* differently
  from how it compiles.
- Real clipboard paste into a focused terminal (`Ctrl+Shift+V`, RFC-018),
  routed through the same RFC-009 policy as everything else that reaches a
  PTY: single-line and empty pastes go through and control-containing pastes
  are blocked outright, both without a dialog. A multi-line paste opens a
  real, rendered confirmation dialog — the first trusted dialog this product
  has — showing an escaped preview of the pasted content; accepting is the
  only thing that writes it, and every other way to leave the dialog
  (Escape, or activating Cancel) leaves the terminal untouched. As of
  `0.7.0` the rest of the window dims behind the dialog while it is open,
  including chrome the terminal's own pane cannot draw into.
- **Workspace trust, grantable for the first time (`0.10.0`, RFC-032).** Every
  project before this release was permanently **Restricted** — there was no
  code path anywhere in the shipped application that could grant trust, so
  the nine restricted features, AgentRun launch among them, were blocked for
  everyone forever. `Ctrl+Alt+U` opens a Workspace Trust surface showing the
  project's real state. Granting opens a confirmation dialog whose focus
  defaults to **Cancel**, so granting takes two deliberate acts; the path
  shown is the **canonical** path (what trust actually binds to), escaped,
  and a symlinked project also shows the path you opened it by. The dialog
  states that the grant covers files not yet written — including anything an
  AI agent run writes there — for this session and every session after, and
  that revoking stops future loading but does **not** undo what already ran.
  Revoking is one action, with no confirmation, because it is the safe
  direction. Trust persists across sessions, and the **audit store**, not the
  user-writable recent-projects cache, is what restores it.
- **AgentRun launch, reachable for the first time (`0.10.0`).** With trust
  granted, `Ctrl+Alt+A` launches a real Claude Code session in a
  project-owned terminal. This is the product's premise, reachable at last —
  with one honest caveat: **the real Claude Code CLI has never been exercised
  by this project's tests.** Every automated proof uses a controlled test
  executable, because the live product needs interactive authentication and
  makes real network calls. The launch pathway is proven end to end against
  production code; the specific behaviour of the real binary under it is not.
  You still cannot **see what a run changed** — there is no diff review or
  change-review surface (RFC-020's PR-020-C, still blocked) — but as of
  `0.11.0` the change set itself is real, and you can now read **what the
  run said**: see the AgentRun report entry below. What it said and what
  it changed remain two different, separately-reachable things.
- **The adapter-spawn pathway and the command-approval dialog** (`0.10.0`,
  RFC-022) — built, audited, and proven end to end. Reachable only by this
  project's own reference adapter, for the reason given under *Command
  approval* below. As of `0.11.0` its **approval-history surface** opens with
  `Ctrl+Alt+H`; see the caveat below about what that does and does not reach.
- **Real change detection for agent runs (`0.11.0`).** Launching a run captures
  a filesystem baseline of the project **before the agent's process starts**,
  and when that run's terminal exits the two are compared, producing a real
  change set naming the files the run actually touched. `.git/`, `target/` and
  `node_modules/` are excluded by design — build output and VCS metadata would
  drown the result — which means **a change an agent makes inside those
  directories is not reported**, git hooks included. A scan that hits its entry
  limit is recorded as *truncated*, never as "nothing changed": those are
  different facts and the product does not collapse them. Two further limits
  are deliberate and disclosed: detection runs only at exit, so a long-lived
  interactive session reports nothing until it ends, and the baseline lives in
  memory, so it does not survive the application closing mid-run.
- **A WCAG contrast gate over the theme (`0.11.0`)**, with the failures it
  caught. Unfocused pane borders measured **2.63:1** against the background,
  below the 3:1 that WCAG 2.1 SC 1.4.11 requires for UI component boundaries;
  the border is now 3.85:1. Text contrast was never the problem — it sits above
  14:1 — and focus indication was unaffected. The pair list this gate checks is
  **derived** from an exhaustive destructure of the theme, not hand-written: a
  future colour role cannot be added to the theme without also being classified
  here, or the crate fails to compile. That derivation caught a second,
  separate defect no fixed pair could have: the modal dialog's real backdrop is
  the scrim composited over whatever was behind it, including terminal
  content, which is arbitrary. Sampling "scrim over the background" and "scrim
  over white" both pass — the failure lives strictly between them, at content
  around 78% grey, where neither the border nor the fill alone clears 3:1
  (worst case measured **2.40:1**). The scrim is now more opaque (`0.55` →
  `0.75`, in `0.12.0`) so that no content value fails; this is a visible appearance change,
  not only a number. The check itself is swept continuously across that range
  rather than sampled at a few points, since sampling is exactly what let this
  one hide.
- **The AgentRun report surface (`0.12.0`)** (`Ctrl+Alt+R`) — real transcript content for
  the most recently launched run in the active project, escaped at render
  (a Unicode directionality override in what an AI CLI printed shows as a
  visible marker, never as an invisible reordering — the same policy every
  other untrusted-text surface in this project uses). Unlike change
  detection, this is reachable **while a run is still active**, not only
  after it exits — the surface says so, distinctly from a finished run's
  transcript. It shows what the run **said**, not what it **changed**; there
  is still no way to see the latter (above). The window is a bounded tail
  (1 MiB) of the real transcript, not the whole thing, and RFC-011's own
  writer-side truncation (if a transcript hit its own retention limit) is a
  separate, independently-shown fact from "this is only a partial view" —
  conflating the two was the specific failure this surface was built not to
  repeat.

It is not yet the full AI CLI workbench. The editor has no undo (a mid-buffer
edit is unrecoverable within the session past what Backspace can still
reach), no syntax highlighting, language server, multi-cursor, or search,
and files above 4 MiB are not editable. There is still no diff/review surface
(RFC-020's PR-020-C, M10's second half), so **you still cannot review what an
agent run changed**. The **AgentRun report surface** (`Ctrl+Alt+R`) does now
exist — real, escaped transcript content for the most recently launched run,
what it *said*, not what it *changed*; see the caveat below about what that
does and does not reach. What `0.11.0`'s change detection changed is the
reason diff review is now **buildable, not reachable** — the input is there,
the change-review surface itself is not. There
is also no Git-based change detection, file watcher, or overwrite-confirmation
UI, no safe-close dialog, and no cross-platform evidence beyond Linux. The **approval-history surface** built in `0.10.0` now opens
(`Ctrl+Alt+H`), but that makes only the *surface* reachable, not command
approval itself: no shipping AI CLI speaks RFC-021's protocol, so
`Managed` command approval is still exercisable only by this project's own
reference adapter, and a real user opening this surface today will see it
empty — correctly, not as a bug. **Terminal input latency is not verified against its
16 ms p95 target.** `0.8.0` removed the structural cause of the previous
failure — a 50 ms polling interval that put the floor near 47.5 ms — but
removing a known cause is not the same as measuring the result, and the
criterion is recorded as still not met rather than assumed fixed. There is **no screen-reader support** — not limited,
not planned, absent for the life of the `iced` substrate decision (RFC-014).
Command approval (below) is now built end to end and still not reachable by
a real user, for a different reason than before. RFC-018's
trusted-UI evidence shows two checkable properties distinguishing the real
paste dialog from terminal output imitating it. First, keystrokes typed
while it is open never reach the terminal, verified with a positive control
proving they were reaching the app. Second, as of `0.7.0`, the window dims
behind the dialog, and that dimming covers chrome the terminal grid can
never draw into. Unlike the dialog's own size — which depends on the
pasted content and is therefore attacker-influenced — the dimmed area is
fixed by the window, so the same tell holds for a one-byte paste and a
large one alike. Neither property makes the dialog unspoofable; they raise
the cost of a convincing imitation. Nothing here claims an untrained user
would notice either property unprompted.

Durable audit currently records trust decisions, managed AgentRun lifecycle, blocked
root/symlink access, audit-store recovery outcomes, plain-terminal session starts and
terminations, paste refusals, command-approval decisions, restricted-feature refusals,
and project-added opens. As of `0.10.0` the trust family has a real user-driven
producer for the first time: granting or revoking workspace trust writes to the store.
RFC-031 (PR-031-A/B) added the restricted-feature producer (a real launch refused for
lacking workspace-discovery trust) and the project-added producer (a real project
opened from the CLI-argument path, distinct from a remembered project merely restored
on boot, which writes nothing). Safe-close, configuration-change, and transcript-purge
producers are defined in the audit schema but not yet wired.

### Command approval

Tekstide implements a command-approval protocol that a cooperating AI CLI adapter can
use: a versioned sideband channel over a per-run Unix domain socket, two-layer peer
authentication, a structural risk classifier, and single-use decisions recorded in the
durable audit trail.

**It is not yet available to users, and `0.10.0` changed why.** The missing pieces are
now built: RFC-022 added the adapter-spawn pathway, capability-token delivery, and a
real rendered approval dialog, all proven end to end against production code. What is
missing is on the other side — **no shipping AI CLI speaks this protocol.** `Managed`
mode, and therefore command approval, can only ever be exercised by this project's own
reference adapter, which is a test artifact. The pathway is proven; the ecosystem does
not exist. Anyone reading "command approval shipped" into this is reading more than the
record says.

The **approval-history surface** — where past decisions and expired requests are
disclosed — is likewise implemented and tested, but no key is bound to it, so it cannot
be opened at all. That is a defect, recorded in `rfcs/future-work.md`, not a design
choice.

**It is cooperative, not enforced.** Approval works only if the adapter asks. Tekstide
does not intercept process execution and has no execution path of its own to withhold,
so an adapter that ignores a rejection — or never submits a proposal — runs its command
regardless. Tekstide does not approve commands, and does not control what an AI CLI can
run.

## Quick Start

Install from crates.io and open a project:

```sh
cargo install tekstide
tekstide /path/to/project
```

**Give it a path.** There is no in-app way to add a project yet
(RFC-038), so a path on the command line is currently the only way to
put one on the Project Board — `tekstide` with no argument opens an
empty board. Until `0.12.1` this section said to run `tekstide` bare and
mentioned the path second, as an option, which left a first-time user
looking at a window with nothing to do and no way to change that.

You can open more than one:

```sh
tekstide /path/to/project /path/to/another
```

`tekstide --help` prints usage and the full keyboard reference below;
the running application also lists every binding on the Project Board.

### Building from a checkout (contributors)

```sh
cargo run -p tekstide
cargo run -p tekstide -- /path/to/project
```

## Keyboard Reference

The shell is keyboard-navigable by design. These bindings exist today
(`crates/tekstide-core/src/navigation.rs`'s `KeybindingPolicy::linux_mvp()` and
`crates/tekstide/src/input.rs`):

| Binding | Action |
| --- | --- |
| `Ctrl+Alt+P` | Open the Project Board |
| `Ctrl+Alt+M` | Toggle Content / Terminal mode for the active project |
| `Ctrl+Alt+T` | Launch a real terminal in the active project (switches to Terminal mode) |
| `Ctrl+Alt+A` | Launch an AI CLI (Claude Code) run in the active project — refused unless the project is trusted |
| `Ctrl+Alt+U` | Open the Workspace Trust surface for the active project (grant or revoke) |
| `Ctrl+Alt+H` | Open the Approval History surface for the active project |
| `Ctrl+Alt+R` | Open the AgentRun Report for the most recently launched run in the active project |
| `Ctrl+Shift+V` | Paste the clipboard into the focused terminal, subject to RFC-009's policy |
| `Ctrl+S` | Save the active document in Content mode |
| `Tab` / `Shift+Tab` | Cycle keyboard focus between shell zones |

`Ctrl+Shift+P` is reserved for a command palette that does not exist yet — it is
bound in the keybinding policy but currently does nothing.

With `Tab` focused on the sidebar in Content mode, `Up`/`Down` move the explorer
highlight and `Enter` opens the highlighted file or directory. With focus on the
main area, typing edits the open document at the real cursor position, `Up`/
`Down`/`Left`/`Right` move the cursor without editing, `Enter` inserts a newline,
and `Backspace` deletes the character before the cursor.

`Esc` (dismiss) and `Enter` (activate) work on the shell's modal layer — real
today for the paste-confirmation, file-changed-on-disk, workspace-trust, and
command-approval dialogs (RFC-018, RFC-019, RFC-032, RFC-022); the
developer-only demo modal gated behind an environment variable still exists
too. Of those four, only the approval dialog is unreachable in practice, and
because no AI CLI speaks the protocol that would raise it — not because it is
unbuilt. The safe-close dialog does not exist yet.

## Local Data and Privacy

Tekstide is local-first: it does not send project data anywhere. On every launch,
the desktop application creates the **recent-projects list**, at
`$XDG_STATE_HOME/tekstide/recent-projects.json`
(`~/.local/state/tekstide/recent-projects.json` if `XDG_STATE_HOME` is unset) —
the paths of projects you have opened, used to restore the Project Board across
sessions. As of `0.10.0` it also caches each project's last-known trust state, so
the board can label rows without opening every project. That cache is a display
hint only: it is user-writable, so **the audit store is what actually restores
trust**, and editing this file cannot grant a project anything. There is no in-app
command to clear it yet; delete the file to reset it.
The only other local state it creates is the audit store described next, and only
once you actually open a terminal.

RFC-013's durable audit store is implemented, tested, and has a real producer
(RFC-017, wired end to end as of the terminal-launch-UX handoff):
**pressing `Ctrl+Alt+T` to open a terminal creates this database**, the first
time you do it. Each launch records a `plain_terminal_observation` `Started`
event; if that session later exits (typing `exit`, or the shell dying on its
own), a matching `Terminated` event is recorded too, naming only whether the
process exited or was signalled — never a command, its output, or a path.
This family's schema has no field for any of those at all, so none can ever be
recorded in it, launch or exit. The store lives at
`$XDG_STATE_HOME/tekstide/audit/audit.sqlite3`
(`~/.local/state/tekstide/audit/audit.sqlite3` if `XDG_STATE_HOME` is unset).
**This is no longer gated behind a developer-only flag** — `TEKSTIDE_TERMINAL_DEMO`
still exists for diagnostic use, but the real `Ctrl+Alt+T` binding is what
ordinary use reaches, and it opens the same store.

Pasting into a terminal (`Ctrl+Shift+V`, RFC-018) writes to the same store when the
paste is refused: a `paste_blocked` event, naming only that a paste was blocked and
which project/terminal it was aimed at — never the pasted content, the clipboard
text, or the command it would have produced. This family's schema has no field for
any of those either. A paste the policy *allows* is not audited at all; only
refusals are, a known and disclosed limitation of the schema rather than an
oversight — see the RFC for why.

There is no in-app command to purge the audit store yet; delete the `audit/`
directory to reset it, or see
[`rfcs/done/013-durable-audit-store-and-local-data-policy.md`](rfcs/done/013-durable-audit-store-and-local-data-policy.md)
for the store's full retention and purge policy.

Granting or revoking workspace trust (`Ctrl+Alt+U`, RFC-032) writes to the same
store: a trust-change event naming the project and the canonical path the grant
binds to. This is what makes trust survive a restart — the store is authoritative,
and it is queried for an *applied* grant specifically, so an interrupted or
authorized-but-not-applied attempt does not restore as trust.

**Launching an AI CLI run records that session's transcript to disk.** This corrects a
claim `0.10.0` and `0.11.0` both made — that Tekstide retains no transcripts. It does, and
has since agent-run launch became reachable in `0.10.0`. The error was ours and is described
under *Corrections* in the changelog; what follows is what actually happens.

Pressing `Ctrl+Alt+A` in a trusted project starts a bounded transcript for that run, written
to `$XDG_STATE_HOME/tekstide/transcripts/<project>/<agent-run>/transcript.log`
(`~/.local/state/tekstide/…` if `XDG_STATE_HOME` is unset). It contains **the terminal output
of that AI session as it was produced** — which means whatever the AI CLI printed, including
anything it quoted from your files.

Capture is **bounded by policy, not by chance** (RFC-011): at most **32 MiB per transcript**,
**256 MiB per project**, **1 GiB across the application**, and **30 days**. Capture is
best-effort: if writing fails mid-session the run marks capture failed and the terminal stays
usable, rather than silently continuing unrecorded.

**Both limitations this section used to describe as unaddressed now have an in-app route**
(RFC-033), from the same `Ctrl+Alt+U` Trust Settings surface trust grants and revocations
already use:

- **Decline capture for future runs, per project.** Space toggles it. This is forward-only —
  declining does not delete any transcript that already exists, and the setting persists
  across a restart.
- **Purge every transcript retained for this project.** Delete opens a confirmation that names
  the scope (this project; other projects are unaffected) and states it cannot be undone. A
  tombstone record remains after a purge, and so does a `transcript_purge` entry in the local
  audit store — recording that a purge happened and its scope, never a path or a byte count. To
  remove transcripts without using the app, delete the `transcripts/` directory directly.

One limitation remains: **a plain terminal (`Ctrl+Alt+T`) is not recorded.** Only AI CLI runs
are.

See
[`rfcs/done/011-transcript-retention-and-local-data-policy.md`](rfcs/done/011-transcript-retention-and-local-data-policy.md)
for the full retention and purge policy.

For a consolidated list of what else is missing or deferred, see
[`rfcs/future-work.md`](rfcs/future-work.md).

## RFCs

Implemented RFCs live under [`rfcs/done/`](rfcs/done/); RFCs that are accepted and being
implemented live under [`rfcs/accepted/`](rfcs/accepted/). [`rfcs/README.md`](rfcs/README.md) is
the index.

Release scope and deferred work are tracked in [`rfcs/done/001-product-scope-mvp-and-non-goals.md`](rfcs/done/001-product-scope-mvp-and-non-goals.md), [`CHANGELOG.md`](CHANGELOG.md), [`ROADMAP.md`](ROADMAP.md), and [`rfcs/future-work.md`](rfcs/future-work.md).
