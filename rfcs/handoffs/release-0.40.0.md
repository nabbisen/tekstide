---
title: "Release record — 0.40.0"
status: "Published 2026-11-14. Tagged `0.40.0` at `815cbc8` and published from that commit."
---

# `0.40.0` — Locales That Survive Installation

RFC-073 closes in full: every locale this product ships now compiles into the binary and resolves
correctly wherever that binary runs, rather than depending on a path that only ever existed on the
machine that built it. The first release where the pluralization proof in `pl.ftl` has actually
been demonstrated rendering on the artifact a user receives, not merely on a checkout.

## What this release is

Every non-`en` catalog (today, only `pl`) is now `include_str!`'d in and selected by name at
runtime, with the disk-loading path and its directory parameter deleted rather than kept as a
fallback — the GitHub release downloads, the macOS artifact, the AUR `tekstide-bin` package, and a
plain `cargo install` all resolve it the same way for the first time. Alongside it: RFC-074's first
slice (a measured gate over `tekstide-core`'s own module graph, with `ProjectId` moved to the
vocabulary tier and `test_support` taken out of the production graph, the gate's bound ratcheting
10 → 9 → 8), and a shared fix for a cargo-toolchain test-helper path that broke six independent
copies at once when cargo 1.100.0 moved where unit-test binaries live. Neither of the latter two is
user-visible.

## Published

`cargo publish --workspace`, core first: `tekstide-core 0.40.0` (crates.io,
`2026-11-14T12:21:07Z`) then `tekstide 0.40.0` (`12:21:11Z`, four seconds later) — both confirmed
present on the crates.io index directly (`https://crates.io/api/v1/crates/<name>/0.40.0`), not
inferred from `cargo`'s own exit status. Tagged **`0.40.0` at `815cbc8`** (annotated, signed, same
key as every previous tag — the commit that wrote both RFC-073's and RFC-074's Final Acceptance
Decisions into their own handoff packs, immediately before tagging), published from that commit.

**`release.yml` run `43594409185`**: `artifacts` (Linux) 4m22s, `artifacts-macos` 3m46s, both
success. Both `--version` smoke steps (the staged artifact, before upload) printed
`tekstide 0.40.0` — the Linux one at `12:25:41Z`, the macOS one at `12:29:30Z`.

## Verified after publishing

- **Downloaded, not listed.** `gh release download 0.40.0` into an empty directory;
  `sha256sum -c tekstide-x86_64-linux.sha256` → **OK**. `tekstide-aarch64-macos`'s own
  `sha256sum`, computed directly against the downloaded bytes, matches its `.sha256` file's
  content exactly (no `shasum` on this platform; the comparison is the same one that tool would
  make).
- `./tekstide-x86_64-linux --version` from the downloaded file: `tekstide 0.40.0`.
- `file` on the downloaded macOS artifact: `Mach-O 64-bit arm64 executable` — the right thing
  under that name, checked rather than assumed from the filename.
- **`./tekstide-aarch64-macos --version` on a Mac: not performed.** No Apple hardware is
  available to this project — the same gap `0.38.0` and `0.39.0` disclosed, extended explicitly
  to this release in `CHANGELOG.md`'s own Status paragraph rather than left silent.
- `rfcs/handoffs/post-publish-check.sh 0.40.0` **PASS** and `rfcs/handoffs/post-publish-check.sh
  0.39.0` **PASS** — both including the real `cargo install --version <v>` build. The second is
  the one that matters: a new core can only break an old app, and only the old app shows it.
- **The documented one-liner resolves on both platforms.**
  `releases/latest/download/tekstide-x86_64-linux` and `…/tekstide-aarch64-macos` both resolve
  (through GitHub's redirect) to `200`.
- `NOTICE` downloaded from the release asset names the versions actually shipped:
  `rusqlite 0.42.0 and libsqlite3-sys 0.40.0`, bundled SQLite `3.54.1` — unchanged from `0.39.0`,
  since this release's own `cargo audit` run found no new vulnerability to fix (see below).

## The gate this release did not pass, and says so

**Run It was performed on Linux only**, through the mandatory launcher's `--no-project` mode
(the same mode built for `0.39.0`'s own candidate round) — a genuinely empty argv, not merely an
empty directory passed as a project. Clean first screen, every label naming a real action; a
second launch under `LANG=pl_PL.UTF-8` against the same release binary rendered real Polish text
(`9 zablokowanych automatyzacji`, the `[many]` CLDR plural form `pl.ftl` exists to prove) — both
screenshots and the launch commands in full are in review 676's own request and in
`.git-exclude/release-evidence/0.40.0/`. On macOS, the same gap `0.38.0` and `0.39.0` disclosed:
Run It is a human display-attached gate and nobody here has the hardware; what stands in its
place is the full suite on a real Apple Silicon runner (CI's `macos-measurement` job, including
the standing `trap '' HUP` containment tests `0.38.0` pinned) and the artifact's own `--version`
smoke test above. `CHANGELOG.md`'s own Status paragraph says so directly this release, rather
than leaving the extension to a reader who has to go find an earlier entry.

## Known state shipped deliberately

- **`cargo audit`: 0 vulnerabilities, 9 allowed warnings** — `paste`, `pin-utils`, `ttf-parser`,
  `zbus_names`, `zvariant`, `zvariant_derive`, `lru`, `async-executor`, `clipboard-win` —
  identical to `0.39.0`'s own tag, matching `rfcs/handoffs/dependency-advisories.md`'s own
  2026-11-12 entry exactly. No new warning this cycle, nothing retired; the register was not
  edited because there was nothing to change in it.
- **AUR first publication is still in progress, not complete — unchanged from `0.39.0`'s own
  record.** Checked directly against the AUR's own RPC rather than assumed:
  `https://aur.archlinux.org/rpc/v5/info?arg[]=tekstide&arg[]=tekstide-bin` still reports
  `resultcount: 2`, both still at the placeholder `Version: 0.0.0-1` with **`Maintainer: null`**.
  The two adoption requests filed 2026-11-12 have not yet been granted by an Arch Package
  Maintainer — a third party on their own schedule, not something either this release or the
  owner's own account can push past. The standing AUR `pkgver` watch stays undischarged until
  that changes.

## Boundaries respected writing this record

Did not touch `packaging/aur/*`. Did not `cargo publish`, tag, or create the GitHub release —
all three were the owner's own actions, verified independently here rather than assumed. This
record and the five-document Status-flip (`CHANGELOG.md`, `rfcs/README.md`'s two tables,
`rfcs/delivery-plan.md`, and both RFC-073/RFC-074 handoff packs' own `source_rfc_status` fields)
are the only things this commit changes.
