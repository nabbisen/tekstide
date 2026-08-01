---
title: "RFC-017 PR-017-B — Filter promotion: detailed instructions"
rfc: "RFC-017"
rfc_file: "../../proposed/017-terminal-renderer-and-immersion-mode.md"
slice: "PR-017-B"
status: "Ready — the security-critical slice; read before writing any code"
created: "2026-08-01"
---

# PR-017-B — Filter promotion

**Read this before `task-breakdown-pr-plan.md`.** This is to RFC-017 what `pr-015-c-input-routing.md` was to RFC-015 and `pr-014-c-filter-interposition.md` was to the spike: the slice where being subtly wrong produces something that looks correct.

Nothing renders emulator output until this slice is accepted. `B → C` is strict.

## What you are actually doing

You are **not writing a filter.** One exists, in `crates/tekstide-gui-spike/src/filter.rs`, and it was reviewed as part of RFC-014 PR-014-C. You are moving it into product code and re-proving that its four properties still hold there.

That distinction matters because the failure mode here is not "the filter is wrong." It is "the filter is right and something reaches the emulator without going through it." A correct filter with a second ingress is worth nothing, and it is the kind of defect that a passing test suite will not show you.

## The architecture, decided in the RFC — do not redesign it

| Layer | Crate | Holds |
| --- | --- | --- |
| **Policy** — which families are accepted, which are inert, and why | `tekstide-core::runtime::terminal::security` | Already exists. `TerminalSequenceFamily`, `TerminalPolicyReason`, the effect enums. **Does not move.** |
| **Interposition** — the `vte::ansi::Handler` impl | shell crate | New. A thin adapter that routes every callback into core's classifier. **Holds no policy of its own.** |

`tekstide-core` has no `vte` or `alacritty_terminal` dependency today, and this slice must not give it one. Verify mechanically, the same way `0.4.0` verified `iced` never leaked into core:

```
cargo tree -p tekstide-core --edges normal | grep -ciE 'vte|alacritty'   # must be 0
```

**The adapter having no policy is the point.** If it cannot decide anything, it cannot drift from core's decisions — that is P3 made structural rather than merely tested. If you find yourself writing a `match` in the shell crate that decides whether a sequence is acceptable, stop: either core's classifier needs extending (raise it) or you are duplicating policy (don't).

## The four properties

These are the acceptance criteria. Each needs independent evidence — one ablation per property, not one per test file.

### P1 — Single ingress

Every byte from the PTY reaches the emulator through exactly one filter entry point.

The spike's shape was `Processor::advance(&mut SecurityFilter::new(&mut term), bytes)`. What makes P1 true is not that line; it is that **no other line anywhere can reach `term`'s mutating API.**

Prove it by enumeration, not assertion: list every construction and every mutable borrow of the emulator in the crate, and show each is inside the filtered path. Then make it structural — if the `Term` can be made private to a module whose only public entry point takes bytes, do that, and P1 stops depending on nobody adding a second path later.

**The paths most likely to become a second ingress**, none of which exist yet and all of which will be tempting:

- test helpers that "just need to set up grid state"
- scrollback restore after a resize or a hidden-session round trip
- a "replay the last N bytes" debugging aid
- any `#[cfg(test)]` constructor on the emulator wrapper

If you add one of these, it goes through the filter or it does not exist.

### P2 — No side channels

The emulator exposes no state-mutation API reachable outside the byte path.

This is P1's converse and it is about `alacritty_terminal`'s surface, not yours. `Term` has methods beyond `Handler`. Enumerate the ones that mutate, and show each is either unreachable from your code or wrapped. Resize is the interesting one: it mutates grid state and it is legitimately driven by the UI, not by PTY bytes. **That is not a violation — it is a second, non-byte input that must be identified and bounded**, not quietly excluded from the analysis.

### P3 — Classification parity

The filter's notion of where a sequence begins and ends is identical to the emulator's.

The spike satisfied this by construction — filter and emulator share one `Processor`, so there is one parse. Preserve that. **A separate pre-scan pass over the byte stream would break P3 silently**, because two parsers disagree at exactly the inputs an attacker chooses.

### P4 — Stream-position independence

Classification does not change with how bytes are chunked across reads.

**This is the property that decays silently, and the one I most expect to find a defect in.** A PTY delivers arbitrary boundaries under load; a filter correct on whole sequences and wrong on split ones fails only when the machine is busy — which is also when the user is least likely to notice a stray sequence taking effect.

The test that matters is not byte-at-a-time and not whole-buffer. It is **every family, split at every internal byte boundary**, asserting the classification and observable grid effect are identical to the unsplit case. For a corpus of *n* sequences averaging *k* bytes that is roughly *n·k* cases — generate them, do not hand-write them.

Include, specifically:

- a split between `ESC` and `[`
- a split inside a multi-digit parameter (`38;5;1|96m`)
- a split inside an OSC string body, and one immediately before its terminator (`BEL` and `ST` both)
- a split inside a UTF-8 multi-byte codepoint
- an inert-family sequence split at each boundary — an unsupported family must stay inert **in every split**, since the interesting attack is one that becomes supported when fragmented

## What the corpus must cover, beyond splitting

- **Every family RFC-009 marks unsupported produces no observable grid effect.** Not "is logged as blocked" — *no effect*. Compare the full grid state before and after, not just the cursor.
- **Every accepted family produces the effect core says it should.** A filter that blocks everything passes every security test and is useless; the corpus proves the boundary is where RFC-009 put it, in both directions.
- **Truncated and malformed sequences** — an `ESC [` with no final byte, an OSC with no terminator, a parameter list longer than any real sequence. Fail closed, and do not consume unbounded memory waiting for a terminator that never arrives.

That last one is a resource question as much as a correctness one: an unterminated OSC is an attacker-controlled unbounded buffer if nothing bounds it.

## Evidence

- **P1-P4 each independently ablated** — break the property, watch the specific test fail, restore. One ablation per property.
- **The enumeration for P1 and P2**, written out, not summarised. "I checked" is not evidence; the list is.
- **`cargo tree -p tekstide-core`** showing zero `vte`/`alacritty` matches.
- **The split corpus**, with its generation method stated and its case count reported.
- Gates as usual, plus `git diff --check`.

## If it does not hold

**Stop and escalate rather than proceeding to PR-017-C.** RFC-014 named **Option B — own the parser** — as the fallback if the filter proves leaky, and choosing it is not a failure; it is the decision the spike existed to inform. An emulator behind a filter that cannot be shown to be single-ingress is worse than no emulator, because it manufactures confidence.

The same instruction RFC-021 PR-021-D carried applies here: escalating is a success.

## One thing I will check that is easy to get right and easy to skip

The spike is `publish = false`. The product crate is published. **`include_str!` and any path reaching outside the crate will break `cargo package`** — that exact defect shipped in PR-015-F and was caught only by running the packaging gate at release time.

If this slice copies anything from the spike that references a sibling path, fix it here rather than at the next release.
