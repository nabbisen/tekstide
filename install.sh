#!/bin/sh
# Tekstide installer.
#
# Downloads the release binary for this platform (Linux x86_64, or macOS on
# Apple Silicon -- RFC-028 D7), verifies it against its own published
# checksum, and installs ONE binary to $HOME/.local/bin/tekstide -- plus the
# directory it goes in, if that does not already exist. Touches nothing else
# -- no shell rc file, no package repository, no sudo. See
# docs/src/users/getting-started.md for what this script does and does not
# do, stated before the command, not only here (RFC-029 D12,
# what-the-installer-must-not-do.md §1).
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
  # RFC-028 D7/PR-028-F: the first macOS artifact is `aarch64-apple-darwin`
  # only -- an Intel Mac is refused by name below, not silently sent the
  # wrong binary, since no `x86_64-apple-darwin` artifact is built yet.
  case "$os" in
    Linux)
      case "$arch" in
        x86_64) asset_name="tekstide-x86_64-linux" ;;
        *)
          echo "tekstide install: no released build for Linux '$arch' yet (x86_64 only)." >&2
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        arm64) asset_name="tekstide-aarch64-macos" ;;
        *)
          echo "tekstide install: no released build for macOS '$arch' yet (Apple Silicon only -- see RFC-028)." >&2
          exit 1
          ;;
      esac
      ;;
    *)
      echo "tekstide install: no released build for '$os' yet (Linux and macOS only -- see RFC-028)." >&2
      exit 1
      ;;
  esac

  version="${TEKSTIDE_VERSION:-latest}"
  base_url="${TEKSTIDE_INSTALL_BASE_URL:-https://github.com/nabbisen/tekstide/releases}"
  if [ "$version" = "latest" ]; then
    asset_url="$base_url/latest/download/$asset_name"
    checksum_url="$base_url/latest/download/$asset_name.sha256"
  else
    asset_url="$base_url/download/$version/$asset_name"
    checksum_url="$base_url/download/$version/$asset_name.sha256"
  fi

  workdir="$(mktemp -d)"
  trap 'rm -rf "$workdir"' EXIT INT TERM

  echo "tekstide install: downloading $asset_url"
  tekstide_download "$asset_url" "$workdir/$asset_name"
  tekstide_download "$checksum_url" "$workdir/$asset_name.sha256"

  echo "tekstide install: verifying against its published checksum"
  # `sha256sum` is GNU coreutils and not present on macOS by default;
  # `shasum -a 256` is the equivalent macOS ships out of the box. Prefer
  # `sha256sum` when it exists (Linux, and a macOS with coreutils
  # installed) since it is the more common name.
  if command -v sha256sum >/dev/null 2>&1; then
    checksum_cmd="sha256sum -c $asset_name.sha256"
  elif command -v shasum >/dev/null 2>&1; then
    checksum_cmd="shasum -a 256 -c $asset_name.sha256"
  else
    echo "tekstide install: neither sha256sum nor shasum is available to verify the download." >&2
    exit 1
  fi
  if ! ( cd "$workdir" && $checksum_cmd ) >/dev/null; then
    echo "tekstide install: checksum did not match -- refusing to install a corrupted or incomplete download." >&2
    exit 1
  fi

  install_dir="$HOME/.local/bin"
  mkdir -p "$install_dir"
  install -m 755 "$workdir/$asset_name" "$install_dir/tekstide"

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
    curl -fsSL -o "$dest" "$url" || tekstide_download_failed "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$dest" "$url" || tekstide_download_failed "$url"
  else
    echo "tekstide install: neither curl nor wget is available." >&2
    exit 1
  fi
}

# Named here, in this script's own voice, rather than left to whatever curl
# or wget print on their own -- wget's own quiet failure (`-q`) gives no
# output at all otherwise, and a bare curl error names the URL but not what
# a reader should do about it.
tekstide_download_failed() {
  echo "tekstide install: failed to download $1" >&2
  echo "tekstide install: this release may not publish an asset for your platform or the requested version -- see https://github.com/nabbisen/tekstide/releases for what it actually carries." >&2
  exit 1
}

tekstide_install
