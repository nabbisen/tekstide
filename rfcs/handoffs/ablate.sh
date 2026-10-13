#!/usr/bin/env bash
# Ablation runner: proves a test can fail, without being able to lose work.
#
#   rfcs/handoffs/ablate.sh <label> <file> <old-text> <new-text> [cargo test args...]
#
# It **refuses to run unless `git status --porcelain` is empty**, so the
# `git checkout -- <file>` that restores the file afterwards can only ever
# discard the ablation itself. Twice in this project a restore discarded
# uncommitted work (RFC-030 PR-030-D, RFC-052 PR-052-B); vigilance was not the
# fix, this check is. Commit first, then ablate.
#
# Prints the failing test names and restores the file, whatever the result.
#
# ---------------------------------------------------------------------------
# WHEN THIS SCRIPT IS THE WRONG INSTRUMENT (added 2026-10-11, RFC-059 review 527)
#
# It always runs `cargo test --workspace --all-targets`, so the whole suite runs
# concurrently. **Some defects are masked by that concurrency.** RFC-059's own
# first-creation race is a SQLite multi-process busy-handler livelock that needs
# low-noise timing: through this script it did not reproduce in five attempts,
# and in isolation it failed 5 of 5. A green run here is then not evidence the
# check cannot fail -- it is evidence this instrument cannot see it.
#
# Do NOT answer that by editing a dirty tree by hand. The whole point of the
# clean-tree refusal above is that "the dirty state was only my deliberate
# one-line change" is the vigilance argument this check exists to replace.
#
# Use a throwaway worktree instead. It is isolated, the main tree is never
# dirty, and nothing you have not committed can be lost:
#
#   WT=$(mktemp -d)/wt
#   git worktree add -q --detach "$WT" HEAD
#   # edit the ablation into $WT, then, inside it:
#   #   cargo build -p <crate> --bins        # helper [[bin]]s: `--lib` does not build them
#   #   cargo test -p <crate> --lib <test>   # the narrow, low-noise invocation
#   git worktree remove --force "$WT"
#
# The `--bins` step is not optional: a test that spawns a helper binary fails
# with "expected the <name> binary at .../target/debug/<name>" when it was not
# built, which looks exactly like the defect failing and is not.
#
# **Keep the invocation narrow in a worktree, and never read a workspace-wide
# run there as clean or dirty evidence** (added 2026-10-14, RFC-029 review 553).
# A linked worktree's own `.git` is a *file* (`gitdir: ...`), not a directory, so
# `real_repository_filesystem_scan_cost_headless_benchmark` -- which asserts it
# is measuring a real repository before it will report a number -- fails there
# **3 of 3, deterministically**, exactly as `release-checklist.md` already
# records it failing from an unpacked package. That is the test doing its job in
# both places. But it means `cargo test --workspace` inside a worktree always
# reports one failure that is the instrument, not the ablation, and a summary
# that counts failures rather than naming them reads it as the ablated check
# failing. Name the failing tests; the count is not the finding.
# ---------------------------------------------------------------------------
set -u
label=$1; file=$2; old=$3; new=$4; shift 4
cd "$(git rev-parse --show-toplevel)" || exit 2
if [ -n "$(git status --porcelain)" ]; then
  echo "ablate.sh: refusing -- the working tree is not clean (commit first):" >&2
  git status --porcelain >&2
  exit 2
fi
if ! python3 - "$file" "$old" "$new" <<'PY'
import sys
path, old, new = sys.argv[1:4]
text = open(path).read()
if old not in text:
    sys.exit("ablate.sh: the text to replace is not in " + path)
open(path, "w").write(text.replace(old, new, 1))
PY
then
  git checkout -- "$file"
  exit 2
fi
echo "=== $label"
# The whole run goes to a log first, so **the filter below can never be the only
# copy of the evidence**: the failing names are for reading, and the assertion
# messages -- what actually failed, and with which values -- are printed after
# them and kept in full in the log. (RFC-055 review 433: a test failed in an
# ablation run and the filter kept the name and lost the message.)
log="$(git rev-parse --show-toplevel)/target/ablate-last.log"
mkdir -p "$(dirname "$log")"
cargo test --workspace --all-targets --no-fail-fast "$@" > "$log" 2>&1
grep -E "^test .*FAILED|^error(\[|:)" "$log" \
  | sed 's/^test //; s/ \.\.\. FAILED//' \
  | grep -v "^error: test failed\|^error: [0-9]* targets\? failed"
if grep -q "panicked at" "$log"; then
  echo "--- assertion messages (full output: $log)"
  grep -E "panicked at" -A3 "$log" | grep -v "^--$" | grep -v "RUST_BACKTRACE" | head -60
fi
git checkout -- "$file"
if [ -n "$(git status --porcelain)" ]; then
  echo "ablate.sh: the tree is not clean after restoring $file" >&2
  exit 3
fi
