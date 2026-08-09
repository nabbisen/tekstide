# RFC-018: Rendered Paste Protection and Trusted-UI Evidence

Status: Accepted by the human owner 2026-08-08 — ready for implementation
Target milestone: M9 (`0.5.x`), second half
Date: 2026-08-08

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md) §M9

Depends on:

- [RFC-009](../done/009-terminal-security-boundary.md) — the paste and trusted-UI policy. **This RFC renders it; it does not amend it.**
- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md) — the frozen v1 schema the `paste_blocked` producer must fit.
- [RFC-015](../done/015-application-shell-and-rendered-surface-model.md) — the modal layer and input-routing model the dialog plugs into.
- [RFC-017](../done/017-terminal-renderer-and-immersion-mode.md) — the terminal surface this protects, and the boundary this RFC is the other half of.

## Summary

Close M9. Wire RFC-009's paste policy to real clipboard input, render its confirmation decision as a real dialog, produce the `paste_blocked` audit family, and establish screenshot-backed trusted-UI evidence in the product rather than in a spike.

**This RFC is the other half of RFC-017**, and it has the same shape: RFC-009 already defines the policy, `tekstide-core` already implements it, and no production code calls it. `TerminalInputPolicy::evaluate` and `TerminalTrustedUiBoundary::assess_terminal_output` exist, are tested, and have **zero production callers** — the same condition `plain_terminal_observation` was in before PR-017-F. The work is promotion and rendering, not design.

## What this closes, and what it does not

**Closes:** RFC-009's deferral of app/UI paste-event wiring and rendered trusted dialogs; the `paste_blocked` audit producer; and RFC-017's explicitly deferred trusted-UI/spoofing evidence.

**Does not close:** `NFR-PERF-004`, which stays with readiness-driven terminal I/O ([`../future-work.md`](../future-work.md) §Readiness-driven terminal I/O). Nor the three-terminal limit, the ~374 KB/s output ceiling, or anything else downstream of the poll defect. **This RFC must not claim to improve terminal performance**; it touches the input path, not the read path.

## Non-goals

- **A general dialog framework.** See §The dialog below — this is a decision, not an omission.
- Semantic detection of dangerous pasted commands. RFC-009 explicitly excludes it, and a classifier that catches *some* dangerous pastes invites the belief that it catches all of them.
- Widening RFC-009's paste classes or trusted-UI states. Any change there is an RFC-009 amendment plus a threat-model amendment.
- Bracketed-paste mode support. RFC-009 treats paste mode as a reviewed capability; the accepted-sequence set is not widened here.
- Clipboard *writes* from terminal output. OSC 52 is already inert under RFC-009 and stays inert.

## The security core

### The policy is promoted, not rewritten

Exactly the split RFC-017 used, and for the same reason:

- **Policy stays in `tekstide-core`.** `TerminalInputPolicy::evaluate` decides `Allow` / `RequiresConfirmation` / `Block` from the paste class, the input source, the target handle, and the trusted-UI state. The shell adds no classification of its own.
- **The shell renders the decision and collects the user's answer.** It may not decide.

If rendering turns out to need a decision core cannot express, **stop and raise it** rather than adding a second classifier. RFC-017 §"Where the code lives" records why: I got this call wrong once in RFC-021, letting escaping land in `approval::coordinator`, and RFC-016 PR-016-C had to consolidate it.

### Paste must not become a second ingress

RFC-017 PR-017-B/C spent two slices proving **P1 (single ingress)** and **P2 (no side channels)** for the *output* path. This RFC adds a new *input* path, and the equivalent property must hold on that side:

**Every byte that reaches a PTY does so through exactly one call site**, and that call site is gated on modal absence. Today that is `shell::update`'s `RoutedInput::Terminal` arm, guarded by `state.modal.is_none()` and `terminal_stream_targets_a_live_terminal`. A paste path that writes bytes anywhere else is the defect this RFC exists to avoid creating.

**Enumerate `write_input`'s callers and ablate the enumeration**, the way `terminal_pane_launch_has_exactly_two_named_production_callers` does for launches.

### The confirmation dialog is itself a security surface

A paste-confirmation dialog is the first dialog in this product that a user is asked to *trust* — it says "this paste contains N lines, allow it?" and the user's answer writes bytes to a shell. Two consequences:

1. **It must be distinguishable from terminal content.** That is the spoofing evidence below, and it is not decoration: a terminal that can draw a convincing fake paste dialog can induce a user to approve bytes they never copied.
2. **The pasted content it displays is untrusted text in trusted chrome.** RFC-016's grid exception does **not** apply — the grid is the exception, the chrome is not. Any preview of pasted content goes through `text_safety::quote_untrusted`, and a paste containing `\u{202E}` or a `Cc`/`Cf` character must render escaped. **Test that specifically**; the project already has a real bidi-override fixture in its own recent-projects state that proves the render path works for project names.

## The dialog — decided here

RFC-022 (the approval dialog) does not exist as a document, and the delivery plan says *"the dialog is RFC-022's job."* This RFC needs a dialog before that one is written, so the ordering question has to be answered rather than inherited.

**Build the paste dialog directly on RFC-015's proven modal layer. Do not generalise it into a dialog framework.**

RFC-015 PR-015-C already established the substrate and proved the properties that matter: modal exclusivity is structural (`SubscriptionMode::for_modal` cannot produce terminal input while a modal is open), focus cycles correctly, and the modal composites above the content layer. `layer_composition_demo_modal` is a demo *of that layer*, not the layer itself.

**Why not generalise now:** one implementor gives nothing to generalise from. This is the same call PR-015-D made for `surface.rs` — concrete methods rather than a `trait Surface` — and the same reasoning applies. **RFC-022 is the second implementor, and that is when a shared dialog model pays for itself.** Building the framework now means designing for a caller nobody has written.

**What RFC-018 owes RFC-022 instead**: a written note in its closeout stating which parts of the paste dialog were paste-specific and which looked general, so RFC-022 starts from evidence rather than from a guess.

## The audit producer, and a frozen-schema constraint worth naming

`paste_blocked` is in the frozen v1 schema with no producer. `valid_paste_blocked` requires `action_kind == TerminalPaste`, `actor_kind == AppPolicy`, `action_source == PolicyEngine`, `reason_code == Some(PastePolicy)`, and — the constraint that matters — **`outcome == Blocked`**.

**So the schema records refusals only.** A paste the user *confirms* has no valid encoding in this family. That is a real gap in the observability story: arguably the more interesting event is a user approving a multiline paste, not the policy refusing a control-containing one.

**Do not amend the schema to fix this.** RFC-013 Amendment 1 shows what an amendment costs, and it needs the owner's authorisation. **Record the gap in the closeout** so it is a known limitation rather than an unnoticed absence, and let the owner decide whether a future amendment is warranted.

The producer goes through `AuditCoordinator`, never `AuditStore` directly, and the **sentinel test must probe raw on-disk bytes after dropping the store**, with a positive control — the shape PR-017-F arrived at only after response 152 caught that reading `database_file()` on an open WAL-mode store scans a page the write never reached. **Do not repeat that**; the fix is written and the same trap is waiting here. No pasted content, no clipboard text, no command text may reach the durable store.

## Trusted-UI evidence

This is the part RFC-017 was forbidden from claiming, and it is the reason this RFC exists as a separate document.

**RFC-014 PR-014-D's spike screenshot does not transfer.** It proved the *spike's* modal composited above the *spike's* terminal. It may not be cited as evidence for the product's boundary — a prohibition that has now held across six slices and must hold here, where it is most tempting.

**The adversarial condition is already reachable**, and better than when it was recorded. RFC-017 noted that setting `TEKSTIDE_LAYER_DEMO` and `TEKSTIDE_TERMINAL_DEMO` together produces a modal over a *live* terminal. Since the terminal-launch-UX slice, `Ctrl+Alt+T` opens a real terminal with no env var at all — so the evidence can now be taken against a terminal a user genuinely opened, with a real paste dialog over genuinely updating output. **A modal over a frozen terminal is much weaker evidence than a modal over a live one.**

Required evidence, and each screenshot states what it proves **and does not**:

1. **Genuine dialog over live terminal output**, with the terminal actively producing output while the dialog is open.
2. **Adversarial imitation**: terminal output drawing its best approximation of the paste dialog, shown alongside the genuine one, with the distinguishing features named in prose rather than left to the reader's eye.
3. **The distinguishing property stated as a claim that could be false** — "the dialog composites above the grid and the grid cannot draw outside its pane bounds" is checkable; "the dialog looks different" is not.

**`NFR-UX-002` applies**: whatever distinguishes genuine from adversarial may not be colour alone.

## Slices

**PR-018-A** — design and handoff acceptance. Nothing to implement.

**PR-018-B** — paste ingress. Clipboard read wired to `TerminalInputPolicy::evaluate`; `Allow` writes through the existing single call site; `Block` writes nothing. No dialog yet — `RequiresConfirmation` blocks conservatively until C lands, and the closeout says so. Gate: `write_input` caller enumeration ablated; modal exclusivity re-proven with a real paste.

**PR-018-C** — the confirmation dialog. `RequiresConfirmation` renders on RFC-015's modal layer; the user's answer is the only thing that releases bytes. Gate: pasted-content preview escaped through `text_safety`, with a bidi/control-character case tested specifically; focus cycle demonstrated; dismissal defaults to **not** pasting.

**PR-018-D** — the `paste_blocked` producer. Gate: conforms to the frozen family with no amendment; sentinel test probes raw on-disk bytes after dropping the store, with a positive control; the confirmed-paste recording gap stated.

**PR-018-E** — trusted-UI evidence. Gate: the three artifacts above, PR-014-D uncited.

**PR-018-F** — closeout. Gate: an explicit claim statement, and the note to RFC-022 about what looked general.

Sequencing: **B → C is strict** (a dialog with no ingress renders nothing). D needs B. E needs C. F needs all.

## Risks

- **The dialog becomes the spoofing target it exists to defeat.** Mitigated by PR-018-E proving distinguishability rather than asserting it, and by the dialog living on RFC-015's already-proven modal layer rather than a new compositing path.
- **Paste becomes a second PTY ingress.** Mitigated by enumerating and ablating `write_input`'s call sites, the way launches already are.
- **Pasted content leaks into the audit store.** Mitigated by the sentinel test — and specifically by not repeating PR-017-F's first version, which scanned a file the write had not reached.
- **`RequiresConfirmation` gets treated as `Allow`** because a dialog is inconvenient to build. Mitigated by PR-018-B blocking conservatively and saying so in its closeout, so the temporary state is visible rather than silently permissive.
- **The evidence is taken against a demo-gated terminal** out of habit. It no longer needs to be; `Ctrl+Alt+T` is real.

## Open questions

1. **What triggers a paste?** `Ctrl+Shift+V` is the terminal convention and does not collide with the existing `Ctrl+Alt+<letter>` bindings, but `KeybindingStatus` reserves `Ctrl+Shift+P` for the command palette, so check the whole table mechanically rather than by eye.
2. **Does the dialog preview the pasted content, or only describe it?** Previewing is more useful and is more untrusted text in trusted chrome. Describing it ("4 lines, 212 bytes") is safer and less helpful. **Decide in PR-018-C with the escaping already in place**, so the decision is about usefulness rather than about risk.
3. **Should a confirmed paste be audited?** Not answerable within the frozen schema. Record it; do not amend without the owner.
