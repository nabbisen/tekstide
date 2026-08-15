# Tekstide

A local-first, multi-project AI-CLI development workbench.

This book is assembled from the repository's canonical documents rather than restating
them. Every chapter is an `{{#include}}` of the file that owns its content, so there is
exactly one place to edit and no copy that can drift.

That constraint is deliberate. This project has repeatedly found documentation asserting
things the code had stopped doing — a README that said "there is no editor" after the
editor shipped, four releases describing themselves as unreleased release candidates, a
board that reported a feature as unimplemented while it ran. A book that paraphrased its
sources would add a fourth place for that to happen.

## What is here, and what is not

`docs/` was created 2026-08-15 at the owner's direction, ahead of RFC-029 (M14), which
owns the full by-persona structure. **This skeleton deliberately does not pre-empt that
decision.** The three sections below are the minimum needed to be useful today, chosen to
be easy to restructure rather than to be final:

- **For users** — how to install and run it.
- **For contributors** — the architecture, the conventions, what is scheduled and what is
  deferred.
- **Project record** — what shipped and what is planned.

When RFC-029 designs the real persona structure, this becomes an input to that work, not a
constraint on it. Chapters that turn out to be wrong should be moved or deleted rather than
preserved for continuity.

## Reading order

If you are new and intend to contribute, read
[Architecture](./contributors/architecture.md) first — it carries the crate boundary, the
invariants, and the evidence conventions that every RFC in this project inherits.

If you want to use it, [Getting started](./users/getting-started.md).

## What Tekstide does not do yet

The [changelog](./record/changelog.md) states this per release, and it is worth reading
before the feature list: this project's convention is that a release note carries more
stated limitations than claimed capability. Deferred work is tracked in
[Deferred work](./contributors/future-work.md), which is a live index rather than a wish
list — items leave it only when they are done or explicitly rejected.
