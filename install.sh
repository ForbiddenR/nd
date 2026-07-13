#!/usr/bin/env bash
#
# install.sh — install the latest release of `nd` to /usr/local/bin.
#
# Downloads the Linux binary tarball from the GitHub releases page, verifies
# the bundled binary runs, and installs it to /usr/local/bin/nd. Designed to
# be curl|bash safe: passes set -euo pipefail, only writes inside the install
# prefix plus a temporary staging directory it cleans up on exit.
#
#   curl -fsSL https://raw.githubusercontent.com/<owner>/<repo>/rs/install.sh | bash
#
# Or, after cloning:
#   ./install.sh
#
set -euo pipefail

# Repo in <owner>/<repo> form. Override with ND_REPO to point at a fork.
: "${ND_REPO:=ForbiddenR/nd}"
# Git ref containing this installer. Override when a fork uses another branch.
: "${ND_INSTALL_REF:=rs}"

INSTALL_DIR="/usr/local/bin"
BINARY_NAME="nd"
TMPDIR_ROOT="$(mktemp -d)"
cleanup() {
    rm -rf "$TMPDIR_ROOT"
}
trap cleanup EXIT

# Detect the machine architecture and pick a matching release asset.
# Falls back to x86_64 if the architecture is unknown, since that is what the
# release workflow currently builds.
arch="$(uname -m)"
case "$arch" in
    x86_64 | amd64) asset_arch="x86_64-unknown-linux-musl" ;;
    aarch64 | arm64) asset_arch="aarch64-unknown-linux-musl" ;;
    *) asset_arch="x86_64-unknown-linux-musl" ;;
esac

asset_name="nd-latest-${asset_arch}.tar.gz"

require() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'error: %s is required but not found on PATH\n' "$1" >&2
        exit 1
    fi
}

require curl
require tar

# Root may be needed to write to /usr/local/bin. Re-exec with sudo if we can't
# write there directly, so the script also works as a non-root user. When piped
# to bash (curl|bash), `$0` is just "bash" with no script path, so in that case
# we download a copy of the script to a temp file and re-run that under sudo.
if [ ! -w "$INSTALL_DIR" ]; then
    if [ "$(id -u)" -ne 0 ] && command -v sudo >/dev/null 2>&1; then
        printf 'install dir %s is not writable; re-running with sudo\n' "$INSTALL_DIR" >&2
        if [ -f "$0" ]; then
            exec sudo -E env ND_REPO="$ND_REPO" ND_INSTALL_REF="$ND_INSTALL_REF" INSTALL_DIR="$INSTALL_DIR" bash "$0" "$@"
        else
            script="${TMPDIR_ROOT}/install.sh"
            curl -fsSL "https://raw.githubusercontent.com/${ND_REPO}/${ND_INSTALL_REF}/install.sh" -o "$script"
            exec sudo -E env ND_REPO="$ND_REPO" ND_INSTALL_REF="$ND_INSTALL_REF" INSTALL_DIR="$INSTALL_DIR" bash "$script" "$@"
        fi
    fi
    printf 'error: cannot write to %s and sudo is unavailable\n' "$INSTALL_DIR" >&2
    exit 1
fi

printf 'Fetching the latest release of %s...\n' "$ND_REPO"

# Resolve the latest release tag. The /latest endpoint redirects to the
# concrete tag URL; -w '%{url_effective}' captures the final location and
# we extract the tag from its trailing path segment.
latest_url="$(curl -fsSL \
    -o /dev/null \
    -w '%{url_effective}' \
    "https://github.com/${ND_REPO}/releases/latest")"

tag="${latest_url##*/}"
if [ -z "$tag" ] || [ "$tag" = "latest" ]; then
    printf 'error: could not determine the latest release tag for %s\n' "$ND_REPO" >&2
    printf '       (does a published release exist?)\n' >&2
    exit 1
fi

asset_url="https://github.com/${ND_REPO}/releases/download/${tag}/nd-${tag}-${asset_arch}.tar.gz"

# Try the versioned asset name first; fall back to a "latest" alias if a
# release uses that naming scheme instead.
if ! curl -fsSL -o "${TMPDIR_ROOT}/${asset_name}" "$asset_url"; then
    fallback_url="https://github.com/${ND_REPO}/releases/latest/download/nd-latest-${asset_arch}.tar.gz"
    printf 'versioned asset not found; trying %s\n' "$fallback_url"
    if ! curl -fsSL -o "${TMPDIR_ROOT}/${asset_name}" "$fallback_url"; then
        printf 'error: could not download the %s release asset for %s\n' "$asset_arch" "$tag" >&2
        printf '       (checked: %s)\n' "$asset_url" >&2
        exit 1
    fi
fi

printf 'Extracting %s...\n' "${asset_name}"
tar -xzf "${TMPDIR_ROOT}/${asset_name}" -C "$TMPDIR_ROOT"

# The tarball contains a staging directory (nd-<tag>-<arch>/) holding the
# binary plus README/LICENSE. Find the binary inside it.
binary="$(find "$TMPDIR_ROOT" -type f -name "$BINARY_NAME" -perm -u+x | head -n 1)"
if [ -z "$binary" ]; then
    printf 'error: %s binary not found in the archive\n' "$BINARY_NAME" >&2
    exit 1
fi

# Sanity check: the binary should be an ELF executable before we install it.
if ! file "$binary" 2>/dev/null | grep -q -e 'ELF' -e 'executable'; then
    printf 'warning: %s does not look like an ELF executable; installing anyway\n' "$binary" >&2
fi

printf 'Installing %s to %s/%s\n' "$BINARY_NAME" "$INSTALL_DIR" "$BINARY_NAME"
install -m 0755 "$binary" "${INSTALL_DIR}/${BINARY_NAME}"

installed="${INSTALL_DIR}/${BINARY_NAME}"
printf 'Installed nd %s -> %s\n' "$tag" "$installed"

# Confirm it is reachable, if a PATH check is cheap to do.
if command -v "$BINARY_NAME" >/dev/null 2>&1; then
    printf 'Run "nd" to start, or "nd --help" if a help subcommand exists.\n'
else
    printf 'note: %s is not on your PATH; add %s to PATH to use it.\n' "$BINARY_NAME" "$INSTALL_DIR"
fi
