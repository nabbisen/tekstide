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

## Run It

- [ ] **Launch the built binary as a first-time user and look at the screen.** No arguments,
      a fresh `XDG_STATE_HOME` (`XDG_STATE_HOME=$(mktemp -d) ./target/release/tekstide`).
      **The release binary, not a debug build** — this gate exists to look at the artifact that
      ships. Confirm the first screen names **only actions that exist**, and that a user who has
      read nothing can tell what to do next.

      Reference for what a correct first screen looks like, and for how to record one:
      [`first-run-correction/evidence/cold-start-empty-board.png`](./first-run-correction/evidence/cold-start-empty-board.png)
      and its
      [sidecar](./first-run-correction/evidence/cold-start-empty-board.md), which states the
      launch command in full. That capture is a reference, taken from a debug build after
      `0.12.1` shipped; it does **not** discharge this gate for any release.

      **Every box in this file stays unchecked.** It is a per-release template: a ticked box
      here would assert "done" for every future release, which is the same
      state-asserting-text failure `ARCHITECTURE.md` records. Tick a copy, or record the run in
      the release's own evidence — never here. (Response 293 promised to tick this one on
      evidence landing; that promise was wrong and is withdrawn.)
- [ ] **Open the project you launch from a scratch directory, never a real one.**
      `mktemp -d`, a fixture file or two inside it, and nothing else. The Run It gate captures
      screenshots, this repository is public, and the close confirmation and Project Board both
      render a project's **canonical path** on purpose. `0.14.0`'s own gate run pointed the
      binary at a directory under `$HOME` and committed three screenshots carrying the
      operator's username and home layout; they were pushed before the owner caught it. Every
      screenshot committed before that had used a `/tmp` fixture — the practice was real and
      unwritten, which is exactly why it broke. See `ARCHITECTURE.md`, "A committed screenshot
      may only ever show throwaway state."
- [ ] **Release evidence goes in `.git-exclude/release-evidence/<version>/`, not in the
      repository.** A release's gate record is for the owner and the team, not for
      publication, and it is the document most likely to carry a real path or a real project
      name. Only per-RFC evidence that a fixture can produce belongs under
      `rfcs/handoffs/<rfc>/evidence/`.
- [ ] **`./target/release/tekstide --help`** prints usage and exits, rather than treating the
      flag as a path.

This section exists because nothing above it opens a window. Every other gate here checks
internal consistency — do the tests pass, does it compile, does the package contain the right
files — and `0.12.0` passed all of them while shipping a Project Board that rendered
"Add Project" and "Open from path" as inert labels for actions that do not exist, with nine
live keybindings named nowhere in the running application. That state shipped in **twelve
releases**, `0.4.0` through `0.12.0`, and was found by the owner running the program.

There was even a passing test over those exact strings: it asserted they resolved to real
catalog text rather than to the raw key. A correct test of the wrong property — the shape of
the claim, never its truth.

Ninety seconds. Run it.

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
