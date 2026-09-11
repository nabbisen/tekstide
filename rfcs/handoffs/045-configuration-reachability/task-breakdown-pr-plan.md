---
title: "RFC-045 — task breakdown and PR plan"
rfc: "RFC-045"
rfc_file: "../../accepted/045-configuration-reachability.md"
source_rfc_status: "Accepted 2026-09-12 — M12"
target_milestone: "M12"
created: "2026-09-12"
---

# Task breakdown and PR plan

Three slices, **A → B → C**. No configuration-defined executable can run before C; B lands
everything that is not one.

## PR-045-A — the parser tells the truth (core only)

D2, D3′, D6's key, D8's key, D9's key. `tekstide-core` only; still no production caller.

- **D3′:** `parse_and_validate` accepts `[agent.profile.<id>] display_name, command`,
  `[agent] default_profile`, `[agent] transcript_retention_days`, `[resources] agent_run_limit`.
  **Every other key is a `ConfigDiagnostic`** naming the key and saying it has no effect yet. Remove
  the unreached fields from the model structs rather than parsing into fields nothing reads —
  `warn_unconsumed` is the wrong instrument here (§1).
- **D2:** the profile id passes `AuditReference::new` or is a diagnostic. **Call it; do not copy
  it.**
- **D8:** `default_profile` naming an undefined id is a diagnostic.
- **D6:** `agent_run_limit = 0` is a diagnostic; absent means unlimited.
- **D9:** `transcript_retention_days` stays; it is already `SecuritySensitiveField`.

**Required tests, each ablated separately:** one refused key per removed section fails on its own;
an id with a space is refused and the diagnostic names the audit trail; `default_profile = "nope"`
is refused; `agent_run_limit = 0` is refused. **Ablation for D2:** replace the `AuditReference::new`
call with a hand-written character check that accepts one character the real one rejects, and watch
the test fail — that proves the test is tied to the real function.

## PR-045-B — boot, board, limits, retention (no executables yet)

D1, D6, D9, and D8's *resolution* — not its launch.

- `boot()` resolves the config path and loads the store **before** CLI project paths are opened.
  Path resolution failure is a diagnostic. `ConfigStore` lives on `State`.
- **D1:** the board renders one line for ignored / loaded-with-warnings / pending (§6); absent when
  clean. Composed in `shell.rs` like `project_board_audit_lines`, for the same reason.
- **D6:** `agent_run_limit` reaches `set_resource_limits` for every project opened after load.
- **D9:** `transcript_retention_days` reaches `TranscriptPrivacyPolicy::max_age_days` for runs
  launched after load.
- **D8:** `default_profile` is resolved to an `AiCliProfile` and stored — **but the launch path
  still uses `claude_code_linux_default()`** until C. A test asserts exactly that.

**Required tests:** an invalid file boots with defaults and the board says *ignored*; a valid file
with `agent_run_limit = 1` refuses the second launch in a project opened afterwards; retention
reaches a real launch's policy (read the policy back, do not assert the config was parsed); and **a
config-defined `default_profile` does not launch in this slice** — ablate by wiring it early and
watch that test fail.

## PR-045-C — the deliberate act (D4, D5, D8's launch, D7)

- **D4 first use:** launching when `default_profile` is config-defined and not in the session's
  confirmed set shows the confirmation (§5: resolved path and source file, while the control is
  live). Confirm → add to the set, record `_increase`, launch. Decline → nothing.
- **D4 reload:** `ReloadConfiguration` (D5, in RFC-044's registry) calls `ConfigStore::reload`;
  each `pending_security_sensitive_changes` entry is confirmed or declined; increase → apply +
  `_increase`; reduce → apply + `_reduce`; `AgentProfiles` pending clears the confirmed set.
  Diagnostics and warnings go to the D1 line.
- **D7:** producers wired as built; nothing added to the record (§4).

**Required tests, ablated separately:** the §3 invariant (no confirmation on record → refused with a
notice; delete the check, watch it fail); the confirmation text contains the resolved executable path
and **fails if it contains only the display name** (§5); confirming writes `Authorized` then
`Applied`, read back from a real store; a reload with a pending `AgentProfiles` change re-arms
first use; the `ReloadConfiguration` action appears in Help and `--help` by construction (RFC-044's
exhaustive mirror should make this a compile-time fact — say so if it does).

**Evidence:** unit-level plus one live capture of the first-use confirmation against a
`mktemp -d` project **with `XDG_CONFIG_HOME=$(mktemp -d)`** holding a fixture `config.toml`,
because the real config path is under `$HOME` by definition. Bounded-evidence rule applies: three
rounds, then the store read-back and the unit tests carry it.

## Not in this plan

Everything the pack README lists. And: **do not restore a refused key because it "looks harmless."**
Every one returns with its consumer, in the same change, or not at all.
