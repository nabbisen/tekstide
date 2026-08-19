---
title: "RFC-033 acceptance and QA checklist"
status: "Open"
rfc_file: "../../proposed/033-transcript-lifecycle-controls.md"
target_milestone: "M11"
created: "2026-08-19"
---

# RFC-033 — acceptance and QA checklist

## Prerequisite (PR-033-A)

```text
[ ] approval_state_root set explicitly at the GUI launch call site.
[ ] A test opts out of capture AND binds an approval channel successfully.
[ ] Ablated: removing the explicit approval_state_root fails that test specifically.
```

## Opt-out (PR-033-B)

```text
[ ] Reachable from a real key press, not a dispatched command.
[ ] A run launched with the opt-out set produces NO transcript file, asserted against the
    documented path shape — not against the request's own field.
[ ] The setting persists per project across a restart (or does not, with a stated reason).
[ ] The surface distinguishes "do not record future runs" from "delete what exists".
```

## Purge (PR-033-C)

```text
[ ] Bytes are gone from the real filesystem, asserted directly.
[ ] The tombstone remains and the surface does not claim otherwise.
[ ] The project-local refusal (UnsafeProjectPath) is preserved, not weakened.
[ ] The confirmation names the scope of what is deleted.
[ ] Retained-data visibility wired — transcript_local_data_summary has a real caller.
```

## Audit (PR-033-D)

```text
[ ] valid_transcript_purge() read before the record was designed; what it permits is stated.
[ ] No path and no byte count in the record.
[ ] The trade is stated in the closeout: the store retains a record of a privacy action.
```

## Claims that must not be made

```text
[ ] Not claimed: purge removes every trace. A tombstone and an audit record remain.
[ ] Not claimed: opting out removes existing transcripts.
[ ] Not claimed: the store is viewable. Nothing renders it.
```

## Published documentation

```text
[ ] README.md's "no in-app way to turn capture off or to purge it" sentence narrowed to what
    is still true, in the same commit as the last slice.
[ ] crates/tekstide-core/README.md's equivalent bullet updated.
```

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
