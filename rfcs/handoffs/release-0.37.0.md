---
title: "Release record — 0.37.0"
status: "Published 2026-10-14. Tagged `0.37.0` at `2e9e002` and published from that commit."
---

# `0.37.0` — Documentation, CI, and Release Automation

The release that stops `cargo install` being the only way in. RFC-029, six slices, first of M14.

## What this release is

A prebuilt `x86_64` Linux binary with a published checksum, a one-liner installer, an Arch
`PKGBUILD`, this project's first build/test CI job, and install documentation rewritten so the first
thing a visitor sees needs no Rust toolchain.

## Published

`cargo publish --workspace --locked`, core first. Tagged **`0.37.0` at `2e9e002`**, published from
that commit, and the tag pushed only after both crates were on crates.io — so a tag exists only for
a release that actually published.

`post-publish-check.sh` **PASS** for `0.37.0` and `0.36.0`, both including the real `cargo install`
build.

**`release.yml`'s first live run succeeded** (4m11s), and published four assets:
`tekstide-x86_64-linux`, its `.sha256`, `NOTICE` and `LICENSE`.

## The check this RFC existed to earn

**The real one-liner, against the real published artifact, on a machine with no Rust toolchain at
all.** Not approximated: `env -i` with a `PATH` containing symlinks to exactly the eight commands
`install.sh` uses, verified in that environment to resolve neither `cargo` nor `rustc`.

```
curl -fsSL https://raw.githubusercontent.com/nabbisen/tekstide/main/install.sh | sh
  downloading … / verifying against its published checksum / installed to $HOME/.local/bin/tekstide
```

- **Exactly three entries landed in `$HOME`**: `.local`, `.local/bin`, `.local/bin/tekstide` — the
  documented claim ("one binary, plus the directory chain leading to it") measured rather than read.
- The installed binary's SHA-256 **equals the published `.sha256`**, byte for byte.
- `tekstide --version` → `0.37.0`.
- `tekstide --licenses` prints `NOTICE` **byte-identically** (md5 `66b657d6…`), from the distributed
  artifact, through the distribution channel a user actually uses.

**The first attempt at this check was invalid and is recorded rather than quietly redone**: filtering
`cargo` out of `PATH` left `/usr/bin/cargo` reachable, so the condition the check exists to create
was never met. The second attempt failed for a different reason that was also the instrument — the
reviewer's shell aliases `mkdir`, so `command -v mkdir` returned the alias text and built a dangling
symlink, and the installer correctly reported `mkdir: command not found`. Both were the harness, not
the product; the third run is the one above.

## Held at the release step, then fixed

**The release was authorised, and I stopped before tagging.** `release.yml` would have uploaded a
bare binary — and `NOTICE` carries the full MIT text for `rusqlite`/`libsqlite3-sys`, whose terms
require it to travel with every copy. `cargo tree -e normal -i rusqlite` confirms a normal
dependency with `bundled`, so rusqlite and SQLite's amalgamation are statically linked into the
artifact. `cargo install` and the AUR `PKGBUILD` already carried the notice; the one-liner and the
release asset did not. The PKGBUILD getting it right in the same release is what made it an
oversight rather than a position.

Fixed by **embedding** rather than teaching each channel a second file: `tekstide --licenses`
(`include_str!`), plus `NOTICE`/`LICENSE` as release assets. Reviews 556 and 557 corrected two
things in that fix — a cross-reference naming a test that did not exist, in published source that
crates.io freezes, and an invariant that guarded four of the six licence copies.

## The gate ran in a fresh clone, and why

The working-tree gate went red **3 of 3** on
`real_repository_filesystem_scan_cost_headless_benchmark`, which asserts the default policy scans
*this repository* `Complete` — and means the directory it runs in. The scanner excludes `target/`,
`node_modules/` and most of `.git/` by name and nothing else, so `.git-exclude/` — this project's
review correspondence, gitignored, **4,345 entries**, 2,892 under `tmp/` — is walked and counted,
against a cap of 4,096. The same commit in a fresh clone scans **1,203** and passes. The line was
crossed by this release's own review rounds.

**No product defect**, checked rather than assumed: a bounded scan reaches the user as *"Partial —
the scan itself was bounded at { $limit } entries; more changes may exist and were never counted"*.
`release-checklist.md` now says to gate a release in a fresh clone, which is what CI checks out and
what a user downloads anyway.

Gate at the released commit, in that clone: **three consecutive green**, 0 fixture entries.

## Run It

Release binary, no arguments, fresh `XDG_STATE_HOME`, throwaway project. The first screen names only
actions that exist — `Audit`, help, `Browse…`, and a focused path field with *"Type a project path
and press Enter (Ctrl+V to paste)"* — and tells a reader who knows nothing what to do next, including
the command-line form. `--help` and `--version` behave. Capture in
`.git-exclude/release-evidence/0.37.0/`.

## Deferred, recorded rather than fixed

- **RFC-072** — D1's non-blocking job for quarantined tests, never built by any slice of RFC-029.
  Found at review 553, after the RFC had closed.
- **Flake register row 1** (`bind_recovers_from_a_stale_socket_file`) recurred on this candidate's
  gate, and its `Accept`'s "no actionable fix" premise no longer holds: `source: None` narrows it to
  the two arms of `clear_stale_socket` that response 231 left indistinguishable.
- **A new row**, `linux_runtime_terminates_session_leader_with_sighup`, 1 in 6 — with the family note
  that three real-process tests failed today under genuine multi-agent contention and none under
  synthetic load, with failures at load 16 and clean runs at 28. The untested variable is the load's
  *shape*, not its magnitude.
- **The window sets no Wayland `app_id`** (it is `""`; the title is `Tekstide`). Found during Run It,
  recorded nowhere before this. No desktop environment can match the window to a `.desktop` entry for
  its icon, grouping or window rules. True of every release so far, so not this release's regression.
- **Branch protection** on `main`, which is what would make `ci.yml` actually blocking, remains an
  owner-only repository setting. The job ships advisory and says so.
