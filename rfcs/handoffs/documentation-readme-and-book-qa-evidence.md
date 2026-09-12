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
