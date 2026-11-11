# Fixtures

Reusable, large or slow-to-produce test fixtures live here — a previous release's build, a
real downloaded asset, anything expensive enough that rebuilding or re-fetching it for every
test run would be the thing that makes a test slow, rather than the test itself.
`.git-exclude/rules/project-instructions-gui-common.md`'s Test Optimization Rules name this
folder: "reusable fixtures... in the project's git-ignored fixtures folder... built or fetched
once and recorded with their checksums, never outside the project."

## What goes here, and what does not

A fixture is **built or fetched once** and then reused across runs — not regenerated per test,
not downloaded per CI job. If a test can produce its own input cheaply (a fixture project with
one file, a short literal string), build it inline instead; this folder is for the cases where
that is not true.

Not here: anything a test could instead construct in a few lines, and anything containing real
user data, a real path, or a real project name — fixtures are committed-adjacent (their checksums
are tracked) and must be as safe to describe in a commit message as any other test asset.

## The files themselves are git-ignored; their checksums are not

This folder's content is **not** committed — see `.gitignore`'s entry for this directory. What
*is* committed is this README and `checksums.sha256`, so a fixture without a recorded checksum
does not pass review: an unchecksummed fixture is an unversioned dependency, and nothing stops it
drifting out from under every test that reuses it without anyone noticing.

**Convention:**

1. Build or fetch the fixture into this directory.
2. `sha256sum <fixture-file> >> checksums.sha256` (append, do not overwrite the whole file).
3. Record, in the same commit, where the fixture came from and how to reproduce it — the exact
   build command, or the exact URL and release tag it was fetched from — as a line in
   `checksums.sha256`'s own comment block, not only in a commit message that a later reader has to
   go find.
4. A test that reuses a fixture verifies its checksum against `checksums.sha256` before trusting
   it, and regenerates (or refuses with a clear message) on a mismatch — never silently uses a
   fixture that has drifted.

## On a fresh clone

This folder exists but is empty (only this README and `checksums.sha256` are tracked). The first
test that needs a fixture not yet present must build or fetch it before using it, following the
same convention above, rather than assuming CI or a previous session already populated it.
