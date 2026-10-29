---
title: "PR-028-F: what it includes beyond the RFC's own text"
status: "Handoff. Opened 2026-10-29, after PR-028-E was accepted and D2's backend decision was made."
rfc_file: "../../accepted/028-cross-platform-support.md"
target_milestone: "M14"
created: "2026-10-29"
---

# PR-028-F — scope additions

The RFC's own PR-028-F paragraph stands unchanged. Four things decided after it was written belong
to this slice, and one thing it might be read as promising does not exist.

## 1. The backend flip comes first

D2 is corrected: the feature becomes `macos_fsevent`. The dated correction in the RFC carries the
A/B and the reasoning. Four items, and the fourth is the one that matters: a test that a **content**
change -- not a create -- travels from the watcher through `record_changed_paths` to a scan request.
Every watch test that runs on macOS today exercises an entry-list change, which `kqueue` already
reported, so nothing currently pins the property the decision rests on.

## 2. A rendered capture from the macOS runner, in place of what Run It cannot do there

The runner is a real macOS VM on Apple silicon -- real window server, real font stack -- but nobody
is looking at it, so it cannot *be* Run It. It can carry the part this release actually risks, since
PR-028-E's output is entirely visual: whether the window opens, whether the Help modal draws
`⌘+⌥+P` rather than a missing glyph, whether the chrome looks like a macOS window.

Add one step to the macOS job that launches the binary headfully and captures its own window, and
attach the image. **Verify rather than assume it works**: a macOS runner has a window server, but
screen capture is TCC-governed there and nobody has measured it. One step tells us; if it refuses,
report that and stop -- it is not worth a second attempt.

Every rule in `.git-exclude/rules/project-instructions-gui-common.md` applies, and two of them bind
here in particular: throwaway `XDG_CONFIG_HOME` **and** `XDG_STATE_HOME`, because a capture made
against real state puts other projects' names into committed evidence; and a fixed window size
obtained **at launch**, never by resizing a running window.

If the capture works, the release notes' wording changes from "never run on real hardware" to
"launched and rendered on real macOS, screenshot attached, not interactively exercised by a human" --
materially more honest, and it is the whole reason the step is worth a CI cycle.

## 3. The ignore list F must publish is **seven**, not ten

"Green on macOS" is a claim with ignores behind it, so state it in a form a reader can check. From
the two jobs' own counts, macOS ignores six more in `tekstide-core` and one more in `tekstide` than
Linux does. Those seven are the macOS-conditional ones -- four APFS non-UTF-8 filename tests, the
`/dev/fd` traversal test, row 8, and the exit-after-wake split. Everything else ignored is ignored on
both platforms and predates this RFC (measurements, benchmarks, diagnostics).

List the seven with their reason strings, and for each say what covers the property elsewhere or that
nothing does. "Green with ten ignored" invites a reader to assume the worst; the seven with their
reasons is the same fact, checkable.

## 4. Branch protection is **not** part of "fully blocking"

F's own "flip the macOS CI job to fully blocking" means exactly what it says in the workflow: a macOS
failure fails the run. It does **not** mean a red run prevents a push, and F must not be written as
though it does.

The owner decided on 2026-10-29: force pushes blocked, deletions restricted, **status checks
deliberately not required**, because a push cannot be gated by a check that has not run and the only
coherent alternative forces branches and pull requests onto a project that pushes to trunk on
purpose. Enforcement of a red gate lives in `release-checklist.md`, run by a human before tagging --
already true of Linux. See that file's own "Required Gates" note for the full reasoning.

## What is still the owner's, inside this slice

The Run It answer itself (real hardware, or the label), after item 2 reports whether a capture is
even possible. Nothing else here waits on them.
