---
title: "Release 0.33.0: the sidebar is not a mode"
status: "**Published 2026-10-10.** Authorised by the owner; published, tagged and verified by the architect. The release that removed interface, and whose last slice correctly built nothing."
rfc_file: "../done/067-the-sidebar-is-not-a-mode.md"
target_milestone: "M13"
created: "2026-10-10"
---

# Release 0.33.0

## What this release is

**One toggle used to govern the whole project tab.** Switching to terminals did not only change the
main area — it took the file tree away and left a sentence in its place:

> Files are listed here in Content mode.

A placeholder apologising for its own absence, in a zone `Tab` could still reach. The explorer now
renders in both modes, the sentence is deleted rather than reworded, and the file tree sits beside
running terminals. **The slice removed interface rather than adding it**, which is the version of
this change that survives the owner's standing criterion that the result must be *finally* clean.

Activating a file from Terminal mode switches to Content mode and shows it — **and nothing else may
move you there on your behalf**.

## The bug underneath

`ProjectSession::open_text_document` — the function every document-open path goes through —
**unconditionally forced `mode = Content` for every caller.** Harmless while the explorer was the
only reachable one; a live violation the moment RFC-027's recovery offer became a second, because
recovering a document would have yanked a user out of a terminal they were watching.

Fixed at the root: the side effect was removed from shared infrastructure rather than special-cased
for the new caller, with an explicit call made once, from the explorer's own arm. **It was found
because the handoff required a test for exactly that hazard**, written during planning from
enumerating the three call sites that open documents — not from noticing it in review.

Two further Content-mode guards (`handle_explorer_key`, `ensure_explorer_scanned`) were found by
reading the functions rather than from the pack. Without removing both, this would have shipped a
tree that was **visible and inert** — satisfying the letter of the decision and missing it entirely.

## The slice that correctly built nothing

The RFC said the third slice could build nothing if the measurement said so, and it did. A mode
switch costs **~20 µs** to redraw in Content mode, **under 5 µs** in Terminal, against a ~16 ms
yardstick for "a user would notice" — three orders of magnitude of headroom. **Near-real-time is
what the switch already was**, so no split, no panes, no third mode.

The first version of that measurement was wrong in an instructive way: a single `as_micros()` sample
per round put every figure on the timer's own floor — each median exactly equal to its own minimum,
every value a multiple of 4 µs. Repeating 200 builds inside the timed region fixed it, **and the fix
avoided a second trap nobody named**: dividing a `Duration` and then calling `as_micros()` would have
reintroduced the identical truncation one step later.

## What the release process learned

**An invariant that had been passing by luck.** Extending the status checks to `rfcs/README.md`'s
rows turned up that `CHANGELOG.md`'s `0.30.0` Status said *"scoped"* and had **never said released
at all** — surviving two further releases only because `0.31.0`'s section happens to mention RFC-065
in passing, which put it in the released set by accident. Three of the architect's own publishes had
left a status stale somewhere; this is the first release where a test watches the README rows.

The right check was also not the one that was asked for: `claims_unfinished` sees only
"Proposed"/"Accepted", and *"candidate"* versus *"released"* contains neither — so the family's own
predicate could not have caught it, and a release-state check against the changelog was built
instead.

## The gate, and the publish

Three consecutive full-workspace runs, clean on the first attempt: `758 + 19 + 1118`, 0 failures,
**0 fixture entries left**. Reproduced independently at the candidate and again at the published
commit.

Published with `cargo publish --workspace` (core first), then tagged. **The tag and the publish are
the same commit, `1339c8e`** — `.cargo_vcs_info.json`'s `sha1` matches it exactly.
`post-publish-check.sh` passes for **`0.33.0` and `0.32.0`**; `LICENSE` and `NOTICE` are in both
published archives and `NOTICE`'s `rusqlite 0.40.2 and libsqlite3-sys 0.38.2` match the published
lockfile. `cargo audit`: the register's three warnings, no vulnerabilities.
`cargo tree | grep accesskit`: empty, so RFC-014 R2 stays closed.

## Left open

- **The audit `CHECK` class** (from `0.32.0`): 52 constraints, nothing asserting a family and
  outcome actually lands. A pre-1.0 item.
- **No view proof for on-screen text** (RFC-063): a line-function test proves text is produced,
  never that a view places it.
- **A user cannot remove a recent project** — RFC-036's decision, with its cost now known.
