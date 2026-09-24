---
title: "RFC-054: User Configuration Completion — implementation handoff"
rfc: "RFC-054"
rfc_file: "../../done/054-user-configuration-completion.md"
source_rfc_status: "Implemented and closed 2026-09-24 — M12 (closed it)"
target_milestone: "M12"
created: "2026-09-24"
---

# Sixteen releases added surfaces. This one adds settings.

Source RFC: [RFC-054](../../done/054-user-configuration-completion.md)

## What this is

`REQ-CONFIG-006`, `REQ-CONFIG-007`, `NFR-UX-004` and `REQ-TERM-004` have been open since the
requirements were written, and the 2026-09-23 audit found **two of them recorded as implemented when
they had never existed**. Keybindings, theme colours, font family, font size and terminal scrollback
become things a user can set. **M12 closes when they do.**

## Read these first

1. [`what-configuration-must-not-do.md`](./what-configuration-must-not-do.md) — **required before
   writing code.**
2. The RFC's **"Decided on acceptance"**, especially **D3′** — the reviewer's D3 was wrong, and the
   correction repairs a category error this project has carried since RFC-022.
3. `future-work.md`'s paragraph beginning *"The category error behind both of the above"*. This slice
   is the keybinding pass it asks for.

## The plan

[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md): A the keybindings and the status repair,
B theme and fonts, C scrollback and the live reload. Checklist:
[`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md).

## Not in this slice

A settings UI (this is a file), per-project settings, theme files or a gallery, icon fonts (RFC-052
D3 rejected the dependency), and any setting that changes **what** the product does rather than how
it looks or which key reaches it.
