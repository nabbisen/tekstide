---
title: "M14 security review, Part 2 — the threat model's recommended security RFC sequence against what shipped"
status: "Part 2 of the review opened in security-review-m14.md. Written 2026-11-15. Findings, not amendments."
rfc_file: "none — review output"
target_milestone: "M14"
created: "2026-11-15"
---

# Part 2 — §13's recommended sequence, against what shipped

The threat model's §13 recommends eight security RFCs by subject, each with its own list of
sub-topics. **The sub-topics are the useful granularity.** A subject-level mapping is easy and
nearly useless — every subject has an RFC against it — and every gap this part found is one
sub-topic inside a subject that is otherwise covered.

Referred to by subject, not reproduced, for the reason `security-review-m14.md` gives.

## The mapping

| § | Subject | Covered by | Verdict |
| --- | --- | --- | --- |
| S1 | Workspace trust and restricted mode | RFC-004, RFC-032 | **Covered.** Trust states incl. `Revoked` (`project/metadata.rs`, `security.rs`), the prompt, restricted-mode blocking, revocation with its own `AuditEventClass::TrustRevoked`, and stored trust identity (Part 1, Q3). |
| S2 | ProjectSession isolation and local state | RFC-002, RFC-058 | **Covered.** Per-project ownership, state layout under `XDG_STATE_HOME`, identity by canonical root, and cross-project contamination prevented by the project lock. |
| S3 | Terminal security and process lifecycle | RFC-008, RFC-009, RFC-018, RFC-054, RFC-066 | **Covered.** PTY behaviour, paste protection and trusted-UI evidence, ANSI policy, the scrollback cap, and a safe close whose refusal path RFC-066 repaired. |
| S4 | AgentRun security contract | RFC-010, RFC-021, RFC-022, RFC-046, RFC-060, RFC-011/049/050, RFC-031 | **Covered, with one half in flight.** Compatibility levels, approval, transcript retention, diff review and audit events all shipped; the Managed adapter half is RFC-069, accepted 2026-11-15. |
| S5 | Environment and secret policy | RFC-004 §7 (+ profile policy types) | **Two sub-topics missing.** See F5 and F6. |
| S6 | File access and symlink policy | RFC-026, RFC-035, plus the canonicalization and root-boundary work | **Covered.** Canonicalization and symlink-component checks (`project/root.rs`, `project/change_detection.rs`), root boundary, external files, watcher policy, conflict handling. |
| S7 | Local audit and purge policy | RFC-013, RFC-031, RFC-047, RFC-059 | **Covered but for export.** Schema, retention, purge (reachable since RFC-059) and a *decided* tamper-evidence question; audit export does not exist. See F7, F8. |
| S8 | Plugin security baseline | — | **Deferred by standing position**, same as Part 1's Q9. No plugin runtime ships; Appendix A's MVP position holds. |

## Findings

### F5. S5's redaction posture is the one real gap, and it is not abstract

RFC-068 (*Redacting What A Run Prints*) is reserved `pre-1.0`, split out of RFC-059 on its
acceptance (2026-10-11), and **unwritten**. Its own delivery-plan row states the situation exactly:
a transcript holds raw run output, and **the AgentRun Report exports 16 KiB of it to a user-named
path**.

That is a live export path, today, in the shipped product: `REPORT_TRANSCRIPT_TAIL_BYTES = 16 *
1024` in `agent/report.rs`, written wherever the user chooses. And the product's own claim about
redaction is already correctly scoped — `RedactionClaimScope` has exactly **one** variant,
`StructuredMetadataOnly`. So the type system says the honest thing and the user is never told it at
the point where it matters.

**This is Part 1's Q10 seen from the other side**, and one RFC closes both. Its dependencies
(RFC-011, RFC-059) are both done, so nothing blocks it but writing it.

### F5 corrected, 2026-11-15, by its own author — the gap is real but much narrower than stated, and the requirement behind it is already met

F5 above called S5's redaction posture *"the one real gap"* and pointed at RFC-068. **Checked
properly before acting on it, after RFC-069's D1 was withdrawn for exactly this failure — reading
a list or a loosely-related requirement as a mandate.** Three things it found:

**1. `REQ-SEC-023` is satisfied, and in the strongest possible way.** Verbatim: *"Environment
variables with secret-like names must be redacted from logs and audit views."* The product puts
**no environment content into audit records at all** — searched `domain/audit.rs` and `audit/*`
and found none — and it operates no logging subsystem that would carry them. Nothing can leak
there because nothing is written there. The one secret the product does hold, the run capability
token, already prints as `RunCapabilityToken(<redacted>)`.

So `delivery-plan.md`'s description of RFC-068 as *"`REQ-SEC-023`'s log half"* overstates the
link. The requirement does not reach the thing RFC-068 is reserved for.

**2. What remains is a third party's output, not this product's.** A transcript holds what the AI
CLI printed. If a secret appears there, the CLI printed it. **Scanning that for secrets is the
same shape as RFC-069's withdrawn D1** — taking responsibility for behaviour this product does not
control — and the reservation's own row already names why it fails: *"a scanner that misses one
secret is worse than no claim."*

**3. The honest instrument is disclosure, which is what the threat model actually asked for.**
§14 Q10 asks whether exported transcripts and handoffs *"should include a safety banner warning
about possible secrets"* — a **question about a disclosure**, not a mandate to redact. And the
product already has the pattern: `RedactionClaimScope` has exactly one variant,
`StructuredMetadataOnly`, which states the limit correctly and is never shown to the user at the
point of export.

**Revised recommendation.** RFC-068 as reserved — *"Redacting What A Run Prints"* — describes a
scanner this project has already reasoned against and a requirement that does not demand it. The
narrow version is **a disclosure at the export point: say what the file may contain, and do not
claim it has been cleaned.** That is one sentence of product behaviour, it closes §14 Q10 honestly,
and it stays inside this project's own boundary.

**This is a recommendation to the owner, not a scoping decision.** Renaming or re-scoping a
reserved RFC is theirs, and the reviewer has just been wrong once this week about exactly this
class of reasoning.

### F6. S5's "provider disclosure" has no mechanism at all, and the adjacent pattern exists

Nothing in the product discloses **where an AI CLI sends what it reads**. Measured: no production
code names a provider, an endpoint, or outbound transmission in any disclosure sense.

What does exist is the neighbouring pattern, one step away:
`AiCliWorkspaceDiscoveryPolicy::MayDiscoverWorkspaceFiles { summary }`, which the built-in profile
populates with a plain-language statement that the CLI reads workspace files. So the product
already knows how to say *"this tool may read your files"* and has no way to say *"and it may send
them somewhere."*

For a product whose posture is local-first, whose non-goals explicitly exclude sandboxing
third-party AI CLIs, and which therefore relies on disclosure where it cannot rely on control,
**this is the disclosure a user is most likely to want and the one that is missing.** It is not
obviously RFC-068's — redaction is about content, this is about destination — and the review's
recommendation is that it be decided rather than absorbed silently into a neighbouring RFC.

### F7. S7's "export" is undecided, not missing, and the review should not report it as a gap

There is no audit-store export, and **no requirement asks for one.** `REQ-SEC-025` names purge, not
export. A run *report* export exists (`agent/report.rs`), which is a different artifact.

So the honest disposition is that §13 listed a sub-topic the project never adopted. The question
for the owner is whether audit export belongs in the product at all — not a gap to close by
reflex, and listing it as one would be the review inventing work the document never justified.

### F8. S7's tamper-evidence question was decided, against, and that deserves recording

RFC-013's own non-goals exclude tamper-evident, cryptographically signed, and
append-only-at-the-filesystem-level logs. §13 asked for a "tamper-evidence decision"; the decision
was made and it was *no*. An answered sub-topic recorded as answered is worth as much as a gap
found, and a later reader should not have to re-derive it from a non-goals list.

### F9. The sequence was followed in substance and not in naming

No RFC in this project is called S1 through S8, and a reader searching for them finds nothing.
Every subject was covered by between one and seven numbered RFCs written as the work arose. That is
not a defect — but it means **the recommended sequence has never been checked off against anything
until now**, which is why a sub-topic like S5's redaction could sit reserved and unwritten for a
month without anyone noticing it was part of a sequence the threat model itself asked for.

## Proposed amendments to the threat model

Proposals only; the source document is the owner's and this review does not edit it.

1. **Record S1–S4, S6 as delivered**, against the RFCs named in the table.
2. **Record S7 as delivered except export**, with the tamper-evidence decision marked *decided, no*
   and export marked *not adopted* rather than outstanding.
3. **Record S5 as partially delivered**, with redaction posture pointing at RFC-068 and provider
   disclosure flagged as having no owner yet.
4. **Record S8 as deferred**, with the same deliberate-closure note Part 1's F4 asks for on Q9.

## What this part does not claim

That any control works. S1 through S8 being "covered" is a statement about which RFCs exist and
what they decided, verified in the code only far enough to confirm the mechanism is present. Part 3
is where presence becomes behaviour.
