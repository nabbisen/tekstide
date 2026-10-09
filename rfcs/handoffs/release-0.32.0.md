---
title: "Release 0.32.0: a refused close means nothing happened"
status: "**Published 2026-10-09.** Authorised by the owner; published, tagged and verified by the architect. One defect went in; three came out."
rfc_file: "../done/066-a-refused-close-must-not-have-already-terminated.md"
target_milestone: "M13"
created: "2026-10-09"
---

# Release 0.32.0

## What this release is

**A dialog that offered an action it then did not perform.** Confirming a project close while a
document was dirty terminated the project's running terminals, then refused to close, and said
nothing. Following that one claim found **three** separate ways the product behaved as though a
refused close had succeeded:

1. **It killed the terminals first.** `terminate_project_live_work` ran before the assessment was
   consulted. A terminal session is process state — unlike a buffer there is no record of it and
   nothing can offer it back.
2. **It said nothing.** The modal was already dismissed, so the project simply stayed open.
3. **It told the audit store the project closed.** `SafeCloseDecision::Closed` was recorded
   unconditionally, outside the branch that actually closes anything.

The modal now **refuses up front**: when the assessment already blocks, it states the reasons and
offers no confirm button at all, because a button that sometimes does nothing teaches a user that
confirmations in this product are unreliable.

## The bug underneath the third repair

Adding a `Blocked` audit outcome produced **zero records**. It passed the Rust-level validator and
was rejected by the family's own SQL `CHECK` constraint, and `append_observation`'s best-effort
design swallowed the error exactly as it would in production. Fixed with a real `2 → 3` schema
migration following RFC-013 Amendment 1's established shape, with a v2 database proven to migrate
with its rows' sequence intact.

**The class is recorded and not yet fixed**: there are 52 `CHECK` constraints in `audit/schema.rs`
and nothing asserts that a given family and outcome actually lands. Any future producer fails the
same way — silently, in the one store this product promises honesty about. It was found only
because a checklist demanded a test for this single case.

## Two corrections worth keeping

- **A false defect nearly shipped into the permanent record.** The RFC's `Closed` section claimed
  `Ctrl+Alt+N` had no production caller and that the book was wrong to document it. Both were false:
  `shell.rs:2479` dispatches it, and it is a deliberate no-op below two projects — the live capture
  that prompted the claim had one project. Caught at the candidate, removed before publish, and
  **the book was deliberately not "fixed"** to match a defect that did not exist.
- **The register-row check earned itself.** `every_delivery_plan_row_agrees_with_its_rfc_folder`,
  required at RFC-027's closeout after the field went stale at two consecutive releases by two
  different people, caught this release's own stale row immediately.

## The gate, and the publish

Three consecutive full-workspace runs, clean on the first attempt: `758 + 17 + 1119`, 0 failures,
**0 fixture entries left**. Reproduced independently at the candidate and again at the published
commit.

Published with `cargo publish --workspace` (core first), then tagged. **The tag and the publish are
the same commit, `7748543`** — `.cargo_vcs_info.json`'s `sha1` matches it exactly.
`post-publish-check.sh` passes for **`0.32.0` and `0.31.0`**; `LICENSE` and `NOTICE` are in both
published archives and `NOTICE`'s `rusqlite 0.40.2 and libsqlite3-sys 0.38.2` match the published
lockfile. `cargo audit`: the register's three warnings, no vulnerabilities.
`cargo tree | grep accesskit`: empty, so RFC-014 R2 stays closed.

## Left open

- **The flake-register disposition pass**, scheduled at review 497 for after `0.31.0` and still
  outstanding. It has grown two rows during this cycle.
- **The audit `CHECK` class**, above — a pre-1.0 item.
- **No view proof for on-screen text** (RFC-063): a line-function test proves text is produced,
  never that a view places it.
