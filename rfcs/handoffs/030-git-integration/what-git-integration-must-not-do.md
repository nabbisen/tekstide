---
title: "What Git integration must not do"
rfc: "RFC-030"
rfc_file: "../../done/030-git-integration.md"
source_rfc_status: "Implemented and closed 2026-09-23 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# What Git integration must not do

**Required reading before writing code.** Everything here is about one property: **opening a project
must not let that project's files decide what runs on this machine.**

## §1 It must not run anything the repository names

Filters, hooks, config-named helpers, aliases, pagers, external diff or credential programs — a
repository directory is **untrusted input**, and every one of these is a way for it to name a
program. This holds for a library as much as for a subprocess: a library that shells out for a filter
has the same defect with a nicer API.

**Assert it against a fixture that tries.** A review that reads the code and concludes it is fine is
not evidence.

## §2 A measurement that depends on the machine measures nothing

The fixture points global and system Git configuration **inside itself** and sanitizes the
environment (D7). Otherwise a green run means "this developer's config happens not to do that", and
the property is untested on every other machine — including CI.

## §3 The fixture must be harmless

The program it names **writes a marker file**. Nothing destructive, nothing networked, nothing
outside the temporary directory. "It ran" must be observable without anything happening.

## §4 No write operation may exist in the path

Not guarded by trust state, not behind a flag, not unused. **Held by the API** — the type offers no
write — not by a grep over call sites (REQ-GIT-007). A dormant write is the RFC-036 shape with worse
consequences.

## §5 It must not block input

Off the UI thread, debounced (NFR-PERF-006, D4). A large repository is slow, and slowness must cost a
*pending* state, never a frozen window.

## §6 It must not show stale state as current

`ProjectGitSummary` already models *pending* and *unavailable*. When a read has not finished, or the
guarantee in §1 does not hold, **say so** rather than showing the last thing that was true.

## §7 Restricted Mode is not the boundary

Detection may run in Restricted projects **only because §1 holds** — that is what makes it read-only
in REQ-GIT-005's sense. If §1 cannot be guaranteed, detection reports `unavailable` there (D3). Trust
state is not a substitute for the guarantee.
