---
title: "What the audit view must not do"
source_rfc: "../../done/059-seeing-the-audit.md"
---

# What the audit view must not do

The audit store is the one part of this product whose promise is **honesty about what happened**.
A viewer changes the stakes: a reader shown a list concludes the list is what happened. Everything
below follows from that.

## §1 It must not imply completeness it cannot demonstrate

A page is bounded by `AuditQuery::limit`. **A bounded page says so, in the words the product
already uses** — `agent/report.rs`'s `count_text` prints `"{kept} listed, {omitted} more not
recorded"`. Do not invent a second idiom for the same honesty, and do not print a count that is
really the number of rows retained while implying it is the number of events that happened.

**If the store's own write path may have dropped records, the view must not claim otherwise.** This
is why PR-059-A lands first: the view's honesty rests on it.

## §2 It must not become a second, disagreeing history

Approval History is in-memory, bounded by `approval_history_limit`, and **evicted**. The audit view
is durable and complete to the limit of the store. **They will disagree, legitimately.** Each must
say which question it answers — *what is live now* versus *what happened* — and neither may be
worded so a reader takes it for the other. Do not quietly retire Approval History; D2 keeps it.

## §3 It must not grow a field that can hold a secret

`audit_events` has **no free-text column**. Every column is an id, an enumerated symbol, a
timestamp, or a reference bounded to `[A-Za-z0-9_.:-]` and 128 bytes. This is what makes
`REQ-SEC-023` already true for audit views, structurally rather than by scanning.

**Adding a free-text column to make the view more informative would destroy that property and
silently move this product into RFC-068's territory.** If a slice believes the view needs text the
schema cannot hold, that is a finding to report, not a migration to write.

## §4 Purge must report what it did, not what it intended

`AuditPurgeReceipt` carries the real `deleted_record_count` from the `DELETE`. **Show that
number.** Do not show a count taken before the delete, do not show the number of rows the view had
listed, and do not report success for a purge whose transaction failed — the whole point of the
receipt is that it is the outcome rather than the intent.

A purge is irreversible and the book says so. The confirmation must say **what scope** is about to
go — this project, or everything — because those are very different acts behind one word.

## §5 It must not leak a project the board no longer shows

The store is app-wide and keeps records for projects long since removed from the board, including
their `project_added` and `trust_change` events. That is correct and is the point of an audit
trail. But a **path** for such a project is not in the store and must not be reconstructed or
guessed for display; what the store holds is a `project_id`. Show what is recorded.

## §6 It must not make the store's failures invisible

`AuditHealth` already exists and is threaded through boot. If the store cannot be opened, or a
write was rejected, **the view is the one place a user could find that out**. Silence here repeats
the exact defect PR-059-A exists to close, one level up.
