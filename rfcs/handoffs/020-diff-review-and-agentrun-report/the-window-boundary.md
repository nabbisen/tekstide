---
title: "RFC-020 — The window boundary and the third escaping position: implementation handoff"
rfc: "RFC-020"
rfc_file: "../../proposed/020-diff-review-and-agentrun-report.md"
status: "Required reading before any RFC-020 code"
created: "2026-08-15"
---

# Two things this RFC gets wrong if nobody writes them down

## 1. The transcript window starts somewhere the filter was never proven for

**This is the security-critical decision in RFC-020, and it is invisible from the RFC.**

RFC-017 PR-017-B/C established four properties for the terminal filter. The fourth,
**P4 (stream-position independence)**, says the filter classifies identically regardless of
how the byte stream is chunked. It was enumerated and ablated, and it holds.

**P4 covers chunking where every byte arrives.** RFC-011 Amendment 1's reader does not
chunk — it returns a **bounded window over the tail**, which means it **drops the prefix**.
That is a different operation and P4 says nothing about it.

### What goes wrong

The first byte of the window can land inside a CSI or OSC sequence. The filter then sees a
stream that begins mid-sequence, which no code path could previously produce, so no
evidence covers it. Two concrete failures:

- The **trailing fragment** of a control sequence is presented as ordinary text — the user
  reads bytes the terminal would never have displayed.
- An **unterminated sequence** at the window start swallows the text that follows it, so
  real transcript content silently disappears from the report.

Both are classification differences produced by **where the read started**. That is exactly
what P4 exists to deny, arrived at through a door P4 does not cover.

### What is required

**Resynchronize.** Advance from the raw window start to the first position where a fresh
parse is sound, and **report the delivered start offset, not the requested one**. A caller
that asked for the last N bytes and received a window starting elsewhere must be able to
see that, or it will compute wrong positions against its own copy.

**Do not split a UTF-8 scalar** at either edge.

**Do not "fix" this by escaping earlier.** Escaping is the surface's job (see below), and
an escape applied before resynchronization escapes a fragment whose meaning has not been
determined yet.

### Evidence owed

- A window starting **inside a control sequence** classifies identically to the same
  content read whole. Construct the fixture deliberately — find a real sequence boundary
  in real captured output, do not synthesise a convenient one.
- **Ablate the resynchronization.** Remove it, and show the *specific* divergence with the
  exact wrong value. A green ablation here is a defect in the ablation, not a pass; this
  project has hit that at least six times.
- The delivered offset is reported and differs from the requested one in the ablated case.

**If resynchronization turns out to be impossible without parser state the filter does not
expose, stop and raise it.** The honest fallback is reading from the transcript's start and
discarding, which costs time but cannot misclassify. Shipping a window that may begin
mid-sequence is not an option.

## 2. Both new surfaces escape, and neither inherits an existing exception

RFC-019 settled two positions. This RFC adds a third, and the reasoning matters more than
the rule, because the rule is what gets copied to the next surface.

| Surface | Treatment | Justification |
| --- | --- | --- |
| Terminal grid | raw | Escaping would corrupt it — the control sequences *are* the rendering |
| Editor text area | raw | The user is editing these bytes; an editor that rewrites what it shows is broken |
| Chrome everywhere | escaped | Tekstide describing something, not the user's content |
| **Diff review** | **escaped** | Reviewed, not edited |
| **AgentRun transcript** | **escaped** | Not a grid |

**Neither existing exception transfers, and it is worth being precise about why.**

The editor exception is justified by **editing**: you must see bytes exactly as they are
because you are about to change them and save them. A diff is *reviewed*, not edited. The
justification does not carry.

The grid exception is justified by **corruption**: escaping terminal output destroys the
thing being rendered, because the sequences drive the grid. A transcript report is not a
grid — it is a record being read. That justification does not carry either.

### For diff review, escaping is the stronger position, not a compromise

A reviewer deciding whether to accept an AI-generated change **wants** to see that the
change introduces `U+202E`. That is the Trojan Source case exactly, and it is why other
review tools warn on bidi controls rather than rendering them faithfully.

**A diff that renders an override invisibly hides the most dangerous thing it could
contain.** Escaping is not a safety tax paid at the cost of fidelity here — it is the
feature.

### Where escaping happens

**At the widget, not in the model.** Both RFC-024 (`DiffContent`) and RFC-011 Amendment 1
(the reader) return **raw, unescaped bytes**, deliberately:

- a model that pre-escapes hides the file's actual content from every non-rendering
  consumer, and makes "what is really in this file" unanswerable;
- RFC-011 already states that captured bytes remain untrusted and that any renderer must
  take them through RFC-009's boundary before display.

So the escape belongs in `crates/tekstide`, at the point of rendering, using
`text_safety::quote_untrusted` and `DisplayText` — the same primitive every other escaped
surface uses. **Do not add a second escaping primitive**, and do not escape twice: a
double-escaped `<U+202E>` renders as literal text about an override rather than a marker
of one, which is a different lie.

### Evidence owed

- **The falsifiable claim, tested:** a generated change containing a bidi override renders
  it visibly as an escape marker in the diff surface. State it as a claim that could be
  false.
- **Raw bytes survive the model layer unaltered**, proven against the same bidi probe
  `text_safety`'s own tests use — the property RFC-024 PR-024-C already proved for
  `DiffContent`, re-proved at the reader.
- **No double-escaping**, shown by rendering content that already contains the literal text
  `<U+202E>` and confirming it is distinguishable from a real override.
