---
title: "RFC-044 acceptance and QA checklist"
rfc: "RFC-044"
rfc_file: "../../accepted/044-surface-local-keyboard-affordances.md"
source_rfc_status: "Accepted 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-27"
---

# Acceptance and QA checklist

## The claim this RFC exists to be able to make

- [ ] **A keyboard-only user can close a project**, proven live, not inferred — the defect that
      widened this RFC's scope.

## D1 — the registry, over the right domain

- [ ] Entries are keyed by **(surface, key)**, so `Enter` on six surfaces is six entries.
- [ ] The domain is **surface actions**, not `NavigationAction`. **Closing a project is in it** —
      if it is not, the registry inherited the blind spot this RFC was accepted to remove.
- [ ] Surface-local keys are **not** in `KeybindingPolicy`, and the reason is recorded at the type.

## D3 — the mirror, exhaustive

- [ ] `ControlCoverage` has a `MouseOnly { reason }` arm.
- [ ] A `MouseOnly` entry **requires** a reason, as `KeyboardOnly` already does.
- [ ] Adding a surface action without deciding its keyboard route **fails to compile**.
- [ ] Ablated: add one without a route, record the compile error.
- [ ] **Not enforced by a source scan.** If dispatch-through-registry proved impractical, that is
      written down with its reason, and this box is unchecked rather than quietly satisfied.

## D4 / PR-044-A — the inventory

- [ ] The inventory exists: every surface action, its keyboard route or its absence.
- [ ] **The count of `MouseOnly` entries is stated.** Nobody has this number today.
- [ ] Whether the gaps were encoded as failures is stated either way, with the reason.

## PR-044-B — access

- [ ] Every `MouseOnly` entry is closed or is a stated, permanent exception.
- [ ] Keys were confirmed unclaimed on their surface **before use**.
- [ ] Independent actions use fixed keys, not a shared cursor.
- [ ] Each closed gap has a test through the real message path.

## D2 / PR-044-C — advertising

- [ ] Help gains a surface-grouped section; `--help` gains the same grouping.
- [ ] Both are **generated from the registry**, not hand-written.
- [ ] Ablated: remove a registry entry, watch it disappear from the generated help.
- [ ] **No key hints added to control labels.**

## Live GUI evidence

- [ ] Against a **`mktemp -d` fixture project with a fresh `XDG_STATE_HOME`**. No path under
      `$HOME`, no real project name, no other project on screen.
- [ ] Shows a project being **closed entirely by keyboard**.
- [ ] Whether a real mouse click was sent is stated either way.

## Gates

- [ ] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`.
- [ ] Full workspace suite, **three consecutive runs**, each logged to a file; any flake named
      against the register **with a row**, not only mentioned.

## Closeout

- [ ] `README`'s keyboard table reflects whatever D2 produced.
- [ ] Any statement this slice makes false is corrected — `0.15.0`'s changelog lists both
      "closing a project is mouse-only" and "surface-local keys are not advertised" as known
      limitations.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
