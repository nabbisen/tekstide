#!/usr/bin/env bash
# Post-publish verification of ONE published `tekstide` release, from the registry.
#
#   rfcs/handoffs/post-publish-check.sh <version> [--no-install]
#
# Added by RFC-056 PR-056-A, after `0.26.0`'s post-publish check found -- by resolving an
# OLDER release rather than the one just published -- that `cargo install tekstide --version
# 0.25.0` no longer compiles: every published `tekstide` pinned its core as `version = "0"`
# (any 0.x), and the app archives' lockfiles named the PREVIOUS core. Published metadata is
# frozen, so this cannot repair a release; it makes the next one prove itself.
#
# Three checks, each of which would have caught it:
#   1. the app's `tekstide-core` requirement is THIS release's version, not "0";
#   2. the app archive's Cargo.lock names the MATCHING core;
#   3. (unless --no-install) `cargo install tekstide --version <v>` into a temporary root
#      builds. Slow -- a full build -- and the only check that runs the compiler.
# Exit 0 only if every check that ran passed. Run it for the release just published AND for
# the previous one: a new core can break an old app, and only an old app shows it.
set -u
version=${1:?usage: post-publish-check.sh <version> [--no-install]}
install=yes
[ "${2:-}" = "--no-install" ] && install=no
work=$(mktemp -d "${TMPDIR:-/tmp}/tekstide-postpublish.XXXXXX")
trap 'rm -rf "$work"' EXIT
fail=0
say() { printf '%s\n' "$*"; }

say "== tekstide $version: fetching the app archive from the registry"
curl -sfL "https://static.crates.io/crates/tekstide/tekstide-$version.crate" -o "$work/app.crate" \
  || { say "FAIL: could not download tekstide-$version.crate"; exit 2; }
tar xzf "$work/app.crate" -C "$work"
app="$work/tekstide-$version"

# 1. the requirement on the core
req=$(python3 - "$app/Cargo.toml" <<'PY'
import sys, re
text = open(sys.argv[1]).read()
m = re.search(r'\[dependencies\.tekstide-core\]\s*\n((?:[^\[]*\n)*)', text)
v = re.search(r'version\s*=\s*"([^"]*)"', m.group(1)) if m else None
print(v.group(1) if v else "")
PY
)
say "   tekstide-core requirement in the app's Cargo.toml: \"$req\""
major_minor=${version%.*}
case "$req" in
  "$version"|"^$version"|"=$version") say "   ok: pinned to this release's own version" ;;
  "$major_minor".*) say "   ok: pinned within this release's minor ($major_minor.x)" ;;
  *) say "FAIL: \"$req\" is not this release's ($version) minor -- an older app will resolve any newer core"; fail=1 ;;
esac

# 2. the lockfile's core
locked=$(python3 - "$app/Cargo.lock" <<'PY'
import sys, re
t = open(sys.argv[1]).read()
m = re.search(r'name = "tekstide-core"\s*\nversion = "([^"]*)"', t)
print(m.group(1) if m else "")
PY
)
say "   tekstide-core in the app's Cargo.lock: \"$locked\""
if [ "$locked" = "$version" ]; then say "   ok: the lockfile names the matching core"
else say "FAIL: the lockfile names core \"$locked\", not $version -- \`--locked\` builds a pair that cannot compile"; fail=1; fi

# 3. install it
if [ "$install" = yes ]; then
  say "== cargo install tekstide --version $version (temporary root)"
  if cargo install tekstide --version "$version" --root "$work/root" > "$work/install.log" 2>&1; then
    say "   ok: it built"
  else
    say "FAIL: it did not build. The errors:"
    grep -E "^error" "$work/install.log" | head -12 | sed 's/^/   /'
    fail=1
  fi
fi
[ "$fail" = 0 ] && say "== PASS" || say "== FAIL"
exit $fail
