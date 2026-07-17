# Release Checklist

This checklist applies before creating a tag or package for a Tekstide release.

## Scope

- [ ] Confirm the intended release scope in RFCs or a release-scope decision record.
- [ ] Confirm README and changelog describe the same implemented and deferred scope.
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

## Review

- [ ] Create a release-candidate review request package.
- [ ] Include observed gate output summaries.
- [ ] Include known limitations and deferred themes.
- [ ] Receive an accepted review response before tagging.

## Tagging

- [ ] Tag name matches the release version.
- [ ] Tag points at the reviewed release commit.
- [ ] Post-publish/post-tag package or artifact verification is recorded.
