---
title: "RFC-042 acceptance and QA checklist"
rfc: "RFC-042"
rfc_file: "../../accepted/042-change-content-legibility.md"
source_rfc_status: "Accepted 2026-08-26 — M12, first of three for 0.15.0"
target_milestone: "M12"
created: "2026-08-26"
---

# Acceptance and QA checklist

## D2 — impersonation is unrepresentable

- [ ] A content value cannot be constructed or rendered where a chrome value is expected.
      **Evidenced by a compile failure**, with the error recorded, not by a runtime test.
- [ ] The spoof fixture exists — a file whose first lines are `Detection: Complete`,
      `Review state: Accepted`, `1 file changed` — and a test asserts none of them can be
      confused with the real lines.
- [ ] The renderer no longer discriminates chrome from content by index.

## D1 — the frame does not scroll

- [ ] Heading, detection disclosure, detection status, both omission counts, review state and the
      "not a diff" label are outside the scroll region.
- [ ] A test asserts the label is present with content long enough to scroll.
- [ ] Ablated: label back inside the scroll region, that test fails, restored.

## D3 — bounded, refusing, and distinct

- [ ] A line bound exists in `DiffPreviewPolicy`, beside the byte bound.
- [ ] Over the bound the preview **refuses**. It does not truncate.
- [ ] The refusal names which bound it hit and is distinguishable from RFC-024's byte refusal, the
      stale-baseline refusal, `omitted_changed_file_count` and
      `changed_files_omitted_by_detection`. **Five facts, five sentences.**
- [ ] The bound's value comes from a **measurement recorded in `qa-evidence.md`**, not from a
      choice.

## Escaping is not weakened

- [ ] A fixture containing a tab, a carriage return, an ANSI escape sequence and a bidi override
      renders all of them escaped.
- [ ] Ablated: relax `quote_untrusted` for one of those, that test fails, restored.
- [ ] The line break is the only character this slice stops escaping.

## Fixtures

- [ ] Multi-line ordinary source.
- [ ] Long enough to scroll (D1).
- [ ] The spoof (D2) — **written first**.
- [ ] Over the bound (D3).
- [ ] Other control characters.

## Live GUI evidence

- [ ] Captured against a **`mktemp -d` fixture project**. No path under `$HOME`, no real project
      name, no real file content. See `ARCHITECTURE.md`, "A committed screenshot may only ever
      show throwaway state."
- [ ] Whether a real mouse click was sent is **stated either way**.

## Gates

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] Full workspace suite, **three consecutive runs** under default parallelism, each **logged to
      a file** rather than filtered live, any flake named against `test-process-leak.md`.
- [ ] `git diff --check`, `rfc_docs_invariants`.

## Closeout

- [ ] `ARCHITECTURE.md` gains D1's rule: *a claim that qualifies content stays visible for as long
      as that content is visible.*
- [ ] `ARCHITECTURE.md` gains the fixture rule: *a fixture that omits the shape under test proves
      nothing about that shape.*
- [ ] README's change-review section corrected — it currently discloses the escaping as a shipped
      limitation, which this slice makes false.
- [ ] `CHANGELOG.md`'s `0.14.0` entry is **not** rewritten; it was true when written.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
