---
title: "RFC-012: Generated Change Review Foundations - Acceptance / QA Checklist"
rfc: "RFC-012"
rfc_file: "../../proposed/012-generated-change-review-foundations.md"
status: "Proposed"
target_milestone: "M6"
source_rfc_status: "Proposed"
created: "2026-07-21"
---

# RFC-012: Generated Change Review Foundations - Acceptance / QA Checklist

## Acceptance Status

This checklist is proposed. It becomes the implementation acceptance checklist only after RFC-012 design review accepts or amends the scope.

## Model Checklist

- [ ] ChangeSet can represent detector source/status.
- [ ] ChangeSet can represent AgentRun association confidence or equivalent authorship caveat.
- [ ] ChangeSet summaries are bounded and content-free.
- [ ] Review state transitions are explicit and validated.
- [ ] Accepted/rejected review states do not edit files or imply tests passed.
- [ ] Artifact references do not store file contents.

## Path / Detector Checklist

- [ ] Changed paths are project-relative after root containment validation.
- [ ] Absolute inputs must canonicalize under the ProjectSession root before becoming changed paths.
- [ ] Relative inputs cannot escape through `..`.
- [ ] Symlink/link cases cannot report outside-project files as ordinary in-project files.
- [ ] Detector unavailable, partial, unsupported, and failed states are represented.
- [ ] Detector summaries do not include file contents or diff hunks.

## AgentRun Integration Checklist

- [ ] AgentRun-linked baselines can be captured before or at launch boundary.
- [ ] Detected changes can be linked to an AgentRun when association is credible.
- [ ] Ambiguous or overlapping AgentRun scenarios do not overclaim authorship.
- [ ] AgentRun `change_set_ids` and ProjectSession ChangeSet ownership stay consistent.
- [ ] AgentRun `ReviewReady` state is used only when review is actually pending.
- [ ] Terminal/runtime lifecycle remains process truth.

## Project Summary Checklist

- [ ] Unreviewed ChangeSets feed ProjectSession review-ready counts.
- [ ] Close-resource summaries count unreviewed ChangeSets without content.
- [ ] Superseded/accepted/rejected ChangeSets do not count as unreviewed.

## Evidence Required

- [ ] Design review response and any amendment.
- [ ] Implementation review responses for each PR-012 slice.
- [ ] Test command output.
- [ ] Model transition evidence.
- [ ] Path containment evidence.
- [ ] Detector unavailable/partial evidence.
- [ ] AgentRun association evidence.
- [ ] Project summary/count evidence.
- [ ] Privacy summary evidence.
- [ ] Migration note or "no migration" statement.
- [ ] Known limitations.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [ ] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Pending design review.
```
