---
title: "Release record — 0.38.0"
status: "Published 2026-11-01. Tagged `0.38.0` at `ae17775` and published from that commit."
---

# `0.38.0` — macOS Support, And What It Does Not Claim

The release that makes this a two-platform product, and says exactly how far that goes. RFC-028,
six slices, second of M14 — and the first release whose own notes name a gate it did not pass.

## What this release is

A macOS build that is built and tested on every change on real Apple Silicon, a published
`aarch64-apple-darwin` artifact beside the Linux one, and three front-page sentences rewritten
from "Linux only. There is no evidence for any other platform" to what is actually true.

## Published

`cargo publish --locked`, core first: `tekstide-core 0.38.0`, then `tekstide 0.38.0`. The app's own
verification build downloaded `tekstide-core 0.38.0` from the registry and compiled against it.
Both confirmed present on the crates.io index before the tag was pushed, not inferred from
cargo's own exit status. Tagged **`0.38.0` at `ae17775`** (annotated, signed, same key as every
previous tag), published from that commit, and the tag pushed only after both crates were on
crates.io — so a tag exists only for a release that actually published.

**`release.yml`'s first live run with a second platform succeeded** (run `41257710555`, `artifacts`
4m13s then `artifacts-macos` 3m41s), publishing six assets: `tekstide-x86_64-linux` and
`tekstide-aarch64-macos`, each with its own `.sha256`, plus `NOTICE` and `LICENSE`.

**Both `--version` smoke steps ran for the first time ever** (added at review 653, never executed
before this tag) and both printed `tekstide 0.38.0` — the Linux one at `00:44:00`, the macOS one at
`00:47:42`. The macOS line is the **first execution of a Tekstide binary on macOS by anything other
than a test**: the exact staged bytes that were then uploaded.

## Verified after publishing

- **Downloaded, not listed.** `gh release download 0.38.0` into an empty directory;
  `sha256sum -c` **OK** for both `tekstide-x86_64-linux` and `tekstide-aarch64-macos`. Because the
  macOS checksum covers the same bytes the runner executed in its smoke step, the chain *the bytes
  that ran are the bytes a user downloads* is closed for macOS without any Apple hardware here.
- `./tekstide-x86_64-linux --version` from the downloaded file: `tekstide 0.38.0`.
- `file` on the downloaded macOS artifact: `Mach-O 64-bit arm64 executable` — the right thing
  under that name, checked rather than assumed from the filename.
- **`./tekstide-aarch64-macos --version` on a Mac: not performed.** No Apple hardware is available
  to this project. This is RFC-028 *Acceptance* option (2), taken by the owner 2026-10-31, and the
  release notes carry its label rather than leaving the gap silent.

`post-publish-check.sh` **PASS** for `0.38.0` **and** `0.37.0`, both including the real
`cargo install` build — the second is the one that matters, since a new core can only break an
old app and only the old app shows it.

**The documented one-liner resolves on both platforms now.**
`releases/latest/download/tekstide-x86_64-linux` and `…/tekstide-aarch64-macos` both return
`200`. `install.sh`'s Darwin branch had been live on `main` since review 653 with no asset behind
it; this release is what closes that window.

## The gate this release did not pass, and says so

**Run It was performed on Linux only.** Against this commit's own `--release` binary, with a
throwaway state and config home: a clean empty Project Board, every label naming a real action,
screenshot captured and read. On macOS, Run It is a human display-attached gate and nobody here has
the hardware; what stands in its place is the full suite on a real Apple Silicon runner, a rendered
cold-start window capture from a real window server, and the artifact's own `--version` above.

That is less than Linux has, and the release notes say so in those terms rather than implying
parity.

## Known state shipped deliberately

- **A five-member `proc_exit`/`P_WEXIT` intermittent cluster on macOS** (`test-process-leak.md`).
  Two of its members are among the seven macOS-conditional ignores the release notes name. The
  fifth was found during this very cut and its mechanism is now pinned to one line —
  `is_gone_or_going`'s early return on a successful `proc_pidinfo` call
  (`runtime/terminal/launch.rs:1351-1366`), which makes libproc authoritative over `P_WEXIT` even
  when `P_WEXIT` is set and available. Reserved as its own post-tag subject: production containment
  code is not something to change on the eve of a release.
- **`cargo audit`: 0 vulnerabilities, 7 allowed warnings.** One was fixed rather than accepted
  during the cut (`tokio` 1.53.1 → 1.54.1, `RUSTSEC-2026-0386`, reachable and patched upstream);
  `clipboard-win 5.4.1` is yanked but lock-only and Windows-gated. The pass condition for the next
  gate is zero vulnerabilities and exactly seven warnings.
- **The AUR is not updated, and cannot be matched yet.** Both repositories are registered, but
  neither `tekstide` nor `tekstide-bin` is published — the AUR RPC returns `resultcount 0` for both
  (checked against a control package to prove the query form). There is no `pkgver` to compare, so
  the standing watch is not discharged by this release; `aur-ci-publishing.md` holds the scope and
  schedule for doing it from CI.
- **Correction, 2026-11-07: the entry above conflated two different steps, and the second one was
  left undone.** The *watch* — "both AUR packages' `pkgver` match the version just released" —
  genuinely could not be discharged, for the reason stated: nothing is published, so there is no
  live `pkgver` to compare against. But the checklist's own **AUR Package** section carries a
  separate step that is not conditional on the package being published: after `cargo publish`
  lands, bump `packaging/aur/PKGBUILD`'s `pkgver`, reset `pkgrel`, replace `sha256sums` with the
  new tarball's sum, regenerate `.SRCINFO`, and prove it with a real `makepkg` and `namcap` run.
  That was not done. `packaging/aur/PKGBUILD` is still pinned at **`pkgver=0.37.0`** with 0.37.0's
  checksum, one release behind, which is exactly the condition reviews 561-562 added the watch
  for — reached this time through the repository file rather than the published page. It is
  unfinished 0.38.0 work, it is mine (I ran this cut), and it is not a defect in the released
  binaries: nothing in the product reads this file. Also recorded here because the owner asked
  about preparing for AUR publication: that preparation starts with this bump, not with the
  reserved CI RFC.
