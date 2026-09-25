---
title: "RFC-055 — what the explorer must not claim"
rfc: "RFC-055"
rfc_file: "../../done/055-ignore-rules-in-the-explorer.md"
source_rfc_status: "Implemented and closed 2026-09-25 — M12 remainder tail"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# What the explorer must not claim

This slice teaches a surface to make a new claim about a file — *this one is ignored* — from an
answer produced by a subprocess, about paths a repository chose the names of. Three ways that goes
wrong, and each has a test in the checklist.

## 1. It must not say "not ignored" when it does not know

`git check-ignore` exits **`1` when none of the batch is ignored** — and also when a path does not
exist. `1` is an answer. `128` is not, and so is anything else. Three states, not two:

| Exit | Meaning | What the explorer draws |
| --- | --- | --- |
| `0` | the echoed paths are ignored | `ignored` on those, nothing on the rest of the batch |
| `1` | none of this batch is ignored | nothing — an ordinary directory |
| anything else | **unknown** | the floor list (D6), and the sidebar says the rule came from it |

Failing open marks ignored files as ordinary. Failing closed hides files from a user who is trying to
see what an AI agent can read. The second is the worse of the two and the easier to write by
accident, because "hide it if in doubt" reads like caution.

## 2. It must not let one filename silence a directory

Measured: a file named `:(glob)evil.log` can be created, and feeding it to
`git check-ignore -z --stdin` aborts the **entire batch** with exit 128 and
`fatal: ... pathspec magic not supported by this command: 'glob'`. Its siblings get no answer.
`GIT_LITERAL_PATHSPECS=1` does not rescue this — check-ignore rejects the `literal` magic too.

The fix is one character pair: every path goes in as `./<path>` and the prefix is stripped off the
echoed reply. This is **structural**, not a convention: the paths enter the query through a single
function, the prefix is applied there, and no caller can pass a raw path. A test must assert the
siblings of a hostile filename still get answers, and must fail if the prefix is removed.

Related, same cause: `ExplorerNode::relative_path` is relative to the **project** root, which need not
be the repository root. The query's `cwd` and the paths handed to it must agree, decided in that same
one function. A project root two levels inside its repository is a fixture, not a thought experiment
— measured: the repository-root `.gitignore` applies from there.

## 3. It must not describe rows it did not ask about

The scanner caps a directory at `max_children_per_directory` (256) and reports how many entries it
left out. **Those entries have unknown ignore state.** They were not in the batch, so nothing was
learned about them, and the count of omitted rows must not acquire an ignore claim it never had.
Equally: the query must be asked only about paths the explorer's existing access policy already
returned. It is not a second way into the filesystem, and symlink and root-escape handling stays
exactly where it is.

## And three smaller ones

**A tracked file that matches an ignore pattern is tracked.** Git says so (no `--no-index`), and the
explorer must not contradict it.

**`-z` output is bytes.** A filename need not be UTF-8; the status parser already works on the raw
byte stream for this reason and the new reader does the same. Do not route the reply through a lossy
decode on the way to a comparison.

**Ignore state is as old as the scan.** There is no watcher until RFC-026. A `.gitignore` edited
after a scan is not reflected until that directory is scanned again — extend the staleness sentence
RFC-052 already shows, rather than inventing a second way of saying the same thing, and let the
changelog own the limitation in plain words.
