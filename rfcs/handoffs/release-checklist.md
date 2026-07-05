# Release Checklist

This checklist applies before creating a tag or package for a Tekstide release.

## Scope

- [ ] Confirm the intended release scope in RFCs or a release-scope decision record.
- [ ] Confirm README and release notes describe the same implemented and deferred scope.
- [ ] Confirm crate versions match the intended tag.
- [ ] Confirm future-work themes are preserved in release notes or follow-up tracking.

## Required Gates

- [ ] `git status --short` shows no unintended changes.
- [ ] `git diff --check`
- [ ] `cargo fmt --check`
- [ ] `cargo test --all-targets`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo build --release --locked`
- [ ] `cargo package -p tekstide-core --locked`
- [ ] `cargo package -p tekstide --locked`

## Package Smoke

- [ ] Inspect generated package contents for missing README, license, Cargo manifests, and source files.
- [ ] Build or test from generated package artifacts rather than only the working tree.
- [ ] Confirm package output does not include `.git/`, `.git-exclude/`, local agent config, `target/`, or temporary state.

## Review

- [ ] Create a release-candidate review request package.
- [ ] Include observed gate output summaries.
- [ ] Include known limitations and deferred themes.
- [ ] Receive an accepted review response before tagging.

## Tagging

- [ ] Tag name matches the release version.
- [ ] Tag points at the reviewed commit.
- [ ] Post-tag package or artifact verification is recorded.
