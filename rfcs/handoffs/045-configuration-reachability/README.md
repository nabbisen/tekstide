---
title: "RFC-045: Configuration Reachability — implementation handoff"
rfc: "RFC-045"
rfc_file: "../../done/045-configuration-reachability.md"
source_rfc_status: "Implemented and closed 2026-09-12 — RFC-045 is in rfcs/done/"
target_milestone: "M12"
created: "2026-09-12"
---

# A parser for a file whose values have no destination

Source RFC: [RFC-045](../../done/045-configuration-reachability.md)

## What this is

RFC-023 built path resolution, `ConfigStore`, atomic validation, a security-sensitive diff with
increase/reduce semantics, profile conversion, and two audit producers. **Nothing in
`crates/tekstide/src` constructs any of it.** `boot()` never reads the file; the launch path
hardcodes `claude_code_linux_default()`.

And the measurement made at acceptance is the thing to hold onto: **every configurable value in the
file is unreached, and the consumer side has nowhere to put most of them.** So this RFC is smaller
than "wire the config system" and stricter than it sounds — the parser will accept four keys plus
profiles and refuse the rest by name, until each gains a consumer.

## Read these first, in this order

1. [`what-a-configuration-file-must-not-do.md`](./what-a-configuration-file-must-not-do.md) —
   **required before writing code.** Six rules; §5 is the one that is a security control.
2. The RFC's D1–D9, especially **D3′, D8 and D9 under "Decided on acceptance"** — they were made
   after the proposal on a measurement, and they change its scale.

## The trap this slice sets

**Accepting a key is a promise.** The instinct when narrowing a parser is to downgrade unreached keys
to warnings — "accepted, no effect." That is the exact failure RFC-036 opened with, and this RFC
exists partly because RFC-023's `args` already does it silently. A key the file accepts and the
product ignores is a lie the user reads. Refuse it, name it, say it has no effect yet.

## Sequencing

**A → B → C, each landable alone.** No configuration-defined executable can run until C lands the
confirmation; B wires everything that is *not* an executable. Depends on nothing open — RFC-046 is
closed and the D2 interaction is settled. **Blocks RFC-025.**

## What is not in this pack

- **A settings GUI, a profile picker, automatic reload, workspace configuration.** All named out in
  the RFC.
- **An argv template on `AiCliProfile`.** RFC-010 amendment, reserved.
- **Restricted Mode policy from configuration.** RFC-004 territory, security-critical; refused at
  parse here, reserved.
- **Any of the ~20 other keys.** Each returns with its consumer, never before.
