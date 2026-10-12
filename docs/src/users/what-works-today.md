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
be lost when there are some. **The editor's header names the active document's own state** — a
change or a deletion on disk, as soon as the watcher sees it — *(changed on disk, not
reloaded)*, *(deleted on disk, not reloaded)*, *(conflict)*. A background document's own disk
change is detected the same way (every open document is watched, not only the active one), but
is not shown until you switch to it — the header, not a sidebar marker, is where it surfaces.
A **Reload** button beside **Save** reaches the same dialog
directly, without first attempting to save.

The explorer is **read-only** — no rename, delete, or create.

**The explorer is there whether you are in Content or Terminal mode.** It used to disappear
behind a sentence the moment you switched to a terminal; it is the same tree, reachable the same
way, in both now. Opening a file while in Terminal mode switches you to Content mode to show it —
the one thing that does; a background refresh or a restored unsaved buffer never moves you out of
a terminal you are watching.

**The explorer is a tree.** Folders open and close in place with `Enter`, so a file inside `src/` is
visible without stepping into `src/`; there is no "go up" row. Each folder is read **when you open it**
(and again each time you reopen it), on a background thread, so opening a huge folder never freezes the
window — the row says *Loading…* until it arrives. **Nothing is hidden silently:** a folder with more
than 256 entries shows the first 256 and a row saying how many more there are; only the rows that fit
the sidebar are drawn, and a line says which rows those are (*Rows 18–60 of 272*). Folders that are ignored (or, outside a Git repository, on a built-in list) are marked *(collapsed)* and can still be opened (see *Ignored files* below).
A folder that cannot be read says so, and a link that points outside the project is marked *(blocked)*
and cannot be opened. Folders are listed first. A folder is `▣` (open: `▢`) and a file `▫`; these are text symbols, so a machine
without those glyphs shows a blank box, and everything the row says is still in words. **Every open
document is marked `[open]`**, not only the one on screen — a project with several files open at once
(see *Multiple documents* below) shows the tag on each of their rows; the `>` is where the keyboard is,
and the editor's own header names which open document is active. Each row is an icon, the **name**, and then the words that describe it
(`(collapsed)`, `[untracked]`, `[open]`…), so a name is never cut short by its own status; in a narrow sidebar a *long* name
is still clipped at the right edge, without a marker, and the line under the tree shows the highlighted row whole.

**The explorer watches the disk.** The project root, every folder you have expanded, and the
folder of every open document (not only the active one) are watched; a file created, deleted, or
renamed in one of them appears or disappears on its own, without closing and reopening the
folder. A burst of changes in
one folder is batched into a single re-read, about 250 ms after the first of them — never one
re-read per change. Watching is per project: if the operating system refuses to place another
watch, it stops entirely for that project, the sidebar says so (*Folders are no longer updating.
Reopen the project to resume.*), and nothing is watched again until you do. Saving a file costs an
extra re-read of the file you just wrote, since a save is itself a change inside a watched folder.

**Ignored files.** Inside a Git repository, Tekstide asks **Git** which entries are ignored — it never reads a `.gitignore`
itself — about the entries the sidebar is about to draw, and marks them `[ignored]`. **By default ignored *files* are not drawn**, and the directory says how many it left out (*2 ignored files hidden*); an
ignored *folder* keeps its row, collapsed and marked `[ignored]`, so it can still be opened. `explorer.show_ignored = true`
in your configuration file draws the files too. Only ignored entries are affected: `.env`, `.gitignore` and every other
dotfile are ordinary rows either way.
A tracked file that matches an ignore pattern is tracked, not ignored. The first two lines of the sidebar say where the
rule came from (*project's Git*, *parent Git repo*, *nested Git repo*, or *built-in* — with why Git was not used) and that
**the marks are as old as the scan**: a `.gitignore` you edit is not reflected until the folder is read again. Outside a
repository, in one Git will not be asked about, or under a repository rooted at your home directory, the built-in list
(`.git`, `node_modules`, `target`) still applies, and says so. **Git is run without your personal Git configuration**, so a
global ignore file you named with `core.excludesFile` is **not** applied (the default `$XDG_CONFIG_HOME/git/ignore`,
`.git/info/exclude` and the repository's `.gitignore` files are).

**The editor draws the lines that fit and follows the cursor, in both directions.** Only the visible lines are laid out, so typing
stays fast in a file of 100,000 lines. The window moves when the cursor leaves it — by the arrow keys or by typing — and there is
no scrollbar or mouse wheel yet. A line wider than the editor **scrolls sideways to keep the cursor visible**, by the same rule as
the vertical window; lines are not wrapped, so a long line's own middle is never on screen at once, only the part around the
cursor.

**The editor has line numbers and a visible caret.** The gutter shows each line's real number, sized to the whole file so the
text column does not shift as you scroll past 10,000 lines (or any other power of ten). The caret is drawn as its own mark, never
as a character inserted into the file's text — a file whose contents happen to include something that looks like a caret is not
confused with the real one.

**The editor has undo and redo** (`Ctrl+Z` / `Ctrl+Shift+Z`), up to 500 edits deep; past that the
oldest edit can no longer be undone, and the editor says so rather than silently dropping it.
Reloading a file that changed on disk constructs a fresh document, so undo does not reach back
across a reload — the history goes with the content it replaced, and the product does not pretend
otherwise. There is no syntax highlighting, language server, multi-cursor, or search, and files
above 4 MiB are not editable.

**Multiple documents can be open at once**, up to 20 per project. Opening a file joins the set
rather than replacing whatever was already open — editing one file and opening another loses
nothing, and each document keeps its own text, undo history, cursor and viewport independently.
`Ctrl+Alt+F` switches to the next open document, cycling with wraparound, restoring exactly where
you left it. The sidebar's `[open]` tag marks every open document, not only the one on screen —
**the editor's own header, above the cursor line, is what names which open document is active.**
Opening a path that is already open switches to it in place rather than opening a second copy of
it. **`Ctrl+Shift+S`** (or the editor's own "Save All" button) saves every open document in one
action, attempting every one regardless of an earlier document's own failure, and reports
afterward how many succeeded and names each one that failed, with why. A document already clean
is saved without being rewritten — the count is honest about that too.

**Opening a project offers back anything it has unsaved records for.** If an earlier session left
dirty documents behind — a crash, or simply quitting without saving — reopening that project lists
each one by its own path and lets you accept or leave it, one document at a time; declining leaves
every file and every record exactly as it was. A recovered document comes back with its text,
cursor and viewport exactly as last written, but never its undo history. If the file on disk has
not changed since, it comes back dirty, ready to save; if it has changed, or is gone, it comes back
through the same *(conflict)*/*(deleted on disk, not reloaded)* states any other open document
would reach, **Reload** button included — never a silent overwrite of a file it never read. See
*Unsaved-buffer recovery data* in
[Local data and privacy](./local-data-and-privacy.md#unsaved-buffer-recovery-data) for what gets
written, where, and how to turn it off.

**A project can be held by only one Tekstide window at a time.** Opening a project root that
another Tekstide process already has open does not open a second copy of it: the attempt is
refused, that window is asked for your attention, and the board says so — *"This project is
already open in another Tekstide window. It has been asked for your attention."* The first window
keeps working, unaffected. The request for attention is best-effort and may do nothing visible:
Wayland does not let a window bring itself to the front, so you may still need to switch to it
yourself, and nothing claims otherwise. See [Local data and
privacy](./local-data-and-privacy.md#the-project-lock-and-attention-socket) for what makes the
refusal real — a lock file, not an in-memory flag — and why it is what stops two windows from
racing to write the same recovery records above.

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

The same surface is where you **classify the run, write notes on it, and export the report**. The
classification is one of coding, review, documentation, testing, refactoring or release, or a label of your
own; the notes are your own words, and nothing the run printed is ever put in them. Both are saved in the
run's record at once, so they are still there after Tekstide is closed and reopened — the reopened run is
listed on the same surface, and says when it does not know how the run ended. `Export report…` writes a
Markdown file **where you ask**; see *The report you export* in
[Local Data and Privacy](./local-data-and-privacy.md).

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

## Command approval: reachable from configuration, and still unused

Tekstide implements a command-approval protocol a cooperating AI CLI adapter can use: a versioned
sideband channel over a per-run Unix domain socket, two-layer peer authentication, a structural
risk classifier, and single-use decisions recorded in the durable audit trail. It is built,
audited, and proven end to end against production code.

**Declaring `adapter = true` on a configured profile (see [Configuration](./configuration.md)) is
real and reaches it.** Before this, no configuration field could even ask for `Managed` mode; now
one can, and a declared profile's launch really does bind a real approval channel and wait for a
proposal. **It still does nothing useful for almost everyone, because no shipping AI CLI speaks
this protocol.** Declaring `adapter = true` on an ordinary AI CLI's own command does not change
what that program does — it only tells Tekstide to expect a protocol conversation that program was
never written to have, so no proposal ever arrives. Today, only this project's own reference
adapter — a test artifact, not a product — actually speaks it. The pathway is proven and reachable;
the ecosystem to use it with does not exist. Anyone reading "command approval shipped" into this is
reading more than the record says.

`Ctrl+Alt+H` opens the approval-history surface, and a real user opening it today sees it
**empty** — correctly, not as a bug, and not because approval failed to run: nothing ever proposed
anything.

**It is cooperative, not enforced, and declaring an adapter does not change that.** Approval works
only if the adapter asks. Tekstide does not intercept process execution and has no execution path
of its own to withhold, so an adapter that ignores a rejection — or never submits a proposal — runs
its command regardless. Tekstide does not approve commands, and does not control what an AI CLI can
run. Declaring a profile as an adapter in configuration is **a claim the file makes**, not a fact
Tekstide can verify about the program `command` names — nothing checks that the declared program
actually behaves like an adapter, because the protocol is cooperative by design.

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

**The store is now reachable, not only written to.** `Ctrl+Alt+V`, or the "Audit" button in the
top bar, opens a top-level view over every recorded event — filterable by project, family, outcome
and operation, paged, and visible with no project open, since the store is app-wide. It leads with
a line naming what it answers: *what happened*, never *what is live right now* (that is Approval
History's own question, and the two will legitimately disagree). The same view can purge records,
per-project or globally, each behind a confirmation naming its scope, its real count, and that the
act cannot be undone — including, for a trusted project, that purging also revokes its trust.

## Git

The active project's status bar shows its current branch and whether it has uncommitted changes,
and the file explorer shows each changed file's own status next to its name, when it can be read
safely: a repository whose configuration names anything that could run a program is read as
branch-only or "not available" rather than guessed at (RFC-030's own safety gate). **A read
happens when a project opens, and again whenever a process Tekstide launched for that project
ends** — a terminal or an AI CLI run closing, not on a fixed interval and not continuously.

**The literal rule: a change is reflected only once the process that could have made it has
ended, or the project is reopened.** A `git commit` typed into a Tekstide terminal you leave
running is **not** reflected while that terminal stays open, even though it happened inside
Tekstide — the same as a commit made in a terminal outside Tekstide entirely. This is a deliberate
cadence, not a bug: it costs nothing when nothing is running, and needs no background timer.

The explorer marks each changed file as modified, added, renamed or copied, untracked, or in
conflict. **A deleted file carries no badge.** It is a real status Tekstide reads and can name —
the status bar's own changed-file count includes it — but the explorer only ever shows files that
still exist on disk, and a deleted file has none left to show a badge on. If the status bar's count
and the number of badges in the tree ever disagree by exactly the number of files you deleted,
that is why.

A wholly new, entirely untracked folder is marked untracked as a whole, the same as a single file
— the files inside it are not marked individually until something inside is tracked or staged.
Git-based change detection does not feed the Change Review surface.

## Settings

A configuration file in your own configuration directory sets the keyboard shortcuts, the colours,
the font family and sizes, and how much scrollback a terminal keeps, and `Ctrl+Alt+C` applies a
change to the running window. A colour or size that would make the window unreadable is not used, a
font is a name and never a file, a terminal's scrollback is capped so one cannot take unbounded
memory, and the Project Board says which setting fell back and why. A project can never supply
configuration. See [Configuration](./configuration.md).

## Not built

There is no command palette (`Ctrl+Shift+P` is reserved and currently does nothing). For a
consolidated list of what else is missing or deferred, see
[Deferred work](../contributors/future-work.md), which is a live index rather than a wish list —
items leave it only when they are done or explicitly rejected.
