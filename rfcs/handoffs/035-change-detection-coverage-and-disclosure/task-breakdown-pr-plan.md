---
title: "RFC-035: task breakdown and PR plan"
rfc: "RFC-035"
rfc_file: "../../done/035-change-detection-coverage-and-disclosure.md"
source_rfc_status: "Implemented and closed 2026-08-25 — RFC-035 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-25"
---

# Two slices, A then B

## PR-035-A — the supervision hole: watch `.git/hooks/` and `.git/config`

**Read [`what-watching-dot-git-must-not-become.md`](./what-watching-dot-git-must-not-become.md)
first.** Both watches are needed and neither alone is sufficient.

**Build:** a narrow allow-list inside the `.git/` exclusion covering `hooks/` and `config`.
Everything else under `.git/` stays excluded.

**Before writing it, enumerate who reads the exclusion.** §4: the explorer collapses `.git/` too.
If detection and the explorer share one list, narrowing it for detection changes the explorer as a
side effect. Say what you found, per reader, before changing anything — the same enumeration
PR-039-C did for the sessions map before adding a `Drop`.

- A changed hook renders as an ordinary changed path. **No new severity, icon or list** (§3).
- `core.hooksPath` is **not** followed to its target; watching `config` reports that the location
  changed, which is the fact that matters. Record the deferral with its reason (§2).

**Evidence:** a real agent run that writes `.git/hooks/pre-commit` appears on the change review
surface. If a real run is impractical, say so and state the substitute — the precedent and its
disclosure are in RFC-020's own closeout.

**Ablate:** remove `hooks/` from the allow-list, confirm the test that sees a written hook fails.
Then the same for `config`, separately. Two properties, two ablations.

## PR-035-B — `max_changed_paths` shows what it found

**Build:** when a completed scan exceeds `max_changed_paths`, keep the first N paths and report
how many were omitted, instead of returning an empty list.

- `ChangeSetSummary` already models this (`shown_changed_files`, `omitted_changed_file_count`) and
  nothing populates it from the detector. Populate it rather than adding a second shape.
- `Partial { limit }` keeps its meaning. A truncated **scan** and a truncated **list** stay two
  facts — RFC-020's surface renders them as two lines and **this is the first slice where both can
  be true at once.** Test that case specifically.
- `max_entries`' behaviour is unchanged and correct: a truncated scan genuinely cannot distinguish
  "unchanged" from "not looked at."

**Ablate:** restore the discard, confirm the test that asserts a bounded-but-populated list fails.

## Closeout

Fold into PR-035-B rather than a separate slice — this RFC is two items, not a programme.

- Correct what this work falsifies. **RFC-020's surface disclosure text names the `.git/`
  exclusion**; after PR-035-A that sentence is no longer wholly true, and the surface says it to
  every user who opens it.
- `README.md` states the exclusions too.
- State the deferrals with reasons: `hooksPath` not followed, mid-run detection triggers still
  exit-only (RFC-035 item 3).

## Standing expectations

- Single-variable ablations, the unit being the design decision.
- Disclose flakes against `test-process-leak.md`, which now records **three** causes; the third is
  unfixed and carries an audit-honesty question scheduled separately.
- **If your slice makes a shipped statement false, correcting it is part of your slice** — and
  this slice falsifies a sentence rendered on a user-facing surface, not only in documentation.
