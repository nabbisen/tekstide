---
title: "Doc invariants completion — QA evidence"
rfc: "none"
source_rfc_status: "No RFC. Implementation evidence for doc-invariants-completion.md."
target_milestone: "M12"
created: "2026-08-27"
---

# Evidence

## Shared predicate, not a second one

`claims_unfinished(status: &str) -> bool` factored out of `every_pack_status_field_agrees_with_its_rfc_folder`'s
own inline expression, unchanged in meaning (`status.contains("Proposed") || status.starts_with("Accepted")`).
Both the existing check and the new one below call it — one predicate, per this slice's own instruction.

## Gap 1 — the RFC's own `Status:` line, checked against its folder

`every_rfc_own_status_line_agrees_with_its_folder`: reads the RFC file's own `Status:` line
(prose near the top — `# RFC-NNN: Title`, a blank line, `Status: **...**...` — not front matter,
unlike the sibling check's `source_rfc_status`), applies the same folder-vs-claim logic the
existing check already has.

**One real wrinkle, found by running the check against the real corpus before fixing anything.**
An RFC's own `Status:` line is a full prose sentence (or several), not a short, single-purpose
field — and this project's own established closeout shape (RFC-041: *"**Implemented and closed
2026-08-26.** Proposed and accepted 2026-08-25 …"*) deliberately narrates history **after** a
bolded, current-state lead. Applying `claims_unfinished` to the *whole* line flagged RFC-041,
042, and 043 as false positives — each says "Proposed"/"Accepted" as **history**, correctly, after
a leading `**Implemented and closed**` claim. Fixed by reading only the bolded lead claim (the
text between the first `**...**` pair, falling back to the whole trimmed line if there is none),
not by loosening the predicate — the predicate itself stays byte-identical to the sibling check's;
only the substring it is applied to changed, which is the actual difference between a
single-field claim and a narrated one.

**Reproduced, then fixed.** Run before any correction, real output:

```
an RFC's own Status: line asserts a state its folder contradicts, and the folder wins (RFC-000):
  rfcs/done/038-first-run-and-project-entry.md
    says "Status: **Accepted by the human owner 2026-08-24.** ..." but sits in rfcs/done/
  rfcs/done/039-interaction-model-and-visible-affordances.md
    says "Status: **Proposed 2026-08-24.** ..." but sits in rfcs/done/
  rfcs/done/040-affordance-completion.md
    says "Status: **Proposed 2026-08-25**, ..." but sits in rfcs/done/
  rfcs/done/035-change-detection-coverage-and-disclosure.md
    says "Status: **Accepted by the human owner 2026-08-18.** ..." but sits in rfcs/done/
  rfcs/done/020-diff-review-and-agentrun-report.md
    says "Status: **Accepted 2026-08-12** ..." but sits in rfcs/done/
```

Exactly the five the handoff named, once the lead-claim fix above removed the three false
positives. Corrected in the same commit as the check, per the handoff's own required shape,
matching RFC-041's own closeout template — the original claim kept visible, un-bolded, folded
into the sentence, not deleted:

| RFC | Old lead | New lead |
| --- | --- | --- |
| 020 | `**Accepted 2026-08-12**` | `**Implemented and closed 2026-08-25.** Accepted 2026-08-12` |
| 035 | `**Accepted by the human owner 2026-08-18.**` | `**Implemented and closed 2026-08-25.** Accepted by the human owner 2026-08-18.` |
| 038 | `**Accepted by the human owner 2026-08-24.**` | `**Implemented and closed 2026-08-24.** Accepted by the human owner 2026-08-24.` |
| 039 | `**Proposed 2026-08-24.**` | `**Implemented and closed 2026-08-25.** Proposed 2026-08-24.` |
| 040 | `**Proposed 2026-08-25**,` | `**Implemented and closed 2026-08-25.** Proposed 2026-08-25,` |

Closure dates taken from each RFC's own row in `rfcs/delivery-plan.md`, not invented.

## Gap 2 — every `accepted/`/`done/` RFC has a delivery-plan row

`every_accepted_or_done_rfc_has_a_delivery_plan_row`: collects every pipe-table row's first cell
across `rfcs/delivery-plan.md`, keeps exactly-three-ASCII-digit ones (a header row's `RFC`, a
separator row's `---`, and an unrelated table's own first column like `RFC-021 command approval`
all fail that check and are silently skipped, matching the handoff's own instruction to match on
nothing else), and flags any `accepted/`/`done/` RFC number not in that set. `proposed/`
deliberately excluded.

**Two exclusions, both found empirically before being added, not assumed.**

1. **RFC-000 through RFC-013.** The unscoped check flagged all fourteen. `delivery-plan.md`'s own
   header says "Covers: M8 through M14," and its own prose names RFC-014 as that coverage's start
   ("RFC-014's substrate outcome constrains every GUI RFC after it"). Verified directly:
   `grep -c "^| NNN |"` for every RFC 000-020 shows zero rows for 000-013 and exactly one row for
   every RFC 014 and above — a clean, real boundary, not a guessed one. The check now skips any
   RFC below 014.
2. **RFC-037.** The five-folder lifecycle *policy* RFC itself — not "startable work," the same
   shape RFC-000 already gets a narrow, named exemption for in the sibling link-check. The
   document's own prose names it directly ("look in `rfcs/accepted/`, which holds exactly those
   (RFC-037, 2026-08-19)") without giving it a queue row. One line, named to this one RFC, not a
   pattern.

With both exclusions in place: clean.

## Both new checks, ablated

**Status-line check**: temporarily rewrote RFC-041's own `Status:` line to a bare `**Proposed
2026-08-25**, ablation test line.` — the check failed, naming exactly that file and quoting the
line. Restored via `git checkout`.

**Delivery-plan check**: temporarily deleted RFC-041's real row from `rfcs/delivery-plan.md` (per
the handoff's own instruction — "temporarily removing a real row, not adding a fake RFC, the unit
is the design decision, not the line") — the check failed, naming `RFC-041`. Restored via
`git checkout`.

Both checks pass again after restoration; all four tests in `rfc_docs_invariants.rs` (the two
pre-existing plus the two new ones) green together.

## Not in scope

The README keyboard-table check, as the handoff itself names — a separate slice needing a
decision this one does not make (generated vs. merely checked).

## Gate

`fmt`, `clippy -D warnings`: clean. Three consecutive full-workspace runs under default
parallelism, each logged to a file (`/tmp/tekstide-doc-invariants-gate-run-{1,2,3}.log`): 449 +
4 + 746 + 0 + 0, clean every time — `rfc_docs_invariants` itself now reports 4 tests, up from 2.
`git diff --check`: clean.
