---
title: "PR-018-G — Background scrim behind the paste-confirmation dialog: implementation handoff"
rfc: "RFC-018"
rfc_file: "../../done/018-paste-protection-and-trusted-ui-evidence.md"
status: "Accepted by the owner 2026-08-11 — ready for implementation"
created: "2026-08-11"
---

# A tell that does not depend on what the attacker pastes

## Why this slice exists

RFC-018 shipped a paste-confirmation dialog whose claimed distinguishing property was
**spatial**: a real dialog occludes trusted chrome, and terminal output cannot draw
outside its own pane, so a user could tell them apart by where the dialog sat.

PR-018-E measured that claim and it did not survive. The dialog's size is driven by the
**pasted content**, and an attacker who keeps a paste short keeps the dialog entirely
inside the terminal's own pane, where imitation is possible (response 175). I got this
wrong twice, in opposite directions, before measuring it properly — first from the
layer-demo modal, then from a capture at a different scale. The honest statement, now in
RFC-018's own text, is that the spatial property is **content-dependent and
attacker-influenceable**, and the load-bearing defence is keystroke suppression, not
geometry.

PR-018-E named a background scrim as the fix for that specific weakness and RFC-018's
task breakdown said twice that PR-018-F should decide whether to recommend it. **PR-018-F
never mentioned it** — found by grep after `0.6.0` shipped. This is that decision,
finally made, and scoped.

## The argument for the scrim, stated precisely — this is what the evidence must prove

A scrim is worth building for exactly one reason, and it is not "it looks modal":

**The scrim's dimensions are fixed by the window, not by the pasted content.** The
spatial tell failed because the attacker controlled the variable it depended on. The
attacker does not control the window size, so the same tell holds for a one-byte paste
and a one-megabyte paste alike.

That argument only works if the scrim covers area **the terminal pane cannot draw into**.
A terminal pane can render a dark rectangle inside itself; what it cannot do is dim the
session bar, the project chrome, or the window margin. So:

- **The scrim must cover the full window, chrome included**, not the content region and
  not the pane. A scrim that stops at the pane boundary reproduces the exact weakness it
  exists to fix, and would be worse than none — it would look like evidence while proving
  nothing.
- State this in the evidence as the property being demonstrated. "The screen dims" is not
  the claim. "Chrome outside the terminal pane dims, and terminal output cannot cause
  that" is.

## Scope

Add a full-window dimming layer beneath the paste-confirmation modal, present exactly
while that modal is open.

**Reuse the existing modal layer.** RFC-018 already has one, generic over content type.
The scrim belongs to it, so that any future modal gets the same treatment without a second
mechanism — the same reasoning that put `SubscriptionMode::for_modal` in one place.

## What must not change, and this is the important half

`SubscriptionMode::for_modal` plus the `is_none()` guard at the write site is what
actually protects the user. The scrim is **additive cosmetics on top of a real defence**
and must not become load-bearing or weaken what is:

- **The scrim must not consume input events.** Not to dismiss the dialog, not to swallow
  clicks, not anything. Input suppression while a modal is open has exactly one mechanism
  and a second one that happened to also work would be indistinguishable from the first
  until the day they disagreed. If your GUI toolkit's overlay primitive captures events
  by default, say so and show what you did about it.
- **A click on the scrim does not dismiss the dialog.** A paste confirmation is an
  explicit decision; click-outside-to-cancel makes a stray click into an answer.
- **PR-018-E's keystroke-suppression positive control must still pass**, unchanged and
  re-run — not assumed to still hold because the change is "only visual."
- **No escaping change.** The escaping asymmetry (three positions) is settled; a scrim
  touches none of it.

## Evidence owed

- **The content-independence property, demonstrated across the range that broke the old
  claim.** At minimum the short paste from response 175 — the one that previously kept the
  dialog entirely inside the terminal pane — with the scrim present, showing chrome
  outside the pane dimmed. One favourable capture is what produced two wrong claims
  already; capture both ends.
- **An ablation.** Remove the scrim from the view path and watch a *specific* named test
  fail. A green ablation is a defect in the ablation, not a pass — this project has hit
  that at least six times.
- **A test that the scrim is present whenever the paste modal is open**, at the layer
  where the two are bound together, so a future modal added without a scrim fails by name
  rather than by someone noticing a screenshot.
- **The suppression positive control, re-run**, with its result stated.
- Screenshots via `niri msg action screenshot-window`; synthetic input with
  `env -u WAYLAND_DISPLAY`, `xdotool windowfocus`, and always `--clearmodifiers`.

## Honesty checklist

- **No claim that the scrim makes the dialog unspoofable.** It raises the cost of a
  convincing imitation; it does not eliminate one. The defence that holds is keystroke
  suppression, and that ordering must survive into whatever this slice writes.
- **No claim that the spatial property is now sound.** It is not; it was replaced, not
  repaired. RFC-018's disclosed limitation stands as written.
- If the scrim turns out to be unimplementable without capturing input in the toolkit,
  **stop and raise it** rather than shipping a version that captures. Losing the scrim is
  cheap; a second input-suppression path is not.

## Review gate

- Full-window coverage including chrome, shown — not the content region.
- Content-independence demonstrated at both ends of the range.
- Ablation performed, with the exact failing test named.
- Scrim consumes no input; click does not dismiss; both shown rather than asserted.
- PR-018-E's positive control re-run and passing.
- `future-work.md`'s scrim entry updated to record the decision and its outcome, in the
  same commit — the entire reason this slice exists is that a recommendation sat
  unactioned for a month because no document owned it.
