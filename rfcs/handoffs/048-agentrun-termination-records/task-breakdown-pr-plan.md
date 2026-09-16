---
title: "RFC-048 — task breakdown and PR plan"
rfc: "RFC-048"
rfc_file: "../../accepted/048-agentrun-termination-records.md"
source_rfc_status: "Accepted 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# Task breakdown and PR plan

**A then B.** Nothing user-facing changes until B, and B is documentation.

## PR-048-A — the producer and its one path

- `AuditCoordinator` gains a method that **applies a terminal outcome and records it**, mirroring
  `purge_project_transcripts`'s shape. **Production's two call sites move to it**
  (`shell.rs:2755` and `shell.rs:4837`), and `ProjectSession::apply_agent_terminal_outcome` is no
  longer called directly by the product.
- The mapping is D1's, and every code it needs exists: `ProcessExited`, `ProcessTerminated`,
  `RuntimeFailure`. `OrphanedUnknown` records nothing and says so in its return, as
  `record_plain_terminal_terminated` does with `NotRequired`.
- **No schema change.** If one seems needed, stop and say so.

**Required tests**, each read back from a real store:

- A real run that exits leaves `Terminated`/`ProcessExited` after its `Started`, same operation id.
- A run killed through the close flow leaves `Terminated`/`ProcessTerminated`.
- A post-start runtime failure leaves `Terminated`/`RuntimeFailure`.
- **A detached run leaves no record at all**, with the reason named in the test (§1).
- **No field carries payload**: assert the record's fields exhaustively, the way
  `purge_persists_a_completed_record_naming_only_the_project_scope` does — that exhaustive check is
  what proves no summary text, exit status or signal reached it.
- **A degraded store does not block termination**, and the run still reaches its end status.
- **The store refuses a termination for a run that never started** (D6) — the producer does not
  re-check ordering, so this asserts the store does.

**Ablations:** record for a detached run; drop the reason code; put the exit status in; call
`append_required` instead of `append_observation`. Each must fail its own test.

## PR-048-B — say what the trail now answers, and what it still cannot

- **`crates/tekstide-core/README.md`** currently says the trail *"does not answer whether a run is
  still going or how it ended (no termination record exists; RFC-048 is reserved, not authored)"*.
  **Correct it in the same change that makes it false**, and say what replaces it: the ending is
  recorded, and a detached run's is not.
- **The changelog** says the same two halves, in a user's words.
- **The book's audit description** gains the limit: a run Tekstide stopped supervising has no recorded
  ending.

**Required test:** the documentation invariants still pass, and no text claims the trail answers
whether a run is still going — it does not, and this RFC does not change that.
