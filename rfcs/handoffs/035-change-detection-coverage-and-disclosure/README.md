---
title: "RFC-035: Change Detection Coverage and Disclosure — implementation handoff"
rfc: "RFC-035"
rfc_file: "../../accepted/035-change-detection-coverage-and-disclosure.md"
source_rfc_status: "Accepted 2026-08-18 — scheduled 2026-08-25, first of three for 0.14.0"
target_milestone: "M12"
created: "2026-08-25"
---

# Two holes in what change review can see

Source RFC: [RFC-035](../../accepted/035-change-detection-coverage-and-disclosure.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-035](../../accepted/035-change-detection-coverage-and-disclosure.md) | Both items, with the `.git/` reasoning that decides the approach |
| 2 | [`what-watching-dot-git-must-not-become.md`](./what-watching-dot-git-must-not-become.md) | **Required before any detector change.** Narrowing an exclusion is a security change |
| 3 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Two slices |
| 4 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |
| 5 | [`qa-evidence.md`](./qa-evidence.md) | Where evidence goes |

## Why this is first for `0.14.0`, and why it is more urgent than when it was written

RFC-035 was accepted **2026-08-18** and says:

> An agent that writes `.git/hooks/pre-commit` has installed code that runs on the user's
> machine, and change review will never show it.

When that was written there **was no change review**. `0.13.0` shipped it on 2026-08-25 — seven
days later — and gave users a surface they can open and trust. The hole went from hypothetical to
live, and the thing that makes it worse is the trust: a user who checks the change review surface
and sees nothing alarming has now actively looked, and been told nothing.

The surface does disclose that `.git/` is excluded. That is honest and it is not a mitigation.
"Excludes `.git/`" does not convey "an agent can install code that runs on your machine and this
screen will not mention it."

**The same shipping event raised the second item too.** When a scan exceeds `max_changed_paths`,
`detect_filesystem_changes` sets `Partial { limit }` and returns an **empty** `changed_paths`
(`change_detection.rs:205-209`). Before `0.13.0` nobody saw that. Now a user opening change review
after a large run sees *"Partial"* and **zero files** — the product knowing 4,097 paths changed
and showing none of them.

## What "done" means

Not "the detector watches more." A user whose agent run installed a git hook **sees it on the
change review surface**, and a user whose run changed more paths than the limit sees the first N
with an honest count of the rest — which `ChangeSetSummary` already models and nothing populates.

## Scope boundaries

**In:** a narrow, named allow-list inside the `.git/` exclusion; populating `changed_paths` up to
the limit instead of discarding them.

**Out:** watching `.git/` generally — the churn argument that justifies the exclusion is sound and
is not being reopened. Mid-run detection triggers (RFC-035 item 3, deferred deliberately). Any
change to `max_entries`' behaviour, which is genuinely correct: a truncated scan cannot
distinguish "unchanged" from "not looked at."
