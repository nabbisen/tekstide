# RFC-029: Documentation, CI, and Release Automation

Status: **Implemented and closed 2026-10-14; released as `0.37.0` 2026-10-14.** See the
*Closed* section. **Accepted by the human owner 2026-10-13.** D1–D13 as written; **the remaining
question — what the install page leads with — answered by the acceptance itself**, see *Decided on
acceptance*. Proposed 2026-10-12. `0.37.0`, M14, the first RFC of that milestone. D9–D13 reverse
this RFC's own first recommendation against a one-liner installer, after the owner challenged it.

## Summary

**`cargo install tekstide` is the only way to get this program, and it asks an unskilled user for a
Rust toolchain and a compile.** That is the owner's stated complaint and this RFC's purpose.

It builds the first build/test CI this project has had, produces prebuilt binaries with published
checksums, packages for AUR, rewrites *Quick Start* and *Install*, and adds a landing page.

## Half of this RFC's reserved scope is already delivered

The reservation reads *"`docs/src` mdBook by persona, CI gates and build matrix, release
automation."* The book **exists**, built incrementally across releases: `docs/src/users/` (six
pages), `docs/src/contributors/` (four), `docs/src/record/` (two), published by
`.github/workflows/docs.yml`. What remains of the documentation half is the install rewrite and the
landing page. The RFC is therefore smaller than its reservation implies, and the remaining weight
is CI and distribution.

`docs.yml` is also the template. It already settles the conventions a second workflow must follow,
with its reasoning in the file: **first-party `actions/*` only, each pinned to a commit SHA**;
downloads **pinned by version *and* SHA-256**; and no `paths:` filter, because a hand-maintained
list of include targets is a drift generator. A new workflow that quietly relaxes any of these is a
regression, not a shortcut.

## The constraint that decides the CI half

**Measured, in this project's own records: about 3 failures in 17 full-workspace runs**
(`test-flake-disposition.md`), which is why the gate rule is three *consecutive* green and why
`0.32.0`'s gate had to be redone twice. **Four register rows carry an `Accept` verdict** — accepted
precisely because a human re-runs, which is cheap when a human is already standing there.

A blocking CI job inherits that rate. At three runs it would be red roughly half the time; at one
run, red about one time in six. **A gate that is red when nothing is wrong teaches everyone to
ignore it**, and this project's entire working method depends on a red result meaning something.

That is not an argument against CI. It is an argument about order.

## Decisions

- **D1. The accepted-flake rows are fixed or quarantined *before* CI becomes blocking.**
  `test-flake-disposition.md`'s open verdicts are a **prerequisite** of this RFC's CI slice, not
  parallel work. A quarantined test still runs — in a separate, non-blocking job that is read, not
  ignored — and a row moved there says so with a reason.
- **D2. The blocking job runs what the local gate runs**: `cargo fmt --check`, `clippy --workspace
  --all-targets --all-features -D warnings`, `cargo test --doc --workspace`, the full workspace with
  `--all-targets --no-fail-fast`, a short fixed `TMPDIR`, and **0 fixture entries left**. A weaker
  subset would prove something other than what this project means by "the gate passed".
- **D3. New workflows inherit `docs.yml`'s own rules** — first-party actions pinned to commit SHAs,
  third-party avoided, every download pinned by version and checksum.
- **D4. Automation produces artifacts; it does not replace judgement.** The release checklist's
  human steps — the Run It gate, the book-against-the-changelog sweep, the acceptance decision —
  **stay human**. `0.12.0` passed every mechanical check this project had while shipping inert
  buttons; a pipeline that reports green is exactly the thing that failure mode wears.
- **D5. Every published artifact has a published checksum**, and the install documentation shows
  the **verified** form — download, check, run — before any convenience form.
- **D6. A packaged binary never rewrites itself.** This is AUR's constraint and it decides RFC-070's
  shape too.
- **D7. The first install instruction a visitor sees is not `cargo install`.** It stays, for people
  who want it; it stops being the front door.
- **D8. The landing page is a truth surface under the book's discipline.** It carries the
  platform statement and the no-screen-reader statement, and may not imply a platform or capability
  that does not exist. Marketing copy drifting from the product is the same defect class as a stale
  book page, which this project has spent many reviews on.

## Non-goals

- **macOS and Windows artifacts** — there is nothing to build for them until RFC-028. The build
  matrix is written so adding a platform is a line, not a redesign.
- **The Microsoft Store** — RFC-071, which depends on RFC-028's Windows half.
- **Check for updates** — RFC-070.
- **Code signing and notarisation.** Checksums are in scope; signing needs an identity and a key
  policy, and is the owner's decision rather than a detail of this RFC.
- **Rewriting the release checklist.** It gains automation where it says "run this"; it does not
  shrink.

## Slices

- **PR-029-A — the flake dispositions land** (D1). Not new work: the pass that
  `test-flake-disposition.md` already defines, finished, so that a red CI means something.
- **PR-029-B — the blocking CI job** (D2, D3). Linux only. Must be shown failing on a deliberately
  broken commit before it is trusted.
- **PR-029-C — release artifacts and checksums** (D5): a tagged build producing a Linux binary and
  its checksum, published on the release.
- **PR-029-D — the one-liner installer** (D9–D12): the script, its own checksum verification, its
  truncation safety, and its stated scope. **Truncation safety is proven by truncating it** — serve
  a prefix, run it, show that nothing was installed — not by reading the wrapper and agreeing it
  looks right.
- **PR-029-E — AUR** (D6) and **the install rewrite** (D7, D13): `README.md`'s *Quick Start* and
  `docs/src/users/getting-started.md`'s *Install*, leading with the one-liner and keeping the
  four-line form documented beside it as the equivalent, never as the safer one.
- **PR-029-F — the landing page** (D8).

## The installer: decided 2026-10-12, against the reviewer's first recommendation

**The owner asked for a one-liner on 2026-10-08. This RFC first recommended against it, and that
recommendation was wrong.** The argument is recorded in full because the reasoning matters more
than the outcome.

**What the reviewer proposed instead** — the verified form, four lines:

```sh
curl -LO https://github.com/nabbisen/tekstide/releases/download/<version>/tekstide-x86_64-linux
curl -LO https://github.com/nabbisen/tekstide/releases/download/<version>/tekstide-x86_64-linux.sha256
sha256sum -c tekstide-x86_64-linux.sha256
install -m 755 tekstide-x86_64-linux ~/.local/bin/tekstide
```

**Why that was wrong.** The owner asked why four is acceptable when one is not, and there is no good
answer. Both forms execute code fetched from the same server. **The checksum is served from the same
release as the binary**, so step 3 catches a corrupted or truncated download — not an attacker who
can write to that release. Recommending it to a beginner who cannot assess that distinction hands
them a ritual that *looks* like verification while resting on the same trust root as the thing it
verifies.

That is the defect this project refuses everywhere else. `what-the-dialog-must-not-lie-about.md` §2:
*"a dialog that looks like a security control while being an honour system is worse than no dialog:
it manufactures confidence that is not warranted."* A four-line checksum ceremony with a same-host
checksum is that sentence, applied to an install page. The fourth line also assumes `~/.local/bin`
is on `PATH`, which for this RFC's own target user is frequently untrue and never obvious.

**What actually survives the comparison** is narrower, and both are properties of the *script*
rather than reasons to withhold it:

- **A truncated download piped to `sh` executes a prefix of the script.** The connection drops and a
  half-installer runs. The four-line form fails its checksum instead.
- **A script's scope is unbounded and unseen.** The binary does what Tekstide does, which the user
  chose; a script can touch shell rc files, system paths, anything, and piping it means the user
  never sees what it did.

### Decisions

- **D9. Tekstide offers a one-liner installer.** The owner's principle governs: for a beginner,
  simpler is better, and the complex form was not buying what it appeared to buy.
- **D10. The script verifies the binary's checksum itself**, before installing it. The verification
  the four lines performed does not disappear — it moves into the script, where it happens whether
  or not the user would have done it.
- **D11. The script is truncation-safe.** Wrapped so that a partial download executes nothing —
  the whole body in a function, invoked on the last line, or an equivalent that fails closed.
- **D12. The script's scope is stated where it is offered, and it stays inside that scope.** It
  installs one binary to one location and touches nothing else — no shell rc edits, no system
  package changes, no repositories added. If it would need to, it says so first and stops.
- **D13. The verified four-line form stays documented**, for people who want it and for anyone
  auditing what the script does — presented as the equivalent, not as the safer one, because on the
  same trust root it is not.

### As it was put to the owner

**Only the headline question.** The installer exists; what the install page *leads* with is the
owner's call: the one-liner (what a beginner should see first), or `cargo install` (what the page
leads with today and what D7 already says must change). The reviewer recommends leading with the
one-liner, now that it is being built.

## Decided on acceptance (2026-10-13)

- **D14. The install page leads with the one-liner.** `README.md`'s *Quick Start* and
  `docs/src/users/getting-started.md`'s *Install* open with it; `cargo install` and the four-line
  verified form follow, as equivalents for people who want them. Decided by acceptance of the
  recommendation, not by a separate instruction — correct this line if that is not what was meant,
  which costs nothing before PR-029-E.

**What this RFC is really for, restated because the slices can obscure it.** A person who has never
heard of Rust should be able to get this program running. Everything here — the CI that makes a
binary trustworthy enough to publish, the artifacts, the script, the AUR package, the rewritten
pages — exists for that one outcome. A slice that lands its mechanism and leaves that person no
better off has not finished.

**And the order is a prerequisite, not a preference.** PR-029-A exists because a blocking CI job
that is red when nothing is wrong destroys the only thing CI is for. Nothing in B–F starts until
the flake dispositions are done.

## Sequencing decided after acceptance (2026-10-13)

**D15. The first release artifact comes from `0.37.0`'s own tag — this RFC's own release — not
from a manual dispatch beforehand.** Owner's decision, option B of three put to them.

The architect had ordered the documentation slice (PR-029-E) ahead of the channel it documents,
and then asked the owner to authorise a one-off `workflow_dispatch` against `0.36.0` to produce
something for those pages to point at. That turned a sequencing error into a standing decision for
the owner, blocked three slices behind it, and produced a parked branch that broke the project's
trunk-only workflow. The decision removes the authorisation rather than granting it.

Consequences, recorded where they bind:

- The install pages land **in the candidate**; `held/pr-029-e-install-rewrite` merges and is
  deleted. No branch is carried.
- **The end-to-end install demonstration moves to post-publish verification** — it needs an
  artifact that acceptance precedes, which the Whole-RFC checklist's first draft did not account
  for.
- `release.yml`'s first live run is the `0.37.0` tag. Its blast radius there is bounded: it touches
  neither crates.io nor the release notes, and `workflow_dispatch` remains as the re-run path.

## Closed (2026-10-14)

Six slices and the Whole-RFC checklist are done. **Released as `0.37.0` 2026-10-14** — see
`CHANGELOG.md`'s own status line, which does not claim "released" ahead of the actual publish.

**PR-029-A (the flake dispositions)** closed three live rows, not the two the task breakdown
named: a third (review 507's own guessed-rather-than-verified entry) was found still unverdicted.
Two were fixed with an identified causal mechanism — a missing `ETXTBSY` warmup guard, and a poll
condition weaker than its own assertion — and one accepted as genuinely unreproducible after a
second independent investigation found no actionable cause. `test-process-leak.md` carries zero
live rows after this pass.

**PR-029-B (the CI job)** built `.github/workflows/ci.yml`, proven failing on a deliberately broken
commit and green again after a revert, through a real throwaway PR rather than read and agreed to
look right — and found a genuine first-run failure before any deliberate breakage: a clean
`target/` does not build the `[[bin]]` helper binaries several tests spawn at runtime. **It is
advisory, not blocking**: branch protection remains an unconfigured repository setting, left to the
owner rather than decided unilaterally here.

**PR-029-C (release artifacts and checksums)** built `.github/workflows/release.yml`, verified live
against a throwaway draft release — built, staged, uploaded, downloaded back, checked against its
own checksum, then deleted. **Measured, not assumed, non-reproducible**: two independent `--locked`
builds of the same tagged source produced two different binaries, byte-for-byte, which is why every
surface naming the checksum says only *verified against its published checksum*.

**PR-029-D (the one-liner installer)** built `install.sh`, with its truncation safety proven by
truncating it at eight separate cutoff points — zero files created at any of them — and a checksum
mismatch proven to refuse and install nothing. A required correction (review 547) found the
script's own header overstated "one path" when it meant "one binary": `mkdir -p` on the destination
directory is part of installing to one location, not a second path.

**PR-029-E (AUR and the install rewrite)** packaged a `PKGBUILD` building from the published crate,
and found a real, 100%-reproducible-under-`makepkg` defect while doing it: this distro's global
`-flto=auto` reaches the `cc` crate's bundled-`sqlite3.c` build, producing LTO-bytecode objects
`rust-lld` cannot resolve. Fixed by scoping the `CFLAGS` strip to that one build. `README.md`'s
*Quick Start* and `getting-started.md`'s *Install* both now lead with the one-liner, with
`cargo install` and a five-line verified form following as stated equivalents. **Held off `main`
until the first real asset existed, per review 548**, then released under D15 below.

**PR-029-F (the landing page)** added the platform and no-screen-reader statements to
`docs/src/introduction.md` — the book's own root, published as `index.html` — word for word what
`README.md` already says, and added the page to the release checklist's book-against-the-changelog
sweep in the same slice that created it.

**D15, decided after acceptance**, replaced the plan of authorising a one-off `workflow_dispatch`
against `0.36.0` to produce an asset for PR-029-E's pages to point at: that sequencing error is
removed rather than granted authorisation, and `0.37.0`'s own tag produces the first artifact
instead. One consequence the decision did not itself state: merging the held pages into `main`
*before* the candidate existed (rather than with it, as D15 intended) left `main`'s README
advertising a working install for a window longer than the "minutes" D15 accepted. Fixed by adding
one sentence to both pages stating the prebuilt binary is published from `0.37.0` onward — true
before the tag and true after it, so nothing needs removing later.

**D1's own second sentence was never built, found at review 553 after this RFC had already
closed.** "A quarantined test still runs — in a separate, non-blocking job that is read, not
ignored" describes a job PR-029-B did not ship: `ci.yml` has exactly one job, and nothing runs the
tests `test-process-leak.md` quarantines except a developer asking for them by name with
`-- --ignored`. PR-029-A's own evidence deferred the job to PR-029-B explicitly; PR-029-B's
evidence never mentions it again, and the acceptance checklist's own box for it
(`acceptance-qa-checklist.md:16`) was ticked against by neither slice before the candidate review
caught it. **Not a slice of this RFC to build now** — the population question (most of this
workspace's dozen `--ignored` tests are measurements, not flake quarantines, and reading all of
them would recreate D1's own failure mode on the job's first day) is real design work, reserved as
**RFC-072**, not a patch to land under a closed RFC's own number.
