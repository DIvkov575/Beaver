#!/usr/bin/env sh
# Beaver installer.
# Detects OS/arch, downloads the matching release binary from GitHub, drops it
# in ~/.local/bin/beaver, and tells you what to do about PATH.
#
# Usage:
#   curl -sSf https://raw.githubusercontent.com/Divkov575/Beaver/main/install.sh | sh
#
# Override the install dir:
#   BEAVER_INSTALL_DIR=/usr/local/bin curl -sSf .../install.sh | sh
#
# Pin a specific version (default: latest):
#   BEAVER_VERSION=v0.3.0 curl -sSf .../install.sh | sh

set -eu

REPO="Divkov575/Beaver"
INSTALL_DIR="${BEAVER_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${BEAVER_VERSION:-latest}"

die() {
    printf 'beaver: %s\n' "$*" >&2
    exit 1
}

info() {
    printf 'beaver: %s\n' "$*"
}

# ---- detect platform ------------------------------------------------------
detect_target() {
    os=$(uname -s)
    arch=$(uname -m)

    case "$os" in
        Darwin) os_tag="apple-darwin" ;;
        Linux)  os_tag="unknown-linux-gnu" ;;
        *) die "unsupported OS: $os (only macOS and Linux are released)" ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch_tag="x86_64" ;;
        arm64|aarch64)  arch_tag="aarch64" ;;
        *) die "unsupported architecture: $arch" ;;
    esac

    echo "${arch_tag}-${os_tag}"
}

# ---- resolve version ------------------------------------------------------
resolve_version() {
    if [ "$VERSION" = "latest" ]; then
        # GitHub redirects /releases/latest to the actual tag.
        url=$(curl -sSfLI -o /dev/null -w '%{url_effective}' \
            "https://github.com/${REPO}/releases/latest")
        VERSION=$(basename "$url")
        [ -n "$VERSION" ] || die "could not resolve latest release"
    fi
    echo "$VERSION"
}

# ---- download + install ---------------------------------------------------
main() {
    command -v curl >/dev/null 2>&1 || die "curl is required"
    command -v tar  >/dev/null 2>&1 || die "tar is required"

    target=$(detect_target)
    version=$(resolve_version)
    archive="beaver-${version}-${target}.tar.gz"
    url="https://github.com/${REPO}/releases/download/${version}/${archive}"

    info "installing beaver ${version} (${target}) to ${INSTALL_DIR}"

    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    curl -sSfL -o "${tmpdir}/${archive}" "$url" \
        || die "download failed: $url"
    tar -xzf "${tmpdir}/${archive}" -C "$tmpdir"

    mkdir -p "$INSTALL_DIR"
    mv "${tmpdir}/beaver" "${INSTALL_DIR}/beaver"
    chmod +x "${INSTALL_DIR}/beaver"

    info "installed ${INSTALL_DIR}/beaver"

    # PATH hint
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            info ""
            info "${INSTALL_DIR} is not on your PATH. Add this to your shell rc:"
            info "    export PATH=\"${INSTALL_DIR}:\$PATH\""
            info ""
            ;;
    esac

    info "run: beaver --help"
}

main "$@"
