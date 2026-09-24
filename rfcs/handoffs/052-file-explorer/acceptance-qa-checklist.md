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

## Required at review 421 — before PR-052-B

- [x] **The suite stops leaving its fixtures behind.** Measured by the reviewer on this machine:
      **42 932 entries under `/tmp`**, of which **14 780 `tekstide-run-*`**, **3 504
      `tekstide-audit-test-default-*`** and **1 517 `approval-audit-*`**. This is what filled a 30 GB
      tmpfs to 100 %, cost this slice a whole gate attempt, and then cost a second one when the
      `TMPDIR` workaround hit the 108-byte socket limit. Give the top three builders RAII cleanup —
      `HostileFixture` and RFC-030's `Fixture` already drop theirs — and pin it with a test that a
      builder's directory is gone after its guard drops. Not RFC-052's subject; it is the thing that
      will break the next gate too.
- [x] **The `TMPDIR` lesson is in `ARCHITECTURE.md`**, not only the flake register: a gate that must
      move its temp directory needs a **short** one, because the approval socket lives under it and a
      Unix socket path is limited to 108 bytes. That is "how to run the gate here", the same class as
      the `wtype` and screenshot notes.

- [x] **Residue, ruled at review 422:** the `tekstide-shell-test-*` family and the short-named
      `t`/`tsr`/`tsms` builders get the same treatment as **PR-052-B's first commit**, before B adds
      fixtures of its own. *Reviewer-measured after the first fix: 498 entries a run, none of the three
      targeted families, 1 492 after three runs.* `ARCHITECTURE.md`'s number should then read zero.
      *(Done as B's first commit, `97576f3`: **a full run leaves 0 entries**, and `ARCHITECTURE.md` says
      so.)*

## PR-052-B — the tree

- [x] Folders expand in place; a change inside `src/` is visible without stepping into `src/`
      (`REQ-FILE-001`). The `Parent` row is gone. *(`enter_on_a_folder_expands_it_in_place_…`;
      `evidence/01-…`.)*
- [x] Bounded **per level**: 256 children per directory; the collapse list unchanged (D7). *(Unchanged,
      and still openable — the judgment call in `qa-evidence.md`, which the reviewer can reverse in one
      line.)*
- [x] A root-escaping symlink is **reported, not followed**; a broken symlink says so; an unreadable
      directory is a row. **Ablation:** follow the symlink; **its own escape test fails, and any other
      test sharing that fixture is named.**
      *Corrected at review 423: the box said "fails alone". It fails the escape test and the
      detail-area test, which legitimately uses the same escaping fixture. Third time a "fails alone"
      box of mine has been falsified by a suite sharing fixtures properly — the wording is the error,
      not the suite.* *(All three
      properties hold and are pinned. **The ablation's "alone" is not literally true**: following the link
      fails the tree's escape test **and** `the_detail_shows_…`, whose fixture is the same escaping row and
      which asserts the row is not marked expandable. Left unticked with the contradiction named rather
      than ticked over it.)*
- [x] Every name renders through `quote_untrusted` — including the non-UTF-8 one, which must not take
      the whole tree down with it. *(Through the type system: `DisplayText` has no raw constructor. The
      non-UTF-8 name is one of three in the core tree test and keeps its exact bytes in its path.)*
- [x] `Enter` toggles a folder and opens a file; global keybindings still win; a modal still
      suppresses. *(`the_explorers_keys_arrive_as_sidebar_surface_input_…` and
      `handle_explorer_key_has_exactly_one_production_call_site_…`: the explorer has no key handling of its
      own, and a modal cannot produce the `ModalAbsent` proof the routing takes.)*
- [x] **A total render bound, decided by measurement** (review 421): ~4 µs a row means ~4 000 rows fill
      a frame, and twenty open directories at the per-level cap is already ~19 ms. Total bound or
      viewport virtualisation, chosen the way D3 was — and **nothing is hidden silently**: a row says
      how many are not shown. *(Virtualisation: 3.0 ms for the window of a 10 000-row tree against 951 ms
      to build it whole, debug build. A line says which rows are on screen; a capped folder and a tree past
      its bound each end in a row naming how many are not shown.)*
- [x] **The scan is `Task`-shaped from the first commit** — 65 ms at depth 1 500 crosses a frame.
      *(A worker thread per scan, the `git_summary_subscription` shape, pinned by
      `the_shell_never_scans_a_directory_on_the_render_thread`; disclosed as a subscription rather than a
      `Task`.)*

## PR-052-C — how it reads

- [x] A file-type icon replaces `[DIR]`/`[FILE]`; **every status is still a word** and the
      colour-alone scan still passes. *(Text-symbol icons, measured: colour emoji cost 6x to lay out. Only
      two kinds of icon — a folder (closed/open) and a file — not per-extension icons, which need an icon
      font; that is a dependency and asset decision, named in `qa-evidence.md`.)*
- [x] Selection and keyboard highlight are distinguishable without colour. *(`>` is the keyboard; the word
      `[open]` is the open file; independent, pinned.)*
- [x] Live capture against `mktemp -d` in the release binary: icons, a Git badge, a nested change
      visible without navigation, the escaping row safe. Throwaway state only. *(`evidence/01-…`,
      `03-…`; the escaping row is in `01-`.)*
- [x] Folders first (ruled at review 423 to belong here).

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
