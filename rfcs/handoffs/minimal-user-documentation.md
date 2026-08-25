---
title: "Minimal user documentation — implementation handoff"
rfc: "none — pulled forward from RFC-029"
status: "Scheduled 2026-08-01, unstarted for three weeks; re-scoped 2026-08-25 and **pinned to commit 711be4b** the same day, after the re-scope was itself overtaken within hours. Items 1 and 2 struck as done, pending verification at the pin."
target_milestone: "M9, alongside RFC-017"
created: "2026-08-01"
---

# Minimal user documentation

**No RFC, deliberately.** This is documentation work, not design work — the same treatment the RFC-004 redaction gap and the `ProjectBoardRow` i18n item already have. The full `docs/src` mdBook by persona stays with RFC-029 at M14; this is the subset that stopped being deferrable.

## Why now, and not M14

`0.4.1` is installable from crates.io and starts a GUI. **There are now users who are not contributors**, and the repository has nothing addressed to them.

The sharpest instance, and the reason this got pulled forward:

```
## Quick Start

    cargo run -p tekstide
```

That is a *contributor* instruction — it runs from a checkout. Someone who ran `cargo install tekstide` has **no correct command anywhere in the repository.** They installed a GUI application and the documentation tells them to build from source they do not have.

## Re-scoped 2026-08-25

Scheduled 2026-08-01 "as soon as possible" and never started. In the three weeks since, the
product gained a path field, a folder browser, a Help modal, `--help`, a project tab strip,
project switching, closing a project, and transcript controls — and this document still describes
the application as it stood at `0.4.1`.

**That is the reason it kept slipping**: it was written against a moving target, and every week
it waited it described the product less accurately. So it is now scoped to **the release being
cut**, not to the product in general. Document what ships in that release. If the product moves
again afterwards, that is a later document's problem, not a reason to wait.

**Items 1 and 2 are done and are struck below** — not by this work, but by RFC-038's, which had
to fix them to keep its own claims true. Do not redo them; verify they are still accurate and say
so.

**Items 3, 4 and 5 stand**, and three subjects have been added that did not exist when this was
written: the tab strip and project switching, closing a project and what that ends, and the
transcript capture opt-out and purge. All three are user-facing, none is documented for users,
and the third is a privacy control a user currently discovers only by opening Trust Settings and
looking.

**If this slips again, escalate rather than deferring.** Three weeks of silent slippage is how a
scheduled item becomes a permanent one.

## Pinned, 2026-08-25 — because re-scoping is what keeps failing

The re-scope above was written this morning and was **already overtaken by the afternoon**:
RFC-040 shipped visible controls across the application (3 → 11 of 13 actions), and RFC-020
shipped the change review surface. That is the fourth time this document has been overtaken by
the product it describes.

Re-scoping a fifth time would fail the same way. So the scope is now **pinned to a commit**:

> **Document the application as it stands at `711be4b`.** Anything landing after that commit is
> explicitly **out of scope for this document** and belongs to a later one.

Write it against a build of that commit. If something changes underneath you mid-slice, that is
not a reason to re-scope — finish, note what moved, and let a later document catch up. **A
document that waits for the product to stop moving is never written.**

### What is at `711be4b`, mechanically rather than by hand

Do not transcribe a list from this document; read it out of the build, the same instruction
item 2 already carries:

- **14 live keybindings**, derived from `KeybindingPolicy::linux_mvp()`. `--help` and the in-app
  Help modal (`Ctrl+Alt+K`) both render them from that one source, so they cannot be stale.
  **Cite them; do not re-list them by hand.**
- **11 of 13 actions have a visible control**; the remaining two are deliberate keyboard-only
  conventions with reasons recorded in `keyboard_help::control_coverage`. If the documentation
  says "keyboard-driven", that is now only half true and the other half is the interesting half.
- **29 clickable controls** across the shell and its surfaces.

### Subjects at this pin that no user-facing text covers

Superseding the three named above, which stand and are joined by two more:

1. **The project tab strip** — seeing what is open, switching, going home.
2. **Closing a project**, and what that ends: its terminals and any agent run. The confirmation
   names the project by canonical path.
3. **Transcript capture opt-out and purge** — a privacy control a user currently finds only by
   opening Trust Settings and looking.
4. **The change review surface** (`Ctrl+Alt+D`) — **and its limits, which matter more than its
   existence**: metadata only, no diff content, detection is conservative and excludes `.git/`,
   `target/` and `node_modules/`. The surface states this itself; the documentation must not
   state it more weakly than the product does.
5. **The folder browser and the path field** — two routes to opening a project, one of which
   (`Browse…`) needs no path typed.

### One thing to check rather than assume

Item 1's Quick Start and item 2's keyboard reference are struck as done. **Verify that is still
true at this pin** — RFC-040 and RFC-020 both touched user-facing text after they were struck.
Saying "verified still accurate" is a real evidence line; assuming it is how this document got
stale four times.

## The five items

None require design decisions. If one turns out to, raise it rather than deciding it here.

### 1. ~~Quick Start, for the people who now exist~~ — DONE (RFC-038 PR-038-B/E)

Lead with the installed path (`cargo install tekstide`, then `tekstide`), and keep the from-checkout instructions clearly marked as being for contributors. Both are legitimate; only one is currently present, and it is the wrong one to lead with.

### 2. ~~Keyboard reference~~ — DONE (`0.12.1`, extended by RFC-038/039): the README table, the in-app Help modal (`Ctrl+Alt+K`) and `--help` all render it, derived from `KeybindingPolicy` so it cannot drift. **Verify it is still complete; do not rewrite it.**

These exist and are documented **nowhere a user would look**:

| Binding | Action | Source |
| --- | --- | --- |
| `Ctrl+Alt+P` | Open Project Board | `KeybindingPolicy::linux_mvp()` |
| `Ctrl+Alt+M` | Toggle Content / Terminal mode | same |
| `Tab` / `Shift+Tab` | Cycle focus between zones | `input::route_non_modal_input` |
| `Esc` | Dismiss modal | `shell::modal_subscription` |
| `Enter` | Activate focused modal button | same |

**Read them out of the code rather than out of this table.** `Ctrl+Shift+P` is also bound (`OpenCommandPalette`, status `Reserved`) but dispatches to nothing — document it as reserved or omit it, not as working.

The shell is keyboard-navigable by design (`NFR-UX-001`), and a keyboard-navigable application whose bindings are undiscoverable is navigable only by its authors.

### 3. What Tekstide does and does not do today

Mostly assembly, not authorship — README's Current Status and `CHANGELOG.md` already say this honestly, and the wording there has been reviewed repeatedly. **Do not restate it in new words**; new words are where accuracy erodes.

Three claims that must survive intact:

- Command approval is **implemented but unreachable**, and **cooperative, not enforced**. Tekstide does not approve commands and does not control what an AI CLI can run.
- There is **no screen-reader support**. Not "limited", not "planned".
- Linux only. No cross-platform evidence.

### 4. Where local state lives, and how to purge it

A privacy question a user can currently answer only by reading source. Cover the recent-projects store and the audit store: where each lives, what each holds, and how to delete it.

RFC-011 and RFC-013 already define the retention and purge policy — **cite them rather than re-deriving**, and make sure what you write matches what the code does rather than what the RFC intends.

### 5. A path to the known limitations

The honest disclosures are real and thorough, and they live in RFC closeouts where no user will find them. A reader who wants to know what is missing should get there from the README in one hop.

## Constraints

- **Everything user-facing you write is subject to RFC-016.** If any of it ends up rendered by the application rather than sitting in a Markdown file, it goes through the catalog — the enforcement scans will tell you.
- **Do not soften a disclosure to make the project sound better.** This documentation's whole value is that it is accurate about a product that is honestly incomplete. Thirty reviews have gone into keeping those claims exact; a friendlier paraphrase undoes that silently.
- **Check the commands you write by running them.** `cargo install tekstide` from a clean state is worth doing once — `0.4.0` shipped an `include_str!` that broke packaging and was caught only by running the gate rather than reasoning about it.

## Evidence

- The installed-path instructions, verified by actually installing and running.
- A statement of which claims were assembled from existing reviewed text versus newly written — the newly written ones are where review attention goes.
