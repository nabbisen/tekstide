# RFC-013: Durable Audit Store and Local Data Policy - Developer Handoff Pack

Source RFC: [RFC-013](../../done/013-durable-audit-store-and-local-data-policy.md)
Target milestone: **M7**
Source RFC status: **Implemented with documented limitations**

## Active work: Amendment 1 — schema v2 migration (2026-07-30)

**Start here if you are picking up this pack now.** RFC-013 is otherwise closed; this is reopened work.

- **[`amendment-1-schema-v2-migration.md`](./amendment-1-schema-v2-migration.md)** — the slice to implement.

Authorised by the human owner on 2026-07-30 as an additive amendment ([RFC-013 §Amendment 1](../../done/013-durable-audit-store-and-local-data-policy.md)), adding one `action_kind` (`command_cwd_mismatch`) and one `outcome` (`anomaly`) for RFC-021, delivered as a v1 → v2 migration.

It exists because commit `3ac794b` widened `CREATE_SCHEMA_V1`'s `CHECK` constraints in place without bumping the schema version or adding a migration step, so the new record silently cannot be written on any existing installation. Verified by probe in review response 117. No data loss; no user-visible effect yet; permanent per install until this lands.

This will be the **first migration step the harness has ever actually run** — `MIGRATIONS` has been empty since RFC-013 closed. It gets a full review, not diff confirmation, and it blocks RFC-021's PR-021-F closeout.

## Files

- `implementation-handoff.md` - developer-facing durable-audit guidance.
- `task-breakdown-pr-plan.md` - recommended implementation slices and review gates.
- `acceptance-qa-checklist.md` - acceptance traceability and required evidence.
- `qa-evidence.md` - placeholder for observed implementation gates, security notes, and known limitations.

This handoff inherits the source RFC lifecycle state. Response 103 accepted PR-013-H as complete with documented limitations. Its original finding requiring a published `crates/tekstide/NOTICE` file was withdrawn after maintainer challenge — the root `NOTICE` alone covers repository and binary/release-tarball distribution, and the `tekstide` source package does not redistribute third-party code — so no crate-level notice was added. RFC-013 is implemented with documented limitations.

## Source Summary

RFC-013 defines a structured, local SQLite audit store with explicit path containment, schema migrations, bounded queries, purge, corrupt-store recovery, and failure semantics for required security decisions versus already-observed facts.

RFC-013 does not implement the final GUI audit viewer, tamper evidence, encryption, secure deletion, cloud sync, automatic retention, exact command/output storage, or general command interception.
