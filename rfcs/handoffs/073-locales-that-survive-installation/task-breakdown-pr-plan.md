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
