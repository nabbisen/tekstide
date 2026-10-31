# Getting started

Tekstide is a local-first, multi-project workbench for supervising terminal-based AI
development workflows. It runs on Linux, and macOS on Apple Silicon — macOS is verified by its
full test suite and a rendered window capture on real hardware in CI; no human has interactively
used it yet.

## Install

Downloads the release binary for this platform (Linux x86_64, or macOS on Apple Silicon), verifies
it against its own published checksum, and installs **one binary** to `$HOME/.local/bin/tekstide`
— plus the directory it goes in, if that does not already exist. Touches nothing else: no shell rc
edit, no `sudo`, no repository added. If a future version needs to do more than that, it will say
so before doing it, not after.

```sh
curl -fsSL https://raw.githubusercontent.com/nabbisen/tekstide/main/install.sh | sh
tekstide /path/to/project
```

Three other forms are documented too, as **equivalents** — never as the safer choice, since every
one of them fetches code from the same server as the one-liner above:

- **Arch Linux**: the `PKGBUILD` in
  [`packaging/aur/`](https://github.com/nabbisen/tekstide/tree/main/packaging/aur) builds from
  the published crate, for anyone who prefers to build it themselves. Not yet published to the AUR
  itself.
- **`cargo install`**, for anyone who already has a Rust toolchain:

  ```sh
  cargo install tekstide
  ```

- **The checksum, verified by hand** instead of by the script — for anyone who wants to see
  exactly what the installer did, one step at a time. On Linux, the asset is
  `tekstide-x86_64-linux`; on macOS (Apple Silicon), `tekstide-aarch64-macos`, verified with
  `shasum -a 256 -c` instead of `sha256sum -c`:

  ```sh
  curl -LO https://github.com/nabbisen/tekstide/releases/latest/download/tekstide-x86_64-linux
  curl -LO https://github.com/nabbisen/tekstide/releases/latest/download/tekstide-x86_64-linux.sha256
  sha256sum -c tekstide-x86_64-linux.sha256
  mkdir -p ~/.local/bin
  install -m 755 tekstide-x86_64-linux ~/.local/bin/tekstide
  ```

  The checksum is published from the same release as the binary: it proves the download arrived
  intact, not that the release itself could not have been tampered with, so none of the forms
  above say more than *verified against its published checksum*.

**On macOS, clicking the asset link in a browser instead of using one of the forms above will
not work without one extra step.** `curl` (what every form above uses) does not set the
`com.apple.quarantine` attribute a browser download does, so only a browser-downloaded binary is
affected: macOS refuses to run it ("cannot be verified") until you allow it once in **System
Settings → Privacy & Security**. There is no developer certificate behind this release, so that
step is expected, not a sign something went wrong.

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

On macOS, press ⌘ wherever this table says `Ctrl`. `tekstide --help` prints usage and every
binding, and the running application lists them all on the Project Board and in the Help modal
(`Ctrl+Alt+K`). The complete table, with the same note, is in
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
