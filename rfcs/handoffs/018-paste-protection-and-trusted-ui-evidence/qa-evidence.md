---
title: "RFC-018: Rendered Paste Protection and Trusted-UI Evidence - QA Evidence"
rfc: "RFC-018"
rfc_file: "../../proposed/018-paste-protection-and-trusted-ui-evidence.md"
status: "Accepted 2026-08-08 — not started"
target_milestone: "M9"
created: "2026-08-08"
---

# QA Evidence

Record results here as each slice lands: gate output, ablations with the exact failure they produced, findings, and limitations.

**This file is where results go. It is not where obligations go.** If a slice discovers something a later slice must handle, put it in that slice's entry in `task-breakdown-pr-plan.md` as well — that is what an implementer reads before starting. This project has lost obligations to that gap four times.

## Recording conventions

- **Ablations name the exact failure**, not "the test failed." A specific wrong value is checkable; a green/red result is not.
- **One ablation per property.** An ablation that breaks two things proves neither.
- **A green ablation is a defect in the ablation**, not a pass. PR-017-C's first P1 ablation passed because `Term::set_title` has no grid effect, so blocking it and bypassing it were indistinguishable — the ablation was redesigned around an observable effect.
- **Screenshots state what they prove and do not.**
- **Disclose rather than manufacture.** Declining to produce an artifact, with the reason, is worth more than a staged one.

## PR-018-A — Design and handoff acceptance

Granted by the human owner 2026-08-08 with RFC-018. Handoff pack authored the same day.

## PR-018-B — Paste ingress

Pending implementation.

## PR-018-C — The confirmation dialog

Pending implementation.

## PR-018-D — The `paste_blocked` audit producer

Pending implementation.

## PR-018-E — Trusted-UI evidence

Pending implementation.

## PR-018-F — Closeout

Pending implementation.

## Known Limitations

Consolidated at closeout. Carried in from RFC-018's own text, to be restated with evidence:

- **The frozen schema records paste refusals only.** `valid_paste_blocked` requires `outcome == Blocked`, so a paste the user approves has no valid encoding in the family. Not a defect in this RFC; a constraint of RFC-013's frozen v1 schema, and amending it needs the owner.
- **No semantic detection of dangerous pasted commands.** RFC-009 excludes it by design. A classifier that catches some dangerous pastes invites the belief that it catches all of them.
- **Nothing here improves terminal performance.** `NFR-PERF-004`, the three-terminal limit, and the ~374 KB/s output ceiling are downstream of the poll defect and owned by readiness-driven terminal I/O.
