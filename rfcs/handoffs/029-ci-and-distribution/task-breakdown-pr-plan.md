---
title: "RFC-029 task breakdown and PR plan"
source_rfc: "../../done/029-documentation-ci-and-release-automation.md"
---

# RFC-029 — task breakdown and PR plan

Six slices. **A gates everything else.**

## PR-029-A — the flake dispositions

Not new work. `test-flake-disposition.md` defines the pass; finish it.

1. Every live row gets a verdict: **fix** (proved by ablation), **quarantine** (moved to a
   non-blocking job, with the reason), or **accept** (with a reason that is not "we have not got to
   it").
2. The two rows this RFC's own reviews added are included: the RFC-058 knock test (review 527) and
   the terminal-resize poll weaker than its assertion (review 538).
3. **Report the resulting rate.** The current number is ~3 failures in 17 runs; after the pass,
   measure it again rather than assuming it improved. That number decides whether PR-029-B can be
   blocking at all.

## PR-029-B — the blocking CI job

1. Linux. One workflow, following `docs.yml`'s rules — first-party actions pinned to commit SHAs,
   downloads pinned by version and checksum.
2. Runs the local gate's content (D2). The fixture-count check needs a short fixed `TMPDIR` on the
   runner; `AF_UNIX` path limits apply there too (see `0.36.0`'s own finding).
3. **Show it failing before trusting it.** A deliberately broken commit on a branch, the job red,
   the breakage reverted. A CI job nobody has seen fail is a badge, not a gate.
4. State in the workflow's own header what it does **not** cover: the Run It gate, the book sweep,
   and anything needing a display.

## PR-029-C — artifacts and checksums

1. A tagged build publishes a Linux `x86_64` binary and its SHA-256.
2. The build is `--locked`, from the tag, and reproducible enough that the published checksum can be
   recomputed from a fresh clone of that tag. **Record whether it actually is**: if the binary is
   not byte-reproducible, say so plainly rather than implying it.
3. The release checklist grows the step that publishes and verifies these.

## PR-029-D — the one-liner installer

1. The script: detect platform, download the binary and its checksum, **verify**, install one file,
   print where it put it.
2. **Truncation safety (§2)** — fail closed, and prove it by serving a prefix and showing nothing
   installed.
3. **Scope (§1)** — one binary, one location, nothing else touched. Stated where the command is
   offered.
4. **Wording (§3)** — *verified against its published checksum*, never *secure*, *safe* or
   *signed*.
5. The install location must be one a beginner can use without `sudo`, and the script says whether
   it is on `PATH` — the gap the four-line form silently left.

## PR-029-E — AUR and the install rewrite

1. AUR: a package that builds from the tag or installs the published binary. **D6: a packaged
   binary never rewrites itself** — this is also what constrains RFC-070.
2. `README.md`'s *Quick Start* and `docs/src/users/getting-started.md`'s *Install* lead with the
   one-liner (D14); `cargo install` and the four-line form follow as equivalents (D13).
3. `getting-started.md` currently says *"It runs on Linux; there is no cross-platform evidence
   beyond it."* **That stays true until RFC-028** — do not soften it because an installer now
   exists.

## PR-029-F — the landing page

1. Carries the platform statement and the no-screen-reader statement (§8).
2. Says nothing about command approval that `REQ-AGENT-013` forbids the book from saying.
3. **Add it to the release checklist's *Book Against The Changelog* sweep** in the same slice that
   creates it, or it drifts at the first feature change.

## The changelog, when the candidate comes

The honest lead is that installing no longer requires a Rust toolchain. Two things to avoid:

- **Do not imply cross-platform.** The artifacts are Linux-only until RFC-028.
- **Do not describe the installer as secure.** §3. It verifies a checksum published beside the
  binary, which is worth doing and is not the same claim.
