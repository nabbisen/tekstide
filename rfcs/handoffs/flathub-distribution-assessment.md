---
title: "Flathub instead of the AUR: what the sandbox costs us, and the one rule that says who may write the manifest"
status: "**Assessment. Opened 2026-11-22, when the owner dropped AUR support and named Flathub as the alternative.** Not an RFC and not a plan: two findings below may change the decision, and both are the owner's to weigh. Supersedes [`aur-ci-publishing.md`](./aur-ci-publishing.md), which is withdrawn."
target_milestone: "M14"
created: "2026-11-22"
---

# Flathub instead of the AUR

The owner's decision, 2026-11-22: **AUR support is dropped; the alternative is to publish on
Flathub.** Recorded here because the tree planned the opposite in five places, and because two facts
measured while recording it bear on whether Flathub is reachable at all.

## Dropping the AUR costs no user anything, measured

**Nothing was ever published.** `aur-ci-publishing.md`'s own dated correction (2026-11-12): both
names were booked with placeholder content and then deleted, *"so neither exists and neither name is
held."* The release checklist's AUR section says the same in its own words — *"Neither package
exists on the AUR, and both procedures below are written as bumps."*

What the tree does carry, and what it says:

| place | what it claims | after this decision |
|---|---|---|
| `packaging/aur/{PKGBUILD,.SRCINFO}` + `tekstide-bin/` | build recipes, in-tree, buildable | **the owner's call** — see question 1 |
| `README.md:80`, `docs/src/users/getting-started.md:25` | points at `packaging/aur/`, *"not yet published to the AUR itself"* | **accurate today**; untouched |
| `CHANGELOG.md:136` (`0.40.0`) | names the `tekstide-bin` package as a channel the locale fix reaches | **a shipped entry, left alone** — the PKGBUILD exists and builds, so nothing in it is false |
| `rfcs/delivery-plan.md` Release Schedule | an *AUR Publishing From CI* row, reserved 2026-10-21 | **withdrawn**, with Flathub named in its place |
| `rfcs/handoffs/release-checklist.md` | a `pkgver` gate item and a whole `## AUR Package` procedure | **struck**, the procedure kept as history under a withdrawal notice |

No user-facing install path changes, no deprecation is owed to anyone, and the one-liner and the
GitHub release assets are unaffected.

## Finding 1 — Flathub may refuse this class of application, and the reason is our own premise

Flathub's requirements page states that applications including *"insecure or harmful design
choices, such as disabling or bypassing security mechanisms"* are rejected, and that this
**explicitly includes shipping overly permissive configurations**.

This product's whole function is to drive the AI CLI the user already installed, in a terminal, on
the user's own project directories. Measured against the sandbox:

- **Executable discovery looks at host paths.** `AgentProfile::claude_code_linux_from_env`
  (`crates/tekstide-core/src/agent/profile.rs:110`) looks in `$HOME/.local/bin`, then
  `/usr/local/bin`, then `/usr/bin`, each registered as an `ExecutableLookupPath::reviewed_system`.
  Inside a Flatpak every one of those resolves **inside the sandbox**: `/usr/bin` is the
  *runtime's*, and `$HOME` is `~/.var/app/<app-id>`. The user's `claude` binary is invisible, and —
  worse than invisible — a different binary may sit at a path this code calls *reviewed*. The
  security claim in that type's own name silently changes trust domain.
- **Reaching the host needs the documented escape.** Running a host binary from inside a Flatpak
  means `flatpak-spawn --host`, which requires `--talk-name=org.freedesktop.Flatpak`. Combined with
  the `--filesystem=` permission that project roots need, the sandbox becomes nominal: the real
  boundary would be this product's own approval and containment machinery, exactly as it is today
  outside a sandbox. **That is an honest position and it may still be rejected**, because what a
  reviewer sees is the permission list, not our approval dialog.
- **A plain terminal would hand the user the runtime's shell.** `launch_terminal`'s shell is
  hardcoded to `/bin/sh` by its one real caller (`crates/tekstide/src/shell.rs:4491`). In a sandbox
  that is the runtime's `sh`, with none of the user's tooling, dotfiles or `PATH`.

**So the question is not how to write a manifest. It is whether a sandboxed build of this product is
the product at all**, and if it is only the product with full host filesystem and host-spawn
permissions, whether Flathub accepts it. That answer should be obtained **before** any work, and the
cheapest way to obtain it is to ask Flathub, not to guess: their reviewers answer permission
questions on the submission PR, and the question can be asked without a submission.

### Measured, and not a blocker: the approval socket's path budget narrows

Worth recording because this project has already paid for `sun_path` once. The Linux approval
socket is `state_root/approval/<12-hex>.sock`, checked against a 107-byte usable `sun_path` before
`bind()` (`approval/channel.rs:955`). Under a Flatpak the state root moves from
`~/.local/state/tekstide` to a per-app path, which costs bytes:

| state root | 8-char username | 20-char username |
|---|---|---|
| `~/.local/state/tekstide` (today) | 63 bytes, margin **+44** | 75 bytes, margin **+32** |
| `~/.var/app/net.scqr.tekstide/data/tekstide` | 82 bytes, margin **+25** | 94 bytes, margin **+13** |
| `~/.var/app/net.scqr.tekstide/.local/state/tekstide` | 90 bytes, margin **+17** | 102 bytes, margin **+5** |

It still fits, and the existing check fails closed rather than binding a path that cannot be
connected to — the failure mode review 113 found and closed. But the margin falls from +44 to +5 in
the worst row, **the app ID's own length subtracts directly from it**, and
`ApprovalChannelErrorReason::SocketPathTooLong` moves from theoretical to something a real user with
a long username could meet. If Flathub is pursued, a short app ID is a design constraint, not a
naming preference.

## Finding 2 — Flathub's own policy says the manifest may not be written by an AI

The same requirements material states that **Flathub manifests must not contain AI-generated or
AI-assisted content**, that disclosure does not exempt a manifest from this, and that the manifest
and the submission PR are to be written and opened by a human.

**This governs who may do this work, and it excludes me.** Both this reviewer and the implementing
team are AI agents. If that policy reads as reported, then:

- I must not write the manifest, the AppStream metainfo, or the submission PR — **not even a draft
  for the owner to edit**, because an edited draft is still AI-assisted content.
- What I *can* honestly contribute is what this document is: measurements of our own tree, the
  permission consequences, and the questions worth asking — none of which ships in the manifest.

**Verify this first-hand before planning anything.** I am relaying a search summary of
[Flathub's requirements page](https://docs.flathub.org/docs/for-app-authors/requirements), not a
page I read in full, and a rule that decides *who is permitted to author the artefact* deserves the
owner's own reading. If it holds, Flathub is a channel only the owner can open, whatever the rest of
this assessment says.

## What Flathub requires, for sizing only

From the same requirements page and the [MetaInfo guidelines](https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines),
to be re-read at the time by whoever is permitted to act on them:

- A reverse-DNS **app ID**, matching the manifest filename and the metainfo `id` exactly.
- An **AppStream metainfo** file that passes validation, with summary, description, an OARS content
  rating, a correctly declared license matching the source, and **screenshots**. Our screenshot
  discipline already covers the hard part: throwaway project *and* throwaway `XDG_STATE_HOME`, so no
  real path or recent-projects list leaks into a published image.
- **Offline builds.** No network during the build, so every dependency must be a source in the
  manifest with a public URL — for a Rust workspace that means vendoring, which interacts with the
  `--locked` discipline the release checklist already enforces.
- The runtime must be one Flathub hosts, at its latest version at submission.
- x86_64 and aarch64 by default; `flathub.json` only for a single-architecture app.
- Submission is a PR to the `flathub/flathub` repository; review is by Flathub maintainers.

**One genuine advantage over the AUR plan, and it is the reason the AUR plan was never scoped**: the
AUR row needed a write credential for a public package repository inside our CI, which was the risk
the owner was asked to accept. Flathub builds on **their** infrastructure from a manifest in
**their** repository, so no publishing credential of ours enters our CI at all. The security
objection that blocked the previous plan does not transfer.

## Questions that are the owner's

1. **Do the `packaging/aur/` PKGBUILDs stay or go?** They are accurate, buildable, and referenced by
   `README.md` and `docs/src/users/getting-started.md` as a build-it-yourself option that is
   explicitly *not* on the AUR. Dropping AUR *publication* does not make them false. Deleting them
   is a packaging change in your own area, and I have not touched them.
2. **Finding 2 first.** Read Flathub's requirements yourself on the AI-authorship rule. If it holds
   as reported, this channel is yours to open alone, and the most I can do is measure and ask.
3. **Finding 1 before any manifest.** Ask Flathub whether an application that spawns host commands
   and reads the user's project directories is acceptable with the permissions that requires. A
   "no" closes this honestly and cheaply; a "yes with conditions" is the scope.
4. **Is there a third option worth naming?** Nothing obliges this product to have a distro channel
   at all: `cargo install`, the one-liner and the GitHub release assets are what every user has
   actually used so far, and the locale fix in `0.40.0` was specifically about making all of those
   work. "No distro channel for now" is a position this document would record without complaint.

## What this does not do

It does not write a manifest, choose an app ID, or create any Flathub artefact — see finding 2. It
does not touch `packaging/aur/*`, which is the owner's area under the release checklist's own terms.
It does not change `README.md`, `docs/src/users/getting-started.md`, or any shipped CHANGELOG entry,
because each is accurate as it stands.
