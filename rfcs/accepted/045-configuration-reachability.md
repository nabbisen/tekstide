# RFC-045: Configuration Reachability

Status: **Accepted by the human owner 2026-09-12.** **D1–D9 decided by the architect on acceptance** — see "Decided on acceptance" at the end, which **widens D3 from three keys to most of the file** on a measurement made after the owner accepted, adds D8 (which profile launches) and D9 (retention is the one key with a consumer waiting). Proposed the same day. Reserved 2026-08-27 by RFC-036's D2 as the named consumer for four
built-and-unreached rows. Authored after RFC-046 closed, because RFC-046's `plan_is_auditable`
changed one of this RFC's answers before it was written — see D2.
Target milestone: **M12**
Date: 2026-09-12

Related RFCs:

- [RFC-023](../done/023-configuration-system.md) — built the configuration system. **All of it
  exists**: path resolution, `ConfigStore::load`/`reload`, atomic validation, the security-sensitive
  diff, profile conversion, two audit producers. Its own OQ3 (first-use confirmation) was answered
  "yes" and deferred to exactly this slice.
- [RFC-036](../done/036-dormant-capability-closure.md) — found the four rows and reserved this
  number rather than delete them.
- [RFC-046](../done/046-managed-agentrun-audit-trail.md) — its `plan_is_auditable` predicate is
  what makes D2 necessary: a profile it cannot audit now launches *unaudited*, silently.
- [RFC-010](../done/010-agentrun-launch-model-and-ai-cli-profiles.md) — owns `AiCliProfile`. D3
  finds it has no slot for an argv template, which is why `args` goes nowhere.
- [RFC-044](../done/044-surface-local-keyboard-affordances.md) — owns the action registry D5's
  reload command must be registered in, so Help and `--help` list it.

## Why

RFC-023 shipped a configuration system, and **nothing constructs it.** `crates/tekstide/src` contains
no `ConfigStore`, no `ConfigPathResolver`, no `to_ai_cli_profile`, no `record_sensitive_config_*`.
`boot()` in `main.rs` restores recent projects, opens CLI paths, resolves the locale catalog, and
builds `State` — and never reads a configuration file. The launch path hardcodes
`AiCliProfile::claude_code_linux_default()`.

So `REQ-CONFIG-001`..`007` are implemented and unmet at once: a user can write a valid
`config.toml` and Tekstide will behave exactly as if they had not.

**This is not a wiring task, which is why it is an RFC.** Reading the built code against the paths
it would plug into turns up four decisions the existing code has already made — one of them wrong
now, one of them silently incomplete, and one of them impossible as built.

## What is already built

- `ConfigPathResolver` / `ConfigPathProvider::linux_default()` → `config.toml` under the XDG config
  directory.
- `ConfigStore::load(path)` → never fails startup; a missing or invalid file yields compiled defaults
  and a `ConfigLoadReport { diagnostic: Some(..) }`.
- `ConfigStore::reload()` → applies safe fields, returns `pending_security_sensitive_changes` —
  whose own doc comment says *"confirming and applying one is not built yet (no confirmation surface
  exists)."*
- `security_sensitive_diff` / `direction` / `apply_safe_fields` — the increase/reduce asymmetry.
- `to_ai_cli_profile(id, &ConfiguredAiCliProfile)` → a `UserGlobal`, `Supervised`, `Minimal`-env
  profile.
- `record_sensitive_config_policy_increase()` (two-phase, best-effort) and `_reduce()` (one-phase).
- `ProjectSession::set_resource_limits(ProjectResourceLimits)`.

Nothing here needs a schema change. Three things need a decision.

## Decisions

### D1 — Load at boot, never refuse to start, and **show** the diagnostic

`ConfigStore::load` already refuses to turn a typo into a denial of service. What it cannot do from
`tekstide-core` is tell anyone. **Decided:** `boot()` loads the store; a `diagnostic` or any
`warnings` render on the project board as their own line, absent when there is nothing to say — the
same rule RFC-047 D3 set for audit health (§2 of its risk document: say something when something is
wrong, nothing when nothing is). A user whose file was ignored must be able to see that it was.

### D2 — A profile that cannot be audited cannot be defined

`extract_agent_profiles` deliberately leaves the profile id — the TOML map key — **unbounded**:
*"`name` itself stays unbounded for the real profile identity… PR-023-E still has to validate on its
own terms."* PR-023-E never did.

RFC-046 then shipped `plan_is_auditable`, which rejects any id `AuditReference::new` rejects — empty,
over-length, or outside `[A-Za-z0-9-_.:]` — and routes such a launch to the **unaudited** fallback.
That was the right call for a launch path: valid data must never reach a `panic!`. It is the wrong
outcome for a *definition*: a user who writes `[agent.profile."my tool"]` gets a profile every
launch of which is silently unrecorded, with nothing anywhere saying so.

**Decided: validate the id at parse, through `AuditReference::new` itself** — not a copied regex,
so the two cannot drift. A profile RFC-046 cannot record is a `ConfigDiagnostic`, and the message
says why: *"profile ids must be recordable in the audit trail."*

### D3 — A configured key that does nothing is a parse error, not a warning

`to_ai_cli_profile` reads `display_name` and `command` and **ignores `args`, `adapter`, and
`environment_policy`**. `AiCliProfile` has no field an argv template could go into; RFC-023's own doc
comment records this. Today that is invisible, because nothing constructs a profile. The moment D1
lands, a user can configure `args = ["--model", "x"]`, see the profile appear, launch it, and get no
`--model` — the failure RFC-036 named in its first sentence: *a reader discovers a configured value
does nothing.*

**Decided:** until a key reaches a consumer, `parse_and_validate` **refuses** it, naming the key.
This RFC wires none of the three. An argv template is an RFC-010 amendment to `AiCliProfile`, not a
rider on reachability; **reserved, not written.** `adapter` and `environment_policy` likewise.

The same rule governs `[resources]` — see D6.

### D4 — One principle, two triggers: no configuration-defined executable runs without a deliberate act

RFC-023 OQ3 answered *yes* to first-use confirmation: *"provenance is not intent… the dangerous
direction gets a deliberate act."* Its §Security-Sensitive Settings separately requires confirmation
on **change**. Read together, and against the built code, these are one principle with two triggers:

- **First use.** A profile present in the file at boot was never *changed* — it was simply there. It
  loads as *defined but unconfirmed*. The first launch of it shows a confirmation that names the
  executable path it resolved to and the file it came from, while the launch control is still live
  (RFC-034 D4's shape, which RFC-047 D4 reused). Confirming records
  `record_sensitive_config_policy_increase` and marks the profile confirmed for the session.
- **Reload.** `pending_security_sensitive_changes` finally gets the surface its doc comment says is
  missing. Each pending field is confirmed or declined; confirming an *increase* applies it and
  records `_increase`; a *reduce* applies directly and records `_reduce`, per RFC-023's asymmetry. A
  reload that changes `AgentProfiles` resets those profiles to *unconfirmed*, so the first-use gate
  re-arms rather than a second dialog stacking on the first.

Same modal shape as `TrustGrantModal`. The wording constraints of RFC-047 §5 apply: state the fact,
do not imply danger the profile does not carry, do not imply the user can fix anything from there.

### D5 — Explicit reload is a command, registered where Help can see it

RFC-023 specifies *"a command or API call"*; no GUI route exists. **Decided:** one action in RFC-044's
surface-action registry, so it appears in the Help modal and `--help` by construction rather than by
remembering. Its warnings and pending changes go to the board line D1 owns. Automatic reload stays
M13.

### D6 — `[resources]` and `ProjectResourceLimits` share no field, and this RFC says so

`set_resource_limits` is one of the four kept rows. `ResourceSettings` carries
`max_terminal_output_mb_per_session`, `max_agent_transcript_mb_per_run`,
`max_file_watch_events_per_batch`. `ProjectResourceLimits` carries `visible_terminal_limit`,
`terminal_session_limit`, `agent_run_limit`. **No field maps to any other.** The consumer RFC-036
kept `set_resource_limits` for has nothing in the file to feed it.

**Decided:** add `agent_run_limit` to `[resources]` — it is the one limit the launch path already
enforces (`project.resource_limits().agent_run_limit` at `shell.rs`), so it is the one whose consumer
is not hypothetical — and have it reach `set_resource_limits` for projects opened after load, per
RFC-023 §Hot Reload's *"resource limits for new tasks."* The three existing `[resources]` keys are
subject to D3: each either reaches its consumer or is refused at parse, and the handoff enumerates
which is which rather than this RFC guessing.

### D7 — The audit record names no field, and this RFC does not reopen the schema

`record_sensitive_config_policy_increase()` takes no arguments. `valid_config_change` forbids
`subject_kind`, `adapter_profile_ref`, and every domain link; RFC-023's acceptance criteria say *"no
configuration values reach durable audit."* So the trail says *a sensitive setting was weakened*,
not which. **Decided:** wire the producers as built. Whether a `SecuritySensitiveField` name is a
"value" is a question about RFC-013's frozen vocabulary, and answering it inside a reachability
slice is how a slice acquires a second design. Named here; not decided here.

## What this RFC must not become

- **A settings GUI.** The file is the interface; the board line and two confirmations are the whole
  surface.
- **Automatic reload.** M13, with the watcher.
- **Workspace configuration.** RFC-023 permits it as a design allowance and this RFC does not touch
  it. `RestrictedModeFeature::WorkspaceConfigLoading` stays as it is.
- **An argv template.** D3 reserves it for RFC-010.

## Risks

- **D2 is a breaking change to a file nobody can currently load** — the strictest moment to make it,
  and the last.
- **D4's first-use gate adds a click to the first launch of every configured profile.** That is the
  cost RFC-023 OQ3 accepted. It is once per session, not once per launch.
- **D6 adds a config key.** Small, but it is new surface; the handoff must state its default (none —
  absent means unlimited, matching `Option<u32>`) so absence is not mistaken for zero.

## Decided on acceptance (2026-09-12)

Made after acceptance, on a measurement the proposal did not contain. The owner accepted the RFC as
proposed; these do not reverse any of D1–D7, but D3′ changes its scale enough that it is stated as a
change rather than folded in silently.

### D3′ — the accepted key set is the set with a consumer, and today that is four keys plus profiles

I grepped every field of `ConfigurationDocument` for a consumer outside `config/` and outside tests.
**Every distinctively-named configurable value is unreached** — not only `args`/`adapter`/
`environment_policy`, but `scrollback_lines`, `shell_path`, all three font families, `font_size`,
`show_status_labels`, `open_duplicate_root`, `recent_projects_limit`, `default_project_board`,
`capture_changed_files`, `max_concurrent_global`/`_per_project`, all three `max_*` resource keys,
`redact_secret_like_environment_names`, `default_environment_policy`, and **all three
`restricted_mode_blocks_*`** — the fields the security-sensitive machinery was built around, which
never reach `RestrictedModeFeature`. And on the consumer side there is nowhere for them to go:
`Theme` is constants, `KeybindingPolicy` has no override API, terminal launch takes no scrollback or
shell path, the recent-project store takes no limit.

RFC-023 built a parser, validator, diff and reload for a file whose values have no destination.
That is not a criticism of RFC-023 — it scoped consumers out explicitly — but D3 applied honestly
means the parser must now refuse what it cannot deliver. **Decided: `parse_and_validate` accepts
exactly these, and refuses every other key with a diagnostic naming it and saying it has no effect
yet:**

| Key | Consumer |
| --- | --- |
| `[agent.profile.<id>]` `display_name`, `command` | `to_ai_cli_profile` → the launch path, via D8 |
| `[agent] default_profile` | **new, D8** — the launch path |
| `[agent] transcript_retention_days` | **D9** — `TranscriptPrivacyPolicy::max_age_days` |
| `[resources] agent_run_limit` | **new, D6** — `set_resource_limits` |

Each future RFC that wires a setting adds its key back **in the same change as the consumer** —
RFC-036 D2's named-consumer rule applied to configuration keys. Restricted Mode policy from
configuration is RFC-004 territory and security-critical; refused here, reserved, not smuggled.

**D3′, corrected by PR-045-A (request 376).** "No effect *yet*" is the wrong sentence for three
keys: `projects.default_trust`, `terminal.multiline_paste_protection`, and
`security.require_approval_for_adapter_destructive_commands`. Each was made unrepresentable by
responses 266/270 because it would bypass a deliberate per-use act another RFC requires; withdrawing
the field deletes that type-level guarantee, so the refusal is now the whole protection, and
promising a future in which configuration grants trust is §1's failure shape pointed at a security
setting. **They refuse at any value, name the governing act, and cite the RFC.** The implementer
saw this and I did not.

**Refuse versus warn, settled the same way.** Keys RFC-023 *defined* that this build cannot deliver
are refused; keys never defined — a typo, or one from a newer Tekstide — warn, so a file written
for a newer version still loads on an older one. The cost is that a typo warns and does nothing,
which is why PR-045-B's "loaded with warnings" line **must name the warned key**.

### D8 — which profile launches: `default_profile`, absent means built-in

`AgentSettings.profiles` is a map. RFC-023 never says which entry the launch button uses, and no
surface lists or chooses one. Without this, D2–D4 make profiles definable and still unlaunchable.

**Decided:** `[agent] default_profile = "<id>"`. The launch path resolves it through
`to_ai_cli_profile`; **absent means `claude_code_linux_default()`**, unchanged from today. A
`default_profile` naming an id not defined in the file is a `ConfigDiagnostic`. A profile may be
defined without being the default — it is reachable in principle through this key, which is the
D3′ test. A picker is a GUI feature and is not this RFC.

### D9 — retention is the one key with a consumer waiting, so wire it

`TranscriptPrivacyPolicy` has a real `max_age_days` and a constructor that takes it; today every
launch gets the compiled `DEFAULT_TRANSCRIPT_MAX_AGE_DAYS`. `transcript_retention_days` is already
`SecuritySensitiveField::AgentTranscriptRetentionDays`. **Decided:** reach it. It applies to runs
launched after load or after a confirmed reload (§Hot Reload: *new tasks*), and it is the one key
that exercises D4's increase/reduce path with a real field rather than a fixture.

**D9, corrected by PR-045-C's review (request 378).** "The one key with a consumer waiting" overstated
it. `transcript_retention_days` reaches `TranscriptRetentionLimits::max_age_days` and is stored on
each launch's `BoundedTranscriptRetention` — and **nothing reads it to enforce anything**: no purge,
expiry or sweep consults `max_age_days`; the only readers are the constructor chain and
`is_bounded()`'s `> 0` check. A stored value nothing reads is what D3′ refuses, and I called it a
consumer because a struct had a slot with the right name. The key **stays**, on the thin but real
grounds that `is_bounded()` reads it and it is stored where RFC-011's purge will read it; refusing it
now and re-admitting it later would be churn. But no user-facing text may say transcripts are
*kept* for that many days until the purge exists — `future-work.md` carries the row.

**D5, reconciled by PR-045-C.** I wrote "surface-action registry" and "a chord" in the same
decision; they conflict, because `SurfaceAction` entries are bare surface-scoped keys. The chord
registry is `NavigationAction`, and that is where `ReloadConfiguration` lives, with a collision test
over every rule's `default_binding`. The requirement that mattered — Help and `--help` by
construction — is met literally: the variant does not compile without its catalog and coverage arms.

### Settled details an implementer must not inherit

- **Boot order:** the store loads **before** CLI project paths are opened, so D6's limit applies to
  them. Failure to resolve the config path is a diagnostic, not an exit.
- **Confirmed-profile state** lives on `State`, session-scoped like `AuditHealth`, as the set of
  confirmed ids; a reload whose pending changes include `AgentProfiles` clears it.
- **D5's action** is `ReloadConfiguration`, in RFC-044's surface-action registry. The chord is the
  handoff's to pick from what is free.
- **D1's board line** renders `key` and `message`, and the config file path — the user's own screen
  may show their own path. **Committed evidence sets `XDG_CONFIG_HOME=$(mktemp -d)`**, because the
  real path is under `$HOME` by definition and no fixture project changes that.
- **`agent_run_limit`'s default is absent = unlimited**, matching `Option<u32>`; `0` is a diagnostic,
  since a limit of zero refuses every launch and is far more likely a typo than an intent.
