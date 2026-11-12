---
title: "AUR publishing from CI: what it must do, what it must never do, and what stays the owner's"
status: "Scope note. Opened 2026-10-21, after the owner registered both AUR packages. Not an RFC yet; it takes a number when the document is written. Corrected 2026-11-12: those packages were booked with placeholder content and deleted, so neither exists and neither name is held; first publication is deferred to 0.39.0."
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

**Corrected 2026-11-12: neither package exists on the AUR, and neither name is held.** Both were
created on 2026-10-21 with placeholder content, to book the names, and were subsequently deleted --
so "registered", above, describes an intent the AUR does not honour. Verified rather than inferred:
the AUR's own RPC returns `resultcount: 0` for both names
(`rpc/v5/info?arg[]=tekstide&arg[]=tekstide-bin`), a name search for `tekstide` returns nothing, and
the same call with a package that certainly exists (`yay`) returns it -- so the zero is a real
absence and not a broken probe. **The AUR has no name reservation**: a package exists only while
something is pushed to it, and deleting it frees the name for anyone. The booking also sat outside
the rules of submission, which require that an upload be *useful* and install "general software and
software-related content"; deleting was the right resolution and that part is closed.

**What survives the deletion is the git history**, which is the part that costs something later. The
submission guidelines state that a deleted package's repository "typically remains available", that
a clone of a deleted `pkgbase` is *not* empty, and that one must "pull and rebase to resolve
conflicts if `pkgbase` matches a deleted package". So the first real push to either name is a rebase
onto the booking commits, not a push to an empty repository -- and because this note already rules
out force-pushing and rewriting AUR history, those commits stay in both packages' public history
permanently. They carry whatever global git identity made them, which the guidelines warn is "very
difficult to change after pushing"; `git config user.name`/`user.email` per clone is the knob, and it
only helps for commits not yet made.

**First publication is deferred to 0.39.0. Decided 2026-11-12.** Publishing both at 0.38.0 was
considered and declined: it is technically correct today -- 0.38.0 is live on crates.io
(`53d88b44191ef911b50f54590a1122d75dc835cdf7e3e7e02035df1a5c0c77c8`, derived by the same method that
reproduces the committed 0.37.0 sum exactly) and its Linux assets exist -- but it costs two full
`makepkg`/`namcap` passes over the source package instead of one, and the first would be superseded
within a single release. Nothing depends on the packages existing: not the release, not crates.io,
not the GitHub assets, and no gate in `release-checklist.md` requires them at a particular version.
The exposure accepted by waiting is that both names stay unclaimed until 0.39.0 ships -- judged small
for a name this unusual, and recoverable through a merge or orphan request if it is not.

**Measured 2026-11-12, at the publication itself — a deleted package cannot simply be pushed back,
and this is the single most important fact in this note.** The AUR's own hook refuses: *"tekstide
was deleted; pushing does not restore it."* The sequence is `restore` (it returns as an **orphan**,
with its old content publicly visible again), then `adopt`, **which only files a request** — *"a
Package Maintainer must grant it before you become the maintainer"* — and until that grant lands a
push is refused outright. So a first publication after a deletion depends on a third party outside
this project, on their schedule, and **cannot be planned inside a release window**. Both requests
for `tekstide` and `tekstide-bin` were filed on 2026-11-12, after `0.39.0` was published and both
packages were built, `makepkg`-proven and `namcap`-clean locally.

**This is also what any CI design must account for.** A workflow that assumes "push to AUR" is one
step is correct only for a package that already exists and is already maintained by the pushing
account. It has nothing to say about restoring, adopting, or waiting on a grant — and it should not
try to: those are account actions, not packaging ones.

**The booking commits are dated 2026-10-20, not 2026-10-21** as this note's own header says — read
from the commits themselves (`8e6ff9d`, `44ab7b3`, both `+0900`). Both are authored as
`nabbisen <19984221+nabbisen@users.noreply.github.com>`, which the real publication commits match
deliberately, and both are harmless `pkgver=0.0.0` placeholders whose `package()` echoes
"Placeholder". They stay in both packages' permanent public history.

**Two checks belong before the cut rather than in it**, and both are read-only: `ssh
aur@aur.archlinux.org help`, which is the only thing that proves the public half of the key is on the
account, and a clone of both repositories to see the booking history for what it is. The key behind
`AUR_SSH_PRIVATE_KEY` was created on 2026-10-29, *after* the bookings, so the key CI holds is not
known to be the key that pushed them, and a local push may use a third.

**`tekstide-bin` is no longer unbuilt**, as the bullet above has it: its `PKGBUILD` landed on
2026-11-10 (`d79cd98`), pinned to 0.38.0 as a verification baseline and proven end to end against
that release's real assets. Its own header comment carries the bump instruction for the cut.

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

## The host key is pinned, in a committed file, not in a secret

`ssh` on a fresh runner has no `known_hosts` entry for `aur.archlinux.org`, so without one the
workflow must either fail to connect or be told `StrictHostKeyChecking=no` -- "trust whatever
answers on that name", with no prior trust to fall back on because the runner is new every run.

**The risk is content authenticity, not key theft.** A man-in-the-middle cannot obtain
`AUR_SSH_PRIVATE_KEY`: public-key auth never transmits it and the signature is bound to the session.
What an unpinned host buys an attacker is the `git push` negotiation -- it can present a history we
then build our commit on top of, and we would push the result to the real AUR under the owner's
name.

**So pin it, and put it in `packaging/aur/known_hosts`, committed.** A server's *public* host key is
public data; filing it as an Actions secret hides from every reviewer the one value they would want
to watch for changes. In the repository it is reviewable in a diff, its history is visible, and if
AUR rotates the key the workflow fails closed and the file is updated deliberately. Use it with
`GIT_SSH_COMMAND="ssh -o UserKnownHostsFile=packaging/aur/known_hosts"`.

Decided 2026-10-29, after the owner asked why a second secret was needed: it was not. The first
design asked for `AUR_SSH_KNOWN_HOSTS` as a secret, which was the wrong container for public data.

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
  the Actions secret. Nobody else can, and nobody else should. **Done, 2026-10-29:**
  `AUR_SSH_PRIVATE_KEY` is configured. Make it an AUR-only key, not a general one, so revoking it
  costs nothing else.
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
