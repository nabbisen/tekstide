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

- [x] The store loads before CLI project paths open; path-resolution failure does not exit.
      `boot()` calls `load_configuration_at_boot` above the CLI-argument loop, so D6's limit
      applies to projects named on the command line. `an_unresolvable_configuration_path_is_a_
      diagnostic_not_an_exit` drives the real loader with an environment that resolves to nothing.
- [x] Board line: one of three states (§6), absent when clean. **The "loaded with warnings" state
      names the warned key** — a typo like `defualt_profile` warns and does nothing (PR-045-A's
      refuse-versus-warn split, adopted at response 376), and this line is the only thing that
      tells the user. A count is not a name. **Ablation:** force the "ignored"
      text on a clean load; the absent-when-clean test fails alone.
      Two of the three states are reachable in this slice; *pending confirmation* arrives with
      PR-045-C and nothing here pretends to render it. One line per warned key.
      **Ablated**: forcing the ignored text failed `a_clean_configuration_renders_no_board_line_
      at_all` and `no_configuration_file_at_all_renders_no_board_line` — the same property for two
      genuinely different clean inputs — and nothing else. (A third test failed on the first
      attempt; its line-count assertion coupled it to this property and was changed to assert
      naming only. See `qa-evidence.md`.)
- [x] `agent_run_limit` reaches `set_resource_limits`: the second launch in a limited project is
      refused, through the real launch path. Plus a second test for a project opened **mid-session**
      through the real path field, since the limit is applied at four call sites rather than one.
      Both ablated separately, each failing alone.
- [x] `transcript_retention_days` reaches a real launch's `TranscriptPrivacyPolicy`, read back.
      **Read back from the plan production builds, not from a launched run** — nothing retains
      `TranscriptRetentionLimits` past the launch (the plan is consumed; `Transcript` stores a fixed
      policy *string*). Widening what a launch retains is an RFC-011 data-model change; flagged in
      `qa-evidence.md` rather than taken here. Ablated by not applying the configured value.
- [x] **A config-defined `default_profile` does not launch in this slice.** Ablate by wiring it.
      Done — wiring `default_profile` into `attempt_agent_run_launch` failed this test alone. The
      test proves the negative through the two profiles' *different refusals*: the built-in profile
      is blocked by RFC-032's trust gate in a fresh project, and a configured profile (always
      `NoKnownWorkspaceDiscovery`) would not be — so that refusal is itself the evidence.

## PR-045-C — the deliberate act

- [x] **§3 invariant:** a config-defined profile with no confirmation on record is refused with a
      notice. Delete the check; this test fails alone.
      `a_configured_profile_with_no_confirmation_on_record_launches_nothing` — the confirmation
      *is* the notice, offered while the launch is still completable. **Ablated**: it fails, and so
      does `a_cleared_confirmed_set_re_arms_the_first_use_gate`, which is labelled in its own doc
      comment as a composition asserting nothing the tests it builds on do not assert separately.
      Two couplings were found by this ablation and removed first — see `qa-evidence.md`.
- [x] The confirmation shows the **resolved executable path** and the source file. **Fails if the
      body carries only the display name** (§5). Ablated exactly that way — fails alone. Confirmed
      live as well: the captured dialog names the path and never the `display_name` ("Demo AI CLI").
- [x] Confirming records `Authorized` then `Applied`, read back from a real store; declining
      records nothing and launches nothing. Both, separately.
- [x] A reload with `AgentProfiles` pending clears the confirmed set and re-arms first use.
      **Split into two tests** (clearing; re-arming as a labelled composition) after the ablation
      showed the response-367 two-properties-one-test shape.
- [x] Increase requires confirmation and records `_increase`; reduce applies and records
      `_reduce`. Both directions, separately. Each ablated: treating every direction as an increase
      fails the reduce test; applying increases without confirming fails the three tests of that
      gate.
- [x] `ReloadConfiguration` is in the registry and therefore in Help and `--help`.
      **`Ctrl+Alt+C`**, collision-checked mechanically. **In `NavigationAction`, not
      `SurfaceAction`** — D5 names the surface-action registry, but its own settled details call for
      "a chord", and `SurfaceAction` entries are surface-scoped bare keys; reloading configuration
      belongs to no surface. Both registries feed Help/`--help` by exhaustive match, and this one
      literally would not compile until both arms existed. Reconciliation named in `qa-evidence.md`
      for the reviewer to overrule cheaply.
- [x] **The `#[allow(dead_code)]` on `default_profile` is removed** — PR-045-B carried it naming
      this slice as the consumer; an allow that names its closing slice must be closed by it.
      Removed, along with the one on the accessor. The `unconfigured()` constructor keeps its own
      (it is a test fixture, and says so).
- [x] Nothing was added to the config-change record (§4). Grep `sensitive_config_changed_record`'s
      call sites for new arguments.
      **Asserted on the records instead, because the producers take no arguments at all** — there is
      nothing a call site could pass, which makes a grep a weak check.
      `no_configuration_value_or_field_name_reaches_the_change_record` reloads a file whose
      `display_name` and `command` are both a sentinel and asserts on every record written: sentinel
      absent, `subject_ref`/`adapter_profile_ref` `None`. Confirmed again in the live session's own
      store.

## Whole-RFC

- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`,
      `rfc_docs_invariants` clean.
- [x] Three consecutive full-workspace runs, **output redirected to a file**, green; any recurring
      flake gets a dated row in `test-process-leak.md`. **507 + 4 + 758**, green every time, at a
      settled load average of 1.43. No flake in the gate itself; one known intermittent
      (`closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`, register
      row 7's PTY-timing shape) was seen once during development and is already registered.
- [x] Live capture of the first-use confirmation with `XDG_CONFIG_HOME=$(mktemp -d)`, or a
      documented gap after three rounds.
      **Captured, first attempt** — `evidence/pr-045-c/`. Also `XDG_STATE_HOME=$(mktemp -d)`,
      because the board renders recent projects from the state home and a committed image must show
      throwaway state only. Two frames: the confirmation naming the resolved path (and never the
      `display_name`), and the launch that followed. Backed by the session's own real audit store,
      read back: the confirmation's `Authorized`/`Applied` pair, then RFC-046's launch trail for a
      configuration-defined profile. **`wtype` reached the application immediately**, which RFC-047
      PR-047-C's six rounds had concluded it could not — recorded as a fact in `qa-evidence.md`, and
      worth trying before assuming the gap again.
- [x] **Any catalog key that gained an argument was run through
      `i18n::enforcement::every_source_locale_key_resolves_in_every_shipped_locale`.** PR-045-B's
      two board lines rendered as their own key names until the full suite caught `$key` missing
      from `generic_args()` — a filtered *test run* is the filtered-*output* rule one level down.
      Run again this slice for `$executable`; it caught that one too, before shipping.
- [x] RFC-023's acceptance criteria re-read against the result: **"no configuration values reach
      durable audit"** still holds. Re-read and re-proven, twice: a sentinel test on the records
      themselves (not the call sites — the producers take no arguments, so a grep proves little),
      and again in the live session's store, where both config records carry `subject_ref: None`
      and `adapter_profile_ref: None`.
- [x] `crates/tekstide-core/README.md` and the RFC-023 row say what the file can now do — and no
      more. README rewritten: what a `config.toml` can change is four settings and a profile
      definition, every other key is refused by name, and no configuration-defined executable runs
      without a deliberate act naming its resolved path. RFC-023's row in `rfcs/README.md` now says
      it was reached by RFC-045 in `0.18.0` — and that its eight-field reload rule is two fields
      now, the other six returning with the code that reads them.
