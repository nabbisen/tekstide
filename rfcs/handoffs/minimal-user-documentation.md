---
title: "Minimal user documentation — implementation handoff"
rfc: "none — pulled forward from RFC-029"
status: "Scheduled to M9 by the owner 2026-08-01 (\"as soon as possible\")"
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

## The five items

None require design decisions. If one turns out to, raise it rather than deciding it here.

### 1. Quick Start, for the people who now exist

Lead with the installed path (`cargo install tekstide`, then `tekstide`), and keep the from-checkout instructions clearly marked as being for contributors. Both are legitimate; only one is currently present, and it is the wrong one to lead with.

### 2. Keyboard reference

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
