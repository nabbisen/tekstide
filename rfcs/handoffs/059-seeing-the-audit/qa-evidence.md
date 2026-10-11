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
