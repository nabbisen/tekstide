---
title: "RFC-035: Acceptance / QA Checklist"
rfc: "RFC-035"
rfc_file: "../../accepted/035-change-detection-coverage-and-disclosure.md"
source_rfc_status: "Accepted 2026-08-18 — scheduled 2026-08-25"
target_milestone: "M12"
created: "2026-08-25"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The acceptance criterion

- [ ] **An agent run that writes `.git/hooks/pre-commit` shows that file on the change review
      surface.** Proven against a real written hook, not a constructed `DetectedChanges`.
- [ ] A run changing more paths than `max_changed_paths` shows the first N **and** an honest count
      of the omitted rest, instead of nothing.

## PR-035-A — the supervision hole

- [ ] `.git/hooks/` and `.git/config` both watched; everything else under `.git/` still excluded.
- [ ] **Readers of the exclusion enumerated before it was changed**, with what each does — the
      explorer among them.
- [ ] The explorer still collapses `.git/`; a user does not get a tree full of `objects/`.
- [ ] A changed hook renders as an ordinary changed path — no new severity, icon or list.
- [ ] `core.hooksPath` not followed; deferral recorded with its reason.
- [ ] Ablated **separately**: `hooks/` removed → its test fails; `config` removed → its test fails.

## PR-035-B — `max_changed_paths`

- [ ] A completed scan over the limit keeps the first N and reports the omitted count.
- [ ] `ChangeSetSummary`'s existing fields populated rather than a second shape added.
- [ ] `Partial { limit }` still distinct from display truncation, **tested with both true at
      once** — the first slice where that is possible.
- [ ] `max_entries` behaviour unchanged.
- [ ] Ablated: restore the discard, the bounded-list test fails.

## Closeout

- [ ] **RFC-020's on-surface disclosure text corrected** — it names the `.git/` exclusion and is
      read by every user who opens change review.
- [ ] `README.md`'s exclusion statement corrected.
- [ ] Deferrals stated: `hooksPath`, mid-run triggers.
- [ ] Gates: `fmt`, `clippy -D warnings`, full suite **three runs under default parallelism**,
      `git diff --check`.
- [ ] Flakes disclosed against `test-process-leak.md`'s three causes.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
