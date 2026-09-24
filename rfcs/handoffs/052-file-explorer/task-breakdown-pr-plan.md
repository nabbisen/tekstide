---
title: "RFC-052 — task breakdown and PR plan"
rfc: "RFC-052"
rfc_file: "../../done/052-a-file-explorer-a-user-can-read.md"
source_rfc_status: "Implemented and closed 2026-09-24 — M12 remainder"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# Task breakdown and PR plan

**A decides the mechanism. B and C only happen once it has.**

## PR-052-A — the hostile fixture, and the decision

**No user-visible change.**

### The fixture, committed

One builder, under a temporary directory, its own state, no dependence on the developer's machine:

| Row | Shape |
| --- | --- |
| escaping | a file named with a **bidi override**, one with a **newline**, one whose bytes are **not UTF-8** |
| root escape | a symlinked directory pointing **outside** the project root, and a symlink whose target does not exist |
| breadth | a directory with **100 000 entries** |
| depth | a directory nested far enough to find any recursion limit |
| unreadable | a directory with permissions that refuse a read |
| control | an ordinary small tree that must render correctly |

**Run the falsifying ablation first**: with the guard removed, the escape row must actually escape
and the bidi name must actually render raw. A fixture that cannot be made to fail proves nothing.

### The measurement

Point **`iced-swdir-tree 0.9.3`** at the fixture and answer D3's eight questions with evidence, then
answer the same eight for **our own composition** where they differ. Report:

- whether the widget renders **our** display text, or its own `file_name()`;
- whether expansion stops at the project root and what it does with each symlink row;
- what it does with 100 000 entries — **timed**, against D8's budget;
- whether its key handling can be made to sit under `KeybindingPolicy`, and what it renders in
  strings of its own;
- what it costs: `swdir` + `rayon` in our process, `lucide-icons`' 561 KB, whether `iced`'s `svg`
  feature becomes required, and the MSRV after adding them — **measured, not assumed**;
- **the decision, by the rule in D3**, recorded as **D3′ in the RFC, in the same commit**, with the
  measurement behind it.

If a library is adopted, **each new crate gets a dated row in `dependency-advisories.md`** in that
same commit (RFC-030 D8's rule).

## PR-052-B — the tree

- Folders **expand in place**; nested children are visible without stepping in. The `Parent` row goes.
- The bounded scan becomes **per level**: 256 children per directory, the collapse list unchanged
  (**D7** — `.git`, `node_modules`, `target` stay collapsed until the ignore slice replaces them).
- Symlink states (`[symlink]`, `[broken symlink]`, `[symlink escapes root]`), `(blocked)` and
  `(unreadable)` all still render, and still as words.
- Keyboard: `Up`/`Down` move, `Enter` opens a file or **toggles** a folder, and the global policy
  still wins.

**Required tests:** a change inside `src/` is visible without stepping into `src/`; each fixture row
renders the state it should; expansion of the 100 000-entry directory stays inside D8's budget;
`Enter` on a folder toggles rather than replaces.

## PR-052-C — how it reads

- A **file-type icon** replaces `[DIR]`/`[FILE]`; **Git status stays a word** (§4).
- Indentation shows depth; the selected row and the keyboard highlight stay distinguishable without
  colour.
- **Evidence:** a live capture against `mktemp -d` in the release binary, showing the tree with
  icons, a Git badge, a nested change visible without navigation, and **the escaping row rendering
  safely**.

## After this

The ignore slice (`REQ-FILE-005`, the `ignored` badge, and a hidden-file toggle) is next, and D7's
collapse list is what it replaces.
