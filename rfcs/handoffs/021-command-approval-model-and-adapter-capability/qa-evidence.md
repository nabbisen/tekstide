# RFC-021: Command Approval Model and Adapter Capability - QA Evidence

Status: Proposed — implementation in progress (PR-021-B landed 2026-07-29)
Date opened: 2026-07-28
Date accepted: Pending

## Scope

RFC-021 defines the command approval model and adapter capability contract. Headless; the rendered dialog is RFC-022.

Evidence in this file must not be used to claim command-level enforcement, OS-level interception, approval for `Plain`/`Supervised` runs, pattern-based auto-approval, a rendered approval dialog, or that any specific AI CLI supports the adapter contract — unless later reviewed implementation explicitly supports that claim.

**Standing constraint on all wording here:** approval is a *cooperative protocol*. Tekstide cannot intercept what a spawned process executes. No entry in this file may imply otherwise.

## Design Review

Per the handoff README, PR-021-A (design/handoff acceptance) is treated as already granted, and PR-021-B..E are authorized to begin immediately and headless, in parallel with RFC-014.

## Implementation Evidence

### PR-021-B — Protocol types and validation

**Module:** `crates/tekstide-core/src/approval/` (new top-level module, `approval.rs` + `approval/protocol.rs` + `approval/tests/protocol.rs`), registered in `lib.rs` alongside `agent`/`audit`.

**What exists:** `CommandProposal` and `CommandDecision`, the two RFC-021 sideband messages, each constructed only through a fallible, validating decoder (`CommandProposal::decode`, `CommandDecision::decode`). There is no way to hold an instance of either type that has not already passed every bound and shape check — deliberately stricter than `audit::record::DurableAuditRecordV1`, whose `validate()` runs as a separate step after construction. That type is built from already-trusted, internally-generated data; these two cross an untrusted boundary from an adapter process, so parsing and validating are one inseparable step here.

**Scope boundary, stated explicitly so it is not mistaken for an oversight:** this slice does not decide whether a token is the *correct* token for a run, whether a proposal id has been seen before, or whether a cwd stays inside the project root. None of that is decidable without context (the real per-run token, prior proposal ids, the project root) that a pure protocol decoder does not have. Those checks belong to `approval::channel` (PR-021-D) and `approval::coordinator` (PR-021-E), and are deliberately left for those slices rather than reached for here.

**argv is a vector, not a shell string, by construction — not by a runtime check.** `CommandProposal::decode` and `CommandDecision::decode` both take `Vec<String>`; there is no code path, in this module or its callers, that could pass a single shell string through this API and have it split into arguments here. The actual "reject a shell-string proposal rather than split it" requirement therefore has to be honored by whatever wire-decoding layer PR-021-D builds (deserializing the raw adapter message into this vector) — that layer must reject a message that supplies a string where the schema calls for an array, not attempt to tokenize it. Recorded here as a boundary condition PR-021-D must satisfy, since PR-021-B's type signature makes the alternative unreachable but cannot make the *wire* format decision.

**Bounds chosen** (all `pub const` in `approval::protocol`, revised per response 109 — see follow-ups below): proposal id and token ≤128/256 bytes respectively, restricted to printable ASCII (`is_ascii_graphic`) so either can be logged or compared without interpretation; argv ≤512 entries of ≤65536 bytes each **and** ≤1 MiB total across all entries combined, no embedded NUL (a NUL cannot appear in a real argv entry passed to `exec`, so its presence is a malformed/adversarial signal, not a legitimate argument) but otherwise permissive of arbitrary content, since Tekstide does not interpret shell grammar; cwd ≤4096 bytes, must be absolute, no embedded NUL; declared intent and declared effects ≤512 bytes each, no control characters (control characters in text a future GUI dialog will render verbatim are a display-spoofing vector in their own right, independent of the RFC-009 terminal-escape concern).

**Empty-string argv entries are accepted**, not rejected. Real commands legitimately take one (`printf '%s' ""`, `grep "" file`, a script passing an empty string as an unset placeholder); the security property that matters is that argv is a vector at all, and entry non-emptiness contributes nothing to that. An initial version of this slice rejected empty entries as a precaution; response 109 Q2 concluded that was a net cost with no corresponding benefit (no constructible attack requires an empty entry, while a rejected proposal pushes users toward routing around approval), and asked me to relax it. Done.

**`UntrustedEffectsHint`** wraps the adapter-declared-effects field so the RFC's rule ("a proposal claiming 'read-only' is a hint for display, never a basis for skipping approval or lowering risk") is visible at the type level: the only accessor is `as_str()`, there is no way to parse it as structured data, and it is a distinct type from the plain `String` used for `declared_intent`. This is type-level groundwork, not the full guarantee — the guarantee is only complete once PR-021-C's risk classifier is built and demonstrably never takes this field as an input, which this slice cannot itself demonstrate.

**`RunCapabilityToken` has a hand-written `Debug` impl that never renders the token value** (`RunCapabilityToken(<redacted>)`), tested directly (`token_debug_output_never_contains_the_raw_value`). This exists because the token is a secret credential per the RFC's "Token leakage" risk, and an accidental `{:?}` in a future log statement should not be able to leak it.

**Tests:** 35 tests in `approval::tests::protocol`, covering: valid decode (with and without optional display fields) and preservation of every field; unsupported protocol version; empty/oversized/whitespace/control-character token, plus an at-the-bound acceptance case; empty/oversized proposal id; empty argv (the zero-entries case, still rejected), too-many-entries, at-the-bound acceptance, oversized entry, NUL-containing entry, a normal entry containing spaces and quotes (demonstrating the "vector, not shell string" property does not require rejecting ordinary punctuation), an accepted empty-*string* entry, and an over-total-size proposal built from entries each individually under the per-entry bound; relative/empty/oversized/NUL-containing cwd; oversized and newline-containing intent; oversized effects hint; every `CommandDecision` combination (`ApprovedOnce`/`Rejected` with and without edited argv, `EditedAndApproved` with and without edited argv, edited argv re-validated through the same argv bounds, unsupported version, invalid proposal id).

**Checklist items this slice satisfies, and items it deliberately does not (see below):** argv-as-vector/shell-string-rejection, unknown-protocol-version rejection, malformed-message rejection with no partial parse, oversized-field rejection, and bounded/content-free diagnostics are all satisfied and tested. Duplicate-proposal-id rejection is not — it requires tracking proposal ids across a run, which needs the coordinator (PR-021-E). "Adapter-declared effects never lower risk or skip approval" has its type-level half done here; the behavioral half needs PR-021-C.

Gates run on 2026-07-29 (after the response-109 follow-ups below): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (410 `tekstide-core`, up from 375 — 35 new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

**Response 109 follow-ups (2026-07-29), all applied:**

1. **Mandatory correction — fields made private.** The stated invariant ("no way to hold an instance that has not passed every bound and shape check") was true for `argv`/`cwd` but not for the `pub` fields (`run_token`, `proposal_id`, `declared_intent`, `declared_effects` on `CommandProposal`; `proposal_id`, `outcome` on `CommandDecision`) — any holder of `&mut CommandProposal` could push `declared_intent` past its bound or swap a token between two decoded proposals, without going through `decode` at all. Response 109 probed this directly (`intent len after mutation = 5120 (bound 512) -> INVARIANT VIOLATED = true`; token swap between two proposals succeeded). All fields are now private with read-only accessors (`run_token()`, `proposal_id()`, `declared_intent()`, `declared_effects()`, `outcome()`), matching the treatment `argv()`/`cwd()` already had. The invariant claim above is now true without qualification.
2. **Recommended, accepted — empty argv entries allowed.** See above; reverses the narrowing recorded in the previous version of this section.
3. **Recommended, accepted — `MAX_ARGV_ENTRIES` raised 64 → 512, `MAX_ARGV_ENTRY_LEN` raised 4096 → 65536, new `MAX_ARGV_TOTAL_LEN` = 1 MiB added.** The count/entry-length bounds were rejecting realistic proposals (`git add` over a large changeset, glob expansion, long commit messages) without a proportionate safety benefit; the new total-size bound answers the actual resource question directly rather than through two proxies, per response 109 Q3.

Noted for PR-021-E, not acted on now: response 109 flagged that RFC-016's escape-and-isolate policy (bidi control characters, which are printable Unicode and not caught by this slice's `is_control` check) is binding on approval surfaces and complements, but does not replace, the control-character restriction here. Read RFC-016 §Security before building the coordinator/dialog-facing surface.

### PR-021-C — Risk classifier

Pending implementation.

### PR-021-D — Sideband channel

Pending implementation.

### PR-021-E — Approval coordinator and audit correlation

Pending implementation.

### PR-021-F — Closeout evidence

Pending implementation.

## What Tekstide May Claim

To be completed at closeout. This section is the honesty artifact — it states in plain language what a user gets, and must be usable verbatim in README and release notes.

Draft constraint for whoever completes it: the claim must distinguish *"Tekstide shows you commands an adapter submits, and does not run them until you decide"* from *"Tekstide controls what the AI CLI runs."* Only the first is true.

## Known Limitations

- Approval applies only to adapter-submitted proposals. An AI CLI that does not implement the contract is unaffected, and `Plain`/`Supervised` runs have no approval path by design.
- No pattern-based or always-allow rules. Every proposal requires a decision, which may prove noisy in practice; the remedy is a reviewed pattern language in a later RFC, not a silent threshold.
- No approval timeout. A pending proposal blocks the adapter indefinitely.
- The exact command is held in memory for display but never persisted, so durable audit cannot answer "what command was approved" — only that an approval occurred, at what risk level, for which run.
