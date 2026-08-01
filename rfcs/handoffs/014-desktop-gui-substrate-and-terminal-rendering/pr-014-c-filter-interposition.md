---
title: "RFC-014 PR-014-C: Filter Interposition - Detailed Developer Instructions"
rfc: "RFC-014"
rfc_file: "../../done/014-desktop-gui-substrate-and-terminal-rendering.md"
target_milestone: "M8"
created: "2026-07-28"
updated: "2026-07-28"
---

# PR-014-C — Filter Interposition: Detailed Instructions

This slice gets its own document because it is not a rendering exercise. It is a **security boundary**, and it is the only part of the spike where getting it subtly wrong produces something that looks correct and is not.

`implementation-handoff.md` §3 states the goal. This document states how to establish it, what will be tested at review, and one concrete defect you will hit if you build the obvious thing.

## 1. The property to establish

> Every byte that reaches the terminal emulator's state machine has passed the RFC-009 accepted-sequence policy, and no other path can mutate emulator state.

That decomposes into four independent claims, each of which needs its own evidence:

| # | Claim | Fails when |
| --- | --- | --- |
| P1 | **Single ingress** — all PTY bytes flow through one filter entry point | The emulator exposes a public write/advance path something else can call |
| P2 | **No side channels** — the emulator has no state-mutation API outside that byte path | A crate offers `set_title()`, `set_clipboard()`, direct grid access, etc. |
| P3 | **Classification parity** — the filter's notion of where a sequence starts and ends matches the emulator's | Filter and emulator disagree on termination, so "inert" bytes get interpreted |
| P4 | **Stream-position independence** — classification does not depend on how the byte stream was chunked | See §3. This is the one that will bite |

P3 and P4 are where real bypasses live. P1 and P2 are mostly answered by reading the emulator crate's API.

## 2. Two facts about the existing code

Verified on `main` at `1f5100b`, before you start:

**Fact 1 — `TerminalSecurityParser` carries no state across calls.**

```rust
pub struct TerminalSecurityParser;          // unit struct — no fields

impl TerminalSecurityParser {
    pub fn parse(&self, input: &[u8]) -> Vec<TerminalSurfaceEffect> {
        let mut effects = Vec::new();
        let mut index = 0;                   // always restarts at 0
```

Each call parses its slice independently and forgets everything afterward.

**Fact 2 — it is not wired into the PTY read path.** It appears only in module re-exports; `launch.rs` and `pty.rs` never call it. This is consistent with RFC-009, which delivered a model-level boundary and explicitly deferred the renderer.

Neither fact is a defect in RFC-009 — it built and reviewed exactly what it claimed. But both matter enormously the moment you put the parser in front of a stateful emulator, which is what this slice does.

## 3. The bypass you will hit: chunk-boundary splitting

**PTY reads are chunked at arbitrary byte offsets.** A control sequence can and will be split across two reads. A stateless filter sees fragments; a stateful emulator downstream reassembles them.

Concretely, with OSC 52 (clipboard write — a family RFC-009 requires to be inert):

```
PTY read 1:  ...output text ESC ]
PTY read 2:  52;c;SGVsbG8= BEL ...more output
```

- Filter call 1 sees a trailing `ESC ]` with nothing after it — an incomplete OSC introducer.
- Filter call 2 sees `52;c;SGVsbG8=` followed by BEL, with **no `ESC ]` prefix**. To a stateless parser that is printable text.
- **Neither call classifies this as OSC 52.**
- The emulator, being stateful, reassembles the two halves and executes the clipboard write.

The filter reports "nothing blocked." The blocked family executed. This is a complete bypass of the RFC-009 boundary, and it requires no adversarial cleverness — it happens by accident whenever a sequence lands on a read boundary, which for a flooding process is constantly.

**Consequence: the filter must be a stateful parser.** It must hold the same sequence-recognition state the emulator holds — in-escape, in-CSI-params, in-OSC-string, in-DCS-string — across calls, and it must not emit a classification for a sequence until it has seen the terminator.

You may implement that stateful filter **inside the spike crate**. Do not modify `tekstide-core`'s parser in this PR — converting it is product work that needs its own RFC amendment and review. Record the requirement; do not smuggle in a core change under a spike.

## 4. Bypass vectors to test

Test all of these. The first is mandatory; the rest are ordered by how likely I judge them to bite.

| # | Vector | Test |
| --- | --- | --- |
| V1 | **Chunk-boundary split** | For each blocked sequence of length N, feed it in two chunks at **every** split point 1..N-1. All N-1 splits must classify identically to the unsplit input |
| V2 | **8-bit C1 controls** | `0x9B` ≡ `ESC [`, `0x9D` ≡ `ESC ]`, `0x90` ≡ `ESC P`. A filter matching only 7-bit forms misses these entirely |
| V3 | **String-terminator divergence** | OSC ends at BEL (`0x07`) *or* ST (`ESC \` / `0x9C`). DCS/APC/PM run until ST. If filter and emulator disagree on which terminates, embedded bytes diverge |
| V4 | **Unterminated sequence at stream end** | Feed `ESC ]52;c;` and stop. Does the filter drop its state while the emulator holds it? Next chunk then continues the emulator's sequence with the filter oblivious |
| V5 | **Parameter overflow** | Feed a CSI with a parameter list far longer than any sane buffer. If the filter truncates and resumes parsing mid-stream, the tail is reinterpreted as a fresh sequence |
| V6 | **Colon sub-parameters** | `CSI 38:2:255:0:0 m` vs `CSI 38;2;255;0;0 m`. A filter splitting only on `;` misparses the colon form |
| V7 | **UTF-8 split** | Multi-byte codepoint split across chunks. Does the filter parse bytes or chars, and does the emulator agree? |
| V8 | **Direct API access** | Read the emulator crate's public surface. Any method mutating grid, title, clipboard, or mode state without going through the byte path breaks P2 |

V1 is not optional and not a spot check. Exhaustive split testing is cheap — a blocked sequence is tens of bytes, so it is tens of cases per sequence — and it is the only way to establish P4.

## 5. Required evidence: an adversarial corpus

Build a **committed, reproducible corpus**, not ad-hoc manual checks.

Structure:

1. A table of byte sequences, each tagged with its RFC-009 classification (accepted family, or inert family).
2. A harness that, for every sequence, feeds it through **filter → emulator** at every chunk split from V1.
3. For every inert sequence, assert the emulator's observable state is **unchanged**: grid contents, cursor position, title, clipboard, mode flags — whatever the crate exposes.
4. For every accepted sequence, assert the expected state change *did* occur — so you are proving a boundary, not a brick wall.

Point 4 matters. A filter that blocks everything passes every security test and is useless. The corpus must demonstrate both directions.

Cover at minimum, from RFC-009's inert list: OSC 52 clipboard, title mutation, OSC 8 hyperlinks, DCS/PM/APC, mouse reporting, terminal identity queries, device status reports.

## 6. What I will probe at review

Publishing this so you can build to it rather than be surprised. At review I will:

- Take your corpus and **add split points you did not test**, especially inside multi-byte parameters and immediately after introducers.
- Feed **8-bit C1 forms** of every sequence your corpus covers in 7-bit form.
- Attempt **direct emulator API calls** that bypass your filter entirely, to test P1/P2 rather than take the architecture on trust.
- Check that a **blocked family leaves no residue** — not merely that it is "not rendered," but that emulator state is untouched.

This mirrors how the RFC-013 SQL CHECK constraints were established: the raw-insert probe is what made the boundary credible, not the code review. Expect the same standard here.

If a probe finds a bypass, that is a normal outcome for this slice, not a failure of your work. Filter-parity bugs are notoriously subtle. Finding them now, in a disposable spike, is the entire point.

## 7. If Option A is falsified

You have falsified Option A if any of the following hold:

- the emulator exposes state mutation outside the byte path that you cannot prevent (P2 fails);
- filter/emulator parity cannot be achieved because the crate's parsing behavior is not observable or documented well enough to match (P3 fails);
- achieving parity requires reimplementing the emulator's parser anyway — at which point Option B is strictly simpler.

**Then stop and report.** Do not attempt to patch around it, and do not fall through to Option C. Falsifying Option A early is a good result: it saves the milestone from a boundary that looks safe and is not.

Fall back to Option B — extend the reviewed RFC-009 parser into a cell-grid model. It is more implementation work and slower to broad shell compatibility, but the security boundary is preserved by construction rather than by interposition.

Option C — adopting an emulator unfiltered and amending RFC-009 — requires maintainer sign-off and a threat-model amendment. It is not a fallback available to this PR.

## 8. Deliverables checklist

- [ ] Stateful filter implemented in the spike crate; `tekstide-core` unmodified.
- [ ] P1 single ingress demonstrated.
- [ ] P2 assessed by reading the emulator's public API; findings recorded.
- [ ] P3 parity demonstrated for every corpus sequence.
- [ ] P4 demonstrated via exhaustive split testing (V1).
- [ ] V2-V8 tested; results recorded including any not applicable and why.
- [ ] Adversarial corpus committed and reproducible.
- [ ] Both directions proven: inert families leave state unchanged, accepted families work.
- [ ] Option A verdict stated plainly — implementable, or falsified with specifics.
- [ ] Licence inventory for every crate introduced.
- [ ] Requirement recorded that `tekstide-core`'s parser needs a statefulness change in M8 product work.
