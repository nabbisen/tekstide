---
title: "RFC-007: Runtime Substrate and PTY Feasibility Gate — Implementation Handoff"
rfc: "RFC-007"
rfc_file: "../../proposed/007-runtime-substrate-pty-feasibility.md"
status: "Proposed"
target_milestone: "M4 feasibility gate"
source_rfc_status: "Proposed"
created: "2026-07-09"
---

# RFC-007: Runtime Substrate and PTY Feasibility Gate — Implementation Handoff

## Purpose

This handoff translates RFC-007 into developer-facing guidance for a **spike-only** implementation. The goal is to collect enough evidence to decide whether Tekstide can proceed to RFC-008 TerminalSession/process lifecycle design.

This handoff does not authorize production TerminalSession behavior, AgentRun launch, transcript persistence, command approval, durable audit storage, or a user-facing terminal feature.

## Source RFC Summary

RFC-007 selects a narrow TUI-first feasibility harness. The harness must prove a real PTY-backed shell loop on Linux, with safe shell startup, bounded output-flood handling, termination/orphan evidence, latency measurement, and terminal security observations.

## Dependencies and Sequencing

- Target milestone: **M4 feasibility gate**
- Source RFC status: **Proposed**
- Required predecessor RFCs: RFC-002, RFC-003, RFC-004, RFC-005, RFC-006
- Implementation should start only after this handoff is reviewed.
- RFC-008 implementation must not start until RFC-007 evidence receives a Go decision or a reviewed replacement plan.

## Spike Scope

The spike must demonstrate:

- start a real PTY-backed shell using the safe spike shell profile;
- render output in the TUI feasibility harness;
- send keyboard input to the PTY;
- run harmless commands and show output;
- resize the terminal and verify the child observes rows/columns;
- terminate the shell cleanly;
- start a foreground child process and observe termination/orphan behavior;
- run a bounded output-flood scenario;
- measure basic input/echo latency;
- record terminal-output containment and paste-interception observations.

## Spike Location

Choose one implementation location before code is written.

Recommended default:

- create a temporary internal spike crate under `crates/tekstide-pty-spike/`, marked as non-publishable and excluded from production API promises.

Acceptable alternatives:

- an example target if dependencies and code stay clearly isolated;
- an internal prototype module only if it does not expose public API and can be removed cleanly.

The selected location must be recorded in `qa-evidence.md`. If the spike code remains in the repository after review, explain whether it is quarantined, deleted, or promoted into an RFC-008 design input.

## Dependency Policy

New dependencies are allowed only when directly needed for PTY, TUI rendering, input, resize, signal/process observation, or measurement.

For each new dependency, record:

- crate name and version;
- purpose;
- whether it is spike-only or candidate production input;
- why a smaller standard-library-only path was not sufficient.

Do not add GUI dependencies in RFC-007 unless the TUI-first spike fails and a reviewed amendment authorizes a desktop GUI comparison.

## Safe Shell Startup

Use a low-leakage shell profile by default:

- synthetic test root or explicitly chosen non-sensitive directory;
- documented minimal environment;
- non-login shell where practical;
- avoid startup files where practical;
- no environment dumps, shell history, private home listings, token-like values, or prompt integrations in evidence.

If a personalized user shell is required, mark that run as noisy and provide sanitized evidence plus a low-leakage fallback run where practical.

## Evidence Requirements

Fill `qa-evidence.md` as implementation proceeds. Evidence must include:

- commands used;
- shell executable and arguments;
- root path category without leaking private paths;
- environment policy summary;
- terminal dimensions;
- resize observations;
- signal/timeout/orphan observations;
- output-flood cap and truncation behavior;
- memory before/after output flood;
- input/echo latency procedure and results;
- security observations;
- known limitations;
- Go/No-Go recommendation.

## Stop Conditions

Pause and request review before continuing if:

- the harness requires production TerminalSession state;
- the implementation starts AgentRun launch, transcript persistence, command approval, or audit storage;
- PTY behavior requires reading or printing secrets;
- output-flood behavior cannot be bounded;
- termination leaves child processes in an unknown state without a clear RFC-008 blocker note;
- the TUI harness creates assumptions that cannot plausibly carry into a desktop GUI;
- a new dependency materially changes the workspace or release posture.

