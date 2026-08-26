---
title: "RFC-034 acceptance and QA checklist"
rfc: "RFC-034"
rfc_file: "../../accepted/034-change-review-actions-and-review-state.md"
source_rfc_status: "Accepted 2026-08-18, amended 2026-08-26 — M12, second of three for 0.15.0"
target_milestone: "M12"
created: "2026-08-26"
---

# Acceptance and QA checklist

## The claim the slice exists to be able to make

- [ ] **`rejecting_a_change_set_does_not_modify_any_file`** exists and passes, against real files
      on disk, real bytes compared before and after, through the real message path.

## D1 — opinions only

- [ ] `Accepted` and `Rejected` are offered from `Unreviewed` and `PartiallyAccepted`.
- [ ] `PartiallyAccepted` and `Superseded` are **never** offered from any reachable state.
- [ ] Ablated: offer `Superseded`, the test fails, restored.

## D4 — final, and said before the click

- [ ] The finality claim renders **while the controls are live**, not after, and not only in a
      modal.
- [ ] After a decision the controls are withdrawn and the state line carries what was decided.
- [ ] Ablated: keep the controls after a decision, the test fails, restored.

## D0 — session-scoped, and said

- [ ] The session-scope claim renders on the surface.
- [ ] It is held by a test. **Ablated: remove the sentence, a test fails.** If nothing fails, the
      words are unguarded decoration.
- [ ] The finality and session-scope claims are visible **at the same time** in the live
      screenshot.

## D2 — no audit record, and no silence mistaken for absence

- [ ] No audit record is written for a review decision.
- [ ] The closeout states this plainly, in `CHANGELOG.md` and the RFC's own closeout.

## D3 — a stale tree is disclosed, not blocking

- [ ] `diff_content_is_stale` reused; no second staleness notion invented.
- [ ] The notice is its own sentence, distinct in wording from `change-review-content-stale`.
- [ ] With a stale tree, the controls remain live and a decision still records.

## §4 — disclosure density, the design work

- [ ] The decision about how these claims reach a reader is **written down in `qa-evidence.md`**,
      not implied by the layout.
- [ ] A screenshot shows the claims readable at a glance, not a stack of caveats.
- [ ] Any existing `change-review-*` string removed or reworded is named, with its reason.

## Layout, measured not argued

- [ ] The controls' home is decided, with the reason, from the pack README's three options.
- [ ] The effect on `pinned_middle`'s height is **measured** by extending RFC-042's headless
      layout test.
- [ ] If the deferred file-row-collapse item was picked up, say so; if not, say that too.

## Modal exclusivity

- [ ] Both new handlers are guarded, in the shape the existing seventeen use.
- [ ] Both are inert while a modal is open, each proven by its own test.
- [ ] Ablated: drop one handler's guard, that handler's test fails, restored.

## Live GUI evidence

- [ ] Captured against a **`mktemp -d` fixture project** — no path under `$HOME`, no real project
      name, no real file content.
- [ ] Whether a real mouse click was sent is **stated either way**.
- [ ] Shows: controls live with both claims visible, then the surface after a decision.

## Gates

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] Full workspace suite, **three consecutive runs**, each **logged to a file**, any flake named
      against `test-process-leak.md` with its assertion message captured.
- [ ] `git diff --check`, `rfc_docs_invariants`.

## Closeout

- [ ] `CHANGELOG.md` describes this at its real size — a session-scoped note that changes no file
      and cannot be taken back. Not "review workflow".
- [ ] The successor question is restated where it will be found: *should the audit store record a
      user's decision about generated code?*
- [ ] README's change-review paragraph no longer describes a read-only surface.
- [ ] `ARCHITECTURE.md` gains *a control may record an opinion; it may not assert a fact*, if it
      has earned it.

## The §6 outcome is an acceptable one

- [ ] If PR-034-A's evidence makes the case that this is not worth shipping as scoped, that is
      reported **instead of** PR-034-B, in writing, with the reason. A held slice is a good
      outcome; a shipped control nobody should trust is not.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
