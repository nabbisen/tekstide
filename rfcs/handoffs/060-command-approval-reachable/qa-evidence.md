# RFC-060 — QA evidence

## PR-060-A — the way in

### The shape decided

`ConfiguredAiCliProfile` grows one field, `adapter: ConfiguredAiCliAdapter` (`None | Managed`),
replacing what the risk doc (`what-the-configuration-must-not-allow.md` §1) names as three
independently-settable facts — `compatibility_level`, `structured_action_approval`, and whether an
adapter was declared at all — with the single field that decides all of them.
`to_ai_cli_profile` reads `configured.adapter` exhaustively (a two-arm `match`, no wildcard) to
produce both `compatibility_level` and `adapter_capabilities.structured_action_approval` from the
same value, in the same function call. There is no second field on `ConfiguredAiCliProfile` either
output could disagree with, because there is no second field at all that affects either one.

TOML: `adapter = true` / `adapter = false` (default) under `[agent.profile.<id>]`, parsed by a new
`take_bool` helper mirroring `take_string`'s own shape. This re-admits the `adapter` key RFC-045
D3' withdrew — removed from `WITHDRAWN_PROFILE_KEYS` — but it is not the same key: the withdrawn
one was an unconsumed, freestanding string with no agreed shape; this one is a boolean with a real,
structured consumer from the day it's reintroduced.

### Unrepresentable, not merely rejected (§1) — demonstrated three ways

1. **Exhaustive enumeration, the whole domain, not a sample.** `ConfiguredAiCliAdapter` has exactly
   two values. `every_declared_adapter_value_produces_an_internally_consistent_profile` iterates
   both and asserts `compatibility_level == Managed` iff `structured_action_approval == true` iff
   `adapter == Managed`, for every value the type can hold — not an assertion about the function's
   intent, a check against its actual output for every input that exists.
2. **The paired tests**: `to_ai_cli_profile_with_no_adapter_declared_stays_supervised` (renamed
   from `to_ai_cli_profile_never_sets_managed_compatibility_or_structured_action_approval`, whose
   old name and doc comment were a claim this slice makes false — see below) and
   `to_ai_cli_profile_with_an_adapter_declared_becomes_managed`, the same function, both outputs,
   from the one input that decides them.
3. **Ablated in a disposable `git worktree`** (main tree never touched): planted a struct literal
   that attempts the natural-but-wrong fix a future maintainer might reach for — setting `Managed`
   directly on `ConfiguredAiCliProfile`, bypassing `adapter`, the way `AiCliProfile` itself is set.
   `cargo build --tests -p tekstide-core` failed with:
   ```
   error[E0560]: struct `ConfiguredAiCliProfile` has no field named `compatibility_level`
     --> crates/tekstide-core/src/config/tests/profile.rs:46:9
   ```
   Worktree removed immediately after.

### A real stale claim found and corrected, not merely worked around

`to_ai_cli_profile_never_sets_managed_compatibility_or_structured_action_approval`'s own name and
doc comment asserted this function *never* produces `Managed` — true before this slice, false the
moment `ConfiguredAiCliAdapter::Managed` exists. Left unrenamed, a passing test with a false name
would have been exactly the "a correct test of the wrong property" failure mode
`release-checklist.md`'s own "Run It" section names from `0.12.0`'s history. Renamed to
`to_ai_cli_profile_with_no_adapter_declared_stays_supervised`, scoped honestly to the one case it
actually covers, with a doc comment explaining the rename and pointing at the new test that covers
the case the old name denied existed.

### §1's "three independent fields" risk, made concrete downstream too

`config/sensitive.rs`'s own `agent_profiles_direction` doc comment enumerated "its `display_name`
or its `command`" as what modifying a profile could mean — written before `adapter` existed, now
incomplete. Corrected, and a new test added:
`agent_profiles_direction_declaring_an_adapter_on_an_existing_profile_is_an_increase` — declaring
an adapter on a profile that already existed, with `display_name`/`command` both unchanged, is
`Increase`. The existing whole-value-equality mechanism already classified this correctly (nothing
needed changing in `sensitive.rs`'s own logic), but nothing had tested it directly, and it is the
one modification this module could least afford to under-classify: it is what grants `Managed` at
all.

### End-to-end: a real config profile reaches `prepare_adapter_approval`

`a_configured_managed_profile_reaches_prepare_adapter_approval_and_gets_a_real_channel` — not
inferred from types, per PR-060-A's own first checklist item. A real `ConfiguredAiCliProfile` with
`adapter: Managed` and a real on-disk executable, through the real translator
(`to_ai_cli_profile`), through `AgentRunLaunchValidator::validate`, through
`AgentRunLaunchPlan::from_validation`, through the real production entry point
`ProjectSession::prepare_agent_run_launch` (not `AgentRunLaunchPlan::prepare_adapter_approval`
called directly) — `Ok(Some(endpoint))`, a real bound `ApprovalChannelEndpoint`. Stops there
deliberately: spawning the adapter process and completing a proposal/decision round trip is
PR-060-C's own job ("the whole path, once, for real"); this slice has no UI and is not where the
reference adapter needs to run.

### The existing guard still means something

`managed_compatibility_level_without_structured_action_approval_is_still_rejected` is unchanged in
behavior and still passes: it manually mutates `profile.compatibility_level` on an `AiCliProfile`
*after* translation, testing `AgentRunLaunchValidator::validate`'s own, unmodified
`validate_compatibility` — a layer this slice's config-shape change does not touch. Only its
`configured_profile` helper literal needed the new `adapter` field, defaulted to `None` to keep
every one of its ten existing callers' asserted behavior unchanged.

### Documentation deferred, not forgotten

`docs/src/users/configuration.md`'s own schema example is **not** updated in this slice. Writing
"`adapter = true` enables command approval" now, without the cooperative-not-enforced caveat PR-
060-B's own D3 sentence exists to carry, risks exactly the "reassuring phrasing" §2 of
`what-the-configuration-must-not-allow.md` warns against — "approval enabled", "supervised",
"protected". PR-060-B's own sweep (D4) is where every place the product describes approval gets
read against what is true, in one pass, including this page for the first time it would say
anything about `adapter` at all. Writing it once, correctly, there, is safer than writing it twice.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean. Three consecutive
full-workspace runs, fresh short `/dev/shm` `TMPDIR` each: `779 + 22 + 1139`, 0 failed, 0 fixture
entries.

### Review 535 required fixes

**Required 1 — a fixture leaked on every passing run, not only a failing one.** `a_configured_
managed_profile_reaches_prepare_adapter_approval_and_gets_a_real_channel`'s own `state_root` was
`create_dir_all`-ed with no drop guard at all — worse than review 524's and 527's own findings,
both failure-only. Fixed by reusing `CleanupOnDrop` (`runtime::terminal::tests`, `pub(crate)` since
review 528). Verified directly: ran the test three times in a row, `ls` for `ta-060a-*` under
`/tmp` before and after each run — `0` every time, not merely "0 at the end." The reviewer's own
count discrepancy (`1139` vs their measured `1140`) reconciled on the next run after this fix —
the gate below reproduces their number exactly.

**Required 2 — the two outputs were derived in two places, not one.** `to_ai_cli_profile`'s first
draft read `configured.adapter` twice: a no-wildcard `match` for `compatibility_level`, a separate
`if matches!(..., Managed)` for `structured_action_approval`. A third `ConfiguredAiCliAdapter`
variant could pass through the `match` (forced) and fall through the `if` (not forced — no `else`
arm to catch it), landing on `Managed` with `structured_action_approval: false` — exactly the
inconsistent profile §1 exists to prevent, produced silently. Fixed by deriving both as one tuple
from one match:
```rust
let (compatibility_level, structured_action_approval) = match configured.adapter {
    ConfiguredAiCliAdapter::None => (AgentCompatibilityLevel::Supervised, false),
    ConfiguredAiCliAdapter::Managed => (AgentCompatibilityLevel::Managed, true),
};
```

**Ablated in a disposable `git worktree`, both halves of the claim**: planted a third variant
(`ConfiguredAiCliAdapter::Experimental`) — `cargo build -p tekstide-core` failed with
`error[E0004]: non-exhaustive patterns: 'ConfiguredAiCliAdapter::Experimental' not covered` at the
tuple match's own line. Added **the one arm the compiler demanded**, supplying both values
together (`(AgentCompatibilityLevel::Managed, true)`) — `cargo build --tests -p tekstide-core`
then succeeded, and `cargo test -p tekstide-core --lib config::` reproduced the reviewer's own
`131 passed; 0 failed` exactly. The shape cannot be fixed by touching only one of the two outputs:
the compiler error names the missing tuple, not a missing level or a missing capability
separately. Worktree removed; main tree never touched.

### Gate, re-run after the required fixes

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean. Three consecutive
full-workspace runs, fresh short `/dev/shm` `TMPDIR` each: `779 + 22 + 1140`, 0 failed, 0 fixture
entries — matching the reviewer's own count exactly.
