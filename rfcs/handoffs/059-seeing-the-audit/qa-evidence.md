# RFC-059 — QA evidence

## PR-059-A — the store can be believed

### The enumeration

**Derived from the type, not a hand-written list.** `crates/tekstide-core/src/audit/tests/every_family_outcome_lands.rs`
dispatches on `AuditEventFamily` via `chains_for_family`, an exhaustive `match` with **no wildcard
arm** — the same bar `DurableAuditRecordV1::validate()`'s own `match self.family { ... }` already
holds this crate to. Adding a thirteenth family variant without adding its own arm here is a
compile error, which is what the checklist's "a reviewer must be able to break it by adding a
family arm and see the test fail" means in practice: the failure is a build failure, reported by
`cargo test` the same as any other.

Every record is checked against `DurableAuditRecordV1::validate()` before being appended — a
mistake in this file's own construction fails loudly there, against the same Rust-level validator
the real producers answer to, rather than being asserted against a store that might reject it for
an unrelated reason.

**First version was wrong, and the store caught it, not a human re-reading.** The first draft built
every `(family, outcome, action_kind)` combination as an independent record, each with its own
fresh `operation_id`. Every `Applied`/`Failed` combination in an authorization-shaped family
(`TrustChange`, `CommandApproval`, `ManagedProcessLifecycle`, `SafeCloseDecision`,
`SensitiveConfigChanged`) was refused by the real store with `AuditStoreError { reason:
MissingAuthorization }` — `AuditStore::append`'s own `validate_operation_phase` enforces a
cross-record contract `validate()` cannot see: an `Applied`/`Failed`/`Started`/`Terminated` record
naming an `operation_id` requires a **prior** `Authorized` record for that exact `operation_id`
already in the store. Not a store defect — the real producers (`AuditCoordinator::grant_project_trust`
and siblings) always write the two phases in order, sharing one `operation_id`; the first draft was
asserting a shape nothing real produces. Fixed by restructuring every outcome into a **chain**
(`Vec<DurableAuditRecordV1>`, inserted strictly in order, sharing one `operation_id` where the
family requires it) rather than a flat independent record.

### The pre-fix run

Run against the tree **before any schema or store change this RFC makes**: 37 chains, 54 records,
covering every `(family, outcome, action_kind)` `DurableAuditRecordV1::validate()` accepts. **All 54
landed and all 54 came back from `query()`.** The one previously-known gap — `safe_close_decision` +
`blocked`, review 503 — is already closed: RFC-066 PR-066-B's own `AUDIT_SCHEMA_VERSION` `2 -> 3`
migration added the `'blocked'` branch to `CREATE_SCHEMA_V3`'s own family `CHECK` constraint
(confirmed by reading the current constant directly, not assumed from the RFC's own prose). **This
enumeration found no other gap.** Per the task breakdown's own instruction ("if that is the only
one, say so with the output; if there are others, stop and report before PR-059-B"): that is the
only one, and PR-059-B proceeds.

### The first-creation race

**Reproduced with two real, overlapping OS processes**, not threads and not sequential runs.
`crates/tekstide-core/src/bin/audit_store_create_probe.rs` (a new test-only `[[bin]]`, the same
shape RFC-058's own `project_process_probe` established): prints `READY`, blocks on one line of
stdin, then calls the real `AuditStore::open`. The harness
(`crates/tekstide-core/src/audit/tests/first_creation_race.rs`) spawns two, waits for **both** to
print `READY` (confirmed by reading the line back, not inferred from "spawned without waiting"),
then releases both with two back-to-back `write_all` calls — the RFC-058 review 513 lesson this
RFC's own task breakdown names explicitly, so it did not need re-finding: two sequential processes
prove nothing.

**Against the tree before the fix, in isolation, 10 of 10 runs failed.** Four shapes were observed
across repeated runs, by me and by the reviewer's own independent worktree ablations — the set was
never knowable in advance, which is itself the argument for the fix's own error-agnostic design
(probe for a store that now exists on *any* failure, rather than matching one specific error):

- `ERR AuditStoreError { reason: Busy }` — a genuine SQLite multi-process busy-handler livelock.
  `busy_timeout`'s own retry schedule is fixed, not jittered; two processes released at close to the
  same instant can retry in lockstep and both exhaust the full 2-second timeout without either
  making progress. This is the shape that dominated my own first repeated runs, not the "already
  exists" conflict the RFC's own prose (written before this reproduction ran) anticipated.
- `ERR AuditStoreError { reason: Sqlite }`, `"table audit_events already exists"` — the loser's own
  `CREATE TABLE` executing after the winner's had already committed, the shape the RFC's prose
  named.
- `ERR AuditStoreError { reason: Io }` — observed in the review 527 worktree ablation; the specific
  I/O condition was not isolated further, since the fix's own design does not need to distinguish it
  from the two shapes above.
- `ERR AuditStoreError { reason: UnsupportedApplication }` — found by the reviewer's own worktree
  ablation, and the most important of the four: **the losing process is not failing to create the
  store at all — it successfully opens a database file the winner has already created but not yet
  stamped.** `open_internal`'s own fresh-install path writes the table and indexes via
  `create_current_schema` before the `application_id`/`user_version` pragmas that mark the file as a
  real Tekstide audit store; a second process opening that exact file in the narrow window between
  the table existing and the pragmas being set reads a torn, half-built store and
  `validate_identity` (`migration.rs`) correctly reports it as foreign. In the pre-fix world this is
  a materially worse symptom than a transient open failure: `diagnostics.rs` maps this reason to its
  own distinct user-facing status, so a first-run race could have told a user their audit database
  is **not a Tekstide audit store at all** — about a file Tekstide itself was mid-write on. Neither
  the RFC's own prose nor this document's own first draft had this shape; it is also the clearest
  argument for why the lock has to cover the whole `exists()`-through-pragmas sequence, not only the
  `CREATE TABLE` statement.

**One bound on the fix, confirmed by review 529**: `lock_creation_section` serialises processes
that take the lock — it does not make the file's on-disk state atomic. A `0.34.0` or earlier
process has no such lock at all (the reviewer's own ablation removed exactly that line to reproduce
the race), so two different installed versions racing to create the store on one machine still
race, and the older one cannot participate; and anything else reading the file mid-creation — a
backup tool, `sqlite3` at a prompt — can still observe the torn state. Neither is a defect in the
fix: a cross-version guarantee would need the older binary to cooperate, which is impossible by
construction, and RFC-058's own project lock has the identical property. The guarantee is "between
participants", not "the file is never torn".

**A real, disclosable limitation of the reproduction's own reliability**: run through
`rfcs/handoffs/ablate.sh` (which invokes `cargo test --workspace --all-targets`, exercising the
whole suite concurrently), the race did not reproduce across five separate ablation attempts, even
with the regression test strengthened to 20 iterations per run. Run in **isolation** --
`cargo build -p tekstide-core --bins` (required first: `--lib` alone does not build `[[bin]]`
targets, so without it the run fails with "expected the audit_store_create_probe binary at
.../target/debug/audit_store_create_probe" on *both* sides of the fix, which reads exactly like the
defect itself and is not — review 527 caught this in the first version of this recipe), then
`cargo test -p tekstide-core --lib two_real_processes_racing_to_create_the_store_both_succeed` --
it failed reliably. The cause is the inverse of an ordinary flake: this specific livelock needs
*precise, low-noise timing* to manifest, and a full-workspace test run's own concurrent activity
perturbs that timing enough to mask it. Documented here, in `ablate.sh`'s own header (commit
`04471ed`), and in the test's own doc comment, not only one of the three -- a gate that, for this
one test, is not the instrument that actually proves the fix.

### The fix

A real OS `flock`, the same primitive `project_lock` (RFC-058) already established for this
codebase, but **blocking** (`std::fs::File::lock`) rather than `try_lock`: this lock is held for one
moment of the store's own open — a single `open_internal` call, released when it returns — not for
the store's own operating lifetime, so refusing is wrong where RFC-058's own lock correctly refuses.
`lock_creation_section` (`store.rs`) acquires `<audit_dir>/.create.lock` before the `exists()` check
and holds it as a function-local for the rest of `open_internal`, releasing it (by dropping the
`File`) only when that function returns — through the migration branch too, not narrowed to only the
decide-and-create half, which is *safer* than a narrower hold: dropping the lock right after the
decision would hand a concurrent migration the identical race this lock exists to close, one step
later. A real kernel wait queue, not a retry schedule: the loser is woken exactly once, when the
winner releases the lock, already knowing a store exists rather than guessing when to look again —
which is what closes the livelock, not only the "already exists" conflict the RFC's own prose
anticipated.

**Ablated in a disposable `git worktree`** (review 527 Required 3: a dirty main tree, "restored"
afterward on the strength of "this is only my own deliberate change," is the exact vigilance
argument `ablate.sh`'s own clean-tree refusal exists to replace — a hand revert-and-checkout was
wrong to do even though nothing was lost this time):

```
WT=$(mktemp -d)/wt
git worktree add -q --detach "$WT" HEAD
# lock_creation_section's own call site removed, inside $WT only
cd "$WT" && cargo build -p tekstide-core --bins && cargo test -p tekstide-core --lib \
  two_real_processes_racing_to_create_the_store_both_succeed
git worktree remove --force "$WT"
```

**5 of 5 runs failed**, the main tree never touched. A third error shape appeared this round, `ERR
AuditStoreError { reason: Io }` — recorded above alongside a fourth, `UnsupportedApplication`, found
independently by the reviewer's own worktree ablation at review 528 and explained there in full:
four shapes now on record, and the error-agnostic design of the fix (probe for a now-existing store
on *any* failure) is what closes every one of them the same way, not only the shapes any single
round of manual testing happened to observe. **Zero fixture directories leaked across all 5 failing
runs** (`CleanupOnDrop`, review 527 Required 2), confirmed by a direct `ls`/count before and after —
re-confirmed independently by the reviewer's own re-run at review 528 (also 0 of 5, against the
identical instrument that had measured 6 leaked directories from 6 failing runs before the fix).
With the fix restored (the worktree discarded, never merged): clean on the main tree, every time,
across 30+ separate runs during development plus the three consecutive full-workspace gate runs
below (which do not themselves exercise the race's own failure mode for the reason stated above, but
do confirm nothing else regressed).

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean. Three consecutive
full-workspace runs, fresh short `/dev/shm` `TMPDIR` each: `763 + 22 + 1134`, 0 failed, 0 fixture
entries. No UI touched — `crates/tekstide/src/` has no diff in this slice.

## PR-059-B — the view

### D7: a new top-level destination, not a `ProjectOpenSurface`

`AppRoute` gained a third variant, `AuditView`, sibling to `ProjectBoard`/`ActiveProjectWorkspace`
(`crates/tekstide-core/src/route.rs`) — not a `ProjectOpenSurface`, since every one of those requires
`AppState::active_project_mut()` to be `Some`, and the audit store is app-wide. `AppCommand::
OpenAuditView` dispatches to it unconditionally (`ApplicationShell::dispatch`, `tekstide-core/src/
shell.rs`); `NavigationAction::OpenAuditView` carries `Ctrl+Alt+V` in `KeybindingPolicy::linux_mvp()`
(`navigation.rs`), checked by its own mechanical collision test
(`open_audit_view_shortcut_is_a_candidate_that_collides_with_no_other_rule`, `navigation/tests.rs`).
The real control is the top bar's own new "Audit" button (`top_bar_title_and_actions_row`), always
shown — the same no-active-project-required shape `Help`'s own button already has, not `Trust
Settings`'s conditional one.

Opening the route alone performs no I/O (`AppCommand::OpenAuditView`'s own `dispatch` arm is a pure
route change); the real query is `open_audit_view`'s own job, special-cased in `update_message`
alongside `LaunchTerminal`/`LaunchAgentRun` for the identical reason — real I/O the route change
itself cannot perform.

### D4: filters are exactly `AuditQuery`'s own fields

`project`, `family`, `outcome`, `operation` — nothing else. No free-text input exists anywhere in
this surface: `i18n::enforcement::no_raw_string_literal_is_passed_to_text_anywhere_in_the_crate`
covers every `text(...)` call in the crate including this module's, and the project filter cycles
only through currently-open projects (never a typed id), consistent with §5's own "do not reconstruct
a path for a project the board no longer shows." The `operation` filter has no dropdown of its own —
it is reached only by clicking a shown record's own "View full operation chain" button
(`AuditViewShowOperationPressed`), which also clears the other three filters at the same time:
viewing one operation's full chain with a leftover family/outcome filter active could silently hide
the other phase of the exact pair the click exists to show (PR-059-A's own cross-record authorization
contract, `AuditStore::append`'s `validate_operation_phase`).

Family and outcome each cycle through their own fixed list (`ALL_FAMILIES`/`ALL_OUTCOMES`) via a
button click, the same "cycle on click, wrap to All" idiom `SwitchActiveProject`/
`SwitchActiveDocument` already establish elsewhere in this codebase, rather than introducing this
crate's first `pick_list` widget for a four-field filter set.

**A real, disclosed gap, not a defect**: `ALL_FAMILIES`/`ALL_OUTCOMES` are hand-maintained arrays,
separate from the compile-time-exhaustive `family_symbol`/`outcome_symbol` matches that actually
render a row's text. Adding a 13th family forces a new arm in `family_symbol` (a real compile error)
but does not force an entry in `ALL_FAMILIES` — the new family would simply be unreachable as a
*filter* until the array is extended by hand, though every one of its records still renders correctly
in the unfiltered list. Recorded in `rfcs/future-work.md` ("The audit view's own family and outcome
filter lists do not fail to compile when a new variant is added").

### D3: the bounded-count wording

`audit-view-bounded-notice`'s own fluent select reproduces `agent::report::count_text`'s exact
wording ("{kept} listed, {omitted} more not recorded", bare number when nothing is omitted) rather
than calling that function directly — `CatalogArgs` has no method for embedding a precomputed
`String` (only `number`/`untrusted`/`trusted_symbol`), so the integration point is the fluent
template matching the same words, not a cross-crate call. `omitted` is computed from a second,
independent query (`AuditStore::count_matching`, new) against the same filters, ignoring
`before_sequence`/`limit` — **not** a running tally kept beside the page.

**§1's own honesty requirement, closed for the case `count_text`'s own callers never had to
consider**: `count_matching` can itself fail even when the page query above it succeeded. The first
draft called `state.catalog.get("audit-view-count-unavailable")` with no args for this case even
though the message interpolates `$kept` — caught by `audit_view_shows_count_unavailable_rather_than_
claiming_nothing_is_omitted`, which found the key rendering as its own literal name (the catalog's
own "a message that needs an arg it was not given is a failure, not a partial render" rule). Fixed by
passing `kept` through. `total_matching: Option<u64>` — `None` renders `audit-view-count-unavailable`
("the total could not be counted"), never read as `omitted == 0`.

### D1/D5: rows and paging

One row per durable record, newest first, via `AuditStore::query` with `before_sequence`/`limit` —
`PAGE_LIMIT = 50`, well under `MAX_AUDIT_QUERY_LIMIT` (200). "Load more" appends via `AuditRecordPage::
next_before_sequence`; no offset path exists anywhere in `audit_view`. A record's own `project_id`,
when present, is shown as the id the store holds — never a path, never reconstructed or guessed (§5).

### §6: `AuditHealth` is not a second, parallel disclosure

The view's own `lines` function opens with `project_board_audit_notifications(state)` — the exact
producer the Project Board already uses — converted to text the same way `project_board_lines` does,
not a second health-reading code path that could disagree with it. `audit_view_surfaces_a_degraded_
audit_health_the_same_way_the_project_board_does` asserts the view's own lines are a superset
containing every line the board shows for a degraded session. A **query** failure (the store open and
healthy, the read itself failing) is a distinct case `AuditHealth` has no slot for (it tracks only
open/write) — surfaced via the view's own `load_error` field and `audit-view-load-error`, which takes
precedence over the bounded-count notice (nothing honest to say about a page that never loaded).

### The wording guard

`audit_view::lines(state) -> Vec<String>` and `audit_view::row_text(catalog, record) -> String` are
the exact functions `content_area`'s `AuditView` arm and its row loop call — the same "factored-out
function, not a string rebuilt in the test" shape RFC-058 review 518 required of `project_board_
lines`. Eight tests assert directly against their output: the bare-count case, the omitted case, the
count-unavailable case, the `AuditHealth`-reuse case, the load-error-takes-precedence case, the
row's project-name-or-"no project recorded" case, plus two real-store end-to-end tests (below).

### A real store, real records, real filtering — not a fixture

`opening_the_audit_view_through_a_real_key_event_loads_real_records_from_the_real_store` opens a real
`AuditStore` at the same path `open_audit_view`'s own route uses (`test_audit_state_dir`), writes a
real trust-grant chain through the real UI action (`press_trust_settings_action`, the same producer
RFC-058's own trust tests rely on), opens the audit view through a real dispatched `NavigationAction::
OpenAuditView`, and cross-checks the loaded page's sequences against an independent, direct
`AuditStore::query` — not against anything the test itself constructed.
`audit_view_filter_cycling_and_show_operation_narrow_and_widen_the_real_page` extends this: cycling
the family filter to one the store holds no records of narrows to empty (not a silent no-op showing
the unfiltered page); cycling once more to `TrustChange` narrows to exactly the real pair; clearing
widens back to both; clicking a record's own real operation id returns exactly its two real phases,
each carrying that same operation id.

### Live capture

A real window, a throwaway project (`mktemp -d` under the session scratchpad) and a throwaway
`XDG_STATE_HOME` (same), per the pack's own recipe — `wl-copy --clear` → `niri msg action
screenshot-window --id <id>` → confirm `wl-paste --list-types` prints `image/png` → `wl-paste` to
file, with focus (`niri msg -j windows`, `is_focused`) re-verified before every `wtype` call on the
shared desktop. Four captures, four distinct checksums (confirming none is `wl-paste`'s own stale
previous-clipboard fallback):

- `pr-059-b-01-first-opened-one-record.png` — `Ctrl+Alt+V` on a freshly opened, untrusted project:
  one real `project_added` record, `Showing: 1`, every filter at "All", no "View full operation
  chain" button (this record has no `operation_id`).
- `pr-059-b-02-trust-granted.png` — Trust Settings after a real grant, through the real
  Tab-then-Enter dialog flow, for context on what produced the next capture's records.
- `pr-059-b-03-three-records-with-view-chain-buttons.png` — the audit view again, now showing all
  three real records (`project_added`, and the `trust_change` `Authorized`/`Applied` pair, newest
  first), each `trust_change` row carrying a real "View full operation chain" button, `Showing: 3`,
  and the status bar reading "Audit View | 1 project".

**A real, disclosed limitation of this round's own capture**: no mouse-click tool was available in
this environment, only `wtype` (keyboard). The filter-cycle and "view chain" buttons have no
keyboard route of their own (by design — they are ordinary in-surface buttons, the same
`click_message_kind::BackgroundControl` classification `ChangeReviewFileRowPressed` already has, not
a `NavigationAction` that `control_coverage` would require one for), so clicking them live was not
possible this round. Covered instead by `audit_view_filter_cycling_and_show_operation_narrow_and_
widen_the_real_page`, dispatching the exact `Message`s each button's own `.on_press` sends, against
the same real store — arguably stronger evidence (exact, deterministic, real) than a live click would
have added, but the live-capture checklist item itself should note this is what stood in for it.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean. Three consecutive
full-workspace runs, fresh short `/dev/shm` `TMPDIR` each: `771 + 22 + 1135`, 0 failed, 0 fixture
entries.

### Review 530 required fixes

**Required 1 — §2's own disclosure.** The view said nothing about which question it answers; a user
comparing it against Approval History had no way to know the two would legitimately disagree
(`what-the-audit-view-must-not-do.md` §2, RFC-059 D2). `audit-view-disclosure` now leads every call
to `audit_view::lines`, unconditionally: "What happened, as Tekstide recorded it -- kept until
purged. This is not a live status: for what is pending right now, see Approval History."
`audit_view_leads_with_the_disclosure_of_which_question_it_answers` asserts it directly.

**Required 2 — a real compile guard, not a disclosed gap.** `family_cycle_index`/
`outcome_cycle_index` (new, `crates/tekstide/src/shell.rs`) are exhaustive matches (no wildcard),
the same discipline `family_symbol`/`outcome_symbol` already have, mapping each variant to its own
position in `ALL_FAMILIES`/`ALL_OUTCOMES`. `cycle_copy` now navigates by that index directly instead
of a `position()` scan, so the array and the match **must** agree for cycling to work at all --
pinned by `all_families_and_outcomes_cover_every_declared_cycle_index_exactly_once`, which fails if
an entry is ever inserted out of order. The `rfcs/future-work.md` entry is marked closed, not
deleted — review 530's own reasoning (PR-059-A's own enumeration-test bar applies here too) is kept
alongside the original finding.

**Ablated in a disposable `git worktree`** (`ablate.sh`'s own header recipe, main tree never dirty):
a 13th `AuditEventFamily` variant (`PlantedThirteenthFamily`) was planted directly in the enum, with
`validate()`'s own match and the `impl_code!` arms patched just enough for `tekstide-core` to
compile cleanly (isolating the one thing under test). `cargo build -p tekstide` then failed with
two `E0004: non-exhaustive patterns` errors, both naming `AuditEventFamily::PlantedThirteenthFamily`
not covered: `shell.rs:13932` (`family_cycle_index`, the new guard) and `shell.rs:14204`
(`family_symbol`, the pre-existing one) — the two exhaustive matches this fix now depends on both
caught it, independently. Worktree removed; the planted variant never touched the main tree.

### Gate, re-run after the required fixes

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean. Three consecutive
full-workspace runs, fresh short `/dev/shm` `TMPDIR` each: `773 + 22 + 1135`, 0 failed, 0 fixture
entries.

## PR-059-C — purge, reachable

### The storage layer was already finished

`AuditStore::purge_project_records`/`purge_all_records` and `AuditPurgeReceipt` (`crates/tekstide-
core/src/audit/purge.rs`) existed before this slice, built ahead of any production caller the same
way `AuditStore::query` was before PR-059-A. This slice is UI only, mirroring `TranscriptPurgeModal`'s
own confirmation-dialog shape (RFC-033) closely: a new `ModalContent::AuditPurge(AuditPurgeModal)`
variant, `AuditPurgeButton` (the same two-item `Purge`/`Cancel` cycle), and `AuditPurgeTarget::{
Project(ProjectId), Global}` naming the dialog's own intent before either real purge function runs.

### §4: what disappears, that it cannot be undone, and the real receipt

The confirmation states scope two ways, the precedent's own "affirmative and by exclusion" shape:
the title names it (*"Purge this project's audit records?"* / *"Purge every audit record, for every
project?"*), the body says what is and is not affected (*"Other projects are not affected."* for the
project-scoped dialog) and ends with the literal sentence *"This cannot be undone."*
`record_count` is captured once, at dialog-open time, from `AuditStore::count_matching` — the
confirmation refuses to open rather than show `0` when that query itself fails, since `0` would read
as "nothing to delete," a different fact from "could not be counted."

The receipt shown afterwards is `AuditPurgeReceipt::deleted_record_count`, read straight off the
`Result` the real `DELETE` returned — never a count taken before the delete, never the number of rows
the page happened to have listed. A purge whose transaction failed (`Err(AuditStoreError)`) is stored
and rendered as a failure (`audit-view-purge-failed`), never silently dropped and never read as a
success: unlike `apply_transcript_purge`, there is no "delete anyway, the record is merely
best-effort" fallback available here, because the audit store is both the thing being mutated and the
thing that would record the action — a failure to open it refuses the whole action rather than
guessing.

The notice is one-shot: `clear_purge_notice` runs at the start of every other action on this view
(each filter cycle, clearing filters, viewing an operation's chain, and re-opening the view after
navigating away) so a receipt from an earlier purge never lingers once the user has moved on to
something else. `reload`'s own call from inside `apply_purge` deliberately does **not** clear it —
that is the one reload that must leave the notice it just set standing.

### Tests

Seven new tests. Two pure wording-guard tests: `audit_purge_dialog_body_names_the_scope_count_and_
irreversibility` (both scopes, both clauses, the real count), `audit_view_purge_receipt_shows_the_
real_deleted_count`/`audit_view_purge_failure_is_shown_not_silently_dropped` (the success/failure
split, and that a failure never also renders a receipt). Three real-store end-to-end tests:

- `purging_a_project_through_the_real_dialog_removes_only_that_project_then_global_purge_removes_
  the_rest` — two real projects, each trust-granted through the real dialog flow (4 real records
  total), purges the first project through the real `OpenAuditPurgeDialog`/`AuditPurgePressed`
  messages, cross-checks via an independent direct `AuditStore::query` that **only** that project's
  records are gone and the other project's **two** are untouched (the dialog's own "other projects
  are not affected" claim, proven rather than only stated), and that the receipt's
  `deleted_record_count` is exactly `2`. Then purges globally and confirms the remaining two are
  gone too.
- `cancelling_the_audit_purge_dialog_deletes_nothing` — opens the dialog (default focus `Cancel`,
  the same safe-default `TranscriptPurgeModal` already uses), activates with that default focus
  still in place, and confirms through a direct store query that nothing was deleted and
  `last_purge` was never set at all.

### The book, corrected in this slice

`docs/src/users/local-data-and-privacy.md`'s *"There is no in-app command to purge the audit
store... Delete the `audit/` directory to reset it"* is false as of this slice. Corrected in place,
per the release checklist's own Corrections rule: names what changed (the Audit view's two purge
controls, what each one's confirmation requires), and says what a reader who relied on the old
sentence should now do — manual deletion of `audit/` still works and is still the only way to remove
the store's own file, not only its rows, which the in-app purge never touches.

### Live capture

Same throwaway-project/throwaway-`XDG_STATE_HOME`/`wl-copy`-`screenshot-window`-`wl-paste` discipline
as PR-059-B. One capture kept
(`evidence/pr-059-c/pr-059-c-01-audit-view-with-purge-buttons.png`): the real audit view after a real
trust grant, showing the disclosure line, three real records, and the "Purge All Records…" button
rendered correctly.

**The same disclosed limitation as PR-059-B's own capture round, and for the identical reason**: no
mouse-click tool was available, only `wtype`. The purge-opening buttons (like the filter-cycle
buttons before them) are ordinary `BackgroundControl`s with no keyboard route, so I could not click
through to the confirmation dialog, confirm a purge, or reach the per-project purge button (which
itself requires first clicking the project filter to narrow to one project — also mouse-only) live
this round. The real-store end-to-end tests above dispatch the exact `Message`s each button's own
`.on_press` sends — `OpenAuditPurgeDialog`, `AuditPurgePressed` — against a real store, which is what
actually proves the deletion, the scope isolation, and the receipt's own correctness; the live
capture's own job here is narrower, confirming the buttons exist and render with the right labels.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean. Three consecutive
full-workspace runs, fresh short `/dev/shm` `TMPDIR` each: `778 + 22 + 1135`, 0 failed, 0 fixture
entries.

### Review 532 required fix — purge also revokes trust, and the dialog did not say so

**The chain, verified independently rather than taken on trust**: `AuditStore::
has_applied_trust_grant` (`store.rs`) decides trust by querying `audit_events` for the newest
`TrustChange`/`Applied`/`TrustGrant` row; `purge_records` (`purge.rs`) is `DELETE FROM
audit_events`, globally or `WHERE project_id = ?1` — the same rows; `verify_restored_trust_against`
(`shell.rs:7735`) re-checks every currently-trusted project against the store on every real
project-open call site and demotes one whose row is gone (`deny_unverified_trust`, confirmed by
reading it directly: sets `WorkspaceTrust::Restricted`). So a purge — per-project or global — also
revokes trust for whatever it deletes the row for, and the confirmation described the act purely
as deleting a log.

**Fixed with a new, conditional notice, not a blanket warning.** `AuditPurgeModal::revokes_trust`
is captured at dialog-open time (the same "captured, not re-read" shape `record_count` already is):
`true` only for `Project` scope when that specific project is trusted *right now* --
`audit-view-purge-dialog-trust-notice-project` renders only then, since an already-untrusted
project's purge has nothing to revoke and a notice there would claim a consequence that does not
happen. `Global` scope states the consequence unconditionally
(`audit-view-purge-dialog-trust-notice-global`): a closed, currently-trusted project's state cannot
be checked from here, and the claim is true regardless. Both say the revocation is not immediate --
the already-open session keeps working until trust is next re-checked -- matching what
`verify_restored_trust_against`'s own call sites actually do.

**Proven, not only worded.** `purging_a_trusted_projects_records_demotes_it_once_trust_is_
reverified` is the mechanism end to end against a real store: grants trust through the real
dialog flow, opens the real purge dialog and asserts the trust notice is present, confirms the
purge, asserts the **already-open** session is still `Trusted` (not demoted synchronously), then
calls `verify_restored_trust` directly (the same function every real open call site calls) and
asserts the project is demoted. `audit_purge_dialog_body_names_the_scope_count_and_
irreversibility` was extended to cover both the present case (trusted project) and the absent case
(already-untrusted project, global scope's unconditional case).

**The book corrected in the same commit**: the new purge paragraph now says the consequence
explicitly, ties it to the page's own existing "the audit store is authoritative" claim rather
than introducing a second, disconnected fact, and says the revocation is delayed (next open), not
immediate.

### Gate, re-run after the required fix

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean. Three consecutive
full-workspace runs, fresh short `/dev/shm` `TMPDIR` each: `779 + 22 + 1135`, 0 failed, 0 fixture
entries.

## Whole-RFC — the `0.35.0` candidate cut (2026-10-12)

All three slices and the Whole-RFC checklist are done. `rfcs/accepted/059-seeing-the-audit.md`
moved to `rfcs/done/`, with a new `## Closed` section; `rfcs/README.md` (the Accepted table, now
empty again; a new Handoffs row; a new Done row), `rfcs/delivery-plan.md`'s own row, and the
handoff pack's `README.md`/`acceptance-qa-checklist.md`/`task-breakdown-pr-plan.md`/
`what-the-audit-view-must-not-do.md` frontmatter pointers all updated in the same commit
(`ce2e06a`). `Cargo.toml`/`Cargo.lock` bumped to `0.35.0`.

### Whole-RFC checklist items, verified directly

- **`REQ-SEC-025` met for audit data** — the first plan row to track it. Said so, by name and not
  only by id, in `CHANGELOG.md`'s own new `0.35.0` section.
- **`REQ-SEC-023` not overclaimed**: `audit_events`'s schema (`schema.rs`, `CREATE_SCHEMA_V3`) was
  read directly — every column is an id, an enumerated `CHECK`-constrained symbol, or a
  length-bounded reference. No free-text column exists. The log-content half is stated as unmet
  and out of scope (RFC-068) in the changelog, the RFC's own `Closed` section, and
  `delivery-plan.md`'s row.
- **`RedactionClaimScope` still has exactly one variant** — `security.rs:282`, read directly:
  `StructuredMetadataOnly` alone.
- **No free-text column was added** — confirmed from the same schema read above; this RFC's own
  diff never touches `schema.rs` at all.
- **Approval History still exists and still answers its own question** — untouched by this RFC;
  the audit view's own §2 disclosure (review 530) is what keeps the two from being read as the
  same thing.
- **The book describes every family the view can display** — all twelve, in
  `local-data-and-privacy.md` (six added at PR-059-B, closing a gap found during that slice: the
  page described only six of twelve before this RFC touched it).
- **`rfcs/handoffs/release-checklist.md` run in full for the candidate** — see below.

### The release checklist, run for the candidate (not the publish)

Per RFC-058's own precedent and the reviewer's confirmation at review 532: the candidate cut
(version bump, changelog, RFC lifecycle move, register rows) is mine; the actual tag, `cargo
publish`, `release-0.35.0.md` and `post-publish-check.sh` are the owner's own later step, not run
here.

- `git status --short`: clean (checked after every edit, not only at the end).
- `cargo fmt --check`: clean.
- `cargo test --doc --workspace`: 2 passed (the `WatchEvents` compile-fail guard included).
- `cargo test --workspace --all-targets`: three consecutive clean runs, `779 + 22 + 1135`, 0
  failed, 0 fixture entries.
- `cargo clippy --workspace --all-targets --all-features -D warnings`: 0 diagnostics.
- `cargo build --release --locked`: clean. `./target/release/tekstide --version` reports `0.35.0`;
  `--help` lists `Ctrl+Alt+V` with its real description.
- `cargo package -p tekstide-core --locked` and `cargo package --workspace --locked`: both clean,
  against the committed tree (an uncommitted `Cargo.toml` refuses packaging by design — confirmed
  directly, not assumed).
- `cargo publish --workspace --dry-run --locked`: clean, both crates.
- **Package Smoke**: no stray `.rs` outside `src/`/`tests/`/`examples/`/`benches/` in either
  archive; `LICENSE`/`NOTICE`/`README.md`/`Cargo.toml` present in both. Built and tested from the
  unpacked `tekstide-core-0.35.0` directly (`cargo build --offline --bins`, then `cargo test
  --offline`): `1134 passed, 1 failed` — the one expected, by-construction failure
  (`real_repository_filesystem_scan_cost_headless_benchmark`, refusing to benchmark a directory
  that is not a real repository), exactly the checklist's own documented shape. Did **not** attempt
  a standalone `cargo build` inside the unpacked `tekstide-0.35.0` directory — the checklist names
  this exact attempt as failing by construction (`tekstide-core 0.35.0` is not yet on crates.io),
  and the real pairing gate is the workspace dry-run above, which already passed.
- `cargo audit`: zero vulnerabilities; three warnings (`paste`, `ttf-parser`, `lru`), matching
  `dependency-advisories.md`'s own register exactly — no new finding, nothing to retire.
- **Standing watches**: `cargo tree -p tekstide | grep -i accesskit` — empty (RFC-014 R2 still not
  reopenable). `README.md:37` still says "No screen-reader support" plainly.
- **Run It**: the release binary launched as a first-time user, fresh `XDG_STATE_HOME`, no
  arguments — empty Project Board, "No projects yet," a real path field, "Browse…," and the CLI
  hint, naming only actions that exist. Captured to
  `.git-exclude/release-evidence/0.35.0/cold-start-empty-board.png` (not committed, per the
  checklist's own instruction).
- **The book against the changelog**: swept `what-works-today.md`'s own `## Audit` section, which
  described the twelve recording families but said nothing about either new capability this
  release adds — added two sentences naming the view and the purge controls. Grepped for stale
  negative claims about audit/purge across both book pages; the one other "no in-app command" hit
  (`local-data-and-privacy.md:19`) is the unrelated recent-projects cache, unaffected by this RFC.

### Confirmed no document claims the release yet

`grep -rnE "^(Status:|status:).*[Rr]eleased" --include="*.md" rfcs/` was read against this RFC's
own five status places, all of which say **"candidate, not yet published"**, never "released":
`CHANGELOG.md`, `rfcs/README.md` (both the Handoffs and Done rows), `rfcs/delivery-plan.md`,
`rfcs/done/059-seeing-the-audit.md`'s own status line, and the handoff pack's `README.md`
frontmatter. No `rfcs/handoffs/release-0.35.0.md` exists — confirmed directly, not assumed — so
`the_newest_release_claimed_in_the_changelog_has_a_publish_record` has nothing to disagree with.
`rfc_docs_invariants`: 22/22, including `the_newest_release_status_matches_whether_it_was_actually_
published` and `an_rfc_a_release_names_lives_in_done`.
