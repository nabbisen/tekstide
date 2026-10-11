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

**Against the tree before the fix, in isolation, 10 of 10 runs failed.** Two shapes observed across
repeated runs:

- `ERR AuditStoreError { reason: Busy }` — a genuine SQLite multi-process busy-handler livelock.
  `busy_timeout`'s own retry schedule is fixed, not jittered; two processes released at close to the
  same instant can retry in lockstep and both exhaust the full 2-second timeout without either
  making progress. This is the shape that actually dominated empirically, not the "already exists"
  conflict the RFC's own prose (written before this reproduction ran) anticipated.
- `ERR AuditStoreError { reason: Sqlite }`, `"table audit_events already exists"` — the loser's own
  `CREATE TABLE` executing after the winner's had already committed, the shape the RFC's prose
  named.

**A real, disclosable limitation of the reproduction's own reliability**: run through
`rfcs/handoffs/ablate.sh` (which invokes `cargo test --workspace --all-targets`, exercising the
whole suite concurrently), the race did not reproduce across five separate ablation attempts, even
with the regression test strengthened to 20 iterations per run. Run in **isolation**
(`cargo test -p tekstide-core --lib two_real_processes_racing_to_create_the_store_both_succeed`,
nothing else from the workspace running concurrently), it failed reliably, 10/10. The cause is the
inverse of an ordinary flake: this specific livelock needs *precise, low-noise timing* to manifest,
and a full-workspace test run's own concurrent activity perturbs that timing enough to mask it.
Documented here rather than silently relying on a gate that, for this one test, is not the
instrument that actually proves the fix — the isolated run is.

### The fix

A real OS `flock`, the same primitive `project_lock` (RFC-058) already established for this
codebase, but **blocking** (`std::fs::File::lock`) rather than `try_lock`: this lock is held for one
moment (deciding whether to create the schema or open an existing one), not for the store's own
lifetime, so refusing is wrong where RFC-058's own lock correctly refuses. `lock_creation_section`
(`store.rs`) acquires `<audit_dir>/.create.lock` before the `exists()` check and holds it through
either branch of the decision, releasing it (by dropping the `File`) before `open_internal` returns.
A real kernel wait queue, not a retry schedule: the loser is woken exactly once, when the winner
releases the lock, already knowing a store exists rather than guessing when to look again — which is
what closes the livelock, not only the "already exists" conflict the RFC's own prose anticipated.

**Ablated, in isolation** (the only invocation shown above to reproduce the race at all): reverting
`lock_creation_section`'s own call site in `store.rs` (clean tree, manual revert matching
`ablate.sh`'s own replace-run-restore shape, then restored via `git checkout` since the dirty state
was this exact, deliberate, single-line reversion) — 10 of 10 runs of the 20-iteration regression
test failed, reproducing the pre-fix behaviour exactly. With the fix restored: clean, every time,
across more than 30 separate runs during development plus the three consecutive full-workspace gate
runs below (which do not themselves exercise the race's own failure mode for the reason stated
above, but do confirm nothing else regressed).

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean. Three consecutive
full-workspace runs, fresh short `/dev/shm` `TMPDIR` each: `763 + 22 + 1134`, 0 failed, 0 fixture
entries. No UI touched — `crates/tekstide/src/` has no diff in this slice.
