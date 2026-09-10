---
title: "Dependency currency: 69 in-semver updates, and a storage engine that moves two minors"
status: "Scoped 2026-09-10 by the architect. Not started."
rfc_file: "none — maintenance slice, scheduled at the 0.16.0→0.17.0 cycle review"
target_milestone: "M12"
created: "2026-09-10"
---

# Dependency currency

Scheduled after RFC-047 at the cycle review, deliberately. **That reasoning held up, but not for the
reason it was written.** Everything below was measured on 2026-09-10 against the released `0.17.0`
tree, not estimated.

## What is actually here

**Two changes, and they are not the same size.**

| | Measured |
| --- | --- |
| `cargo update` (in-semver) | **69 upgraded, 1 added, 0 removed.** Builds; gate green at 487 + 4 + 746. |
| Lockfile **downgrades** | **None.** Checked explicitly (below). |
| `rusqlite` 0.39.0 → 0.40.2 | **Zero Rust code changes.** Builds with no errors; gate green. |
| `libsqlite3-sys` | 0.37.0 → 0.38.2 |
| **Bundled SQLite** | **3.51.3 → 3.53.2** |

**The Rust API bump is a non-event. The last row is the slice.** A durable, security-relevant audit
store gets a storage engine two minor versions newer, and `cargo build` has nothing to say about
that.

## Why this was scheduled after RFC-047, and why that was right

The stated reason was that bumping the engine mid-RFC would entangle upstream behaviour changes with
our own. True, but the better reason is the one that only became visible now:

**RFC-047's corruption fixtures are the acceptance suite for a storage-engine change, and nothing
else in this project is.** Corruption detection, quarantine-by-rename, recovery, and resume all
depend on how SQLite *reports* a damaged file — error codes and messages that are not part of any
stability guarantee. Before RFC-047 there was no test that would have noticed if 3.53 classified a
corrupt database differently from 3.51.

**They pass.** That is what licenses this bump, and it should be said in the commit message rather
than left as "tests were green."

## The work

1. **`cargo update`.** 69 upgrades. No manifest change.
2. **`rusqlite` to `"0.40"`** in the workspace `[workspace.dependencies]`. One line.
3. Gate as usual: `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`, three
   consecutive full-workspace runs.

## Required checks specific to this slice

### 1. The lockfile-downgrade check — run it even though it came back clean here

A `cargo update` that makes a lockfile **shrink** can re-resolve a transitive dependency *downward*.
arama hit it via snora: `gpu-allocator` pulled `windows` to 0.56 while `wgpu-hal` still needed 0.58 —
ten compile errors, **Windows only, invisible on Linux**. We carry `gpu-allocator` and 29 `windows*`
crates in `Cargo.lock` via `iced` → `wgpu` and compile none of them.

**Measured on this update: 0 downgraded.** The trap did not fire. Run the check anyway and record
the numbers — a green check that was actually run is evidence; an unrun one is an assumption.
Compare `name`/`version` pairs across the before/after lockfile and assert nothing moved backwards,
rather than reading the `Updating …` lines, which only ever say "Updating".

**Compare pairs, not a name-keyed map (added 2026-09-10, response 375).** Dozens of crates appear at
two major versions at once — `bitflags` 1.x and 2.x, `syn` 2.x and 3.x, `getrandom`, `hashbrown`,
and more. A check that builds `map[name] = version` silently keeps whichever entry it read last and
compares the wrong pair, or none. **The reviewer's own first check had exactly this defect**, and
the implementer's did not. Build a multiset of `(name, version)` pairs, group by `(name, major)`,
and compare the maximum within each track. Counting by track and counting by name differ by design:
this update is **73 tracks** or **72 names**, because `syn` moved on both of its.

**Span both steps as one before/after (corrected 2026-09-10, response 374).** The first attempt ran
this check across the `cargo update` alone, where nothing was removed, and reported "0 names lost
entirely". Two were: `rusqlite 0.39.0` depends on `sqlite-wasm-rs` (and `rsqlite-vfs` behind it) and
`0.40.2` does not, so **the manifest bump is the half that shrank the graph** — and a shrinking
lockfile is the trap's own trigger condition, because it leaves the resolver fewer constraints and
frees it to pick a *lower* version elsewhere. Checking the step that did not shrink is checking the
safe half. Take one lockfile snapshot before any change and one after both.

### 2. A store written by the new engine must still be readable by the old one

**Not tested, and I could not test it cheaply — this is the one real risk and it is yours to
close.** SQLite's file format is stable across versions as a rule, but a newer engine can enable
behaviour an older one rejects, and this store is a durable record a user may need after
*downgrading* tekstide.

**Keep the scaffolding (added 2026-09-10, response 374).** This check recurs at every `rusqlite`
minor, because each one carries a new bundled SQLite. Whatever performs it belongs in
`crates/tekstide-core/examples/` with a header saying it must be run under two different lockfile
pins — not written, run once, and deleted. The default of "verification scaffolding is not product
code" is right in general and wrong here, because the property being verified belongs to the storage
engine and changes on someone else's release schedule.

**The test: create and populate an audit store under `rusqlite` 0.40 (SQLite 3.53.2), then open and
query it under 0.39 (3.51.3).** Both directions matter, but this is the one that can lose a user's
records. If it fails, say so plainly — it does not necessarily block the bump, but it makes the bump
a thing the changelog must warn about, and it is far better found here than by someone who
downgraded.

### 3. Say what moved, in the changelog

A user reading "dependency updates" learns nothing. The bundled SQLite version changing under an
audit store is the kind of fact this project states plainly — the same honesty rule the `0.17.0`
entry applied to what it did *not* do.

## Not in this slice

- **Any change to how the audit store is used.** If the new engine makes something newly possible,
  that is its own decision, not a rider on a version bump.
- **Pinning `rusqlite` more tightly**, or moving off `bundled`. Both are real questions; neither is
  this.

## Evidence to produce

The usual gate, plus: the downgrade check's numbers, and check 2's result stated either way. Package
smoke is not needed — this slice ships nothing on its own; it lands ahead of the next release, which
runs the full release checklist.
