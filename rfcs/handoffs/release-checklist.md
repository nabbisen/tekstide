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
- [ ] `cargo publish -p tekstide-core --dry-run --locked`
- [ ] `cargo publish -p tekstide --dry-run --locked`

For crates.io releases, run the crate checks in dependency order:

1. `cargo package -p tekstide-core --locked`
2. `cargo publish -p tekstide-core --dry-run --locked`
3. Publish `tekstide-core`.
4. `cargo package -p tekstide --locked`
5. `cargo publish -p tekstide --dry-run --locked`
6. Publish `tekstide`.

`tekstide` package verification requires `tekstide-core` to be available from the registry because Cargo removes the local path dependency during packaging.

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
- [ ] Tag points at the reviewed commit.
- [ ] Post-tag package or artifact verification is recorded.
