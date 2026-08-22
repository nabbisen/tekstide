# Release Checklist

This checklist applies before creating a tag or package for a Tekstide release.

## Scope

- [ ] Confirm the intended release scope in RFCs or a release-scope decision record.
- [ ] Confirm README and changelog describe the same implemented and deferred scope —
      check **every** `readme` target named by a published crate's manifest, not only
      the workspace root. For this workspace that means `README.md` (`tekstide`, via
      `readme = "../../README.md"`) **and** `crates/tekstide-core/README.md`
      (`tekstide-core`, via its own `readme = "README.md"`). The per-crate page went
      two releases (`0.4.x`, `0.5.0`) without a check because this item didn't say
      which README, and it is the page crates.io actually renders for that crate.
- [ ] Confirm crate versions match the intended tag.
- [ ] Confirm future-work themes are preserved in the changelog or follow-up tracking.

## Corrections

- [ ] **A correction names the re-check, not only the correction.** If a release corrects a
      claim the product previously made, the entry must say what a reader who *relied on* the
      old claim should now do — not only what the truth is. Adopted 2026-08-19 after the snora
      team reported that a withdrawn claim of theirs had already propagated into three
      downstream projects' accessibility work, none of whom learned of the withdrawal from the
      release notes that withdrew it. Our own `0.11.1` transcript correction had the same gap
      and was amended retroactively.
- [ ] If the correction concerns data on a user's disk, say **where it is** and **what removes
      it**, in the entry itself — not only in the README it points at.

## Required Gates

- [ ] `git status --short` shows no unintended changes.
- [ ] `git diff --check`
- [ ] `cargo fmt --check`
- [ ] `cargo test --all-targets`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo build --release --locked`
- [ ] `cargo package -p tekstide-core --locked`
- [ ] `cargo package -p tekstide --locked --no-verify` — **`--no-verify` is required, not
      optional.** Without it this gate **cannot pass** for any release that adds
      `tekstide-core` API. The dependency is declared `version = "0"` (deliberately, for
      development cost), so packaging `tekstide` alone resolves core from crates.io — the
      *previous* release — and compiles the new binary against the old library. `0.10.0`
      produced 31 errors this way, every one of them naming API that release had just
      added. It is an artifact of the version range, not a defect, and the only thing
      this gate can honestly check is package **contents**. Pairing is checked by the
      workspace dry-run below, which is why that is the real gate. Do **not** "fix" a
      failure here by pinning the dependency to the current minor version — that trade
      was considered and rejected by the owner.
- [ ] `cargo publish --workspace --dry-run --locked`

For crates.io releases, use the workspace publish flow:

1. `cargo package -p tekstide-core --locked`
2. `cargo package -p tekstide --locked`
3. `cargo publish --workspace --dry-run --locked`
4. Publish with `cargo publish --workspace --locked`.

The workspace dry-run is the release-candidate gate for same-workspace dependency pairing. Individual package checks are still useful for package contents, but they are not the final publish-order model.

## Package Smoke

- [ ] Inspect generated package contents for missing README, license, Cargo manifests, and
      source files. **Name the files and check the archive, not the repository.** `LICENSE`
      and `NOTICE` live at the workspace root; cargo only auto-includes them from the
      *package* root, so for fourteen releases both published crates shipped without either
      — an Apache-2.0 §4 gap, since the licence and NOTICE are required to travel with the
      distribution. Copies now sit in `crates/tekstide/` and `crates/tekstide-core/` and
      must stay in sync with the root originals. The check is
      `tar tzf target/package/<crate>-<version>.crate | grep -iE 'LICENSE|NOTICE'`, and it
      must be run against the generated archive: this item existed and was ticked every
      time, because "license" read as "is the crate licensed" rather than "is the file in
      the tarball." Same failure shape as the README item above, which had to be amended
      for the same reason.
- [ ] Build or test from generated package artifacts rather than only the working tree.
- [ ] Confirm package output does not include `.git/`, `.git-exclude/`, local agent config, `target/`, or temporary state.
- [ ] Confirm crates.io package pages and README badges describe the intended release scope and do not overclaim the full AI CLI workbench.
- [ ] Any distributed prebuilt binary must ship with `NOTICE` alongside it. Releases assembled as project-structure tarballs satisfy this automatically because `NOTICE` sits at the archive root; a bare binary uploaded on its own does not.

## Standing Watches

Checked every release, because a watch that depends on someone remembering is not a watch.

- [ ] **Accessibility: has `iced` gained an accessibility bridge?** One command: `cargo tree -p tekstide | grep -i accesskit`. A non-empty result means RFC-014 R2 is reopenable and the owner should be told — screen-reader support is a real social need, not a nice-to-have, and the only reason it is out of scope is that the substrate offers no path to it. If the result is empty, tick this having *run* it, not having assumed it.
- [ ] **Public accessibility wording unchanged.** README and release notes say Tekstide has **no** screen-reader support. Not "limited", not "planned", not "partial".

## Review

- [ ] Create a release-candidate review request package.
- [ ] Include observed gate output summaries.
- [ ] Include known limitations and deferred themes.
- [ ] Receive an accepted review response before tagging.

## Tagging

- [ ] Tag name matches the release version.
- [ ] Tag points at the reviewed release commit.
- [ ] Post-publish/post-tag package or artifact verification is recorded.
