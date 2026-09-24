---
title: "What a surface must not say"
rfc: "RFC-053"
rfc_file: "../../done/053-what-the-window-says-is-true.md"
source_rfc_status: "Implemented and closed 2026-09-24 — released as 0.23.0"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# What a surface must not say

**Required reading before writing code.**

## §1 It must not name our own machinery

`RFC-017`, `PR-045-B`, a module path, a type name — none of these mean anything to a user. D1's scan
makes that mechanical. **Doc comments stay legal** (D9): the scan covers catalog strings, and its
failure message must say which it covers, or the next person will delete a correct doc comment to
make a test pass.

## §2 It must not make a claim wider than the fact behind it

Change Review knows about **agent-generated change sets**. "No changes have been detected in this
project yet" is a claim about the project, and the status bar in the same frame says otherwise.
An empty state says what it is empty *of*.

## §3 It must not say "unknown" about something it knows

A freshly opened project has zero terminals, not an unknown number of them. `Unknown` is for facts
that are genuinely not known — and the moment it is used for a fact that is, the word stops carrying
information anywhere else.

## §4 It must not use one word for two facts

*Dirty files* counts unsaved editor buffers. *Changed* counts what Git reports. They sit on adjacent
surfaces in near-identical words, and no user can be expected to know which is which. Rename one.

## §5 It must not hide its own controls

A modal that clips its `Close` button off-screen has trapped a pointer user, whatever the keyboard
can still do. Content scrolls; actions stay.

## §6 It must not state an invariant it does not enforce

`status_bar`'s doc comment says a second line "would silently shrink every PTY". At 520px it wraps to
three. **Either the layout enforces one line, or the layout measures the real height** — the RFC
chooses measuring, because `REQ-NOTIFY-002` names five fields and eliding one to protect a constant
is how a surface starts lying again.

## §7 It must not lose the checks it already passes

The colour-alone scan and the i18n completeness scan both pass today. Changing words and layout must
leave them passing, and the new scan joins them rather than replacing anything.
