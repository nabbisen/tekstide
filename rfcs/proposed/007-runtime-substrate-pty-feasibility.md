# RFC-007: Runtime Substrate and PTY Feasibility Gate

Status: Proposed  
Target milestone: M4 feasibility gate  
Date: 2026-07-09

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-roadmap-milestones-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md)

Depends on:

- [RFC-002](../done/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)
- [RFC-003](../done/003-information-architecture-and-ui-mode-model.md)
- [RFC-004](../done/004-security-baseline-and-restricted-mode.md)
- [RFC-005](../done/005-application-shell-and-project-board.md)
- [RFC-006](../done/006-projectsession-state-and-file-explorer-editor-basics.md)

## Summary

This RFC defines a feasibility gate before Tekstide commits to the next terminal/process architecture. It does not implement the production terminal feature. It requires a small, disposable Linux spike that proves the runtime substrate direction can start a shell, render PTY output, send keyboard input, resize, terminate, and measure basic latency/output-flood behavior.

The output of this RFC is a decision record and evidence package. RFC-008 must not begin production TerminalSession/process-lifecycle implementation until this gate has a clear Go decision or a reviewed replacement plan.

## Motivation

Tekstide's next major risks are interactive, not only domain-model risks. The product depends on a terminal/agent-immersion surface that is fast, controllable, honest about process state, and resistant to terminal-output spoofing. The current core is GUI-agnostic and testable, but that does not prove the future renderer/input/process loop.

The feasibility gate prevents the project from building TerminalSession, AgentRun, transcript, approval, and audit features on assumptions about a runtime substrate that has not yet been exercised.

## Goals

- Choose the first implementation direction for the runtime substrate: desktop GUI-first, TUI-first, or a narrowly justified hybrid path.
- Prove a minimum PTY loop on Linux:
  - start a user shell;
  - render visible output;
  - send keyboard input;
  - resize the terminal;
  - terminate the shell;
  - observe behavior under output flood.
- Measure basic input/echo latency and output handling.
- Identify whether the chosen surface can support later Terminal / Agent Immersion Mode requirements.
- Identify security boundaries that must be designed in RFC-008 and RFC-009 before production terminal code lands.
- Keep spike code isolated from stable public APIs unless reviewed otherwise.

## Non-Goals

- Production TerminalSession implementation.
- AgentRun launch.
- Transcript persistence.
- Durable audit storage.
- Full terminal emulator correctness.
- Full ANSI/VT policy.
- Full paste-protection UI.
- Cross-platform implementation beyond documenting portability risks.
- Shipping a user-facing terminal feature.
- Adding plugin, LSP, Git, debugger, remote, or container behavior.

## Decision Scope

RFC-007 decides whether Tekstide has enough evidence to proceed to production terminal/process RFCs.

It does not decide every detail of the terminal architecture. In particular:

- RFC-008 owns the production TerminalSession and process lifecycle design.
- RFC-009 owns terminal security policy: paste protection, ANSI/VT supported subset, output containment, and approval-dialog spoofing boundary.
- RFC-010 owns AgentRun launch on top of the terminal foundation.

## Selected Comparison Plan

RFC-007 uses a **narrow TUI-first feasibility harness** as the default spike direction.

This is not a product-direction decision. Tekstide remains a one-window local workbench, and the roadmap still expects a desktop GUI surface for native approval, paste, review, audit, and project-board interactions. The TUI harness is chosen because it is the shortest credible path to proving the PTY loop, resize behavior, termination behavior, output flood handling, and latency measurement before production architecture begins.

The bounded comparison is:

1. Spike Direction B only for PTY/input/output/process evidence.
2. During the spike, record whether the PTY/process abstraction remains clean enough to support Direction A later.
3. Reject Direction B as a foundation if it requires TUI-specific process semantics, terminal-buffer semantics, or input-routing assumptions that cannot be carried into a desktop GUI.
4. Do not implement Direction A during RFC-007 unless Direction B fails for reasons that a desktop GUI spike could plausibly resolve.

The spike may use Direction C only as a small internal boundary between PTY/process handling and the temporary renderer. Direction C must not become a broad runtime abstraction unless reviewed before RFC-008.

### Direction A: desktop GUI-first

Use the intended desktop GUI substrate as the first interactive surface, with a PTY backend and terminal rendering/input path integrated directly enough to measure feasibility.

Strengths:

- Closest to Tekstide's product goal: one local desktop workbench.
- Exercises native dialog boundaries earlier.
- Reduces risk that terminal/process assumptions conflict with the later GUI.

Risks:

- Terminal rendering inside a retained GUI may require non-trivial custom widget work.
- Latency and output-flood behavior may be hard to evaluate without a mature terminal widget.
- Early GUI choices can become sticky before enough evidence exists.

### Direction B: TUI-first feasibility harness

Use a TUI stack as a temporary spike harness for PTY, rendering, keyboard input, resize, and termination evidence.

Strengths:

- Fastest route to PTY/input/output proof.
- Good fit for terminal-centric behavior and Linux smoke evidence.
- Useful for isolating process and PTY abstractions from desktop concerns.

Risks:

- Does not prove the desktop GUI security surfaces.
- May accidentally create a second product direction.
- Approval-dialog spoofing boundaries remain unresolved until a GUI surface exists.

### Direction C: narrow hybrid

Use a small runtime abstraction with one spike surface, while explicitly preserving an escape path to the desktop GUI.

Strengths:

- Can isolate the PTY/process layer from the renderer.
- Allows a TUI harness without pretending it is the final product UI.

Risks:

- Easy to over-abstract too early.
- Two surfaces can double maintenance if the harness is kept too long.
- Requires discipline to delete or quarantine spike code.

## Safe Spike Shell Profile

The spike must prefer repeatable, low-leakage shell startup over fully personalized user-shell behavior.

Required startup policy:

- Use a synthetic test root inside the repository or another explicitly chosen non-sensitive directory.
- Start with a documented environment policy. The default should be a minimal environment containing only values required for shell execution, terminal behavior, and locale.
- Avoid login-shell startup.
- Avoid interactive startup files where practical.
- Record the exact shell executable, arguments, working directory, environment policy, and whether startup files were allowed.
- Do not capture environment dumps, token-like values, home-directory listings, shell history, or prompt integrations in evidence.

If the implementation must start the user's configured shell with startup files, the evidence package must explicitly mark that run as noisy, explain why it was necessary, sanitize all captures, and include a second low-leakage fallback run where practical.

## Required Spike Behavior

The spike must use a real PTY-backed process, not a mocked command runner.

Required Linux behaviors:

1. Start a shell using the safe spike shell profile.
2. Start the shell in the synthetic test root or selected project root.
3. Render prompt/output in the spike surface.
4. Send typed input to the PTY.
5. Run a simple command and show its output.
6. Resize the terminal and verify the child process observes the new rows/columns.
7. Terminate the shell cleanly.
8. Start a foreground child process, terminate the PTY/session, and record whether the child exits.
9. Record the signal sequence, timeout behavior, process-group/session behavior if exposed, and orphan detection result.
10. Run a bounded output-flood scenario and record responsiveness, buffering, truncation, and memory behavior.
11. Record input/echo latency with a repeatable local procedure.

Recommended commands for manual evidence may include harmless commands such as:

```text
printf 'tekstide-pty-ok\n'
stty size
sleep 60
yes tekstide-output | head -n 10000
```

The evidence must avoid printing secrets, environment dumps, home-directory listings, tokens, private paths beyond the selected test root, or command output from unrelated user projects.

Unresolved process-group semantics are blocking input to RFC-008. If the spike cannot prove safe foreground-child termination, RFC-008 must explicitly design the missing process-group/session model before implementation starts.

## Performance Evidence

The spike must report measurements instead of claiming "fast" or "instant".

Minimum evidence:

- reference machine description at a non-sensitive level;
- terminal dimensions used for the test;
- input/echo latency procedure;
- p50, p95, and worst observed input/echo latency, if measurable;
- output-flood size and duration;
- provisional maximum buffered bytes and/or lines;
- truncation marker behavior;
- whether input remains responsive during bounded output;
- memory before and after output flood;
- recovery behavior after the output-flood command exits;
- known measurement limitations.

Provisional spike bounds:

- The output-flood scenario must produce at least 10,000 lines or 1 MiB of output, whichever is easier to reproduce safely.
- The harness must enforce an explicit temporary buffer cap before the test runs.
- The evidence must report the cap value, what was dropped or truncated, and whether the UI/harness remains usable after truncation.
- The cap is not the final product scrollback default; it is only a feasibility control until RFC-008/RFC-009 define production limits.

The target from the requirements remains p95 <= 16 ms for terminal input latency while one visible terminal and background jobs produce bounded output. If the spike cannot meet or credibly measure that target, RFC-008 must treat performance as an open blocking risk rather than assuming success.

## Security Evidence

RFC-007 does not design the full terminal security boundary, but it must collect enough evidence to inform RFC-009.

Required notes:

- whether terminal output is constrained to the terminal surface in the spike;
- whether unsupported control sequences are ignored, rendered inertly, or passed through by the candidate renderer;
- whether the renderer can prevent terminal output from changing application chrome, trust state, approvals, clipboard, or command history outside the terminal buffer;
- whether multiline paste can be intercepted before bytes reach the PTY;
- whether native approval/paste dialogs can be visually separated from terminal output in the chosen direction;
- which concerns must move to RFC-009.

The spike must not claim complete ANSI/VT safety. It may only record observed behavior and candidate mitigation points.

## Acceptance Criteria

RFC-007 is accepted only when review agrees that the following are clear enough:

- chosen next substrate direction or explicitly bounded comparison plan;
- spike location and deletion/quarantine policy;
- safe spike shell profile;
- Linux PTY behavior evidence plan;
- latency/output-flood evidence plan;
- security-boundary evidence plan;
- Go/No-Go rule before RFC-008 implementation starts.

The feasibility gate passes when evidence shows:

- shell start, output render, input, resize, and termination work on Linux;
- the child process observes terminal resize;
- foreground-child termination behavior is observed, including signal/timeout and orphan result;
- output flood is bounded and does not make the surface unrecoverable;
- provisional buffer cap, truncation behavior, memory before/after, and recovery behavior are reported;
- input latency is measured and does not obviously violate the terminal performance target, or the miss is explained with a concrete mitigation plan;
- terminal output cannot obviously escape the spike surface into app-level controls;
- multiline paste interception appears feasible before PTY write;
- the selected direction is compatible with Tekstide's one-window, multi-project, Terminal / Agent Immersion Mode requirements.

The gate fails or requires redesign when:

- PTY input/output cannot be made reliable;
- resize, foreground-child termination, or orphan detection behavior is not controllable enough for RFC-008;
- output flood causes unbounded memory growth or persistent unresponsiveness;
- the renderer cannot support a credible ANSI/VT containment boundary;
- the chosen path cannot support distinct native security dialogs outside terminal output;
- performance evidence is missing, non-repeatable, or materially below requirements without a mitigation path.

## Proposed Evidence Package

The implementation review package for this RFC should include:

- a short decision record naming the chosen direction and rejected alternatives;
- exact commands used for shell/input/resize/output-flood tests;
- latency and output-flood measurements;
- screenshots or text captures that do not expose secrets;
- security observations for paste interception and terminal output containment;
- notes on Linux-only assumptions and expected Windows/macOS risks;
- recommendation to proceed to RFC-008, revise RFC-007, or change substrate direction.

## Implementation Notes

Spike code should be isolated. Acceptable locations include a temporary crate, example, or internal prototype module, as long as it is clearly marked as spike-only and does not become a public API by accident.

Production code introduced during the spike must be small and justified. If a reusable abstraction emerges, it should be reviewed before becoming the foundation for RFC-008.

The spike should not add workspace dependencies casually. New dependencies must be explained in the evidence package, including why they are needed for PTY, rendering, input, resize, or measurement.

## Review Plan

- Architecture review for substrate direction and whether the evidence is sufficient.
- Security review focused on terminal boundary, paste interception, and spoofing risks.
- Implementation review of any spike code that remains in the repository.
- Release/process review if dependencies or workspace structure change.

## Open Questions

- Which crate combination gives the lowest-risk PTY and renderer path for Linux first?
- Is a TUI harness acceptable as evidence if the product remains desktop GUI-first?
- Should spike code be committed for reproducibility or kept as disposable evidence only?
- What is the minimum acceptable latency measurement method before a real GUI exists?
- Which Windows/macOS checks must be pulled forward before M10?
