---
title: "Release record — 0.39.0"
status: "Published 2026-11-12. Tagged `0.39.0` at `e981de0` and published from that commit."
---

# `0.39.0` — Readable By Default

RFC-076 closes in full: D1 through D5, all implemented, the default theme held to WCAG AAA on
top of the enforced AA floor every configured theme still clears. The release that answers the
owner's own question — whether the view layout, typography and colour could be kinder to older
readers — everywhere the code can say yes.

## What this release is

A whole-UI scale factor (`Ctrl+Alt+1`-`5`/`0`) with a top-bar "Text size"/"Reset" control for a
reader who cannot read the chords that would otherwise find it; raised defaults (body 16 / status
16 / heading 20, amending external design §4.2's authored 14 px); the default theme verified
against WCAG AAA, with one unmeasured pair found and removed as dead code rather than tuned.
Alongside it: `TerminationOutcome::CommittedToExit`, a macOS file-watcher fix, two more rounds of
the `proc_exit`/`P_WEXIT` containment tracing started at review 656, and a real `cargo audit`
vulnerability found and fixed in the storage engine under the durable local audit store.

## Published

`cargo publish --workspace`, core first: `tekstide-core 0.39.0` (crates.io, `2026-11-12T11:22:49Z`)
then `tekstide 0.39.0` (`11:22:52Z`, three seconds later) — both confirmed present on the
crates.io index directly (`https://crates.io/api/v1/crates/<name>/0.39.0`), not inferred from
`cargo`'s own exit status. Tagged **`0.39.0` at `e981de0`** (annotated, signed, same key as every
previous tag — the commit that wrote RFC-076's Final Acceptance Decision into its own handoff
pack, immediately before tagging), published from that commit.

**`release.yml` run `43208760627`**: `artifacts` (Linux) 4m52s, `artifacts-macos` 2m42s, both
success. Both `--version` smoke steps (the staged artifact, before upload) printed
`tekstide 0.39.0` — the Linux one at `11:32:22Z`, the macOS one at `11:45:38Z`.

## Verified after publishing

- **Downloaded, not listed.** `gh release download 0.39.0` into an empty directory;
  `sha256sum -c tekstide-x86_64-linux.sha256` → **OK**. `tekstide-aarch64-macos`'s own
  `sha256sum`, computed directly against the downloaded bytes, matches its `.sha256` file's
  content exactly (no `shasum` on this platform; the comparison is the same one that tool would
  make).
- `./tekstide-x86_64-linux --version` from the downloaded file: `tekstide 0.39.0`.
- `file` on the downloaded macOS artifact: `Mach-O 64-bit arm64 executable` — the right thing
  under that name, checked rather than assumed from the filename.
- **`./tekstide-aarch64-macos --version` on a Mac: not performed.** No Apple hardware is
  available to this project — the same gap `0.38.0` disclosed, extended explicitly to this
  release in `CHANGELOG.md`'s own Status paragraph (review 671 Required 2) rather than left
  silent a second time.
- `rfcs/handoffs/post-publish-check.sh 0.39.0` **PASS** and `rfcs/handoffs/post-publish-check.sh
  0.38.0` **PASS** — both including the real `cargo install --version <v>` build. The second is
  the one that matters: a new core can only break an old app, and only the old app shows it.
- **The documented one-liner resolves on both platforms.**
  `releases/latest/download/tekstide-x86_64-linux` and `…/tekstide-aarch64-macos` both resolve
  (through GitHub's redirect) to `200`.
- `NOTICE` downloaded from the release asset names the versions actually shipped:
  `rusqlite 0.42.0 and libsqlite3-sys 0.40.0` — matching the fix this release's own
  `### Dependencies` entry records, not the pre-fix versions.

## The gate this release did not pass, and says so

**Run It was performed on Linux only**, through the mandatory launcher's new `--no-project` mode
(review 671's own candidate round) — a genuinely empty argv, not merely an empty directory
passed as a project, since the app's own usage text distinguishes the two. Clean first screen,
every label naming a real action, screenshot captured through the compositor and read, described
in the round's own review-request file. On macOS, the same gap `0.38.0` disclosed: Run It is a
human display-attached gate and nobody here has the hardware; what stands in its place is the
full suite on a real Apple Silicon runner (CI's `macos-measurement` job, including the standing
`trap '' HUP` containment tests `0.38.0` pinned), a rendered cold-start window capture, and the
artifact's own `--version` smoke test above. `CHANGELOG.md`'s own Status paragraph says so
directly this release, rather than leaving the extension to a reader who has to go find
`0.38.0`'s entry.

## Known state shipped deliberately

- **A real `cargo audit` vulnerability was found and fixed during this cut, not merely
  disclosed.** `RUSTSEC-2026-0459`/`CVE-2026-119375` (6.8 medium, the bundled C SQLite reached
  through `libsqlite3-sys`) — fixed by bumping `rusqlite` `0.40.2` → `0.42.0` (bundled SQLite
  `3.53.2` → `3.54.1`). `RUSTSEC-2026-0486` (`rustix`, unsound) — fixed, lockfile-only,
  `1.1.4` → `1.1.6`. **`cargo audit`: 0 vulnerabilities, 9 allowed warnings** — `paste`,
  `pin-utils`, `ttf-parser`, `zbus_names`, `zvariant`, `zvariant_derive`, `lru`, `async-executor`,
  `clipboard-win` — matching `rfcs/handoffs/dependency-advisories.md`'s own 2026-11-12 entry
  exactly. The pass condition for the next gate is zero vulnerabilities and exactly nine
  warnings.
- **AUR first publication is in progress, not complete.** Per `release-checklist.md`'s "First
  publication — 0.39.0 only" and the ordering it sets (gates → `cargo publish` → GitHub release
  assets → the two `PKGBUILD` bumps → the owner's two pushes): the gates, publish and release
  assets above are done; both `PKGBUILD` bumps (`tekstide` and `tekstide-bin`, each to `0.39.0`,
  real `makepkg`/`namcap` runs) are committed (`a26941a`, `6a41133`); the booking repositories'
  own restore/adoption sequencing is recorded (`8a3b016`). **The owner's own two pushes to the
  AUR have not happened yet** — checked directly against the AUR's own RPC rather than assumed:
  `https://aur.archlinux.org/rpc/v5/info?arg[]=tekstide&arg[]=tekstide-bin` reports
  `resultcount: 2`, but both still carry the placeholder `Version: 0.0.0-1`
  (`FirstSubmitted`/`LastModified` timestamps from today, 2026-11-12, consistent with a very
  recent restore of the booking history rather than a stale record) — not `0.39.0`. The standing
  AUR `pkgver` watch stays undischarged until that changes; not this record's job to push it.
