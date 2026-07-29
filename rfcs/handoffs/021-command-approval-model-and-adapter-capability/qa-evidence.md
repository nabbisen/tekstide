# RFC-021: Command Approval Model and Adapter Capability - QA Evidence

Status: Proposed — implementation in progress (PR-021-B, PR-021-C landed 2026-07-29)
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

**Module:** `crates/tekstide-core/src/approval/risk.rs` (+ `approval/tests/risk.rs`), one public function: `classify(argv: &[String], cwd: &Path, project_root: &Path, state_root: &Path) -> domain::RiskLevel`. Reuses the existing `domain::RiskLevel` type (`Low`/`Medium`/`High`/`Destructive`) rather than defining a new one, per the handoff.

**Design: `Low`/`Medium` are only reachable through an explicit allowlist; everything else is `High` by construction.** The RFC's mandatory property — "unclassifiable input classifies `High`, never `Low`" — shaped the whole function shape, not just one branch: `classify` checks Destructive triggers, then High triggers, then a small `Low` allowlist, then a small `Medium` allowlist, and falls through to `High` if none matched. There is no code path that reaches `Low` or `Medium` without a positive match; an unrecognized program cannot silently default to safe. The corresponding test (`unrecognized_program_classifies_high_never_low`) is written first in the test file, per the handoff's instruction to write that test before anything else.

**Path escaping is checked lexically, not via `fs::canonicalize`.** A proposed command may reference a path that does not exist yet (a new output file, a not-yet-created directory), so requiring the filesystem to agree before classifying anything would be wrong for a proposal that hasn't executed. Every argv entry is resolved against `cwd` (if relative) and normalized by manually walking `.`/`..` path components — no filesystem access, no symlink resolution — then checked for containment under `project_root` (escalate if outside) and separately under `state_root` (escalate if inside). Checking *every* argv entry (not just ones that "look like" flag values) is deliberately conservative and does not produce false positives: an ordinary non-path token like `status` or `-la`, joined with `cwd`, trivially resolves to somewhere inside `cwd` and therefore inside the root.

**Escalation rules implemented, matching `implementation-handoff.md` §4:** path outside project root; privilege elevation (`sudo`/`doas`/`pkexec`, checked by basename so `/usr/bin/sudo` is caught, and scanned across all argv entries so `env sudo ls` is caught too); Git remote-mutating operations (`push`, `remote`, `tag -d`/`--delete`, any `--force`/`-f`/`--force-with-lease`); secret-like path patterns; writes targeting the Tekstide state root. Destructive: `rm`/`rmdir` with a recursive flag, known disk-level programs (`dd`, `mkfs*`, `fdisk`, `parted`, `wipefs`, `shred`), and Git history rewriting (`rebase`, `filter-branch`, `filter-repo`, `reset --hard`).

**One addition beyond the handoff's explicit list, added because the handoff's own logic implies it:** a shell interpreter (`sh`/`bash`/`zsh`/`dash`/`ksh`/`fish`) invoked with `-c` escalates to `High` unconditionally. The RFC says this module does not interpret shell grammar — but a `-c` argument to a shell interpreter *is* an opaque shell string from this classifier's point of view, and treating it as an ordinary argument would let a proposal hide an arbitrary command behind one level of indirection. Flagging this as an addition, not something the handoff asked for verbatim, in case it should be scoped differently.

**Two gaps recorded prominently in the module doc, not left for the reviewer to find first:**

1. **Wrapper indirection is only partially handled.** `env sudo rm -rf /` is caught (the elevation check scans every argv entry's basename). `sh -c '...'` is caught (the addition above). An arbitrary wrapper — `env`, `nice`, `timeout`, `xargs` — running `git push` is **not** unwrapped to recognize the `git` invocation underneath, so `env git push` would currently classify as unrecognized-`High` rather than the more specific "Git remote-mutating" reason, which happens to land on the same level here but would not in general (e.g. `env git status` would also be unrecognized-`High` instead of the correct `Low`). Full wrapper-unwrapping is a materially larger problem than this slice's scope and is left as a known limitation.
2. **The handoff's premise about "secret-like patterns already defined for environment redaction" does not hold.** Searched the codebase (`grep` for `secret`, `redact`, common credential-variable names) and found no existing pattern set — RFC-004 states the *policy* ("Tekstide may redact known secret-like environment variable values...") but no concrete pattern list exists in code today. `SECRET_LIKE_PATH_PATTERNS` in `risk.rs` (`.ssh`, `.gnupg`, `.netrc`, `.pgp`, `id_rsa`/`id_ed25519`/`id_ecdsa`, `.pem`, `.git-credentials`, `credentials`, `.aws`, `.docker/config.json`) is therefore newly written for this slice, not reused. Flagged in the review request as a possible RFC-004 implementation gap, since another RFC may independently need this same pattern set and would benefit from one shared, reviewed list rather than two independently-invented ones.

**Fixture corpus:** `approval::tests::risk::corpus()`, 34 `(name, argv, expected_level)` cases, covering both directions per the checklist requirement — ordinary/ambiguous forms that should *not* escalate (`git status`, `ls`, `cat README.md` → `Low`; `git add`/`commit`, `cargo build`, `npm install`, `cp`, `mkdir` → `Medium`) alongside every escalation rule (root escape by absolute path, by simple `..`, and by deeply-nested `..`; elevation directly, via absolute path, and via `env` wrapper; shell `-c`; each Git remote-mutating form; secret-like paths both absolute and relative; a state-root write; an unrecognized program; each Destructive trigger). Plus three targeted tests beyond the table: subdirectory-relative resolution (both the non-escaping and escaping cases, confirming resolution is relative to the proposal's own `cwd`, not always the root), Destructive-outranks-High when a single proposal triggers both, and empty-argv defensive behavior (unreachable from a validated `CommandProposal` but must not panic).

**Checklist items this slice satisfies:** every Risk Classifier Checklist item — path-outside-root, privilege elevation, Git remote-mutating, secret-like patterns (with the reuse caveat above), state-root writes, Destructive classification, unclassifiable-is-High, corpus covering both directions, no shell-grammar interpretation.

Gates run on 2026-07-29: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (416 `tekstide-core` — up from 410, 6 new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

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
