---
title: "RFC-016 PR-016-E — Enforcement: implementation handoff"
rfc: "RFC-016"
rfc_file: "../../proposed/016-internationalization-and-localization.md"
slice: "PR-016-E"
status: "Ready for implementation — unblocked 2026-08-01 by RFC-015 PR-015-D's render_text decision"
created: "2026-08-01"
---

# PR-016-E — Enforcement

`task-breakdown-pr-plan.md` describes this slice in three lines, written on 2026-07-29:

> Scope: no-hardcoded-strings scan; catalog-completeness test; advisory unused-key report.
> Review gate: the scan actually catches a deliberately introduced hardcoded string — demonstrate it, do not assert it.

That is still the scope. **Everything else this slice inherited accumulated afterwards, across five review responses, and was recorded in `qa-evidence.md`'s Known Limitations — which is where results get written, not what you read before starting.** This document consolidates it. The gap was mine.

## Why this slice was blocked, and why it no longer is

PR-016-E's scan scope depended on an open question: whether `tekstide_core::shell::render_text` — a pre-GUI text harness holding ~16 hardcoded user-facing strings — would still exist when the scan ran.

**Settled 2026-08-01.** RFC-015 PR-015-D investigated and **kept it deliberately**: it is the primary assertion mechanism for roughly twenty `tekstide-core::shell::tests`, and deleting it is a separate core test-suite refactor. So the scan cannot be written on the assumption that core is clean by the time it runs. It is not, and will not be soon.

## The four sites, precisely

All four are user-facing English the catalog cannot reach. Three are in `tekstide-core`, which by design owns state and policy and never renders — so it cannot depend on `crates/tekstide/src/i18n`.

| # | Site | What | Found in |
| --- | --- | --- | --- |
| 1 | `tekstide-core::shell::render_text` | ~16 strings (`"content status: "`, `"(project root)"`, `"active file: none"`, …) | response 122 |
| 2 | `tekstide-core::project_board::CountDisplay::label()` | `"not available"`, `"not implemented"`, `"unknown"` | response 123/124 |
| 3 | `crates/tekstide/src/main.rs` | four `eprintln!("{error}")` — text from `tekstide-core` error `Display` impls, not literals this crate wrote | response 128 |
| 4 | `tekstide-core::project_board::ProjectBoardRow` | `trust_label`, `security_mode_label`, `availability_label`, `blocked_automation_labels` | response 132 |

Site 4 is the one that matters most: those are **genuinely rendered by the real GUI today** (`crates/tekstide/src/surface/board.rs`), so they are live untranslated strings in a shipped surface, not latent ones.

Site 2 is already defended in the other direction: PR-015-D never calls `label()` and added a crate-wide scan (`no_count_display_or_attention_label_is_called_anywhere_in_the_crate`) that fails if anyone reaches for it. The strings still exist in core; nothing in the GUI renders them.

## The decision this slice must make, and how to make it

**What does the scan do about `tekstide-core`?** Three options, and only one is honest:

- **Scan only `crates/tekstide`.** Rejected: RFC-016's rule is that *no* user-facing string is hardcoded. A scan that cannot see three quarters of the violations reports a clean bill of health that is false.
- **Scan both, no exemptions.** Rejected: it fails on day one and blocks this slice behind a `tekstide-core` API change (exposing enums instead of pre-rendered labels) that is genuinely out of scope here.
- **Scan both, with a closed exemption list.** ✅ Take this one.

"Closed" is doing the work in that sentence. An exemption list that anyone can append to is how enforcement decays — response 129's warning, and the reason PR-015-B's file-scoped seam scan had to be generalised in the first place. So:

1. Each exemption names a **specific site**, not a file glob or a whole crate.
2. Each carries a **disposition**: who fixes it and under which RFC.
3. **A test fails if a violation appears outside the list**, and **also fails if a listed exemption no longer violates** — a stale exemption is a lie in the other direction, and it is the one nobody notices.

That third property is the difference between a tracked debt and a permanent excuse.

## Disposition for the four sites — decide these, do not leave them floating

My reading, which you should push back on if you disagree once you have the scan in front of you:

- **Sites 1 and 2** are core-internal. Nothing in the GUI renders them. The honest disposition is that they are exempt while unreachable from a rendered surface, and the exemption dies when `render_text` does. Whichever RFC refactors `tekstide-core::shell::tests` off `render_text` owns removing it.
- **Site 3** (`main.rs`'s `eprintln!`) is not a hardcoded literal, so a literal-matching scan will not catch it anyway. It belongs in the report as a *known category* the scan does not cover, not as an exemption from a rule it does not violate. Say so rather than silently omitting it.
- **Site 4** is the live one and needs a real owner. Fixing it means `ProjectBoardRow` exposing enums rather than pre-rendered English, so the shell selects catalog keys — a `tekstide-core` API change. **Raise it as a scope question rather than absorbing it here**; it is plausibly its own small slice, and it is the item most likely to be quietly exempted forever if nobody names it.

## Also in this slice

**The Fluent-type-exposure guard** (response 126, recommended and folded here). PR-016-D closed the untrusted-text interpolation bypass structurally — `CatalogArgs` with `number`/`untrusted(&DisplayText)`/`trusted_symbol(&'static str)`, and no public `FluentArgs`/`FluentValue` re-export. But `tekstide` is `[[bin]]`-only with no `[lib]` target, so a `compile_fail` doctest could not be written; the guarantee currently rests on a one-time manual build probe recorded in `qa-evidence.md`.

Nothing automated stops someone re-adding `pub use fluent_bundle::…` later. A mechanical check that `i18n` exposes no `fluent_bundle` type, and offers no raw-`&str` interpolation path, belongs in this harness because the harness is being built anyway.

**Catalog completeness** now has something real to check. `en.ftl` has grown from two keys to roughly twenty across PR-016-B/C/D and RFC-015, and `pl.ftl` exists as a genuine second locale. Completeness means every key in the source locale resolves in every shipped locale, with the advisory unused-key report as the other direction.

**Scan duplication with RFC-015.** PR-015-B built crate-tree seam scans over `crates/tekstide/src` (no raw string literal to `text()`, no raw `Color::from_rgb`, no raw `.size()` literal), generalised in response 128 to walk the tree so new files are covered automatically. PR-016-E's scan overlaps that policy for strings. **One should absorb the other — do not let both survive.** Two mechanical checks for one policy is how they drift apart, which is the lesson `text_safety` already taught this RFC. My preference is that PR-016-E's scan becomes canonical for strings and PR-015-B's string rule delegates to it, keeping colour and font-size where they are; but you are the one who will see both, so decide with the code in front of you and record why.

## Review gate

Unchanged and non-negotiable: **the scan must be demonstrated catching a deliberately introduced hardcoded string, not asserted to.** Ablate it — introduce a violation in a file the scan is supposed to cover, watch the named test fail with the offending file and line, revert.

Then do the same for the exemption list's second property: make a listed exemption stop violating, and confirm the test fails for *that* reason too. That one is easy to skip and it is the half that keeps the list honest.

## What this slice does not do

- It does not fix the four sites. It makes them visible, attributed, and impossible to grow silently.
- It does not translate anything. Content is RFC-016 §Non-Goals.
- It does not add `Cn` (unassigned-codepoint) coverage to the escape predicate — deferred at response 118 pending a dependency decision, and unchanged.
