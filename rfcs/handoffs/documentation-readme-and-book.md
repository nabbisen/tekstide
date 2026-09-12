---
title: "Documentation: a 554-line landing page, and a book that includes it"
status: "**PR-DOC-A and PR-DOC-B complete and verified live 2026-09-13.** The book publishes at `https://nabbisen.github.io/tekstide/`; six owned user chapters answer 200, and `SUMMARY.md` completeness is guarded by two tests. **PR-DOC-C not started, and unblocked** — `README.md` is **deliberately untrimmed** until it lands, so its content exists in both the README and the book for exactly this interval. Scoped 2026-09-13 by the architect."
rfc_file: "none — documentation slice; no behaviour, no security surface. Owner authorized a handoff rather than an RFC."
target_milestone: "M12"
created: "2026-09-13"
---

# Documentation: README and the book

**Source guideline:** `.git-exclude/rules/DOCUMENTATION_GUIDELINES.md` — written for a different
project. The owner authorized adapting it. **Adopt its principles; the five deviations below are
where this project differs, and each has a reason specific to how this project ships.**

## Measured, 2026-09-13

| | Guideline | Here |
| --- | --- | --- |
| `README.md` | 100–200 lines | **554 lines**, 36 KB |
| `## Current Status` | — | **254 lines — 46% of the file**, with **25** release-version mentions |
| `## Local Data and Privacy` | — | 87 lines |
| `## Working With Projects` | — | 60 lines |
| `## Keyboard Reference` | — | 45 lines, hand-maintained |
| TOC threshold | a TOC means split it | 10 sections, no TOC — past the threshold with no signal |

**`Current Status` is the slice.** It is a running per-RFC implementation narrative that must be
edited every release — *"as of `0.10.0`… as of `0.18.0`"*, twenty-five times. Its content already
has two canonical homes: `CHANGELOG.md` for what shipped when, and `rfcs/README.md` for RFC status.
**Text that must be re-edited every release is a drift generator**, and the `0.18.0` gate found
three README staleness defects in one release.

## The blocking fact, and the ordering it forces

```
docs/src/users/getting-started.md   →   {{#include ../../../README.md}}
```

**The book's only user chapter is the README.** Six of its nine pages are pure includes; only
`introduction.md` and `contributors/security-decisions.md` own content. So "offload to `docs/src/`"
has no destination, and doing it into that file is circular.

**And the book is not published** — no `.github/workflows` at all.

**So the order is fixed, and doing it out of order makes the documentation worse:**

1. **A — publish the book.** CI → Pages. This produces the absolute URL every later step needs.
2. **B — give the book real user chapters.** Somewhere for content to go.
3. **C — trim the README**, linking absolutely.

**Trimming first is the tempting order and the wrong one**: one commit, looks like progress, and it
deletes reachable content in favour of unreachable content for every crates.io reader.

## Five deviations from the source guideline

### 1. `README.md` is a registry landing page for two published crates — this is a hard floor

```
crates/tekstide-core/Cargo.toml:  readme = "README.md"
crates/tekstide/Cargo.toml:       readme = "../../README.md"
```

**`docs/` is in neither published archive** (checked: zero `docs/` entries in both `0.18.0`
`.crate` files), and relative links do not resolve on crates.io — established at `0.14.0`, which is
why the logo is an absolute `raw.githubusercontent.com` URL.

**So:** anything a reader needs to **evaluate** or **install** stays in `README.md`, and every link
out of it is **absolute** to the published book. The guideline's "offload aggressively" is bounded
by this, and the bound is not negotiable while both crates point here.

### 2. The book includes canonical documents; it may own user chapters

`docs/src/introduction.md` states a deliberate principle — *"every chapter is an `{{#include}}` of
the file that owns its content, so there is exactly one place to edit and no copy that can drift"* —
with reasoning this project earned.

**They reconcile:** includes govern documents with a canonical home elsewhere (`ARCHITECTURE.md`,
`CHANGELOG.md`, `ROADMAP.md`, `rfcs/`). The book may **own** chapters that never had a root home,
which is exactly what user guides are.

**And that sentence is already false**: `introduction.md` and `security-decisions.md` own their
content, so "every chapter" describes seven of nine. **Correct it in A** — it is state-asserting
text about the book, inside the book.

### 3. A visual aid must obey the committed-screenshot rule

The guideline suggests a demo GIF. This project's rule: **a committed image may only ever show
throwaway state** — `mktemp -d` fixtures, never a path under `$HOME`. `0.14.0` shipped three
screenshots carrying the operator's home layout before the owner caught it.

**So:** any visual is captured against a fixture, with its launch command recorded in a sidecar,
per `ARCHITECTURE.md`. A demo GIF is **optional and not required by this slice** — the cost of
getting it wrong is disclosed private data, and there is no deadline that justifies rushing it.

### 4. `CONTRIBUTING.md` does not exist, and §3 wants a link to it

**Write a minimal one in C** — how to run the gate, where the RFC lifecycle lives, that
`.git-exclude/` is not published. **A link to a missing file is worse than no link**, so the link
and the file land together or neither does.

### 5. The keyboard table stays, trimmed — it is not wrong, it is hand-maintained

The 45-line table is **currently accurate**, including its honest note that `Ctrl+Shift+P` is
reserved for a palette that does not exist — which is why `--help` prints 15 chords and the table
lists 16. Verified against `KeybindingPolicy::linux_mvp()` and the release binary.

**But it is a hand copy of content RFC-044 generates**, and it drifted within one release:
`0.18.0`'s gate found `Ctrl+Alt+C` missing from it while Help and `--help` both had it.

**So:** keep the **three to five** bindings a reader needs to get started, point at
`tekstide --help` for the rest, and move the full table into the book. **Do not delete it outright**
— a crates.io reader cannot run `--help` before installing, and a landing page that shows nothing of
the keyboard model hides the thing this application is navigated by.

## The work

### PR-DOC-A — publish the book

- CI workflow building the mdBook and publishing to Pages. `docs/book/` stays git-ignored
  (`.gitignore:28`); **publishing must not commit build output.**
- Correct `introduction.md`'s "every chapter is an include" to what is true.
- **Acceptance:** the book has a stable absolute URL, and `SUMMARY.md` lists every page under
  `docs/src/` — the guideline's own §4 note.

### PR-DOC-A — verified live, 2026-09-13

The owner switched Pages to GitHub Actions; two workflow runs have succeeded (≈20s each). **A's
acceptance is now fully discharged**, including the half the implementer correctly declined to
claim:

| Checked against the live site | Result |
| --- | --- |
| `https://nabbisen.github.io/tekstide/` | **HTTP 200**, serving `<title>Introduction - Tekstide</title>` |
| All **nine** chapter URLs | 200, every one |
| `{{#include}}` content from outside `docs/` actually renders | README's `cargo install tekstide`, the `0.18.0` changelog entry, and `ARCHITECTURE.md`'s synthetic-input note all present |
| `404.html`'s `<base href>` | `/tekstide/` — and its assets, which are **relative** and content-hashed in mdBook 0.5.4, all resolve. The `site-url` change does exactly what PR-DOC-A measured. |
| `.git-exclude/` on the live site | **404.** Nothing internal is published. |
| Every `{{#include}}` target | resolves to a **tracked** file — no chapter reaches untracked or git-excluded content |

**The URL may now be written into documents a reader sees**, which is what C was waiting for.

### PR-DOC-B — real user chapters

`docs/src/users/getting-started.md` stops being an include of `README.md` and becomes a chapter that
owns its content. New chapters own what moves out of the README: the status narrative's durable half,
project workflows, configuration reference, and the local-data/privacy detail.

- **`Current Status`'s release-by-release narrative does not move — it goes away.** `CHANGELOG.md`
  already owns "what shipped when" and `rfcs/README.md` owns RFC status. Moving 25 version mentions
  into the book relocates the drift generator; deleting them and linking the two canonical homes
  removes it.
- **`SUMMARY.md` completeness, as two tests in `rfc_docs_invariants`**: every page under
  `docs/src/` is listed, and every `SUMMARY.md` entry names a page that exists. `mdbook build`
  succeeds either way, so an unlisted page renders nowhere and nothing warns. *(Added 2026-09-13.
  Response 383 said this was "added to B's boxes" and the handoff was never edited — PR-DOC-B
  implemented it from the response and flagged the gap at request 384. Delivered in `d23d1e1`.)*
- **Acceptance:** no chapter under `docs/src/users/` is a pure include of `README.md`, and every
  claim moved out is either in the book or in a document that already owned it.

### PR-DOC-C — trim the README

Target the 3–30–3 shape: what it is and 3–5 features; `cargo install tekstide` and a minimal
example; then links out.

- **Keep:** the one-line summary, key features, Quick Start's install and bare-run behaviour, a
  short and honest local-data summary, 3–5 keybindings, and absolute links to the book.
- **Cut:** `Current Status`'s narrative; the Quick Start paragraph explaining what this section
  *used to say before `0.12.1`* (changelog content on a landing page); the full keyboard table.
- **Write `CONTRIBUTING.md`** and link it.
- **Acceptance:** **`README.md` ≤ 200 lines**, and **every link in it resolves** — including
  absolute ones, checked by fetching them, not by reading them.

**One required test:** extend `rfc_docs_invariants`' link resolver to cover `README.md` and
`docs/src/`. It already proves every relative link in the RFC tree resolves, and it caught a broken
link in RFC-046 written by the architect. **Link rot in the front door should be a failing test, not
a release-gate finding** — and this slice is the moment to buy that, because it is the slice that
creates the links.

## What this must not become

- **A rewrite of the technical content.** The README's claims are unusually careful — it discloses a
  reserved binding, an unenforced retention key, and the absence of screen-reader support. **Length
  is the defect; honesty is not.** Every disclosure that survives the cut must survive intact.
- **A promise the book cannot keep.** Nothing links to a chapter that does not exist yet; A and B
  precede C for exactly this reason.
- **A second copy of anything.** If content has a canonical home, link or include it. The book's own
  founding principle is the rule here.
- **A crates.io regression.** After C, read both crates' rendered README on crates.io and confirm
  every link works from there. A landing page that is lean and broken is worse than long.
