# RFC-042: Change Content Legibility

Status: **Proposed 2026-08-26.** Scoped at the human owner's request as the first theme for
`0.15.0`, after the `0.14.0` release gate found the defect this RFC exists to answer.
Target milestone: **M12**
Date: 2026-08-26

Related RFCs:

- [RFC-041](../done/041-change-content-preview.md) — shipped the preview this RFC makes legible.
  Its own security document, `what-a-content-preview-must-not-claim.md` §5, is the constraint
  here, not an obstacle to work around.
- [RFC-024](../done/024-diff-preview-policy.md) — owns the gate, the 4 MiB bound, the binary
  sniff and the staleness check. **None of that changes.** This RFC is about layout only.
- [RFC-018](../done/018-paste-protection-and-trusted-ui-evidence.md) — owns the trusted-UI
  spoofing boundary, which is the question this RFC actually has to answer.
- [RFC-034](../accepted/034-change-review-actions-and-review-state.md) — **depends on this.** See
  "Why this is first".

## Summary

A previewed file is currently rendered as one logical line with every control character escaped,
so a multi-line file is unreadable. Preserve line structure without weakening the escaping, and
decide — in advance, in writing — what an untrusted file is allowed to do to the surface that
frames it.

## The defect, as observed

Found by the `0.14.0` release gate's **Run It** step, against the release binary and a real
four-line file. Not by the test suite, and not by the implementer's live walkthrough.

Given this file:

```rust
fn main() {
    let x = 1;
    println!("{}", x);
}
```

the preview renders:

```
fn main() {<U+000A>    let x = 1;<U+000A>    println!("{}", x);<U+000A>}<U+000A>
```

`change_review_content_body_text` passes the whole file through `quote_untrusted` as a single
string. A 300-line source file becomes a wrapped block with `<U+000A>` between every line.

**The escaping is correct and is not the defect.** File content is untrusted text drawn in
trusted chrome, exactly as `what-a-content-preview-must-not-claim.md` §5 says, and escaping
control characters is what stops a file an agent wrote from forging the interface around it.
What was never decided is whether a newline is a *character to neutralise* or a *layout
instruction to honour*, and the implementation answered by default rather than by decision.

**Why neither slice's evidence caught it:** every fixture used was single-line. The demo seed
writes an 80-byte one-liner; the tests use short strings. A fixture whose content has no newline
cannot show what happens to newlines. That is the transferable lesson and it belongs in
`ARCHITECTURE.md` at closeout, not only here.

## Why this is first for `0.15.0`

**RFC-034 depends on it.** RFC-034 lets a user record a decision about a change set. Its own
status note asks whether metadata-only inspection is sufficient to act on — a question RFC-041
was supposed to settle by making content inspectable. Content is inspectable and **not
readable**. Shipping "mark this change set Accepted" while the only route to inspect it renders a
real source file as one line builds an approval control on top of a review surface a user cannot
actually read.

This project has corrected a control that implied more than it delivered three times. Ordering
these two the other way round would be the fourth.

## The question that makes this an RFC

**May an untrusted file occupy N visual lines inside trusted chrome, and what bounds N?**

Today it occupies one logical line the layout wraps, so the answer is "no, by accident."
Splitting on `\n` and escaping each line separately is mechanically small — perhaps five lines of
code — but it hands a file control over how many rows of the surface it consumes and where each
row begins. That is precisely what §5 was answering. **The code change is trivial and the
decision is not**, which is why this is an RFC and not a wiring slice.

## Decisions required

**D1 — can content push or scroll the surface's own claims out of view?**
A 100,000-line file is comfortably inside RFC-024's 4 MiB bound. The "not a diff" label and the
detection disclosure are what make this surface honest. If content can scroll them away, the
label is defeated by the thing it labels. Decide whether the framing text is pinned, whether the
content region scrolls independently, or whether line count is bounded so the question cannot
arise — and prove the chosen answer with a file large enough to test it, not a fixture.

**D2 — can a line of file content be mistaken for a line of Tekstide's own?**
A file containing `Detection: Complete` or `Review state: Reviewed` at the start of a line, drawn
in the same style as the real ones, is a spoof of exactly the kind RFC-018 exists to prevent.
Decide the answer — a visual boundary, a distinct style for the content region, a per-line
gutter, or something else — and make it falsifiable: a test whose fixture *is* a spoof attempt.
**Do not answer this by asserting that users will notice.**

**D3 — what bounds the line count, and what does the surface say when it truncates?**
RFC-024 bounds bytes. Bytes do not bound rows: a 4 MiB file of single-character lines is four
million rows. If a line bound is introduced it is a **third** omission fact, and this project
already keeps two apart on this very surface because one number cannot say which happened
(`omitted_changed_file_count` vs `changed_files_omitted_by_detection`, split in `0.14.0` after
review response 326). A truncated preview must say it is truncated, in its own words, and must
not be confusable with either existing count.

## Scope

1. Render previewed content with its line structure preserved, every character other than the
   line break escaped exactly as today.
2. Whatever D1, D2 and D3 decide — pinning, bounding, visual separation — implemented, not
   assumed.
3. The surface says what it does to content it cannot show in full.

## Non-goals

- **Weakening `quote_untrusted` for anything but the line break.** Every other control character
  stays escaped. If the answer to D2 turns out to require escaping the line break too, that is a
  legitimate outcome of this RFC and the defect is then closed by disclosure instead.
- **Syntax highlighting, line numbers as a feature, or an editor.** Legibility here means "the
  lines are lines," nothing more.
- **A two-sided diff.** Still blocked on RFC-030.
- Changing RFC-024's gate, bound, sniff or staleness check.

## Risks

- **Answering the cheap half and skipping the expensive half.** The rendering change is small
  enough to look finished before D1–D3 are answered. If it lands first, RFC-041's own security
  document is weaker after this RFC than before it. **The decisions are the deliverable; the
  rendering falls out of them.**
- **A fixture that cannot show the failure.** This defect survived two rounds of evidence because
  every fixture was single-line. Fixtures here must include: a multi-line file, a file long
  enough to test D1, and a file that attempts the D2 spoof.

## Acceptance-time decisions

**D1, D2 and D3 are decided by the architect on acceptance and recorded in this file before any
implementation begins.** An implementer must not inherit an unresolved architecture decision —
the same rule RFC-041 was accepted under.
