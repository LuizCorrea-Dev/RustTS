#!/usr/bin/env bash
# scripts/install.sh — Instalador Oficial do Compilador RustTS (rtsc) para Linux / macOS
set -euo pipefail

REPO="${RTSC_REPO:-LuizCorrea-Dev/RustTS}"
INSTALL_DIR="${HOME}/.rtsc/bin"
BIN_PATH="${INSTALL_DIR}/rtsc"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
  Linux)
    ASSET_NAME="rtsc-linux-x64"
    ;;
  Darwin)
    if [ "${ARCH}" = "arm64" ]; then
      ASSET_NAME="rtsc-macos-arm64"
    else
      ASSET_NAME="rtsc-macos-arm64"
    fi
    ;;
  *)
    echo "Unsupported OS: ${OS}" >&2
    exit 1
    ;;
esac

mkdir -p "${INSTALL_DIR}"
RELEASE_URL="https://github.com/${REPO}/releases/latest/download/${ASSET_NAME}"

echo "==> Baixando binário nativo de ${RELEASE_URL} ..."
curl -fsSL "${RELEASE_URL}" -o "${BIN_PATH}"
chmod +x "${BIN_PATH}"

echo "==> RustTS instalado em ${BIN_PATH}"
echo "==> Adicione ao PATH no seu shell (~/.bashrc ou ~/.zshrc):"
echo "    export PATH=\"\$HOME/.rtsc/bin:\$PATH\""

