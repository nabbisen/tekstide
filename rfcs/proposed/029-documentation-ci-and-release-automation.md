# RFC-029: Documentation, CI, and Release Automation

Status: **Proposed 2026-10-12.** `0.37.0`, M14. Reserved since the plan's first draft; scoped now
under the owner's M14 direction (2026-10-12): prebuilt binaries, AUR, and install documentation an
unskilled user can follow. D1–D8 and one open question are for the owner, on acceptance.

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
- **PR-029-D — AUR** (D6) and **the install rewrite** (D7): `README.md`'s *Quick Start* and
  `docs/src/users/getting-started.md`'s *Install*, with the verified form first.
- **PR-029-E — the landing page** (D8).

## Open question for the owner, to decide on acceptance

**Does this project offer a `curl … | sh` one-liner?**

The owner asked for one on 2026-10-08 (*"To offer one-liner shell installation method"*). The
analysis recorded at the time pushed back, and it still holds: a one-liner **executes remote code
with the user's privileges**, which sits badly beside a product whose posture is local-first,
trust-gated and audited, and whose own dialogs refuse to overclaim safety. What costs an unskilled
user today is the toolchain and the compile — and **prebuilt binaries remove that whether or not a
one-liner exists**.

**The saving, counted rather than asserted — one line against four.** The verified form reads:

```sh
curl -LO https://github.com/nabbisen/tekstide/releases/download/<version>/tekstide-x86_64-linux
curl -LO https://github.com/nabbisen/tekstide/releases/download/<version>/tekstide-x86_64-linux.sha256
sha256sum -c tekstide-x86_64-linux.sha256
install -m 755 tekstide-x86_64-linux ~/.local/bin/tekstide
```

**And one wrinkle that argues against my own recommendation, so it is stated here rather than
left out:** that fourth line assumes `~/.local/bin` is on `PATH`, which for the unskilled user this
RFC exists to serve is exactly the sort of thing that is not true and not obvious. A one-liner
handles it silently. So the honest comparison is four lines plus a `PATH` caveat against one line —
not four against one. The recommendation below still stands, but it stands on what the form
*executes*, not on how much typing it saves.

Three options:

1. **No one-liner.** Prebuilt binaries, checksums, AUR, and a short verified-download snippet. The
   most consistent with everything else this product says about trust.
2. **A one-liner, with the verified form shown first** and the convenience form second, labelled
   with what it does.
3. **A one-liner as the headline**, matching what most projects do.

My recommendation is **(1)**, and I want to be clear that this is a disagreement with the owner's
earlier instruction rather than a neutral presentation: I think a project that tells users it
cannot promise to stop a rejected command from running should not also ask them to pipe a URL into
a shell. If the owner still wants the one-liner, **(2)** is the version I would build, and I will
build it without further argument.
