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
