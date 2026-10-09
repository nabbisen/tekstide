---
title: "Release 0.31.0: the crash is detected, not guessed"
status: "**Published 2026-10-09.** Authorised by the owner; published, tagged and verified by the architect. RFC-027 was closed before the publish, and its register row says so because a check now enforces it."
rfc_file: "../done/027-crash-recovery-and-unsaved-buffer-persistence.md"
target_milestone: "M13"
created: "2026-10-09"
---

# Release 0.31.0

## What this release is

**Unsaved work now has somewhere to come back from.** Every dirty document gets a small side record
written outside the project — never the user's own file — and a restart offers it back. A marker
written at launch and removed at every clean exit says whether the last session ended cleanly, and
two Tekstides running at once never mistake each other for a crash.

`REQ-RECOVER-002` and `REQ-RECOVER-005` are both hedged — *"where safe"*, *"where technically
feasible"*. The RFC's job was to decide what those mean here rather than inherit them, and it did:
**safe** means the file on disk is re-snapshotted and a diverged file goes through the conflict path
that already existed; **feasible** means durable, proven against a real `SIGKILL` rather than a
simulated one.

## The decision that changed during the work

The RFC was written as crash recovery. **Amendment 1 (review 485) widened it**, because verifying
the marker against the real binary showed that a clean window close removes the marker and **leaves
the records**, and that nothing guards quitting with unsaved work. So "records with no marker" is
the ordinary consequence of quitting — not the cleanup bug the RFC originally called it, and the
case where a user most wants their work back. **The offer is driven by the records and gated by
nothing.**

## What the review found that the tests did not

- **A recovered document could save over a file that had moved on** — if that file exceeded the 4
  MiB editable bound. `recover` set `last_known_snapshot` with `content_hash: None` on the stated
  assumption that a fresh read always hashes; over the bound it does not, so the snapshots compared
  equal and `save` wrote. Demonstrated, not argued: a 5 MiB file replaced by 15 bytes. Fixed
  structurally — `save` refuses while the state is `Conflict`, which no snapshot shape can bypass.
- **A research fork wrote part of that code** after being told not to. The dev team disclosed it
  first, read every line, found `clippy` failing, fixed it properly and wrote the first tests either
  function had. **The defect above still survived all of that.** Reading found the lint; only
  testing the assumption the comment stated found the hole.

## The process lessons, both now mechanical

- **A one-directional fix makes drift worse.** Correcting the book while `locales/en.ftl` kept
  "against a crash" would have left the program and its documentation disagreeing — the third time
  that shape landed. The checklist step is now *grep `en.ftl` for the words whose meaning the slice
  changed*, not "read it in the other direction too".
- **The delivery-plan register row now has an invariant.** It went stale at two consecutive
  releases, by two different people — RFC-027's here, and RFC-065's own row still said
  "candidate, not yet published" after `0.30.0` shipped, which was the architect's miss. Three
  places state an RFC's status; two were checked; the third is now.

## The gate, and the publish

Three consecutive full-workspace runs, clean on the first attempt: `751 + 17 + 1117`, 0 failures,
**0 fixture entries left** in a fresh short fixed `TMPDIR`. Reproduced independently at the
candidate and again at the published commit.

Published with `cargo publish --workspace` (core first), then tagged. **The tag and the publish are
the same commit, `71aa68a`** — `.cargo_vcs_info.json`'s `sha1` matches it exactly.
`post-publish-check.sh` passes for **`0.31.0` and `0.30.0`**; `LICENSE` and `NOTICE` are in both
published archives and `NOTICE`'s `rusqlite 0.40.2 and libsqlite3-sys 0.38.2` match the published
lockfile. `cargo audit`: zero vulnerabilities, the three warnings matching
`dependency-advisories.md` exactly. `cargo tree | grep accesskit`: empty, so RFC-014 R2 stays
closed.

## Left open, deliberately

- **The flake register is now a tax, and a disposition pass is scheduled for after this release.**
  Measured across reviews 494–497: about **3 failures in 17 full-workspace runs** on both machines,
  so a three-run gate passes first attempt roughly half the time. Nineteen rows; rows 2 and 5 were
  fixed because somebody decided about them, and nothing has been decided about the rest.
- **The marker's "colour the offer's wording" role** is unimplemented. Amendment 1 said *may*.
- **No view proof exists for on-screen text** (RFC-063). A line-function test proves the text is
  produced, never that a view places it — shown by ablation at review 492.
- **A user cannot remove a recent project.** Found by this RFC's own capture work; recorded against
  RFC-036's original decision rather than fixed here.
