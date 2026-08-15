# RFC-017: Terminal Renderer and Immersion Mode

Status: Implemented with documented limitations — closed 2026-08-04 by PR-017-H, with the owner accepting `NFR-PERF-004` as **not met**. A real, filtered PTY-backed terminal renders as a surface with P1-P4 re-proven against product code and independently ablated; input reaches it with modal exclusivity and global-keybinding precedence demonstrated under a live terminal; `plain_terminal_observation` has its first producer with a sentinel-proven privacy boundary. **Not met:** `NFR-PERF-004` (terminal input latency p95 ≤ 16 ms) — the 50 ms poll tick puts the floor near 47.5 ms p95; the fix is readiness-driven terminal I/O, scheduled as follow-up (`../future-work.md` §Terminal / PTY Runtime). **Not claimed:** trusted-UI separation or spoofing resistance — that is RFC-018, and RFC-014 PR-014-D's spike screenshot does not transfer. See `../handoffs/017-terminal-renderer-and-immersion-mode/qa-evidence.md` §PR-017-H.
Target milestone: M9 (`0.5.x`)
Date: 2026-08-01

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md) §M9

Depends on:

- [RFC-008](../done/008-terminalsession-process-lifecycle.md) — the PTY lifecycle this renders.
- [RFC-009](../done/009-terminal-security-boundary.md) — the accepted-sequence policy. **This RFC renders it; it does not amend it.**
- [RFC-014](./014-desktop-gui-substrate-and-terminal-rendering.md) — the substrate decision and Option A.
- [RFC-015](./015-application-shell-and-rendered-surface-model.md) — the surface contract and input-routing model this surface plugs into.
- [RFC-016](../done/016-internationalization-and-localization.md) — text safety, and the terminal's deliberate exception to it.

## Summary

Turn the reviewed terminal security boundary into a rendered surface: a PTY-backed terminal pane, Terminal/Agent Immersion Mode with at most two visible panes, a split policy driven by real font metrics, a session bar, and hidden-session handling.

**This is the first surface where untrusted bytes reach a renderer.** Everything before it rendered Tekstide's own state. RFC-009 defined what a terminal may do and `tekstide-core` classifies it; RFC-014's spike proved an interposition strategy works. This RFC promotes that from a spike to product code and puts a real emulator behind a real security boundary.

## What this closes, and what it does not

**Closes:** RFC-009's deferral of a rendered surface; the `plain_terminal_observation` audit producer.

**Corrected 2026-08-04 at closeout.** This line originally also claimed to close RFC-014's C3 latency criterion (`NFR-PERF-004`) "at product scale rather than spike scale." **It does not.** PR-017-G recorded `NFR-PERF-004` as **not met** — see §Performance below and `rfcs/handoffs/017-.../qa-evidence.md`. The criterion now has its first real verdict, which is a different and lesser thing than being closed.

**Does not close, and must not claim to:** rendered paste protection and its confirmation dialog, the `paste_blocked` producer, and screenshot-backed spoofing evidence under adversarial output. **Those are RFC-018.** Nor `NFR-PERF-004`, per the correction above — closing it requires readiness-driven terminal I/O (Option B), which is scoped follow-up work, not this RFC's. ROADMAP M9 lists them under the same milestone; the delivery plan splits them across two RFCs, and this document is the first half.

**A configuration RFC-018 will want, found during PR-017-D's review (2026-08-03).** `terminal_demo_subscription()` sits outside the modal gate, so **terminal output keeps rendering while a modal is open** — correct behaviour, since a real terminal does not stop producing output because a dialog appeared. Setting `TEKSTIDE_LAYER_DEMO` and `TEKSTIDE_TERMINAL_DEMO` together is reachable (they are constructed independently at boot) and produces exactly the adversarial condition RFC-018 must prove against: a trusted dialog distinguishable from terminal content *while that content is actively updating*. A modal over a frozen terminal is much weaker evidence than a modal over a live one. PR-017-D correctly declined to manufacture that screenshot for its own scope; RFC-018 should seek it deliberately.

That split matters for one reason. RFC-014 PR-014-D already produced a genuine-versus-adversarial screenshot **in the spike**, and it is the strongest artifact this project has on trusted-UI separation. It does not transfer: it proved the spike's modal composited above the spike's terminal. RFC-018 must re-establish it in the product, and **RFC-017 must not cite PR-014-D's image as evidence for the product's boundary.**

## Non-goals

- Terminal-grid bidi reordering. Real terminals do not implement it, and reordering breaks cursor and column arithmetic. RFC-016 already records this as out of scope by design.
- Widening RFC-009's accepted subset. Any sequence family not already accepted stays inert. Widening requires an RFC-009 amendment and a threat-model amendment, not a renderer convenience.
- Search, hyperlink activation, image protocols, or shell integration.
- Multiplexing beyond the existing `visible_terminal_limit` model.

## The security core

### The filter is promoted, not rewritten

RFC-014 decided **Option A**: `alacritty_terminal` as the emulator, with RFC-009's policy interposed as a `vte::ansi::Handler` wrapper in front of it. The spike's `SecurityFilter` proved four properties, and they are the acceptance criteria here, restated for product code:

| | Property | What it means in the product |
| --- | --- | --- |
| **P1** | Single ingress | Every byte from the PTY reaches the emulator through exactly one filter entry point. No second path exists — not for replay, not for tests, not for scrollback restore. |
| **P2** | No side channels | The emulator exposes no state-mutation API reachable outside that byte path. If one exists, it is wrapped or unreachable. |
| **P3** | Classification parity | The filter's notion of where a sequence begins and ends is identical to the emulator's. |
| **P4** | Stream-position independence | Classification does not change with how bytes are chunked across reads. |

**P4 is the one that decays silently.** A PTY delivers arbitrary chunk boundaries; a filter that classifies correctly on whole sequences and incorrectly on split ones is a filter that fails only under load. Test it with adversarially split input, not just with byte-at-a-time and whole-buffer.

### Where the code lives — decided here, not left to the implementer

`tekstide-core::runtime::terminal::security` **already holds RFC-009's classification** — `TerminalSequenceFamily`, `TerminalPolicyReason`, the effect enums — and `tekstide-core` has **no `vte` or `alacritty_terminal` dependency** today. That is worth preserving.

The split is therefore:

- **Policy stays in `tekstide-core`.** Which families are accepted, which are inert, and why, is already there and does not move. The renderer adds no classification of its own.
- **Interposition lives in the shell crate**, because implementing `vte::ansi::Handler` requires `vte`. It is a thin adapter that routes every emulator callback into core's existing classifier and holds **no policy of its own**.

The adapter having no policy is the point, and it is P3 made structural rather than tested: if the shell-side filter cannot decide anything, it cannot drift from core's decisions.

**I am stating this because I got the analogous call wrong once.** In RFC-021 I let escaping be implemented inside `approval::coordinator` because the surface needed it and the shared primitive did not exist yet; RFC-016 PR-016-C then had to consolidate it. The lesson is not "never put security code near a surface" — it is that **the decision must live in one place and the surface must call it.** A filter in the shell crate that classifies is a repeat of that error. A filter in the shell crate that delegates is not.

If interposition turns out to need policy-shaped decisions core cannot express, **stop and raise it** rather than adding a second classifier.

### The terminal's exception to RFC-016

RFC-016 requires untrusted text to be escaped and isolated at render. **The terminal grid is the deliberate exception**, and it is already recorded as such: escaping terminal output would corrupt it, and bidi reordering breaks column arithmetic.

Two boundaries follow, and both are this RFC's to hold:

1. **The exception is the grid, not the chrome.** A session bar showing a session title derived from terminal output, a pane header showing a working directory, a tooltip showing a command — all of that is untrusted text in trusted chrome and goes through `text_safety`. The exception must be *narrow and stated*, not ambient.
2. **The grid renders as data, never as chrome.** Nothing drawn from PTY bytes may occupy, overlap, or imitate a region the user reads as Tekstide's own. RFC-018 proves this adversarially; RFC-017 must not build a layout that makes the proof impossible.

## The rendered surface

The terminal pane is a surface under RFC-015's contract, and inherits it without exception:

- It holds **no state duplicating `tekstide-core`**. The emulator owns grid state; `TerminalSession` state stays in core.
- It **cannot render trusted chrome, and cannot reach modal state**. It emits messages the shell interprets.
- Untrusted spans outside the grid go through `tekstide_core::text_safety`.

### Input: `TextStream` gets its first real producer

RFC-015 PR-015-C built three input classes and proved `TextStream` is constructible only inside `input::terminal_surface`, with a `pub(super)` constructor deliberately narrower than `pub(crate)` so a future surface module could not synthesize one. **That future surface is this one.**

It was built and tested headless, with `terminal_focus` hard-coded `None`. This RFC supplies the real value. Three obligations:

1. **`TextStream` still cannot address shell or modal state.** The type prevents it; keep it that way when the routing gains a real terminal id.
2. **Modal exclusivity still holds.** RFC-015's property is that while a modal is open `SurfaceInput` and `TextStream` are *not produced* — not produced-and-discarded. A terminal that keeps writing to a PTY while an approval dialog is open defeats the dialog. The `ModalAbsent` gate already enforces this at the call site; verify it under a real terminal rather than assuming the headless proof transfers.
3. **Global keybindings still win.** RFC-015 checks `KeybindingPolicy` before terminal focus precisely so a terminal cannot capture them. A terminal that swallows the shell's own navigation is a terminal the user cannot escape.

One question RFC-015 deliberately left open and this RFC must answer: **should Tab reach the terminal?** RFC-015 routes Tab to the shell focus cycle, ahead of terminal focus, and recorded that the tradeoff could not be judged without a terminal to judge it against. Now there is one. Shell completion makes Tab-to-terminal genuinely useful; an inescapable focus trap makes it dangerous. Decide it, state the escape hatch, and test it.

## Immersion mode, split policy, and the session bar

**At most two visible panes.** `visible_terminal_limit` already defaults to `2` in `ProjectResourceLimits`, and `TerminalPanePolicy::for_layout` and `TerminalLayoutClass` already exist in `tekstide-core::navigation`. **Use them.** This RFC adds no parallel layout model — that is RFC-015's "no shell-local shadow copy" rule applied to the terminal.

**Split policy is driven by real font metrics and DPI, not by fractions.** A split that produces panes too narrow for a minimum column count is not a split; it is a rendering bug that shows up on someone else's display scaling. The spike's `font_metrics.rs` established the approach.

**The session bar labels state without relying on colour** (`NFR-UX-002`). Hidden sessions remain addressable and their state visible — a session that is producing output, has exited, or is blocked must be distinguishable while hidden.

**Scrollback is bounded**, consistent with RFC-008's bounded-IO discipline. An unbounded buffer is a memory-exhaustion path driven by untrusted output.

## Performance

`NFR-PERF-004`: terminal input latency p95 ≤ 16 ms **with bounded background output**.

**Correction, 2026-08-04 — the original sentence here was wrong when written.** It read "RFC-014's C3 measured this in the spike; this RFC re-establishes it in the product." RFC-014 records C3 as **"Not verified — see R1"** ([`014`](../done/014-desktop-gui-substrate-and-terminal-rendering.md), acceptance table), and R1's discharge assigns it to this RFC. The spike never measured it. That error was the architect's, made at authoring time and repeated unchecked until PR-017-G; had anyone relied on it, this RFC would have been framed as re-establishing a known-good figure rather than producing the criterion's first verdict — which would have made a not-met result read as a regression instead of as new information.

**Outcome, PR-017-G: not met.** `terminal_demo_subscription`'s 50 ms poll tick is the only path by which PTY bytes reach the grid, so poll-wait alone contributes an expected p95 near 47.5 ms — roughly 3× the budget, before any PTY, VTE, layout or paint cost. Arithmetic, independent of any live run. Corroborated headlessly: `poll()` costs ~10.3 ms against the 50 ms period (21% duty), dominated by a hardcoded 10 ms `WouldBlock` sleep in `read_available_bounded_for` that overshoots its own 5 ms budget.

**Correction, 2026-08-08 — this paragraph originally added "so the update loop does **not** saturate," without qualification.** That figure was measured with **one** pane, and the tick handler polls every live pane *sequentially*, so the cost scales linearly: measured at 10.1 ms per pane, saturation begins at **five**. It was unreachable while only the three-pane demo existed and became reachable when the terminal-launch-UX slice gave users a keybinding. The unqualified sentence was the architect's. The per-pane scaling is recorded with its fix in [`../future-work.md`](../future-work.md) §Readiness-driven terminal I/O, and the session limit is now chosen against the tick budget rather than against process count alone.

Two consequences recorded rather than fixed here: terminal output throughput is capped near **374 KB/s** by that same sleep, and the sleep is currently **masking** a stream-truncation risk — `poll()`'s 64 KiB cap truncates mid-read and discards the remainder, and a faster reader would begin exceeding it. **The sleep fix and the cap policy are one change, not two.**

Reuse RFC-015 PR-015-F's harness and its lessons:

- **Do not reintroduce `iced::window::frames()`** for latency. It forces continuous redraw and produced RFC-014's degenerate all-`0µs` figures. The input-to-state-change and view-cost decomposition is `frames()`-free.
- **Prove non-contamination for the new criterion**, not by inheritance.
- **Report delivery-loss rate**, and stop on confirmed on-disk sample counts rather than dispatched ones (R9).
- **The flood condition is the point.** Latency under an idle terminal measures nothing interesting; the requirement is explicitly *under output flood*, which is also the condition where P4 failures surface.

## Audit

**`plain_terminal_observation`** is wired by this RFC — the family exists in the frozen v1 schema and has no producer. It records observations about a plain (non-managed) terminal session; it must carry no command text, no output, and no path, consistent with RFC-013's sentinel-privacy rule. A sentinel test asserting no terminal-derived text reaches the durable store is required, matching the one RFC-021 PR-021-E2 used.

`paste_blocked` belongs to RFC-018.

## Risks

- **The filter is bypassable in a way the spike did not surface.** The spike drove a demo script; a real shell session drives far stranger byte streams. Mitigation: P1-P4 re-proven against product code with adversarially chunked input, and the filter treated as a reviewed security boundary rather than promoted plumbing.
- **The grid becomes a spoofing surface.** Mitigation: RFC-018 proves separation adversarially; this RFC must not produce a layout that makes the proof impossible — no chrome-adjacent grid regions, no terminal-controlled text in shell chrome.
- **Latency measured only when idle.** Mitigation: the flood condition is a stated gate, not an option.
- **The pane duplicates core state for rendering convenience.** Mitigation: RFC-015's contract, enforced the same way — if a rendering need appears to require new core state, raise it rather than shadowing it.
- **Scrollback growth under hostile output.** Mitigation: bounded, with the bound stated and tested.
- **Tab routing traps the user.** Mitigation: whichever way the decision goes, an escape hatch that does not depend on the terminal cooperating.

## Implementation plan

1. **PR-017-A** — design and handoff acceptance.
2. **PR-017-B** — filter promotion: `SecurityFilter` from spike to product, delegating to core's classifier, with P1-P4 re-proven including adversarial chunking. **The security-critical slice.**
3. **PR-017-C** — terminal pane rendering the emulator grid, under RFC-015's surface contract; no input yet.
4. **PR-017-D** — input: `TextStream` with a real `TerminalId`, modal exclusivity and global-keybinding precedence verified under a real terminal, and the Tab decision made and tested.
5. **PR-017-E** — immersion mode, split policy from real font metrics, session bar, hidden sessions.
6. **PR-017-F** — `plain_terminal_observation` audit producer with its sentinel test.
7. **PR-017-G** — measurement: `NFR-PERF-004` under flood.
8. **PR-017-H** — closeout evidence.

**B → C is strict.** Nothing renders emulator output before the filter is proven in product code. D needs C. E needs C. G needs D and E.

If P1-P4 cannot be re-established in product code, **stop and escalate** rather than proceeding to C. RFC-014 named Option B (own the parser) as the fallback if the filter proves leaky; that fallback is still live and choosing it is not a failure.

## Test and evidence requirements

- **P1-P4 re-proven** against product code, each independently ablated.
- **Adversarial chunking corpus**: every accepted and inert sequence family split at every byte boundary, asserting classification is identical to the whole-sequence case.
- **Inert-family corpus**: each family RFC-009 marks unsupported produces no observable grid effect.
- **A real PTY**, not a synthetic byte source, for at least the round-trip and flood tests.
- **Modal exclusivity under a live terminal**: a dialog open means no PTY write, demonstrated rather than argued.
- **`NFR-PERF-004` under flood**, non-degenerate, with delivery loss reported. **Met as a gate, failed as a budget**: measured non-degenerately (not the all-`0µs` outcome this gate existed to prevent) and delivery loss reported at 0%, but the criterion itself is **not met** — see §Performance.
- **Sentinel privacy** for `plain_terminal_observation`.
- **Bounded scrollback** under sustained output.
- Screenshots per response 127's convention: `--id` and `--path`, stored under `evidence/pr-017-*/`, committed, each with an explicit statement of what it does **and does not** prove.

## Open questions

1. **Does Tab reach the terminal?** Decided in PR-017-D, with the escape hatch stated.
2. **Where does the emulator's grid state live when a session is hidden** — retained in memory, or torn down and rebuilt from scrollback? The first costs memory per hidden session; the second loses state and changes what "hidden" means. Decide in PR-017-E against the bounded-scrollback decision.
3. **Does the filter belong in a separate crate** (`tekstide-terminal`) rather than the shell crate? Only if something needs the terminal without the GUI. Nothing does today; revisit if RFC-024 or a headless test harness changes that.

## Amendment 1: Readiness-driven terminal I/O

**Status:** Authored by the architect 2026-08-15. **Requires the owner's authorisation** —
it is not additive. It removes the poll tick this RFC established, changes the shape of the
one ingress path PR-017-B/C's P1 and P2 were enumerated and ablated against, and changes
observable behaviour under load. Under the standing delegation, that comes to the owner.

**Amendment type:** Structural. Touches RFC-008's `read_available_bounded_for` as well as
this RFC's subscription.

**Why an amendment and not an RFC.** The properties at stake (P1-P4) are this RFC's, the
poll tick is this RFC's, and the change replaces a mechanism rather than adding a
capability. A new RFC would re-derive this RFC's own constraints to reach the same place.

### The problem, measured

`NFR-PERF-004` (terminal input latency p95 <= 16 ms) is **not met**, and cannot be met by
tuning. The 50 ms poll tick is the only path by which PTY bytes reach the grid, so poll-wait
alone contributes an expected p95 near **47.5 ms** — arithmetic over a code-visible
constant, not a measurement artifact. The update loop is not saturating (`poll()` costs
~10.3 ms against the 50 ms period, 21% duty), so the ceiling is the interval itself.

Two coupled defects in `read_available_bounded_for`
(`crates/tekstide-core/src/runtime/terminal/launch.rs:147-150`):

1. **A hardcoded 10 ms `WouldBlock` sleep** against a caller-supplied 5 ms bound, run
   synchronously on `iced`'s update thread. Caps real throughput at roughly **374 KB/s**
   (measured) while the reader sustains ~69 MB/s when actually reading.
2. **A 64 KiB per-poll cap that truncates mid-read**, discards the remainder, keeps
   reading — feeding the emulator a stream **with a hole in it** — while `TerminalPane::poll()`
   discards the `TerminalOutputSummary` carrying `dropped_bytes`.

**They must be fixed in the same change.** `dropped_bytes` is zero today *only because the
sleep starves the reader*: ~18.7 KB accumulates per poll against a 64 KiB cap. Fix the
sleep alone and a 5 ms window offers ~104 KB against that cap, and the truncation goes live.
**Fixing the sleep in isolation trades a throughput cap for a stream-corruption bug.**

### D1 — Backpressure, not dropping, and this decides how much must be re-proven

The cap needs a real policy. Three were available: block, grow, or drop-with-a-reported-count.

**Take backpressure.** A dedicated reader thread blocks on PTY readability and pushes into
a **bounded** channel; when that channel is full the reader stops reading, the PTY buffer
fills, and the child process blocks on `write()`. That is what a real terminal does, and
the UI thread never blocks because the blocking lives on the reader thread — which is the
whole point of moving off the poll tick.

**The reason this is not merely the nicest of three options** is that it determines the
proof obligation, and that is not obvious:

- **P4 (stream-position independence) covers chunking where every byte arrives.** It does
  **not** cover dropped bytes. A hole landing mid-escape-sequence leaves the parser
  consuming later output as that sequence's parameters — a classification difference P4
  never proved anything about. (RFC-011 Amendment 1's D2 hit the same gap from the other
  direction, with a window that drops a prefix.)
- Choosing **drop-with-a-count** would therefore require establishing an entirely new
  property, with its own enumeration and ablation, for a failure mode with no upper bound
  on how wrong the rendering gets.
- Choosing **backpressure makes dropping structurally impossible**, so **P4's existing
  proof continues to cover the system unchanged.**

Grow was rejected outright: an unbounded buffer against a hostile or merely verbose
producer is a memory-exhaustion path.

**`dropped_bytes` must become unreachable, and that must be proven rather than asserted** —
an enumeration showing no code path can produce a non-zero count, not a comment saying it
cannot happen. If it cannot be made unreachable, stop and raise it; a silently discarded
`TerminalOutputSummary` is how this defect survived in the first place.

### D2 — P1 and P2 are re-enumerated and re-ablated, not assumed

PR-017-B/C proved **P1 (single ingress)** and **P2 (no side channels)** against the current
shape. A dedicated reader thread and a channel are a **new ingress path**, and the fact
that the old proofs passed says nothing about the new one.

Required: the same treatment those slices got — enumeration naming every production write
site into the emulator, and an ablation per property showing the specific failure. A new
ingress that bypasses the filter is exactly the shape P1 exists to deny, and a channel is a
plausible way to introduce one by accident.

**P3 and the modal exclusivity guarantee must also survive.** `SubscriptionMode::for_modal`
plus the `is_none()` guard currently rely on the subscription not producing input while a
modal is open. A reader thread that keeps pushing regardless would defeat that at the
source. State how the new design preserves it.

### D3 — The terminal-count limit rises only after re-measurement

`terminal_session_limit` is `Some(3)` — not a product judgement but a consequence of this
same sleep: each `poll()` costs ~10.1 ms, measured linear per pane against the 50 ms tick,
saturating at 5. Three was the largest count with real headroom.

This is the most user-visible of the three motivations: a user with a build running, a log
tailing and a shell open is already at the limit.

**Raise it in the same change, but from a new measurement, not by assumption.** The limit
is a function of a cost this amendment removes; the new cost must be measured before a new
number is chosen. Raising it without measuring reopens the saturation risk the default
exists to prevent.

### D4 — `NFR-PERF-004` is measured, not declared met

The criterion is this amendment's to discharge. Measure it; do not infer it from the
mechanism being better. **Never reintroduce `iced::window::frames()`** as a measurement
path, and prove non-contamination per criterion, per this RFC's own measurement discipline.

If it is still not met, record that as this amendment's honest outcome. A second evidenced
"not met" is worth more than an unevidenced "met".

### Out of scope

- **Any rendering or UX change.** No new surface, no new keybinding.
- **Changing the security filter's policy.** The filter's classification is untouched; only
  what feeds it changes.
- **Windows/macOS readiness primitives.** Linux only, consistent with everything else here.
