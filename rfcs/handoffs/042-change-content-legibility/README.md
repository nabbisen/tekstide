---
title: "RFC-042: Change Content Legibility — implementation handoff"
rfc: "RFC-042"
rfc_file: "../../done/042-change-content-legibility.md"
source_rfc_status: "Implemented and closed 2026-08-26 — RFC-042 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-26"
---

# Make the preview readable without making it forgeable

Source RFC: [RFC-042](../../done/042-change-content-legibility.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-042](../../done/042-change-content-legibility.md) | Three decisions already made; do not re-open them |
| 2 | [`what-a-legible-preview-must-not-become.md`](./what-a-legible-preview-must-not-become.md) | **Required.** The whole risk of this slice is here |
| 3 | [RFC-041's security document](../041-change-content-preview/what-a-content-preview-must-not-claim.md) | §5 is the constraint D2 exists to satisfy, not to work around |
| 4 | [RFC-024](../../done/024-diff-preview-policy.md) | Owns the byte bound, gate, sniff and staleness. D3 adds a bound beside them and changes none of them |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Three slices |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |

## The one-sentence version

A previewed file renders as one escaped line; give it back its lines **without** giving an
untrusted file the ability to impersonate the surface or to scroll that surface's own claims out
of view.

## Where the defect lives

`crates/tekstide/src/shell.rs`:

- `change_review_content_body_text` — passes the whole file through `quote_untrusted` as one
  string. This is where the line breaks die.
- `change_review_content_lines` — returns `Vec<String>`, mixing lines Tekstide wrote with lines an
  agent's file wrote. **This is the real defect**; the escaping is a symptom of it.
- `render_change_review` — ends in `scrollable(column(lines).spacing(8))`, one scroll region for
  everything, and tells chrome from content with `if index == 0`.

## Read the decisions as decisions

D1, D2 and D3 are settled in the RFC. Three things about them worth saying twice:

**The rendering change is the small part.** Splitting on `\n` is a few lines. If it lands before
D1 and D2 are implemented, RFC-041's own security document is weaker after this slice than
before it, because a file gains line control over a surface that has no defence against it. The
decisions are the deliverable.

**D3's bound is measured, not chosen.** The RFC deliberately does not name a number. Measure,
record the measurement in `qa-evidence.md`, set the constant from it. "Expect the low thousands"
is a sanity check on your measurement, not the answer.

**D2 asks for unrepresentability, not care.** A convention that content lines get a different
style is a convention. A type that cannot carry a content line where chrome is expected is a
guarantee. This project has `DisplayText`, the exhaustive `NavigationAction` matches, and
`DiffContent`'s constructor-carried Added/Modified distinction as precedent — use the idiom that
is already here.

## Fixtures, because this is how the defect survived two rounds of evidence

Every fixture used by RFC-041 was **single-line**: an 80-byte demo seed and short test strings.
A fixture whose content has no newline cannot show what happens to newlines, which is why the
suite passed and the implementer's live walkthrough passed and the defect shipped.

This slice needs, at minimum:

1. **A multi-line file** — ordinary source, four to ten lines. The base case.
2. **A file long enough to test D1** — long enough that the content region genuinely scrolls, so
   "the label is still visible" is a real assertion and not a vacuous one.
3. **A spoof file** — first lines reading `Detection: Complete`, `Review state: Accepted`,
   `1 file changed`. This is D2's test and its fixture *is* the attack.
4. **A file over the D3 bound** — proving refusal, and proving the refusal says which bound it
   hit.
5. **A file whose lines contain other control characters** — tab, carriage return, an ANSI escape
   sequence, a bidi override. These must still be escaped. The line break is the only character
   this slice stops escaping.

**Fixture 5 is the one most likely to be skipped and is the one that proves the slice did not
weaken `quote_untrusted` generally.**

## Live GUI evidence

Required, and per `ARCHITECTURE.md` as amended in `0.14.0`: **capture against a `mktemp -d`
fixture project**, with fixture file names and fixture content. Never a real project, never a path
under `$HOME`. A committed screenshot is a verbatim publication of whatever was on screen, and
this application renders canonical project paths on purpose.

If you can send a real mouse click, do, and say so. `0.14.0`'s own gate could not — no pointer
injection tool is installed — and said so. Either is fine; silence is not.

## Deferrals to state, not to solve

- No syntax highlighting, no line numbers as a feature, no editor.
- No two-sided diff. Still RFC-030.
- If D2's container turns out to require escaping the line break after all, that is a legitimate
  outcome — the defect then closes by disclosure instead. Reach it by argument, in writing, not
  by finding the layout hard.
