---
title: "What a clickable modal must not become"
rfc: "RFC-040"
rfc_file: "../../done/040-affordance-completion.md"
source_rfc_status: "Implemented and closed 2026-08-25 — RFC-040 is in rfcs/done/"
status: "Required reading before adding a button to any modal"
created: "2026-08-25"
---

# What a clickable modal must not become

Nine modals gain buttons in this RFC. Two of them decide destructive things — purging a
project's transcripts, closing a project and killing its processes — and all of them are
**trusted chrome**, the surface RFC-018 exists to keep unspoofable. Adding a click target to
that is not a cosmetic change.

## 1. A button must not weaken keystroke suppression

This project's modal safety does **not** come from focus trapping. It comes from
`SubscriptionMode::for_modal` plus the `is_none()` guard at the write site: while a modal is
open, keystrokes cannot reach a terminal at all. `shell.rs`'s own doc calls the scrim "additive
cosmetics on top of it, never a second one."

`opaque(center(...))` already captures clicks full-window, so the layer beneath is unreachable
by mouse — verified during PR-038-G's review. **Do not add a second interaction-capturing layer,
and do not reach for `mouse_area`**, which does not exist anywhere in this crate today. Whatever
you add lives inside the existing modal element.

**Required:** a test that a control behind an open modal cannot be activated — by click. The
keyboard half is proven; the mouse half never has been, because until PR-038-G there was nothing
to click.

## 2. Default focus must not move to the destructive choice

`ProjectCloseModal` and `TranscriptPurgeModal` default to the safe option — Cancel — and that is
deliberate. A button changes what a stray `Enter` or a mis-aimed click does.

**Required:** the destructive button is never the default focus, is never the first thing a
`Tab` reaches, and adding it does not change which option a bare `Enter` selects. Ablate that:
open the modal, press `Enter`, assert nothing was destroyed.

## 3. A click must be the same decision as the keystroke, recorded the same way

`ProjectCloseModal` records `safe_close_decision` for **both** outcomes, `Cancelled` included —
uniquely in this crate, Escape is a real decision there. A button must route to the same handler,
not a parallel one.

**Required:** clicking Cancel and pressing Escape produce the same audit record. Two paths to one
decision is how the two halves drift, and a drifted audit trail is worse than none because it
reads as authoritative.

## 4. Labels are trusted chrome; the values in them may not be

A close confirmation names a project by **canonical path**, escaped and bounded — that is
RFC-039 D3, and it exists because a misleading label on a destructive control is a wrong action,
not merely a wrong belief. Putting that text on a *button* does not change the requirement.

**Required:** no untrusted value reaches a button label unescaped. The type system already
enforces this — `CatalogArgs::untrusted` takes `DisplayText`, whose only constructor is
`quote_untrusted` — so the way to get this wrong is to build a label by string concatenation
outside the catalog. Do not.

## 5. What this document does not cover

Whether a given modal *should* be dismissible by clicking the scrim. It is a real question —
click-away is conventional — and it is **out of scope here**: for a destructive modal, an
accidental click outside is exactly the input that should not decide anything. If you want it,
raise it as a finding rather than adding it.
