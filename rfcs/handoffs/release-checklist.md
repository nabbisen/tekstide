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

## Required Gates

- [ ] `git status --short` shows no unintended changes.
- [ ] `git diff --check`
- [ ] `cargo fmt --check`
- [ ] `cargo test --all-targets`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo build --release --locked`
- [ ] `cargo package -p tekstide-core --locked`
- [ ] `cargo package -p tekstide --locked`
- [ ] `cargo publish --workspace --dry-run --locked`

For crates.io releases, use the workspace publish flow:

1. `cargo package -p tekstide-core --locked`
2. `cargo package -p tekstide --locked`
3. `cargo publish --workspace --dry-run --locked`
4. Publish with `cargo publish --workspace --locked`.

The workspace dry-run is the release-candidate gate for same-workspace dependency pairing. Individual package checks are still useful for package contents, but they are not the final publish-order model.

## Package Smoke

- [ ] Inspect generated package contents for missing README, license, Cargo manifests, and source files.
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
