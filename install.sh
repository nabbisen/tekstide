#!/bin/sh
# Tekstide installer.
#
# Downloads the Linux x86_64 release binary, verifies it against its own
# published checksum, and installs ONE file to ONE location:
# $HOME/.local/bin/tekstide. Touches nothing else -- no shell rc file, no
# package repository, no sudo. See docs/src/users/getting-started.md for
# what this script does and does not do, stated before the command, not
# only here (RFC-029 D12, what-the-installer-must-not-do.md §1).
#
# RFC-029 D9-D12.
#
# **Truncation-safe (D11, §2).** The whole body is one function, invoked
# only on the last line, after the function's own closing brace has been
# read. A connection that drops mid-download of *this script* leaves `sh`
# holding a syntactically incomplete function definition with no trailing
# call to run -- it refuses to execute anything, rather than running
# whatever prefix happened to arrive.
#
# **The checksum is published from the same release as the binary** (D10,
# §3): it proves this download arrived intact, not that the release itself
# is untampered or that the binary can be independently rebuilt from
# source -- Tekstide's own release build is not byte-reproducible (measured
# at the `0.36.0` tag, two independent builds of identical source produced
# different checksums). This script only ever says "verified against its
# published checksum" for that reason, never "secure", "safe", or "signed".
set -eu

tekstide_install() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux) ;;
    *)
      echo "tekstide install: no released build for '$os' yet (Linux only -- see RFC-028)." >&2
      exit 1
      ;;
  esac
  case "$arch" in
    x86_64) ;;
    *)
      echo "tekstide install: no released build for '$arch' yet (x86_64 only)." >&2
      exit 1
      ;;
  esac

  version="${TEKSTIDE_VERSION:-latest}"
  base_url="${TEKSTIDE_INSTALL_BASE_URL:-https://github.com/nabbisen/tekstide/releases}"
  if [ "$version" = "latest" ]; then
    asset_url="$base_url/latest/download/tekstide-x86_64-linux"
    checksum_url="$base_url/latest/download/tekstide-x86_64-linux.sha256"
  else
    asset_url="$base_url/download/$version/tekstide-x86_64-linux"
    checksum_url="$base_url/download/$version/tekstide-x86_64-linux.sha256"
  fi

  workdir="$(mktemp -d)"
  trap 'rm -rf "$workdir"' EXIT INT TERM

  echo "tekstide install: downloading $asset_url"
  tekstide_download "$asset_url" "$workdir/tekstide-x86_64-linux"
  tekstide_download "$checksum_url" "$workdir/tekstide-x86_64-linux.sha256"

  echo "tekstide install: verifying against its published checksum"
  if ! ( cd "$workdir" && sha256sum -c tekstide-x86_64-linux.sha256 ) >/dev/null; then
    echo "tekstide install: checksum did not match -- refusing to install a corrupted or incomplete download." >&2
    exit 1
  fi

  install_dir="$HOME/.local/bin"
  mkdir -p "$install_dir"
  install -m 755 "$workdir/tekstide-x86_64-linux" "$install_dir/tekstide"

  echo "tekstide install: installed to $install_dir/tekstide"
  case ":$PATH:" in
    *":$install_dir:"*)
      echo "tekstide install: $install_dir is already on your PATH. Run 'tekstide' to start."
      ;;
    *)
      echo "tekstide install: $install_dir is NOT on your PATH yet."
      echo "tekstide install: add it yourself (for example: export PATH=\"$install_dir:\$PATH\"), or run $install_dir/tekstide directly."
      ;;
  esac
}

tekstide_download() {
  url="$1"
  dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$dest" "$url"
  else
    echo "tekstide install: neither curl nor wget is available." >&2
    exit 1
  fi
}

tekstide_install
