---
title: "What a file tree must not do"
rfc: "RFC-052"
rfc_file: "../../done/052-a-file-explorer-a-user-can-read.md"
source_rfc_status: "Implemented and closed 2026-09-24 — M12 remainder"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# What a file tree must not do

**Required reading before writing code.** A file explorer is two hazards at once: **it renders names
an attacker controls, and it walks a directory structure an attacker controls.** This project already
holds both boundaries. Keep them.

## §1 It must not render a name we did not escape

A repository can name a file with a bidi override, a newline, or bytes that are not UTF-8. Every name
reaches the screen through `text_safety::quote_untrusted`, whoever draws it. **A widget that renders
`file_name()` itself cannot be used**, however good it looks — that is not a preference, it is
RFC-016.

## §2 It must not leave the project root

A symlinked directory pointing outside the root is **not expanded**, and a symlink whose target
escapes is **reported, not followed** (`REQ-SEC-041`, `043`). Today's scanner does this. A new one
inherits the obligation, and it is proven against the fixture, not read from a README.

## §3 It must not be unbounded

256 children per directory is the current cap and becomes per level, not per view. A directory with
100 000 entries must not stall a frame or the process (D8), and an unreadable directory is a **row**,
not a panic.

## §4 An icon must not be the only thing that says it

`NFR-UX-002` and `REQ-NOTIFY-005` are mechanically checked. A file-type icon may replace
`[DIR]`/`[FILE]`, because kind is also carried by the name and by position. **A Git badge stays a
word.** "Untracked" does not become a dot, in any colour.

## §5 It must not write anything

No rename, no move, no delete, no drag-and-drop acceptance. If the chosen widget reports a drag as an
intent, this slice **ignores the intent**.

## §6 It must not outrank the keyboard policy

Global keybindings win, and a modal suppresses input beneath it (`RFC-015`). A widget with its own
key handling must sit inside that, not beside it.

## §7 It must not lose what the row can prove today

One row is one assertable string right now, and the escaping and i18n tests lean on that. If
composition scatters the row across widget calls, those tests must keep holding the same properties —
name the replacement in the request before the tests change shape.
