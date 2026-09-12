# Working with projects

## Choosing a folder

**The folder browser** opens with the **Browse...** button or `Ctrl+Alt+B`, over your home
directory by default. Arrow keys move the highlight; `Enter` navigates into the highlighted
folder, updating the **Current:** line at the top; `Space` — or the **Open this folder** button —
chooses whatever **Current:** now shows. `Escape` cancels without opening anything.

`Ctrl+Alt+O` opens a field to type or paste a path instead.

## Switching between open projects

Opening more than one project shows a tab strip: one tab per project, a filled dot on the active
one, plus a permanent **Projects** tab that returns to the board. Click a tab to switch, or press
`Ctrl+Alt+N` to cycle to the next with wraparound.

A project you closed earlier is remembered on the board, with its own **Open** button — no
retyping its path.

## Closing a project

`×` on its tab, or `Delete` with the tab highlighted.

If nothing is live in it — no terminal, no agent run — it closes immediately, with no dialog. If
something is, a confirmation names exactly what will end (for example, *"This will end: 1 running
process"*) and shows the project's **canonical** path, so you know precisely what you are about to
close. Focus defaults to **Cancel**; `Enter` activates whichever button is focused, `Escape`
always cancels. Closing is never one accidental keystroke away.

**Closing ends what those terminals started**, including a job you backgrounded with `&` — the
dialog says so before you click. A process you deliberately detached with `nohup`, `disown` or
`setsid` survives, because those work by leaving the terminal's session, and the session is the
boundary this respects. That is the opt-out: use it if you want something to outlive the project
you are closing.

## Reviewing what an AI agent changed

`Ctrl+Alt+D`, or the **Change Review** button on Trust Settings.

After a run exits, this surface lists the files it touched, a count, a detection-status line, and
a review state. Click a file, or highlight it and press `Enter`, to preview its content.

**Read its limits as closely as its existence.** What it shows, what it cannot show, and what
detection excludes are set out in
[What works today](./what-works-today.md#what-a-run-changed) — the short version is that there is
no before/after diff, and changes inside `target/`, `node_modules/` and most of `.git/` are never
reported.

### Recording a decision

**Mark accepted** or **Mark rejected** is offered until you decide. **Read what that is as
closely as what it is not:** it changes no file, it cannot be taken back once recorded, and it
does not survive closing Tekstide — no audit record, no persistence, nothing left when you reopen
the project. It is a note to yourself for the session, not a review workflow.

If the files on disk have moved since the change set was detected, the surface says so and still
lets you decide. A decision is about what was detected, which a later, unrelated change does not
undo.

A preview that went stale — the file changed again after you opened it — refuses and says so,
rather than silently showing newer content as though it still matched what you selected.

## Trust and transcript controls

`Ctrl+Alt+U` opens Trust Settings for the active project. Alongside granting and revoking trust,
it carries the two transcript privacy controls: declining capture for future runs, and purging
what is already retained, both per project.

What each does, and what neither does, is in
[Local data and privacy](./local-data-and-privacy.md).

## Surface-local keys

Every surface-local key is listed in the Help modal (`Ctrl+Alt+K`, or the `?` button) and in
`tekstide --help`, grouped by the surface it belongs to. The complete table is in
[Keyboard reference](./keyboard-reference.md).
