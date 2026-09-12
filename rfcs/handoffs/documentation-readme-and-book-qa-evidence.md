---
title: "Documentation: README and the book — QA evidence"
rfc: "none"
source_rfc_status: "No RFC. Implementation evidence for documentation-readme-and-book.md."
target_milestone: "M12"
created: "2026-09-13"
---

# Evidence

## PR-DOC-A — publish the book

No Rust changed. Three files: `.github/workflows/docs.yml` (new — the repository's **first**
workflow), `docs/book.toml`, `docs/src/introduction.md`.

### What is done, and the one step that is not mine

The workflow builds the book and publishes it to Pages. **The URL is not live yet, and this
document does not claim otherwise.** GitHub Pages must first be pointed at Actions — repository
Settings → Pages → *Build and deployment* → Source: **GitHub Actions** — which is a repository
settings change and therefore the owner's, not something an implementer should make on their
behalf. Once it is set, the first push to `main` publishes, and the URL is
`https://nabbisen.github.io/tekstide/` (derived from the `origin` remote,
`https://github.com/nabbisen/tekstide.git`, not assumed).

**PR-DOC-C's absolute links must not be written until that URL answers.** The handoff's own rule —
*"Nothing links to a chapter that does not exist yet"* — covers a book that is not reachable at all,
and C's acceptance requires links checked "by fetching them, not by reading them."

### The workflow's two load-bearing choices

**No `paths:` filter.** Six of the book's nine pages `{{#include}}` files *outside* `docs/`, found
mechanically rather than by eye:

```
$ grep -rn "{{#include" docs/src
docs/src/users/getting-started.md      -> README.md
docs/src/record/changelog.md           -> CHANGELOG.md
docs/src/record/roadmap.md             -> ROADMAP.md
docs/src/contributors/architecture.md  -> ARCHITECTURE.md
docs/src/contributors/delivery-plan.md -> rfcs/delivery-plan.md
docs/src/contributors/future-work.md   -> rfcs/future-work.md
```

A path filter would have to enumerate all six and would silently publish a **stale** book the first
time a chapter includes a seventh file — which PR-DOC-B is about to start doing. A hand-maintained
list of include targets is the drift generator this whole handoff exists to remove, so the workflow
rebuilds on every push to `main` and pays a one-minute build instead. If the build frequency is
unwanted, the alternative is a filter **plus** a test asserting the filter covers every include
target; a filter alone is the one option that fails silently.

**`mdbook` pinned by version and checksum, no third-party actions.** The job holds `pages: write`
and `id-token: write`. A version pin alone still trusts whatever that URL serves later, so the
SHA-256 is recorded too (`3f28de05…58cd7`, against the asset as served 2026-09-13). Actions are
first-party `actions/*` only, each pinned to a commit SHA with its tag in a trailing comment; the
SHAs were resolved through the API and each tag confirmed to point at a **commit** rather than an
annotated tag object, which `uses:` would not accept.

`actions/configure-pages` is deliberately **not** used: nothing here needs a dynamic base path
(`site-url` is static, below), and its `enablement: true` option would change repository settings —
the step this document says is the owner's.

### Verified rather than asserted

| Claim | How |
| --- | --- |
| The book builds at the pinned version | `mdbook build docs` with local `mdbook v0.5.4`, clean |
| The download, checksum and extraction work | ran the step's own commands: `sha256sum --check --strict` passes on the real asset, the extracted binary reports `mdbook v0.5.4` |
| A bad checksum aborts the job | same command with a zeroed digest: `FAILED`, exit **1**, which under `set -euo pipefail` ends the step |
| The workflow is parseable and has the shape intended | parsed with PyYAML: two jobs, `deploy` needs `build`, the three permissions, `concurrency: pages` with `cancel-in-progress: false` |
| `SUMMARY.md` lists every page under `docs/src/` (A's acceptance) | enumerated `docs/src/**.md` and matched each against `SUMMARY.md` — **8 of 8, zero orphans** |
| Every relative link under `docs/src/` resolves | 13 links checked, none broken (includes the one this slice adds) |

`mdbook --version` is asserted in the install step itself, so a release that silently changes
version fails in CI rather than publishing a differently-built book.

**Nothing is committed from the build.** `docs/book/` stays git-ignored (`.gitignore:28`); the book
is built in CI and published from the uploaded artifact.

### `site-url`, and the one thing it actually changes

`site-url = "/tekstide/"` was added to `docs/book.toml`. Its effect was measured by diffing two
builds rather than taken from the documentation — it changes exactly one line:

```
$ diff 404-without-site-url.html 404-with-site-url.html
7c7
<         <base href="/">
---
>         <base href="/tekstide/">
```

That page is served for **any** missing path on a project site, and its own asset and navigation
references are relative. With `<base href="/">` they resolve against the domain root — wrong for a
project site — so the 404 page renders bare. It is the one page a reader reaches only by following a
broken link, which makes it the worst one to get wrong.

### The false sentence in the book, about the book

`docs/src/introduction.md` said:

> Every chapter is an `{{#include}}` of the file that owns its content, so there is exactly one
> place to edit and no copy that can drift.

Seven of nine pages are includes; `introduction.md` and `contributors/security-decisions.md` own
their content. Corrected to state the **rule** rather than a count — *a chapter owns what no file
outside `docs/` owns* — because a count is itself a drift generator, and PR-DOC-B is about to change
it.

The rule is phrased against where a content's canonical home **is**, not where it has ever been. A
first draft said "never had a home outside `docs/`", which would have read as forbidding PR-DOC-B's
own moves out of `README.md`. That is the same defect as the sentence being replaced: text about the
book's structure that the book's next change makes false.

### Gate

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -D warnings`, `git diff --check`:
clean. Full workspace suite: **507 + 4 + 773, green**, identical to PR-049-A's baseline — **one run,
not three**, because this slice changes no compiled code and a three-run flake sweep measures
nothing it did not already measure.

## PR-DOC-B — real user chapters

Six chapters under `docs/src/users/`, none of them an include. `getting-started.md` was a
one-line `{{#include ../../../README.md}}` and now owns its content; five are new.

```
$ grep -rn "README.md" docs/src/
(no matches)
```

**B's acceptance, both halves, checked mechanically:** no chapter under `docs/src/users/` is a
pure include of `README.md` — **no chapter anywhere in the book references `README.md` at all**
— and the claim map below accounts for every claim.

### What B deliberately does not do

**`README.md` is not trimmed here.** That is PR-DOC-C, and the handoff's order is explicit about
why. The consequence is that the content exists in **two** places for exactly one slice, which
sits against the handoff's own *"a second copy of anything"* rule.

That is inherent in A → B → C rather than a lapse: B must build the destination before C can cut
the source, and the alternative — cutting the README in B — would leave a crates.io reader with
less than they have today if C stalled. **The claim map is what makes C's cut safe**, because it
names, per claim, what C is allowed to delete and where a reader will find it afterwards.

Read B's acceptance — *"every claim moved out is either in the book or in a document that already
owned it"* — as being about **where each claim lands**, since in B nothing has yet left the
README.

### The claim map

`## Current Status` is 254 lines, 46% of the README, with 25 release-version mentions. The split
below is the judgment this slice exists to make: **what the product does** is durable and goes to
the book; **when it started doing it** is changelog content and is not copied anywhere.

| README claim | Where it lives now | |
| --- | --- | --- |
| Projects, root-bound access, bounded explorer, UTF-8 buffers, safe save with external-change detection | `users/what-works-today.md` | book |
| Explorer is read-only; no undo; no highlighting/LSP/multi-cursor/search; 4 MiB editable ceiling | `users/what-works-today.md` | book |
| File **names** escaped, file **contents** deliberately not, and the bidi consequence | `users/what-works-today.md` | book |
| PTY terminals, bounded IO, resize, exit detection, six concurrent | `users/what-works-today.md` | book |
| Paste policy: single-line through, control-containing blocked, multi-line confirmed | `users/what-works-today.md` | book |
| **Input latency not verified against its 16 ms p95 target** | `users/what-works-today.md` | book |
| AgentRun launch, Plain/Supervised/Managed labels, Restricted Mode blocks | `users/what-works-today.md` | book |
| **The real Claude Code CLI has never been exercised by the tests** | `users/what-works-today.md` | book |
| AgentRun report: escaped, reachable mid-run, 1 MiB tail, truncation shown separately | `users/what-works-today.md` | book |
| Change detection, its four disclosed limits, the `.git/hooks/`+`.git/config` exception | `users/what-works-today.md` | book |
| **No two-sided diff**, and why the before-bytes are gone | `users/what-works-today.md` | book |
| Command approval: built, cooperative not enforced, no AI CLI speaks it, history empty | `users/what-works-today.md` | book |
| Trusted-UI tells, and that neither makes the dialog unspoofable | `users/what-works-today.md` | book |
| **No screen-reader support**, for the life of the `iced` substrate decision | `users/what-works-today.md` | book |
| Twelve audit families, all with a producer | `users/what-works-today.md` + `users/local-data-and-privacy.md` | book |
| `Ctrl+Shift+P` reserved and does nothing | `users/what-works-today.md` + `users/keyboard-reference.md` | book |
| Folder browser, tab strip, switching, closing and what closing ends | `users/working-with-projects.md` | book |
| Change Review's decision recording, and what it is not | `users/working-with-projects.md` | book |
| The whole `## Configuring It` reference, incl. the retention caveat | `users/configuration.md` | book |
| The full 16-chord keyboard table | `users/keyboard-reference.md` | book |
| The whole `## Local Data and Privacy` section, every path and schema limit | `users/local-data-and-privacy.md` | book |
| What granting trust authorises, and what revoking does not undo | `contributors/security-decisions.md` | already owned |
| Full retention/purge policies | RFC-011 and RFC-013, under `rfcs/done/` | already owned |
| The consolidated deferred list | `rfcs/future-work.md` | already owned |
| *"as of `0.8.0`… `0.10.0`… `0.18.0`"* — 25 version mentions | `CHANGELOG.md` | **not copied** |
| Throughput 374 KB/s → 17–18 MB/s; terminal limit 3 → 6 | `CHANGELOG.md` | **not copied** |
| Border contrast 2.63:1 → 3.85:1; scrim `0.55` → `0.75`; worst case 2.40:1 | `CHANGELOG.md` | **not copied** |
| *"Until `0.12.1` this section said to run `tekstide` bare…"* | `CHANGELOG.md` | **not copied** |
| *"This corrects a claim `0.10.0` and `0.11.0` both made"* | `CHANGELOG.md`, under *Corrections* | **pointed at** |

**Every "not copied" row was checked to be in `CHANGELOG.md` already, not assumed.** Grepped
before dropping: the throughput figures (4 hits), `2.63` (1), the scrim change (1), `16 ms` (3),
screen-reader (16), the never-exercised CLI (1), recent-projects (2).

The one exception is the transcript-correction history: the durable claim (**transcripts are
recorded, and contain whatever the AI CLI printed, including what it quoted from your files**) is
in the book, and the story of the two releases that claimed otherwise is left in the changelog
with a link to it. A reader who wants to know the product lied about this once is one click away;
a reader who wants to know what happens to their data is not made to read release history first.

### RFC-049 §6 is respected

No chapter says a transcript is removed because of its age. `users/configuration.md` carries the
caveat — *"no age-based purge reads it yet… transcripts are **not** kept for that many days and
then removed"* — and `users/local-data-and-privacy.md` links to it rather than restating it. The
only purge either page describes is the manual, per-project one.

### The required test, and it is two tests

`rfc_docs_invariants` gained `every_page_in_the_book_is_listed_in_its_summary` and
`every_summary_entry_names_a_page_that_exists`. **Two tests, not one with two assertions**: an
unlisted page and an entry pointing at nothing are different defects with different fixes, and a
single test failing for either reason cannot tell a reader which happened (response 367's lesson).

| Box | Ablation | Result |
| --- | --- | --- |
| A page not listed in `SUMMARY.md` is caught | add an unlisted `docs/src/users/*.md` | `every_page_in_the_book_is_listed_in_its_summary` **fails alone** |
| A `SUMMARY.md` entry naming nothing is caught | add a `SUMMARY.md` bullet pointing at `./users/ghost-ablation.md` | `every_summary_entry_names_a_page_that_exists` **fails alone** |

**And the existing link resolver caught this very document while I was writing it.** The
ablation row above first spelled its example as real markdown link syntax, so
`every_relative_link_in_the_rfc_tree_resolves` failed on
`rfcs/handoffs/documentation-readme-and-book-qa-evidence.md -> ./users/ghost-ablation.md` — a
link in the RFC tree that resolved to nothing. Rewritten as prose. Unplanned evidence that the
check the handoff asks C to extend already earns its place.

Both skip, reported rather than silently, when `docs/` is absent — the published crate does not
package it, the same convention the four existing checks use.

**This is worth having because `mdbook build` succeeds either way.** An unlisted page renders
nowhere and nothing warns; it is precisely the silent class of documentation failure the other
four checks exist for.

### One line of `README.md` changed, and why

`README.md` said, of the approval-history surface:

> *"…but no key is bound to it, so it cannot be opened at all. That is a defect, recorded in
> `rfcs/future-work.md`, not a design choice."*

**False, and contradicted three times in the same file** (lines 128, 203, and the keyboard table
at 432). `Ctrl+Alt+H` is bound in `KeybindingPolicy::linux_mvp()`
(`crates/tekstide-core/src/navigation.rs:277`), and `rfcs/future-work.md:533` — the document the
sentence cites as recording the defect — reads **"Discharged 2026-08-18"**.

Corrected to what is true: it opens with `Ctrl+Alt+H`, and a real user sees it **empty**, which is
correct rather than a bug. The honest disclosure is kept; only the false half is gone.

This is the one README edit in B, made rather than deferred to C because the book now states the
true version on a **published** page: leaving it would have two published documents contradicting
each other, which is worse than either slice boundary.

### Links and anchors

Every relative link in the book source resolves, and every cross-page anchor resolves **against
the built HTML** rather than against a guess at mdBook's slug rules:

```
36 md links checked (3 with anchors)
all resolve, anchors included
```

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`: clean. `mdbook build
docs`: clean. Full workspace: **507 + 6 + 773, green** — the two new doc-invariant tests are the
only change from PR-DOC-A's 4. One run; no compiled product code changed, only a test binary.
