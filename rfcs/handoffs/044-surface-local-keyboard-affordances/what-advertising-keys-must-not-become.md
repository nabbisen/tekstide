---
title: "What advertising keys must not become"
rfc: "RFC-044"
rfc_file: "../../accepted/044-surface-local-keyboard-affordances.md"
source_rfc_status: "Accepted 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-27"
---

# What advertising keys must not become

**Required reading before writing code.** Nothing in this slice is dangerous the way RFC-043's was
— no processes are killed and no descriptors are exposed. The failure mode here is subtler and
this project has already lost ground to it once: **making a surface worse while technically fixing
it.**

## §1 A surface where everything is annotated is a surface nobody reads

RFC-034 had to answer this for disclosures. `en.ftl` carries **28** `change-review-*` strings —
detection disclosure, detection status, two distinct omission counts, review state, six content
refusals — each one correct, each one added because a reviewer proved the surface would otherwise
overclaim. The answer there was to **consolidate into one sentence attached to the control it
qualifies**, not to add three more lines.

The equivalent mistake here is turning every label into `Mark accepted (a)`, `Open (Enter)`,
`Purge… (Delete)`. Each is individually defensible. Together they are the same wall of text,
rebuilt out of parentheses.

**D2 decided against it.** Help, grouped by surface, is the route. If you find yourself arguing for
label hints on one particular surface because it is "the important one," that is the argument that
produced 28 strings, one at a time.

## §2 Being technically honest is not the same as being honest

A key that appears in a generated list nobody opens is advertised in the sense that a compliance
checkbox is ticked. The property that matters is whether **a keyboard user, on first contact with a
surface, can find out what keys it has.**

That is why D2 says Help and `--help` rather than "somewhere discoverable": those two have known,
already-advertised entry points (`Ctrl+Alt+K`, the `?` button, the CLI). Anything you add must be
reachable from a route a user already knows about, or it is a file nobody opens.

## §3 The access half is not the same problem, and it comes first

Two defects wear the same clothes:

- **Discoverability** — the key exists, nothing names it. Annoying; the user can guess or read
  Help.
- **Access** — no key exists. The user **cannot act**.

Closing a project is the second kind, and it was found by accident during a release gate.
**Do not let the advertising work absorb it.** A slice that ships a beautiful surface-grouped Help
section while a keyboard user still cannot close a project has fixed the comfortable half.

## §4 A registry that inherits the old domain inherits the old blind spot

`control_coverage` is exhaustive and has been since RFC-040. It still missed a mouse-only control,
for one reason: **its domain is `NavigationAction`, and closing a project is not one.**

An exhaustive match is only as good as the set it is exhaustive *over*. If the new registry is
keyed on `NavigationAction`, or on "the keys the eight handlers currently match," it will be
exhaustive over exactly the things that were already visible, and the next mouse-only control will
be found the same way the last four were — by someone noticing.

**The inventory slice exists to make that impossible.** It should end red, with a real count.

## §5 What you may not do

- **Do not put bare keys in `KeybindingPolicy`.** `matching_global_action` would turn `Enter` into
  a global action and shadow every surface handler. The two registries stay separate, and the
  reason belongs at the type.
- **Do not add a `MouseOnly` arm without requiring a reason.** `KeyboardOnly` carries one
  (`PasteIntoTerminal`'s terminal convention, stated and permanent). Its mirror must too, or it
  becomes a bin for gaps nobody had to justify.
- **Do not enforce with a source scan.** RFC-042's first guard was one; the reviewer defeated it by
  respelling the same construct, with every assertion still green.
- **Do not answer §2 with "the keys are documented."** So were the fourteen global bindings, and
  that is the system that already worked. This RFC exists because the other twenty-nine were not.

## §6 If the honest answer is that some control should stay mouse-only

Possible, and legitimate. A drag target, or something where a key would collide with a surface's
primary interaction, may genuinely have no good binding.

**Say so in a `MouseOnly { reason }` entry and let it be counted.** That is the difference between
a decision and a gap: `PasteIntoTerminal` is keyboard-only *on purpose*, with the reasoning at the
entry, and nobody has had to rediscover it.

What is not acceptable is a control that is mouse-only because nobody asked.
