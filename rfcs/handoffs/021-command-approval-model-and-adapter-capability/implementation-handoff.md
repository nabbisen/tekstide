---
title: "RFC-021: Command Approval Model and Adapter Capability - Implementation Handoff"
rfc: "RFC-021"
rfc_file: "../../proposed/021-command-approval-model-and-adapter-capability.md"
target_milestone: "M11"
source_rfc_status: "Proposed"
created: "2026-07-28"
updated: "2026-07-28"
---

# RFC-021 Implementation Handoff

Covers PR-021-B through PR-021-F. All headless — no GUI dependency.

This is a **security boundary**, like RFC-013's audit constraints and RFC-009's terminal parser. Expect the same review standard: I will probe it empirically rather than accept that it looks right.

## 1. Where this code lives

New module `crates/tekstide-core/src/approval/`, siblings of the existing `audit/` and `agent/` modules:

```
approval.rs           module root and re-exports
approval/protocol.rs  CommandProposal, CommandDecision, validation
approval/risk.rs      argv-based risk classifier
approval/channel.rs   sideband endpoint, token, listener lifecycle
approval/coordinator.rs  proposal → ApprovalRequest → audit correlation
approval/tests/       per the project's test-module convention
```

Reuse, do not replace: `domain::ApprovalRequest`, `domain::ApprovalDecision`, `domain::RiskLevel`. They already exist and already align with RFC-013's frozen audit vocabulary — `ApprovedOnce`/`Rejected`/`EditedAndApproved` map exactly onto `command_approve`/`command_reject`/`command_edit_and_approve`.

## 2. The channel — get this right first

**Unix domain socket, one per AgentRun, under the Tekstide state root, outside every project root.**

Apply the path discipline RFC-011 and RFC-013 already established: absolute, canonicalized, symlink-rejecting, never inside a project root. Reuse that reasoning rather than inventing new path handling.

Lifecycle:

1. Tekstide creates the endpoint **before** process start.
2. Generates a per-run capability token.
3. Passes the token through the environment allowlist — this is the one place an approval secret crosses into the child. It must not be written to disk unencrypted and must never reach an audit record.
4. Destroys the endpoint when the AgentRun terminates.

Socket file permissions must restrict access to the owning user. A world-accessible approval socket is a critical defect.

**Never the PTY stream.** If you find yourself parsing stdout for approval markers, stop — that is the RFC-009 violation this design exists to prevent.

## 3. Protocol

Two messages, bounded and versioned. Model the validation on `audit/record.rs`, which is the project's reference for strict, bounded, fail-closed decoding.

`CommandProposal` (adapter → Tekstide): protocol version, run token, proposal id, **argv as a vector**, cwd, bounded intent text, optional adapter-declared effects.

`CommandDecision` (Tekstide → adapter): proposal id, decision, edited argv when `EditedAndApproved`.

Hard rules:

- **argv, never a shell string.** A shell string lets quoting make the rendered command differ from the executed one. Reject any proposal that supplies one.
- **Adapter-declared effects are display hints and nothing more.** A proposal claiming "read-only" must never lower risk or skip approval. Treat it exactly as you would treat terminal output: untrusted.
- Unknown protocol version → reject, do not negotiate down.
- Every string field bounded. Reuse the `AuditReference`-style validation approach for opaque identifiers.
- Malformed input produces a bounded, content-free diagnostic.

## 4. Risk classifier

Structural inspection of argv and cwd only. **Tekstide does not interpret shell grammar** — that is an explicit non-goal, and pretending otherwise would be the semantic-analysis overclaim the requirements forbid.

Escalate to at least `High`:

- any path argument outside the canonical project root;
- privilege elevation (`sudo`, `doas`, `pkexec`, platform equivalents);
- Git remote-mutating operations (`push`, `remote`, force variants, tag deletion);
- paths matching the secret-like patterns already used for environment redaction;
- any write targeting the Tekstide state root.

Escalate to `Destructive`: recursive deletion, disk-level operations, history rewriting.

**Unclassifiable input is `High`, never `Low`.** Write that test first.

Build a **fixture corpus** — a table of argv vectors with expected classifications — the same shape as RFC-013's adversarial corpus. It is the artifact I will extend at review with cases you did not think of.

## 5. Fail-closed matrix

Implement every row. Each needs a test.

| Condition | Required behavior |
| --- | --- |
| User does not respond | Pending indefinitely; adapter blocks. **No timeout may approve.** |
| Adapter disconnects while pending | Abandon, audit as rejected-by-disconnect, no execution implied |
| AgentRun terminates while pending | Same as disconnect |
| Audit append fails for the authorization | **Block execution.** Approval is a required audit write — same precedent as RFC-013 trust grant |
| Token absent, invalid, or from another run | Reject without surfacing a dialog; bounded diagnostic; treat as possible spoofing |
| Duplicate proposal id | Reject the second |
| Malformed or oversized message | Reject; never partially parse |
| Unknown protocol version | Reject |

There is deliberately **no approval timeout**. A timeout that denies is hidden policy; one that approves is a vulnerability. Blocking is the honest behavior.

## 6. Audit correlation

Conform to RFC-013's frozen `command_approval` family. Do not amend the schema.

| Event | action_kind | operation_id | outcome | actor / source |
| --- | --- | --- | --- | --- |
| Proposal received | `command_request` | none | `requested` | `app_policy` / `adapter` |
| Approve | `command_approve` | required | `authorized` → `applied`/`failed` | `user` / `trusted_ui` |
| Edit and approve | `command_edit_and_approve` | required | `authorized` → `applied`/`failed` | `user` / `trusted_ui` |
| Reject | `command_reject` | none | `applied` | `user` / `trusted_ui` |

The schema also requires `approval_id` and `risk_level` present, `terminal_id` and `subject_kind` absent. Read the CHECK constraint in `audit/schema.rs` before writing records — it will reject anything that does not conform, which is by design.

**No command text, argv, cwd, or intent text in any durable record.** `ApprovalRequest.display_command` is in-memory only. Write a test asserting a sentinel command string never appears in the audit database, modelled on RFC-012's sentinel privacy test.

Use `AuditCoordinator` (`audit/integration.rs`) rather than writing to the store directly — it owns ordering and health, and the trust-grant path there is your reference for authorization-before-effect.

## 7. Edit-and-approve

When the user edits argv, **re-run the risk classifier on the edited vector**. The audit record must describe what was approved, not what was proposed. An edit that lowers apparent risk must not carry the original classification, and an edit that raises it must be reflected.

## 8. What you must not build

- Any approval path for `Plain` or `Supervised` runs.
- Pattern-based or "always allow" rules. The threat model permits these only behind a reviewed pattern language with its own RFC.
- A rendered dialog. RFC-022 owns that; expose a decision API the UI will call.
- Approval for user-typed terminal commands — those are explicit user action, and paste protection already covers the accidental case.
- OS-level interception of any kind.

## 9. What I will probe at review

Published so you can build to it:

- **Impersonation:** a process that is not the adapter attempting to submit a proposal, and separately a decision. Both must be rejected.
- **Token reuse** across AgentRuns.
- **Classifier gaps:** argv forms you did not include in the corpus, especially paths that reach outside the root by indirection.
- **Audit leakage:** grepping the database file for command text after an approval cycle.
- **Timeout behavior:** confirming that leaving a proposal pending never results in execution.

## 10. Gates

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
git diff --check
```

Unlike the RFC-014 spike, this is product code and **is** expected to carry thorough tests, following the project convention of `src/some_mod/tests.rs` rather than inline `#[test]` modules.
