---
title: "AUR publishing from CI: what it must do, what it must never do, and what stays the owner's"
status: "Scope note. Opened 2026-10-21, after the owner registered both AUR packages. Not an RFC yet; it takes a number when the document is written."
rfc_file: "none — the RFC is not written yet, so it has no number (see §Why this is an RFC)"
target_milestone: "M14"
created: "2026-10-21"
---

# AUR publishing from CI

**What changed.** The owner registered two AUR packages on 2026-10-21 and stated the expectation that
they be integrated with GitHub Actions CI:

- `https://aur.archlinux.org/packages/tekstide` — the source package. `packaging/aur/PKGBUILD`
  already builds it, from the **crates.io** tarball (RFC-029 PR-029-E, D6), and the release checklist
  already carries its bump, `makepkg`, `namcap` and version-check steps.
- `https://aur.archlinux.org/packages/tekstide-bin` — **new, and nothing in this repository builds it
  yet.** It needs its own `PKGBUILD` against a published Linux release asset.

This note is the scope, the constraints and the schedule. It is deliberately not a workflow: the
credential and the publishing surface are new, so the design gets reviewed as its own **RFC** before any
job can push -- and it takes a number when that document is written, not now.

## Why this is an RFC and not a chore

It adds a write credential for a public package repository to CI. Nothing else in this project has
one: `cargo publish` and the AUR push are both the owner's own actions today, and the checklist says
so in as many words — *"Nothing here publishes to the real AUR. That is the owner's own account and
action."* Moving either into CI changes who can cause a publish, which is a security boundary, not a
convenience.

## What the automation must do

1. **Trigger on a published release, not on the gate.** Both packages depend on artefacts that do not
   exist until after the release: the source package needs the crates.io tarball, `-bin` needs the
   uploaded Linux asset. `release: published` (or a tag push on the default branch), never
   `pull_request`.
2. **Compute every checksum from the published artefact**, downloaded in the job. Never from a local
   build, and never carried forward from a previous version.
3. **Regenerate `.SRCINFO`** with `makepkg --printsrcinfo`, in an `archlinux:base-devel` container.
   A hand-edited `.SRCINFO` that disagrees with its `PKGBUILD` is the most common AUR breakage.
4. **Keep the checklist's bar, not a lower one.** A real `makepkg --noconfirm -f` build for the
   source package; for `-bin`, unpack and run `usr/bin/tekstide --version` and confirm it reports the
   released version; `namcap` on both the `PKGBUILD` and the built package. If CI cannot do what the
   human step already does, it is not a replacement for it.
5. **Fail closed.** Any step failing must leave the AUR untouched. No partial pushes, no "push and
   validate after".

## What it must never do

- **Never run on `pull_request`, and never expose the key to a workflow a fork can trigger.**
- **`permissions: contents: read`**, and the SSH key in a GitHub **Environment with the owner as a
  required reviewer**, so a push cannot happen without a human approving that specific run. This is
  the whole point: automation removes the typing, not the authorisation.
- Never force-push, and never rewrite AUR history.
- Never publish a `pkgver` that is not already live on crates.io (source) or attached to the release
  (`-bin`). An AUR package pointing at a URL that 404s is worse than a stale one.

## `tekstide-bin`, specifically

It does not exist yet and has two correctness requirements beyond the obvious:

- `provides=('tekstide')` and `conflicts=('tekstide')`, so the two packages cannot both be installed.
  Without these, AUR's own guidelines are violated and a user can end up with two `usr/bin/tekstide`.
- `depends` lists **runtime** libraries only, and no `cargo`/`rust` build dependency — it builds
  nothing. Whatever the Linux release asset actually links against is what belongs there, read from
  the asset (`ldd`), not guessed from `Cargo.toml`.

## What stays the owner's, and cannot be delegated

- Creating the AUR SSH key, adding its public half to the AUR account, and adding the private half as
  the Actions secret. Nobody else can, and nobody else should.
- Approving each publishing run, if the Environment-reviewer design above is adopted.
- Accepting that RFC once it is written.

## Schedule, and what happens in the meantime

**It is drafted after the capability-boundaries RFC**, which is itself after RFC-028. The reason is not priority but
sequence: that work moves modules between crates, and a packaging change that pins paths and feature
flags across that move would be written twice.

**Until it lands, publishing stays manual, and the gap is covered by the checklist** — including the
version-match watch added today, which exists because this project has already shipped with the AUR
package a release behind (reviews 561-562). That watch is cheap and stays useful whether or not the
automation ever lands.
