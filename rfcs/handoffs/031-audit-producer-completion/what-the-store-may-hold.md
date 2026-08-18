---
title: "What the audit store may hold — required reading before RFC-031 code"
status: "Required reading — RFC-031 closed 2026-08-19; still binding for any future producer"
rfc_file: "../../done/031-audit-producer-completion.md"
target_milestone: "M11"
created: "2026-08-19"
---

# What the audit store may hold

**Read this before writing a producer.** The audit store is append-only and local-first.
A record written wrongly is not a rendering bug you can fix in the next release — it is
durable data on a user's disk, and `purge_all_records` has no production caller.

## The good news: RFC-013 already made most mistakes unrepresentable

`DurableAuditRecordV1` has **no free-text field**. Every field is a typed enum or an id.
If you are looking for somewhere to put a helpful message, there isn't one, and that is
deliberate.

The one string-shaped field is a validated newtype:

```rust
pub fn new(value: impl Into<String>) -> Option<Self>   // AuditReference
// non-empty · bounded length · only [A-Za-z0-9-_.:]
```

**A filesystem path cannot be stored.** `/` is not in the permitted set, so
`AuditReference::new("/home/u/project")` returns `None`. This is the same design as
`DisplayText` in `text_safety`: the mistake is not expressible in a value the API accepts.

**Do not treat that as permission to stop thinking.** It restricts the character set, not the
meaning.

## The two things the type system does not decide

### 1. `subject_ref` will accept a single path segment

`my-project` passes. So does `..`, and so does a directory name chosen by whoever created it
to be confusing or misleading. That is **untrusted, attacker-influenceable text**, and this
project escapes it at every widget for exactly that reason — but **the store is not escaped on
read**, and nothing downstream of it is obliged to escape either.

**Both producers in this slice leave `subject_ref` as `None`.** `project_added` identifies the
project by its generated `project_id`. If you find yourself wanting to put a name in so a
future reader can tell which project it was, that is the recent-projects file's job, and it
already does it.

### 2. `reason_code` is coarser than the truth, and that is the accepted trade

`AuditReasonCode::RestrictedMode` already exists. RFC-004 blocks **nine** features; this one
code cannot say which. Use it anyway — finer granularity is a frozen-schema change — and
**record the coarseness in the evidence** rather than letting a reader assume the store
distinguishes them.

## The test that matters more than the obvious one

The obvious test asserts a record appears. Write it, then write the one that would have caught
the mistake:

**Assert the record's `subject_ref` is `None`.** Presence-of-event tests pass just as happily
when a record carries a project's directory name as when it does not. The absence assertion is
the one that fails if someone later "improves" the producer by adding context.

## One thing this slice does not change

**Nothing renders the audit store.** There is no user-facing view of it, at all. Recording an
event does not make it visible to anyone, and the evidence must not imply otherwise — this
project shipped an approval-history surface that nothing could open for a full release, and
the lesson is that "it is recorded" and "a user can see it" are separate claims.
