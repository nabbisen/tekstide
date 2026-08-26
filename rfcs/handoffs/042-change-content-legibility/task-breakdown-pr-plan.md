---
title: "RFC-042 task breakdown and PR plan"
rfc: "RFC-042"
rfc_file: "../../accepted/042-change-content-legibility.md"
source_rfc_status: "Accepted 2026-08-26 — M12, first of three for 0.15.0"
target_milestone: "M12"
created: "2026-08-26"
---

# Three slices, in this order

The order is load-bearing. **PR-042-A carries no user-visible change at all** and exists so that
the visible change in PR-042-C cannot be made unsafely. Doing C first and A afterwards ships the
concession without the payment.

## PR-042-A — chrome and untrusted content stop being the same type (D2, structural half)

**No behaviour change. No visible change. This is the point.**

`change_review_content_lines` returns `Vec<String>` today, mixing lines Tekstide wrote with lines
an agent's file wrote, told apart by the renderer's `if index == 0`.

Give the two a type each. Shape is yours; the property is not: **after this slice it must not be
possible to pass a content value where a chrome value is expected, or to render one through the
other's path.** Use the idiom already here — `DisplayText`'s single `quote_untrusted` constructor,
`DiffContent` carrying Added/Modified in the constructor rather than a field, the exhaustive
matches in `keyboard_help.rs`.

The renderer stops discriminating by index. `index == 0` becomes a variant match.

**Ablation:** make a content value constructible where a chrome value is expected. It must fail to
compile. A compile failure *is* the ablation here — record the error, not a test name.

Gate: full suite green with no test changed except where the type moved.

## PR-042-B — the frame stops scrolling (D1)

Split `render_change_review`'s single `scrollable(column(lines))`. Heading, detection disclosure,
detection status, both omission counts, review state and the **"not a diff" label** render in a
fixed frame. Only the content region scrolls, inside it.

**Still no line-splitting.** Content is one escaped line at this point and the frame must already
be correct, so that B's evidence is about the frame and nothing else — one variable per slice.

**Ablation:** put the "not a diff" label back inside the scroll region; the D1 test fails.

**Evidence:** the release binary, a fixture file long enough that the content region genuinely
scrolls, screenshot after scrolling to the bottom, showing the label still on screen. **A `mktemp
-d` fixture project — never a path under `$HOME`.**

## PR-042-C — lines become lines, bounded (D2 visible half, D3)

Now, and only now:

1. `change_review_content_body_text` splits on `\n` and escapes **each line** with
   `quote_untrusted`. Every other control character stays escaped.
2. Content renders inside its own visually distinct container — its own bounds and background,
   visibly not part of the surrounding surface.
3. A line bound in `DiffPreviewPolicy`, beside the byte bound. **Over it, refuse; never truncate**,
   matching RFC-024's own "refused whole above that, never truncated."
4. The refusal names which bound it hit, distinct from RFC-024's byte refusal, the stale-baseline
   refusal, and both omission counts.

**The bound is measured.** Render at candidate bounds, measure against this project's existing
latency criteria (state change, not pixels — `ARCHITECTURE.md`), set the constant from the number,
put the measurement in `qa-evidence.md`. Do not ship a bound you chose.

**Ablations, separately:**
- Escape the line break again → the multi-line test fails.
- Relax `quote_untrusted` for a character other than the line break → the control-character
  fixture test fails.
- Truncate instead of refusing over the bound → the refusal test fails.

**Fixtures, all five from the pack README**, and the spoof fixture written **first**.

## Not in this plan

- Syntax highlighting, line numbers as a feature, an editor.
- A two-sided diff (RFC-030).
- Any change to RFC-024's gate, byte bound, binary sniff or staleness check. D3 **adds** a bound
  beside them; it alters none of them.
