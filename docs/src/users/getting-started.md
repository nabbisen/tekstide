# Getting started

Tekstide is a local-first, multi-project workbench for supervising terminal-based AI
development workflows. It runs on Linux; there is no cross-platform evidence beyond it.

## Install

```sh
cargo install tekstide
tekstide /path/to/project
```

## Give it a path, or run it bare

`tekstide` with no argument opens an empty Project Board with a field to type or paste a path
into — press `Enter` to open it — and a **Browse...** button for choosing a folder without
typing a path at all.

Once a project is open, `Ctrl+Alt+O` opens the same field again for adding another, and
`Ctrl+Alt+B` opens the folder browser. A project you opened before and later closed is
remembered on the board: highlight its row with `Up`/`Down` and press `Enter`, or click its own
**Open** button.

You can open several at once:

```sh
tekstide /path/to/project /path/to/another
```

## The keys worth knowing first

| Binding | Action |
| --- | --- |
| `Ctrl+Alt+P` | Open the Project Board |
| `Ctrl+Alt+T` | Launch a real terminal in the active project |
| `Ctrl+Alt+M` | Toggle Content / Terminal mode |
| `Ctrl+Alt+U` | Open Workspace Trust for the active project |
| `Ctrl+Alt+K` | Open the keyboard reference, from anywhere |

`tekstide --help` prints usage and every binding, and the running application lists them all on
the Project Board and in the Help modal (`Ctrl+Alt+K`). The complete table is in
[Keyboard reference](./keyboard-reference.md).

## Before an AI CLI run will start

Opening a folder never grants trust. A project starts **Restricted**, which blocks nine
features — AgentRun launch among them. `Ctrl+Alt+U` opens Workspace Trust, and granting takes
two deliberate acts: the confirmation dialog's focus defaults to **Cancel**.

What the grant covers, and what revoking does not undo, is set out in
[Security decisions](../contributors/security-decisions.md#workspace-trust) — that page is the
canonical home for it, so this chapter points there rather than paraphrasing it.

With trust granted, `Ctrl+Alt+A` launches an AI CLI run in a project-owned terminal.

## Building from a checkout

```sh
cargo run -p tekstide
cargo run -p tekstide -- /path/to/project
```

## Where to go next

- [What works today](./what-works-today.md) — the capabilities, and the limits stated as
  plainly as the capabilities.
- [Working with projects](./working-with-projects.md) — tabs, closing, reviewing what an agent
  changed.
- [Configuration](./configuration.md) — the four settings that take effect, and why the file is
  narrow.
- [Local data and privacy](./local-data-and-privacy.md) — every file Tekstide writes, and how to
  remove it.
- [Changelog](../record/changelog.md) — what shipped in which release.
