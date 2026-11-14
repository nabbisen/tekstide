---
title: "M14's security review: what it is, what it may conclude, and what it must not reproduce"
status: "Scope note. Opened 2026-11-15, after RFC-074's first slice closed in 0.40.0. Not an RFC; the amendments it proposes become one."
rfc_file: "none — this is the plan for a review gate, not a design"
target_milestone: "M14"
created: "2026-11-15"
---

# M14's security review

`ROADMAP.md`'s M14 names two things this is: *"final security review and threat-model update"*, and
a review gate, *"security and privacy review across every surface."* RFC-074 was inserted ahead of
it on the argument that it is *"what makes the rest of M14 reviewable"* — that argument is now
testable, because RFC-074's first slice closed in `0.40.0`.

This note is the plan. It is deliberately not the review.

## The fact that shapes everything below

The threat model exists and it is **older than almost everything it would now be reviewing**. Its
own header dates it 2026-07-04, refined 2026-07-28 *"after architecture review, requirements
refinement, external design refinement, UI/UX refinement, and M5-M7 implementation"*, and scopes
itself to the **MVP / v0.1 design baseline**.

Since then this product has shipped M8 through M14's substance: the GUI shell, the terminal
surface, content surfaces, approval dialogs, configuration, the file workflow, cross-platform
support, CI and release automation — forty-odd RFCs. **A baseline written before the work is not a
record of the work.** So the "update" half of M14's gate is not a formality attached to the review;
it is most of the job.

## Governance: nobody edits the source document

`.git-exclude/specs/tekstide-security-threat-model-v0.md` is the owner's, in the same class as
`tekstide-external-design-v0.md`, whose §4.2 RFC-076 D2 superseded **by RFC** while RFC-076's own
pack said in as many words: *"The accepted RFC supersedes it and is the record; the source document
is the owner's. Do not edit it."*

The same rule holds here. This review **proposes**; an accepted RFC **supersedes**; the source stays
the owner's to change or not. A review that edited it would be deciding the owner's own baseline on
their behalf, which is not what a review is for.

**And it must not be copied out, either.** That file is gitignored — the owner chose not to publish
it. This note, and anything the review produces, lives in the tracked tree, so it cites sections by
number and title and quotes only what a finding cannot be stated without. Reproducing a private
security document into a public repository by quotation is the same disclosure as publishing it,
performed slowly.

## What the review is, in three parts, cheapest first

### Part 1 — §14's ten open security questions: answered, or still open

The threat model's own §14 lists ten questions it says *"should be resolved before or during RFC
development."* Several plainly have been, by RFCs this project accepted and shipped since; at least
one is in flight as this is written. **Each question gets one of two dispositions: the accepted RFC
that resolved it, named and dated — or "still open", said plainly.**

This is the cheapest part and the highest value: it is entirely checkable against `rfcs/done/`, it
produces the spine of the threat-model update, and it is the part where "we decided this years ago
and never wrote it down against the question" is most likely to be true.

One worked example, so the shape is not in doubt: §14's question about which AI CLI should receive
the first Managed adapter, *if any*, is the subject of **RFC-069, accepted 2026-11-15** — and the
honest disposition is "being answered now, by a slice whose first deliverable is a measurement that
may yet answer *if any* in the negative", not "answered".

### Part 2 — §13's recommended security RFC sequence against what shipped

§13 recommends eight security RFCs by subject (S1–S8). Every one maps onto delivered work or onto a
deliberate deferral, and **the mapping is the deliverable**: subject, the RFC(s) that covered it,
and the gap if there is one.

At least one gap is already visible without doing the work: **S5's redaction half has no RFC.**
Redacting what a run prints is reserved as RFC-068, `pre-1.0`, and is not written. A security
sequence the threat model itself recommended has an unwritten entry, and the review should say so
rather than discover it at 1.0.

### Part 3 — §8's controls and §12's acceptance criteria, surface by surface

The longest part, and the one RFC-074 was supposed to make possible. §8 enumerates sixteen controls;
§12 enumerates MVP acceptance criteria in seven groups. Each is verified against what the product
actually does.

**The enumeration basis is the module list**, which is now a defensible way to say "every surface"
rather than a hopeful one: RFC-074's own gate derives it from source and asserts its shape, and
after `0.40.0` the production graph is eight strongly-connected modules plus the rest, with
`test_support` out of it entirely. A review that says "every surface" and means "the ones I thought
of" is the failure this project has a name for; a review that enumerates from the gate's own module
set can be checked.

## What counts as evidence

This project's standing standard, unchanged for a security review: **measured, not asserted.** A
control that has only ever been seen passing is not known to work — the same rule that produced
RFC-074's ablation requirement and the release checklist's *"every containment guard has been shown
to FAIL"* item. Where a control can be made to fail, the review shows it failing. Where it cannot,
the review says why, and that is a finding about the control.

A grep is not evidence of absence. A passing test is evidence about the property the test holds,
not about the property its name suggests.

## What this review may not conclude

- **Not a penetration test, not a cryptographic review, not an audit of dependencies' internals.**
- **Not a claim of sandboxing.** The threat model's own purpose section says it *"intentionally
  avoids overclaiming full sandboxing for arbitrary terminals or arbitrary third-party AI CLIs"*,
  and the review inherits that restraint rather than quietly relaxing it.
- **Not a statement about any platform it did not run on.** macOS evidence is constrained the same
  way every other gate here is constrained — no Apple hardware is available to this project — and
  the review discloses that in its own conclusions rather than letting CI's green stand in for it.

## Known gaps, named now so they are not reported later as discoveries

- **The plugin boundary (§6.6, §8.16) is deliberately out of scope.** No plugin runtime ships;
  Appendix A's MVP position is unchanged. The review records it as deferred, not as unreviewed.
- **The AI CLI boundary (§6.5) is in flight**, as RFC-069. The review states what is true on the day
  it is written and does not pre-judge a measurement that has not happened.
- **The threat model's status line says "Draft".** Whether the update leaves it a draft is the
  owner's call, and it is the kind of thing worth deciding deliberately rather than inheriting.

## Who does what

- **The reviewer** writes Parts 1 and 2 — they are documentary, they are about decisions and their
  records, and they are the half that produces the proposed amendments.
- **Part 3's per-surface verification is implementation work** and belongs with whoever can make a
  control fail on demand; it is scoped into slices after Part 1 names what is actually open.
- **The owner** decides what the amendments do to their own document, and whether this review gates
  1.0 on its own findings or merely records them.

## What is not in this note

The findings. Any claim about whether a control holds. Any schedule. Part 1 is the next thing
written, and it is the reviewer's.
