---
title: "RFC-045 — acceptance and QA checklist"
rfc: "RFC-045"
rfc_file: "../../accepted/045-configuration-reachability.md"
source_rfc_status: "Accepted 2026-09-12 — M12"
target_milestone: "M12"
created: "2026-09-12"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it to a different slice stays unticked with the
contradiction named — that is the reviewer's error to fix, not the implementer's to paper over.

## PR-045-A — the parser tells the truth

- [x] Exactly the D3′ key set parses; every other former key is refused **by name** with "no effect
      yet" in the diagnostic. One test per removed section, each failing alone.
      `the_whole_accepted_key_set_parses` covers the positive half; one `*_is_refused_because_
      nothing_reads_it` test per section covers the negative. **Ablated two ways, each failing one
      test alone**: removing a `WITHDRAWN_KEYS` row (live sections), and dropping a section's
      refusal call (withdrawn sections). **Deviation, flagged for review**: three keys
      (`projects.default_trust`, `terminal.multiline_paste_protection`,
      `security.require_approval_for_adapter_destructive_commands`) are refused with a *permanent*
      message rather than "no effect yet", because that phrase would promise a future version
      granting what responses 266/270 decided must never be grantable. Nothing is accepted either
      way; see `qa-evidence.md`.
- [x] A profile id `AuditReference::new` rejects is a diagnostic naming the audit trail.
      **Ablation:** swap in a hand-written check that admits one extra character; the test must fail.
      Done exactly that (admitted a space) — `a_profile_id_the_audit_trail_cannot_record_is_refused`
      failed alone.
- [x] `default_profile` naming an undefined profile is refused. Two tests (an undefined name
      alongside a defined profile, and with no profiles at all); ablated by dropping
      `validate_default_profile` — both failed, nothing else did.
- [x] `agent_run_limit = 0` is refused; absent is unlimited. `Option<u32>` plus a dedicated
      `take_agent_run_limit`, so absent and zero are structurally distinguishable rather than
      sharing a sentinel. Ablated by making the zero check unreachable.
- [x] The unreached fields are **gone from the model structs**, not parsed-and-ignored.
      `ConfigurationDocument` is two sections; six section types and three one-valued guard types
      are deleted, along with six `SecuritySensitiveField` variants. Full inventory in
      `qa-evidence.md`.

## PR-045-B — boot, board, limits, retention

- [ ] The store loads before CLI project paths open; path-resolution failure does not exit.
- [ ] Board line: one of three states (§6), absent when clean. **Ablation:** force the "ignored"
      text on a clean load; the absent-when-clean test fails alone.
- [ ] `agent_run_limit` reaches `set_resource_limits`: the second launch in a limited project is
      refused, through the real launch path.
- [ ] `transcript_retention_days` reaches a real launch's `TranscriptPrivacyPolicy`, read back.
- [ ] **A config-defined `default_profile` does not launch in this slice.** Ablate by wiring it.

## PR-045-C — the deliberate act

- [ ] **§3 invariant:** a config-defined profile with no confirmation on record is refused with a
      notice. Delete the check; this test fails alone.
- [ ] The confirmation shows the **resolved executable path** and the source file. **Fails if the
      body carries only the display name** (§5).
- [ ] Confirming records `Authorized` then `Applied`, read back from a real store; declining
      records nothing and launches nothing.
- [ ] A reload with `AgentProfiles` pending clears the confirmed set and re-arms first use.
- [ ] Increase requires confirmation and records `_increase`; reduce applies and records
      `_reduce`. Both directions, separately.
- [ ] `ReloadConfiguration` is in the registry and therefore in Help and `--help`.
- [ ] Nothing was added to the config-change record (§4). Grep `sensitive_config_changed_record`'s
      call sites for new arguments.

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`,
      `rfc_docs_invariants` clean.
- [ ] Three consecutive full-workspace runs, **output redirected to a file**, green; any recurring
      flake gets a dated row in `test-process-leak.md`.
- [ ] Live capture of the first-use confirmation with `XDG_CONFIG_HOME=$(mktemp -d)`, or a
      documented gap after three rounds.
- [ ] RFC-023's acceptance criteria re-read against the result: **"no configuration values reach
      durable audit"** still holds.
- [ ] `crates/tekstide-core/README.md` and the RFC-023 row say what the file can now do — and no
      more.
