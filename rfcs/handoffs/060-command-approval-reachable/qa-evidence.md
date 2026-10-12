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

## PR-060-B — the sentence and the sweep

### D3's sentence: already shipped, verified rather than assumed

**Finding, not a feature added**: the sentence D3 asks for — "stating that approval is an answer
to a request and that Tekstide does not prevent a rejected command from running" — already exists.
`approval-dialog-cooperative-notice` (`locales/en.ftl:908`) was added under **RFC-022 PR-022-E,
response 222, commit `7e9b553`, 2026-08-16** — about two months before RFC-060 was even proposed.
Read directly, not taken on a subagent's word: the key, its render wiring
(`approval_dialog_view`, `shell.rs`), and the existing test
(`approval_dialog_cooperative_notice_states_all_three_required_non_claims`,
`shell/tests.rs:11213`) all confirmed. RFC-060's own text is aware of this and is not claiming
otherwise — its own "Decided on acceptance" section says *"D3 is where its highest-consequence
sentence ... finally has to appear **in front of a real user**"* — the distinction is reachability
(no production profile was ever `Managed` until PR-060-A, so nobody had ever seen this dialog),
not that the wording was missing.

**"Does not prevent execution"** was already directly tested. **"Approval answers a request"** was
not tested as its own claim — carried by the dialog's title (*"Command Approval Requested"*) and
body (*"An AI CLI is asking to run..."*), not repeated inside the notice itself (which would be
redundant with text a reader has already read one line above it). Added
`approval_dialog_states_both_halves_of_rfc_060_d3` to assert this explicitly, through the same
catalog calls the view itself makes (`approval_dialog_body`, `catalog.get("approval-dialog-title")`,
`catalog.get("approval-dialog-cooperative-notice")`) — the wording-guard shape, not a string
assembled in the test.

**No new catalog key was added.** If review wants the existing sentence's wording tightened, moved,
or restated for some reason not evident from reading it, that is a real follow-up; as written, it
already clears §2 of `what-the-dialog-must-not-lie-about.md`'s bar.

### D4, the sweep — recorded even though two findings were stale claims, not new risks

Swept `locales/en.ftl`, `docs/src/users/what-works-today.md`, `docs/src/users/local-data-and-
privacy.md`, `README.md`, and drafted the changelog language below, per the checklist's own five
targets. Grepped each for "approval", "approve", "reject", "Managed", "adapter", and separately for
§6's own list of words that imply enforcement without the word "approval" — *protected*, *blocked*,
*prevented*, *secured*, *requires approval before running*.

**Two real findings, both staleness, not enforcement-claim risk.** Every existing sentence
describing approval already stated the cooperative-not-enforced model correctly — the actual defect
was that two of them claimed the pathway was **unreachable**, which PR-060-A just made false:

- `README.md`: *"Command approval is built but unreachable."* → corrected to name that it is now
  reachable from configuration, while keeping "no shipping AI CLI speaks its protocol" and
  "cooperative, not enforced" exactly as they were.
- `docs/src/users/what-works-today.md`: *"## Command approval, and why you cannot use it"* and its
  body (*"`Managed` mode... can only be exercised by this project's own reference adapter"*) →
  retitled and rewritten to say what changed (`adapter = true` reaches it now) and what did not
  (nothing you would install answers; cooperative, not enforced; the declaration is the file's own
  unverified claim, not a fact Tekstide checks about the named program).

**`docs/src/users/local-data-and-privacy.md`**: no reachability claim either way in its own
"Command approvals" paragraph — already accurate, nothing to correct.

**`locales/en.ftl`**: no enforcement-claim or reachability-claim risk found beyond the dialog itself
(already covered under D3 above). `approval-history-classifier-notice` already correctly disclaims
risk-level as "not a guarantee"; `change-review-disclosure` already correctly says it is "not a
review, an approval, or a claim that a change is safe" (a different feature, same discipline).

### `docs/src/users/configuration.md`: the deferred documentation, written now

PR-060-A deferred this deliberately (review 535 confirmed the deferral was correct). Added a new
`## Declaring an adapter` section and the `adapter = false` line to the schema example. Follows §2
and §4 of `what-the-configuration-must-not-allow.md` directly: states plainly that declaring
`adapter = true` is **a claim the file makes, not a fact Tekstide checks** about the named program;
names that nothing ships that actually speaks the protocol today; repeats the cooperative-not-
enforced sentence rather than assuming a reader carries it over from elsewhere; cross-links to
`what-works-today.md` and `local-data-and-privacy.md` rather than re-deriving either page's own
claims. No mention of the capability token at all (§4: nothing to describe, so nothing to
upgrade).

### The draft changelog entry

Per the task breakdown's own instruction ("this release's entry is where `REQ-AGENT-013` is
easiest to breach... write the entry so that a reader who stops after the first sentence has not
been misled"). Not committed to `CHANGELOG.md` — that happens at the candidate cut, same
discipline as RFC-059's own cut. Draft, for that round to refine or replace:

> **The command-approval UI and API are reachable from configuration for the first time, for a
> profile that declares an adapter — and no shipping AI CLI implements the protocol, so for most
> users nothing changes.** A configuration file can now set `adapter = true` on a profile to
> request `Managed` mode; before this release, no field existed to ask for it at all, and the
> impossibility of asking incorrectly was structural, not a validation refusal
> (`ConfiguredAiCliAdapter`, a two-variant enum one match derives both outputs from). Rejecting a
> proposal does not prevent execution — nothing in Tekstide intercepts a spawned process — and
> that has not changed. The one thing an ordinary user will notice: the Audit view (`0.35.0`) can
> now show real `command_approval` records, if a declared adapter is ever reached.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean. Three consecutive
full-workspace runs, fresh short `/dev/shm` `TMPDIR` each: `780 + 22 + 1140`, 0 failed, 0 fixture
entries.

## PR-060-C — the whole path, once, for real

### The test that closes the loop

`the_decided_approval_is_visible_through_the_real_audit_view`
(`crates/tekstide/src/shell/tests.rs`, right after
`command_approval_family_produces_real_durable_audit_records_through_the_pipeline`) is the first
test in this project to join a real `Managed` launch of `reference_adapter` — **this project's own
test-and-proof artifact, named here exactly because it is not, and proves nothing about, any real
AI CLI** (`what-the-dialog-must-not-lie-about.md` §4) — through a real proposal, a real decision via
the real `update()`/`Message::ModalActivate` route, and then reads the resulting
`command_approval` records back through the real `audit_view` itself
(`state.audit_view.records_for_test()`, loaded by a real `Ctrl+Alt+V` key event), **not by an
independent `AuditStore::query` run beside it**. The pipeline test already proved the producer side
(a record exists, with the right `outcome`, reachable by direct query); this test's own job is the
consumer side D7 exists for — a user who opens the audit view after deciding a real approval must
find that decision's own records there, through the same `AppRoute::AuditView` a real key press
reaches. Finds the real `CommandRequest`/`Requested` record and both the `Authorized` and `Applied`
halves of the real `CommandApprove` pair, filtered to this run's own `AgentRunId`, inside the
view's own loaded page.

### Live capture — the whole path, actually run once

Built `reference_adapter` (`cargo build -p tekstide-core --bin reference_adapter`) and ran the real
`tekstide` binary against a throwaway `XDG_CONFIG_HOME`/`XDG_STATE_HOME`/project directory —
nothing committed to the repo, nothing under the developer's real state root. Config declared one
profile, `command` pointing at a tiny wrapper script that `exec`s the real `reference_adapter`
binary with a hardcoded `rm -rf <nonexistent marker>` argv (never actually executed — the adapter
only ever proposes and prints the decision; that claim is pinned by
`reference_adapter_binary_never_executes_the_argv_it_proposes`,
`crates/tekstide-core/src/approval/tests/reference_adapter.rs`), and `adapter = true`.

**One real lesson, worth recording because it would bite again:** the first attempt used a longer
throwaway path (`/dev/shm/tekstide-pr060c-capture-2/...`) and every launch failed with the generic
`agent-run-launch-refused` catch-all, no specific reason surfaced. The real `approval/` directory
was never even created — the failure was before the channel bound at all. The cause was the same
one this project's own test-gate instructions already warn about for `TMPDIR`, just not previously
connected to a *production* run: `ApprovalChannel`'s real socket path is `<state_root>/approval/
agent-run-<uuid>.sock`, and `<state_root>` here is `$XDG_STATE_HOME/tekstide`, not a test-only
value — a long `$XDG_STATE_HOME` for a real GUI launch hits the same ~104-byte `AF_UNIX` path limit
a long test `TMPDIR` does. Rebuilding the throwaway tree under a short path (`/dev/shm/t60c/...`,
full socket path 75 bytes) fixed it immediately, no code change involved. Recorded here rather than
filed as a defect: `resolve_agent_run_state_dir`'s own real branch just reads `$XDG_STATE_HOME`
as given, which is correct — a short path is this capture's own obligation, not a thing the
product should paper over by shortening a user's real XDG directories for them.

Three screenshots, committed under
`rfcs/handoffs/060-command-approval-reachable/evidence/pr-060-c/` (distinct SHA-256 per file,
confirming each is a genuinely new capture and not `wl-paste` replaying a stale clipboard):

- `pr-060-c-01-approval-dialog-cooperative-notice.png` — the real, promoted dialog, `Risk:
  Destructive`, focus defaulting to `Reject`, **the full frame**, not cropped to the buttons, so
  `approval-dialog-cooperative-notice` ("This choice is advisory, not a safeguard...") is fully
  visible — the reviewer's own ask from review 537, for exactly this capture, the first time
  anyone outside this project sees that sentence in context.
- `pr-060-c-02-decided-adapter-exited.png` — after `Approve Once` (Tab from the default `Reject`
  focus, then Enter): the terminal row reads "Exited", the real adapter process is gone
  (`pgrep` confirmed no `reference_adapter`/wrapper process survives the decision).
- `pr-060-c-03-audit-view-command-approval-records.png` — `Ctrl+Alt+V`, the real audit view,
  showing `Command approval · Requested`, `Command approval · Authorized`, and `Command approval ·
  Applied` rows for the real project, alongside the launch's own `Managed process lifecycle ·
  Started`/`Authorized`/`Terminated` rows — the same shape the new test above asserts, now seen
  through the actual rendered surface rather than only `records_for_test()`.

Process and window state verified before and after: `niri msg -j windows`/`is_focused` checked
immediately before every `wtype` call (one stray keystroke did land on an unrelated window mid-
session when another agent's app stole focus between the check and the send — refocused and
retried rather than guessing what it did there, since that window is not this project's to
inspect or correct); the real `tekstide` process and all throwaway directories
(`/dev/shm/t60c`, and the two abandoned longer-path attempts) were killed/removed after capture.

### A flake disclosed, not fixed, in this slice's own gate

The third of three full-workspace runs failed one test unrelated to this slice's own change —
`surface::terminal::tests::real_output_wraps_at_the_new_column_count_not_the_old_one` — then passed
5 of 5 in isolation. Disclosed as a new row in `test-process-leak.md` (same file, same
`test-flake-disposition.md` register this project already tracks these under), not fixed here:
out of this slice's own scope, and reported as the same shape (category A, "the wait is weaker
than the condition it waits for") an existing sibling test already carries a live, unverdicted row
for. Two subsequent full-workspace runs were clean.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`: clean.
Six full-workspace runs total this slice (one new test added, `780 → 781`): runs 1–2 clean
(`781 + 22 + 1140` each), run 3 failed one unrelated test (disclosed above), runs 4–6 clean —
**three consecutive clean runs, runs 4/5/6**, counted per `test-flake-disposition.md` rule 5 ("do
not count a gate run that failed" — run 3 resets the streak, it does not contribute toward the
three). 0 failed in every clean run, 0 fixture entries in a fresh short `/dev/shm` `TMPDIR` each
time.

## PR-060-D — review 538's required item: name the socket-path refusal

### What PR-060-C's own live capture walked into

`ApprovalChannelDirectory::bind()` already distinguishes
`ApprovalChannelErrorReason::SocketPathTooLong` (response 113 Required 1, RFC-047's own cycle) —
diagnosed loudly at `bind()` specifically so a too-long `sun_path` could not instead bind
successfully and fail later, unreadably, at `connect()`. But before RFC-060 no configuration could
ever reach `Managed` at all, so a real launch could never exercise this reason, and nothing above
`bind()` carried it to the surface. `agent_run_launch_refusal_symbol`
(`crates/tekstide/src/shell.rs`) had exactly four branches — `limit`, `not-found`,
`workspace-blocked`, and a catch-all `*[error]` — and `Runtime(_)` (which wraps this reason, four
layers down: `AgentRunLaunchRefusal::Runtime(ProjectAgentRuntimeLaunchError::AdapterApproval(
AgentAdapterApprovalError::Bind(ApprovalChannelErrorReason::SocketPathTooLong)))`) fell straight
into the catch-all. PR-060-C's own live capture hit exactly this on its first attempt, reasoned
its way from "Couldn't start an agent run." back to a long `$XDG_STATE_HOME`, and recorded that as
a one-off lesson rather than a defect to fix — review 538 named it correctly as RFC-060's own
business: the release that makes `SocketPathTooLong` reachable in production is the release that
owes it a message.

### The fix

A new, specific match arm in `agent_run_launch_refusal_symbol`, matched before the generic
`Runtime(_)` catch-all, producing `"approval-socket-path-too-long"`. A new `en.ftl` branch under
`agent-run-launch-refused` names what to change — set `XDG_STATE_HOME` to a shorter path — and
deliberately states no byte number: `sockaddr_un`'s real capacity differs by platform, and the
exact budget left for the state directory also depends on the agent run's own id and the fixed
`approval/` segment, so a number correct on this machine would be wrong on another.

### The test, and the ablation that proves it

`a_managed_launch_whose_state_root_is_too_deep_for_the_approval_socket_names_the_reason`
(`crates/tekstide/src/shell/tests.rs`) reuses the identical deep-path recipe
`bind_rejects_a_state_root_deep_enough_to_exceed_sun_path`
(`tekstide-core`'s own approval-channel test) already uses — six 40-character path components,
confirmed by assertion to push the real socket path past 107 bytes — to drive a real `Managed`
launch through `attempt_agent_run_launch_with_profile_state_root_and_capture` and assert both the
exact nested refusal shape and that the catalog text actually renders the new, named message
rather than the generic one.

**Ablated in a disposable worktree** (`git worktree add`, never a hand `git checkout` on the main
tree): removed the new match arm, rebuilt `tekstide`'s bins and `tekstide-core`'s
`reference_adapter` bin (the test spawns a real adapter process; the first ablation attempt
without the second build failed with "expected the reference_adapter binary... the `[[bin]]`
target may not have built" — a different failure than the one being proven, caught and rebuilt
before trusting the result), re-ran the one test. It failed, correctly, on the exact assertion the
fix exists for:

```
the rendered refusal must name the state directory and XDG_STATE_HOME, not fall through to the
unnamed generic message: "Couldn't start an agent run."
```

Worktree removed after (`git worktree remove --force`); the main tree was never dirty during the
ablation.

### The doc, where a user or packager would find it before hitting it live

New `## The approval socket` section in `docs/src/users/local-data-and-privacy.md`, alongside the
file's existing project-lock/attention-socket section (the nearest sibling — same shape, a local
Unix socket under `$XDG_STATE_HOME/tekstide/`). States the path, that `Managed` is what creates it,
that its length is capped by the OS and shared between the state directory, the fixed `approval/`
segment, and the run's own id, and that a too-long `XDG_STATE_HOME` is named directly rather than
failing silently — again, no byte number, same reasoning as the catalog text. Cross-linked from
`configuration.md`'s own `## Declaring an adapter` section (the anchor `#declaring-an-adapter`,
matching the link-style convention already used throughout this file).

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`: clean.
One new test added, `781 → 782`. Three consecutive full-workspace runs, fresh short `/dev/shm`
`TMPDIR` each: `782 + 22 + 1140`, 0 failed, 0 fixture entries.
