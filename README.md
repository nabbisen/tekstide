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
- bounded local transcript capture with retention limits, per-run opt-out, and purge;
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
  AgentRun report surface (RFC-020), and nothing runs change detection.
- **The adapter-spawn pathway and the command-approval dialog** (`0.10.0`,
  RFC-022) — built, audited, and proven end to end. Reachable only by this
  project's own reference adapter, for the reason given under *Command
  approval* below.

It is not yet the full AI CLI workbench. The editor has no undo (a mid-buffer
edit is unrecoverable within the session past what Backspace can still
reach), no syntax highlighting, language server, multi-cursor, or search,
and files above 4 MiB are not editable. There is no diff/review surface or
AgentRun report surface yet (RFC-020, M10's second half) — and the reason is
worth stating plainly rather than as a date: **nothing in the shipped
application runs change detection**, so no change set can exist for either
surface to render. Launching an agent run works; reviewing what it did does
not. There is also no Git-based change detection, file watcher, or
overwrite-confirmation UI, no safe-close dialog, and no cross-platform
evidence beyond Linux. The **approval-history surface** built in `0.10.0` now opens
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
terminations, paste refusals, and command-approval decisions. As of `0.10.0` the trust
family has a real user-driven producer for the first time: granting or revoking
workspace trust writes to the store. Restricted-feature, safe-close,
configuration-change, transcript-purge, and project-added producers are defined in the
audit schema but not yet wired.

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

Install from crates.io and run:

```sh
cargo install tekstide
tekstide
```

Open one or more local project paths from the command line:

```sh
tekstide /path/to/project
```

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

RFC-011's transcript retention is implemented and tested, but **is not wired
into the desktop application yet** — running `tekstide` does not retain any
transcripts, and that remains true in `0.10.0` even though agent runs are now
launchable. Checked rather than assumed for this release: the launch request
does declare a local bounded retention policy, but nothing in the desktop crate
configures a transcript **writer**, so no transcript is ever written. See
[`rfcs/done/011-transcript-retention-and-local-data-policy.md`](rfcs/done/011-transcript-retention-and-local-data-policy.md)
for the retention and purge policy that applies once it is.

For a consolidated list of what else is missing or deferred, see
[`rfcs/future-work.md`](rfcs/future-work.md).

## RFCs

Implemented foundation RFCs live under [`rfcs/done/`](rfcs/done/).

Release scope and deferred work are tracked in [`rfcs/done/001-product-scope-mvp-and-non-goals.md`](rfcs/done/001-product-scope-mvp-and-non-goals.md), [`CHANGELOG.md`](CHANGELOG.md), [`ROADMAP.md`](ROADMAP.md), and [`rfcs/future-work.md`](rfcs/future-work.md).
