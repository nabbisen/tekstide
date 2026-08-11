---
title: "RFC-024 — The four decisions: implementation handoff"
rfc: "RFC-024"
rfc_file: "../../done/024-diff-preview-policy.md"
status: "Ready for implementation — accepted 2026-08-11 with the RFC"
created: "2026-08-11"
---

# How to implement each decision, and how each is proven

RFC-024 states *what* was decided and *why*. This document is *how*, and what evidence
each one owes.

## Decision 1 — on demand, detected paths only, never retained

The first two clauses are call-site discipline. **The third should be structural**, and
that is the most important instruction in this pack.

"Content is never retained beyond the request" enforced by a rule in a doc comment is a
rule the next caller forgets. Enforced by a **type that cannot outlive the request**, it
is not a rule at all — it is the only thing that compiles.

This project has done this three times and each one held: `DisplayText` (the only
constructor is `quote_untrusted`, so unescaped text cannot reach a widget requiring it),
`VerifiedCwd`, and `CommandProposal::decode`. RFC-018 PR-018-B's
`paste_bytes_within_bound` returning `Option` rather than a truncated `Vec` is the most
recent: the type had no representation for "shortened content," so the invalid state was
unrepresentable rather than checked for.

**Do the same here.** Borrowed content tied to the request's lifetime, or an owned value
with no path into `ProjectSession`, are both fine. What is not fine is a `String` field on
a long-lived struct plus a comment saying not to keep it.

**Evidence owed:** an enumeration test naming every production call site that reads
generated-change content — the shape `terminal_input_policy_evaluate_has_exactly_one_production_call_site`
and `write_terminal_input_has_exactly_the_three_named_production_call_sites` use. A new
call site fails the test by name rather than being caught in review.

## Decision 2 — refuse, never truncate

**Refuse before reading**, not after. The paste fix is the precedent and its ordering is
the point: `paste_bytes_within_bound` checks the length and returns `None` *before* the
classifier ever sees bytes, so there is no code path on which a shortened input reaches a
decision.

Here that means the size check happens against file metadata, before content is read into
memory at all. A bound enforced after reading has already paid the memory cost the bound
exists to prevent.

**Evidence owed:**

- A boundary test pinning `== bound` accepted and `bound + 1` refused, so the comparison
  cannot silently drift to `>=`.
- An end-to-end test proving an over-bound change produces a **refusal**, and that the
  refusal's identity distinguishes it from any other outcome. PR-018-D's sentinel test is
  the model: it asserted the notice was `TooLarge` specifically, because a weaker
  "nothing was produced" assertion would also pass for a version that truncated and then
  failed for an unrelated reason.
- **No truncation behaviour left to test.** If a truncation test exists at the end, the
  fix is incomplete.

## Decision 3 — reuse the snapshot machinery

`FileSnapshot`, `TextDocument::last_known_snapshot()` and `ExternalChangeDecision` already
answer "has this changed underneath what I last saw." They are reviewed, tested, and have
already caught a real defect — RFC-019 PR-019-E, where a status derived from a source that
had stopped being authoritative told a user their local changes would be discarded when
they had none.

**Do not build a second mechanism.** If the existing machinery does not fit — for example
if a `ReviewBaseline` needs staleness for many paths at once and the per-document API is
awkward — **stop and raise it** rather than writing a parallel one. That is the same escape
hatch RFC-019 gave, and it produced RFC-006 Amendment 1 (a narrow forwarding method that
preserved an invariant) instead of a workaround.

**Evidence owed:** a test proving a stale baseline is *reported as stale* rather than
silently diffed, against a real file changed on disk after baseline capture — not a
synthesised staleness value. RFC-019 PR-019-D's conflict test is the shape: real file,
real external write, real operation, real refusal.

## Decision 4 — classify before reading

Binary detection must not be "read the file as UTF-8 and handle the error." That ordering
reads the whole file to answer a question that costs a bounded sniff — and it defeats
Decision 2, because you have already loaded the bytes the bound was meant to keep out.

Order: size check (Decision 2) → kind classification (Decision 4) → content read, only if
both pass.

**Evidence owed:** a test proving a binary change is reported as a change *without* its
content being read. Proving a negative needs care — an enumeration or an instrumented
reader is more convincing than asserting an absence.

## What this pack does not decide

RFC-024's three open questions are deliberately yours:

1. **The bound's number**, and per-file versus per-review. **Measure the memory profile;
   do not estimate it.** Two estimated figures in this project were wrong once measured.
2. **Whole-review or per-path staleness invalidation.** Decide against the real cost of a
   per-path check, not against which sounds better.
3. **Lazy per-path or eager whole-set diffing.** This one interacts with Decision 2's
   bound shape — answer it before fixing the number.

Record each answer with its reasoning in `qa-evidence.md`, and if any of them changes what
a later slice must do, put that in the later slice's entry too.
