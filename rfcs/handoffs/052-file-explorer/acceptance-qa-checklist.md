---
title: "RFC-052 — acceptance and QA checklist"
rfc: "RFC-052"
rfc_file: "../../accepted/052-a-file-explorer-a-user-can-read.md"
source_rfc_status: "Accepted 2026-09-24 — M12 remainder"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over.

## PR-052-A — fixture and decision

- [x] **The fixture is hostile, proven by ablation, run first**: with the guard removed the symlink
      row escapes the root and the bidi name renders raw. *(`with_the_guards_removed_…`, then each
      production guard removed in turn; hashes restored. See `qa-evidence.md`.)*
- [x] Every row exists: bidi override, newline, non-UTF-8, root-escaping symlink, broken symlink,
      100 000 entries, deep nesting, unreadable directory, and an ordinary control tree.
      *(The unreadable row asserts its own precondition and skips under root, where mode 000 refuses
      nothing.)*
- [x] **D3's eight properties answered with evidence** for the widget, and for our own composition
      where they differ.
- [x] The 100 000-entry expansion is **timed** against D8's budget. *(Widget: 2.2–2.3 s build +
      layout. Composition: 2.1–2.3 ms.)*
- [x] Cost measured, not assumed: `swdir`/`rayon`, `lucide-icons`' size, whether `iced`'s `svg`
      feature becomes required, MSRV after the additions. *(Seven crates, no pool built, no
      `lucide-icons`, no `svg`, +159 KB, 1.90 holds. Measured for `ItemTree`; `DirectoryTree` is
      rejected first.)*
- [x] **D3′ recorded in the RFC in the same commit**, by D3's rule, with the measurement behind it.
      *(Own composition. `ItemTree` measured too and recommended against — named in D3′ as a judgment
      the reviewer can overturn.)*
- [x] Any new crate has a dated row in `dependency-advisories.md` in that same commit. *(Vacuously:
      no crate is adopted. The cost measurement's `cargo audit` is in `qa-evidence.md`: the same three
      carried advisories, nothing new.)*

## PR-052-B — the tree

- [ ] Folders expand in place; a change inside `src/` is visible without stepping into `src/`
      (`REQ-FILE-001`). The `Parent` row is gone.
- [ ] Bounded **per level**: 256 children per directory; the collapse list unchanged (D7).
- [ ] A root-escaping symlink is **reported, not followed**; a broken symlink says so; an unreadable
      directory is a row. **Ablation:** follow the symlink; the escape test fails alone.
- [ ] Every name renders through `quote_untrusted` — including the non-UTF-8 one, which must not take
      the whole tree down with it.
- [ ] `Enter` toggles a folder and opens a file; global keybindings still win; a modal still
      suppresses.

## PR-052-C — how it reads

- [ ] A file-type icon replaces `[DIR]`/`[FILE]`; **every status is still a word** and the
      colour-alone scan still passes.
- [ ] Selection and keyboard highlight are distinguishable without colour.
- [ ] Live capture against `mktemp -d` in the release binary: icons, a Git badge, a nested change
      visible without navigation, the escaping row safe. Throwaway state only.

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
