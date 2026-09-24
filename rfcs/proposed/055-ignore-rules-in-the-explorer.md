# RFC-055: Ignore Rules In The Explorer

Status: **Proposed 2026-09-24.** `0.26.0` in the authorised schedule, directly after RFC-052's tree —
expansion is what makes RFC-052 D7's fixed collapse list load-bearing, and this is what the schedule
said would replace it. Closes `REQ-FILE-005`, `REQ-FILE-006`, and the `ignored` category of
`REQ-FILE-002` that the model has never carried.

## Summary

The explorer decides what to collapse from a **three-name array** — `[".git", "node_modules",
"target"]` — matched by exact name at any depth. A project's own `.gitignore` is not read, `git
status` is invoked without `--ignored`, and `FileGitStatus` has no `Ignored` variant, so the one
category `REQ-FILE-002` names that the tree could show, it cannot. This RFC makes the explorer ask
**git** what is ignored, bounded by the rows it is about to draw, and gives a user one setting that
shows the ignored rows anyway.

It does not add a `.gitignore` parser, and the measurements below are why.

## What is true today, measured

| | Measured |
| --- | --- |
| 1 | `IGNORED_DIRECTORY_NAMES` (`crates/tekstide-core/src/project/ignored_directories.rs:18`) is one shared array of three names, sourced by both `FileExplorerScanPolicy::linux_mvp` and `GeneratedChangeDetectionPolicy`. Its own doc comment already says it is **"not `.gitignore` parsing, and not meant to become it here"**. |
| 2 | The one status invocation is `git status --porcelain=v2 --branch -z --ignore-submodules=all` (`crates/tekstide-core/src/runtime/git.rs:509`). **No `--ignored`.** `FileGitStatus` (`project/metadata.rs:303`) has `Modified, Added, Deleted, Renamed, Untracked, Unmerged` — **no `Ignored`**, and `crates/tekstide/src/surface/explorer.rs:98` has six badge words to match. |
| 3 | **Dotfiles are not hidden.** No filter exists in either the scanner or the surface; `.gitignore`, `.env` and `.git-exclude` are ordinary rows today. |
| 4 | Cost of the three status shapes, measured on this repository (whose `target/` is **145 GB**): today's call **4 ms / 121 B**; `--ignored=traditional` **109 ms / 225 B**; `--ignored=matching` **4 ms / 244 B**. `traditional` pays to descend into ignored directories that do not themselves match a pattern. |
| 5 | **`--ignored=matching`'s output is bounded by the ignored *file* count, not by the rows drawn.** Hostile fixture — 20 000 files named `*.log`, `.gitignore` containing `*.log`: `matching` returns **20 000 records / 349 KB**; `traditional` returns **1**. Writing the same rule as `logs/` instead returns 1 from both. A user's choice of pattern decides whether the status parse allocates a `BTreeMap` of twenty thousand paths, and that map is cloned into the GUI. |
| 6 | `git check-ignore -z --stdin` answered **21 paths in 1 ms**, and its output is bounded by its input. Exit `0` means some path is ignored; exit **`1` means none of them is — and also means a path does not exist**; `128` is failure. `1` is an answer, not an error. |
| 7 | **A file named `:(glob)evil.log` can be created, and it aborts the whole batch.** Fed to `check-ignore -z --stdin` alongside a sibling, git exits **128** with `fatal: :(glob)evil.log: pathspec magic not supported by this command: 'glob'` and answers *nothing*. `GIT_LITERAL_PATHSPECS=1` does **not** help — it converts the magic and check-ignore then rejects `'literal'`. **Prefixing every path with `./` defuses it**: measured, both paths answered, exit 0, and the echoed paths carry the prefix back. |
| 8 | Without `--no-index`, check-ignore reports a **tracked** file matching an ignore pattern as *not* ignored — git's own semantics, and the correct badge. With `--no-index` it says ignored. Measured on a tracked `tracked.log` under `*.log`. |
| 9 | Trust is **deliberately not a parameter** of the git gate (`runtime/git.rs:321`, D1′ item 7): Tekstide never runs a program a repository names, in any trust state. Ignore knowledge is therefore available exactly when `git status` already is — untrusted workspaces included. |
| 10 | The explorer scan already runs on a worker thread (`explorer_scan_subscription`, applied by `apply_explorer_scan` with newer-scan-wins). A query added to the scan needs **no new threading**. |
| 11 | RFC-054 shipped the configuration mechanism: `ThemeSettings`, `FontSettings`, `TerminalSettings`, `KeybindingSettings`, with an unknown key **named on the Project Board** and the default standing. An explorer setting is an addition to a working mechanism, not a new one. |

## Decisions required

**D1 — Git is the oracle. Tekstide never parses `.gitignore`.** Negation, precedence, nested ignore
files, `core.excludesFile` and `.git/info/exclude` are the specification, and `REQ-FILE-005`'s "user
ignore configuration" is *already inside* the oracle's answer — a second implementation would have to
agree with git exactly or the badge lies about a file. It is also attacker-influenced content: a
parser puts a repository's text through our own matcher, in our address space, for no gain. No new
dependency.

**D2 — The query is `git check-ignore`, bounded by the rows the explorer draws — not `--ignored` on
the status call.** Measurements 4 and 5 are the whole argument: `--ignored=matching` is free on this
repository and returns twenty thousand records on a one-line `.gitignore` someone could write
tomorrow, and `traditional` pays 109 ms to descend. `check-ignore` costs 1 ms for 21 paths and is
bounded by its input, which the scanner already caps at `max_children_per_directory` (256). One
process per directory scan, on the worker thread that already exists (measurement 10). **If the
100 000-entry fixture contradicts this, that is a finding, not a tuning note.**

**D3 — Every path is prefixed `./` on the way in and the prefix stripped on the way out, at one
function.** Measurement 7: one file a repository names must never be able to deny ignore knowledge to
its siblings. The hostile fixture must contain a file named `:(glob)evil.log` and the test must
assert every *sibling* still receives an answer. This is structural, not a rule to remember — the
paths enter the query through a single function, so a second call site added later cannot reintroduce
the abort.

**D4 — Exit `1` is an answer; `128` is not, and unknown is shown as unknown.** `0` → the echoed paths
are ignored. `1` → none of this batch is. Anything else → **unknown ignore state**, said plainly.
Failing open marks ignored files normal; failing closed hides files from a user auditing what an
agent can read. Neither is acceptable silently, and measurement 6 shows why the distinction is easy
to get wrong: `1` looks like failure and is not.

**D5 — `FileGitStatus` gains `Ignored`, and the explorer gains an `ignored` badge.** `REQ-FILE-002`
names five categories; the model has carried four of them and two others since RFC-030. Do not pass
`--no-index` (measurement 8): a tracked file that matches a pattern is tracked, and saying otherwise
would be the explorer's own untruth.

**D6 — The fixed list is demoted to a floor, not deleted, and the sidebar says which it is using.**
Inside a repository git's answer governs, and a repository that does not ignore `target/` gets an
expandable `target/`. Outside a repository, or when D4 says unknown, `IGNORED_DIRECTORY_NAMES`
remains — without it the first `target/` a user opens is a hundred thousand rows, which is exactly
what RFC-052 D7 protected against. **This corrects D7's wording**: the measurement says replacement
is conditional, so the honest form is a floor plus a stated source, not a replacement.
`GeneratedChangeDetectionPolicy` keeps the list unchanged in this RFC, which means the two consumers
of that shared array are **no longer symmetrical** — its doc comment must say so, or the next reader
will assume one list still means one thing.

**D7 — `REQ-FILE-006` is one setting, it governs ignored entries only, and dotfiles stay visible.**
`explorer.show_ignored`, default `false`, in RFC-054's mechanism. Measurement 3 is the reason for the
second half: nothing hides dotfiles today, and hiding `.env`, `.gitignore` and `.git-exclude` by
default would remove from view precisely the files a user opens when deciding what an AI agent can
read. `REQ-FILE-006` is permissive — "**may** be shown through an explicit user setting" — and it is
satisfied for ignored entries; for hidden ones no setting is needed because none are hidden. **This
is a deliberate reading of the requirement, recorded here so it is not rediscovered as a gap.**

**D8 — Ignore state is exactly as stale as the scan that produced it, and the product says so.**
There is no watcher until RFC-026 (`0.29.0`), so a `.gitignore` edited after a scan is not reflected
until the directory is scanned again. RFC-052 already discloses folder staleness in the sidebar;
extend that sentence rather than inventing a second vocabulary for the same fact, and the `0.26.0`
changelog owns the limitation in its own words.

## Non-goals

Parsing `.gitignore`. A file watcher, or any change of when scans happen. Hiding dotfiles. Changing
what `GeneratedChangeDetectionPolicy` counts, or the `changed_file_count` on the status bar. A
per-directory or per-pattern ignore UI. `--ignored` on the status invocation — measurement 5 rules it
out and this RFC does not leave it as an option.

## Risks

**R1 — one process per expansion.** A user expanding directories quickly spawns one `check-ignore`
each. Bounded by user action and measured at 1 ms, but the budget must be stated and held, not
assumed.

**R2 — the `./` discipline erodes.** D3 is structural for this reason: if the prefix is applied at
call sites rather than at one function, the next call site is the vulnerability.

**R3 — no work tree.** Bare repositories, a project that is not a repository, and a gate that refuses
git all reach D4's unknown path. Each must be exercised, not reasoned about.

**R4 — paths the scan never admitted.** `check-ignore` must be asked only about paths the explorer's
existing access policy already produced; the query is not a second way into the filesystem, and
symlink and root-escape handling stays where it is.

**R5 — the omitted tail.** The scanner caps a directory at 256 children and reports how many it left
out. Those rows have **unknown** ignore state and must not be drawn or described as anything else.

**R6 — bytes, not strings.** `-z` output is raw bytes and a filename need not be UTF-8. The status
parser already works on the raw byte stream for this reason; the new reader does the same.

## Acceptance criteria

- The hostile fixture contains a file named `:(glob)evil.log`, and every **sibling** of it still
  receives an ignore answer — the D3 falsification, failing without the `./` prefix.
- A repository whose `.gitignore` does **not** name `target/` shows `target/` as an ordinary
  expandable directory; one that does shows it collapsed with the `ignored` badge.
- A project that is not a repository shows the fixed-list floor **and says that is where the rule
  came from** (D6).
- A forced exit `128` shows unknown ignore state, not "not ignored" (D4), and a directory with
  nothing ignored (exit `1`) is drawn normally rather than as a failure.
- A tracked file matching an ignore pattern shows its tracked status, not `ignored` (D5, D8 of the
  measurements).
- `explorer.show_ignored = true` draws the ignored rows with the `ignored` badge; `false` collapses
  them; an unknown value is named on the Project Board and the default stands, as RFC-054's
  mechanism already does.
- The 20 000-`*.log` fixture: the number of paths Tekstide asks about stays bounded by the rows drawn
  (≤ 256 per directory), not by the ignored file count. **This is the test that falsifies D2 if D2 is
  wrong.**
- The 100 000-entry fixture from RFC-052 still expands within its budget, with the query added.
- `ignored_directories.rs`'s doc comment states that its two consumers are no longer symmetrical
  (D6).
- **Carried from RFC-053's closeout:** `rfc_docs_invariants` gains one check — an RFC the changelog
  names as released must live in `done/`. RFC-053 shipped in `0.23.0` and sat in `accepted/` for
  three releases because no invariant relates a release to a folder.
- The colour-alone, i18n completeness and internal-identifier scans still pass.
- Gate green three times with `--no-fail-fast`, and **0 fixture entries left in a fresh short
  `TMPDIR`**.
