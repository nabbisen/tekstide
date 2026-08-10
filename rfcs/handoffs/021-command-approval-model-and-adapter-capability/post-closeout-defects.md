---
title: "RFC-021 — Post-closeout defects: implementation handoff"
rfc: "RFC-021"
rfc_file: "../../done/021-command-approval-model-and-adapter-capability.md"
status: "Ready for implementation — recorded 2026-08-04, unblocked"
created: "2026-08-10"
---

# Two defects in a closed RFC

Both were found while reviewing later work and are recorded in
[`qa-evidence.md`](./qa-evidence.md) §Open defects found after closeout. **That file is
where results go; this one is what you read before starting** — the distinction exists
because an obligation recorded only in evidence has been lost four times here.

Neither is blocked. Do them in either order.

## 1. The sentinel test cannot detect what it claims to check

`approval/tests/coordinator.rs:980`:

```rust
let raw_bytes = std::fs::read(test_audit.store.storage_path().database_file())
```

The store is **still open** at that point and runs in WAL mode, so the records it just
wrote live in `audit.sqlite3-wal` until the connection closes and SQLite checkpoints. The
main database file at that moment is a 4 KiB header page holding none of them. SQLite's
auto-checkpoint threshold is 1000 pages, which five-plus records do not approach.

So the raw-byte assertion is **vacuously true** — it would pass unchanged if the
coordinator wrote `SENTINEL_ARG` and `SENTINEL_CWD` verbatim into the schema. The claim
*"no command text or cwd reaches the durable store"* is therefore **currently
undemonstrated**. The typed-query half of the test is unaffected and still holds.

**This is very likely an evidence defect, not a leak.** The equivalent path under RFC-017
PR-017-F was probed directly: the record does land in `audit.sqlite3` once the store is
dropped, and `DurableAuditRecordV1`'s validation forbids the relevant fields structurally.
**Do not go hunting a leak.** Fix the test so it *could* find one.

**The fix is already written** — RFC-018 PR-018-D's
`sentinel_pasted_content_never_reaches_the_durable_audit_store` is the shape to copy:

1. Capture `audit_dir()`, then **`drop(store)`** — that is what makes SQLite checkpoint
   and remove the sidecars, reproducing the on-disk state a real session leaves.
2. Scan **every file** in the audit directory, not `database_file()`. Robust to SQLite's
   sidecar set changing.
3. Add a **positive control** asserting a genuinely persisted field is present, with a
   message saying why — otherwise the negative assertions pass merely because nothing was
   read at all.

One thing PR-018-D got right that is worth copying deliberately: it made its sentinel
content take the **audited path** (wrapped in `\x1b[31m…\x1b[0m`, so it classified as a
real `Block`). A sentinel that never reaches the producer tests nothing. Check that
whatever writes here actually writes.

**Review gate:** the corrected test fails when the store is *not* dropped before scanning
— demonstrate that, because it is the exact defect being fixed and a fix that cannot
detect its own absence is not evidence.

## 2. `bind_recovers_from_a_stale_socket_file` fails intermittently

`approval/tests/channel.rs:143`. Observed failing once under full-workspace parallel
execution, passing in isolation and on re-run (2026-08-03, during RFC-017 PR-017-E), and
disclosed rather than re-run away.

Undiagnosed. It is either test-isolation noise or a real TOCTOU window in the bind path
— **and that path does not get the benefit of the doubt**, because two genuine defects
have already been found in it: a socket placeable outside the state root via symlink
swap, and then via ancestor swap after the first fix.

**Diagnose before concluding.** If it is isolation — a shared path, a fixed socket name,
a temp directory colliding under parallel runs — say which and make it deterministic. If
it is a real race, that is a security finding in the bind path and stops being a test fix.

**Do not mark it `#[ignore]` or add a retry.** Either would convert a signal into
silence, and this is the one path in the codebase where that trade is clearly wrong.

## What this does not include

No behaviour change to the approval model itself. RFC-021 is closed and remains so; these
are defects in its *evidence* and its *test suite*, not in its design. If either
investigation turns up something that would change the model, stop and raise it rather
than absorbing it here.
