---
title: "RFC-073 — task breakdown and PR plan"
rfc: "RFC-073"
rfc_file: "../../done/073-locales-that-survive-installation.md"
source_rfc_status: "Implemented and closed 2026-11-14, as part of the 0.40.0 candidate cut (not yet published) — RFC-073 is in rfcs/done/"
target_milestone: "M14"
created: "2026-11-12"
---

# Task breakdown and PR plan

One slice. The RFC is small, the decisions are four, and splitting it would separate the fix from
the assertion that keeps it fixed — which is the whole subject.

## PR-073-A — compile every catalog in, and gate the list

**Deliverable**, in one commit:

1. **D1.** Every `locales/*.ftl` compiled in with `include_str!`, selected at runtime by name.
   `catalog.rs:21` already does this for `en`; the shape is established, not invented here.
   **`load_locale_from_disk` and `locales_dir()` are deleted**, not kept as a fallback. A disk path
   retained "just in case" preserves every behaviour this RFC removes, including the `$srcdir`
   reference in the shipped binary.
2. **D2.** An explicit `const` list of `(name, contents)` in source — **no build script**. Plus a
   test asserting that list is exactly every `.ftl` in `locales/`, so a catalog added to the
   directory and not to the list fails rather than being silently unreachable. **Resolve
   `locales/` from `CARGO_MANIFEST_DIR`**, never by walking up for `.git/`, so the gate keeps
   working from an unpacked package — same rule as PR-074-A's own gate, same reason.
3. **D3.** `i18n/tests.rs`'s `real_polish_catalog_loads_from_the_shipped_locales_directory` is
   replaced. What replaces it asserts a catalog resolves **with no locales directory present at
   all** — the state every installed copy is in. The rest of that file's scratch-directory tests
   exercise a loader that will no longer exist; re-point or delete them deliberately, and say
   which you did for each and why.
4. **D4.** Fallback semantics unchanged. A locale this build does not carry still falls back
   through the chain to the source bundle.

## What finished means

**The `$srcdir` reference is gone from a real `makepkg` build.** That is the external witness, and
it is the one this project has been walking past since `0.37.0`:

```
cd packaging/aur && makepkg --noconfirm -f
strings pkg/tekstide/usr/bin/tekstide | grep -c 'packaging/aur/src'   # must be 0
```

It must print `0`. Today it prints `1`, and the one reference is immediately followed by the
literal `locales`. Run it before your change as well, so the `1 → 0` is yours and measured rather
than assumed.

**And a non-`en` catalog resolves with the build tree gone.** The RFC's own reproduction, inverted
— build, move the source directory away, run the packaged binary under a non-`en` `LANG`, and get
the catalog rather than the English fallback.

## Verification this slice needs

The ordinary gate (fmt, clippy `-D warnings`, `cargo doc -D warnings`, doc tests, full workspace
suite under a short fixed `TMPDIR`, zero fixture leftovers), plus the two witnesses above, plus:
**the new list-vs-directory gate shown failing** — add a throwaway `.ftl` to `locales/`, confirm
red, remove it. A gate that has only been seen passing is not known to work.

## What is not in this pass

The `--locale` flag and the CLI-parsing item it belongs to. The RFC's *Out of scope* records why
the earlier coupling argument no longer binds once D1 lands; do not reopen it here, and do not
quietly implement half of it.

Translation. `pl.ftl` stays what its own header says it is.

## Final Acceptance Decision

**Accepted 2026-11-14 by the reviewer, for release as `0.40.0`.** Written here because
`.git-exclude/reviewed/` is correspondence and is not tracked; the pack is the record
(`release-checklist.md`'s Review item, added at review 526 after four shipped releases left this
section blank).

**The round.** One slice, review **673**, two rounds.

| Round | Required | What it was |
| --- | --- | --- |
| 1 | 1 | `load_compiled_locale`'s own doc comment claimed a parse failure was *"covered by `tests::source_locale_catalog_parses`'s sibling for every additional locale"*. **No such sibling existed.** A locale added to both `locales/` and `ADDITIONAL_LOCALES` with broken FTL would compile, pass the new list-vs-directory gate, and silently serve English — *listed but unparseable*, the same silent shape D4 says stops existing, re-entered through the new mechanism. |
| 2 | — | Both halves fixed: the comment no longer claims what is not there, and `every_additional_locale_actually_parses` iterates the list. Accepted. |

**What the acceptance rests on**, as distinct from what was claimed:

- The runtime path is **deleted, not supplemented** (D1). Verified on the artifact: the release
  binary built from the candidate carries **zero** references to the repository path, and the two
  remaining `/locales` string matches are `sys_locale` and `intl_pluralrules` symbol names, read
  rather than counted. `Tablica projektu` is inside the binary.
- The list is **gated against the directory** (D2), and the gate was ablated by the reviewer
  independently (`zq.ftl` → red, removed → green, tree clean).
- The test that hid the defect is **replaced by one an installed copy could fail** (D3).
  `Catalog::resolve` no longer has a directory parameter, so "no locales directory involved" is
  what the call site looks like rather than a condition a test constructs.
- **Demonstrated on the shipped artifact**, which is the part that makes this closable: the
  `0.40.0` Run It evidence shows `9 zablokowanych automatyzacji` — the `[many]` CLDR arm, a
  category English does not have — rendered by the release binary under `LANG=pl_PL.UTF-8` from a
  directory with no source tree in sight.

**Deferred rather than fixed, recorded so nobody rediscovers them as defects:**

- **The `--locale` flag and the CLI-parsing item.** `future-work.md` argued they must ship as one
  slice with this RFC, because a flag would let a user select a catalog they structurally cannot
  have. D1 removes that, so the coupling ends with the defect; the flag is ordinary CLI work now.
- **Translation.** `pl.ftl` remains what its header says: three keys proving the pluralization
  machinery, not a translation. This RFC makes it reach the user; it does not make it one.

**What this acceptance does not cover.** Nothing in `packaging/aur/`, and no claim about locales
beyond the one `pl` catalog that exists.
