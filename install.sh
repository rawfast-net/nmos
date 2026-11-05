#!/usr/bin/env bash
# nmos — build & install on Linux VDS
#   bash install.sh
#   bash install.sh --run

set -euo pipefail

APP_NAME="nmos"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUN_AFTER=0
OFFLINE_BUILD=0

for arg in "$@"; do
  case "$arg" in
    --run) RUN_AFTER=1 ;;
    --offline) OFFLINE_BUILD=1 ;;
    -h|--help)
      echo "Usage: bash install.sh [--run] [--offline]"
      exit 0
      ;;
  esac
done

need_cmd() { command -v "$1" >/dev/null 2>&1; }

echo "nmos install"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "warn: target OS is Linux, current: $(uname -s)"
fi

install_pkgs() {
  if [[ "$OFFLINE_BUILD" -eq 1 ]]; then
    echo "skipping apt/dnf (--offline)"
    return 0
  fi

  if need_cmd apt-get; then
    export DEBIAN_FRONTEND=noninteractive
    sudo apt-get update -y
    sudo apt-get install -y curl ca-certificates build-essential gcc pkg-config iproute2 pciutils util-linux
  elif need_cmd dnf; then
    sudo dnf install -y curl ca-certificates gcc make pkgconf iproute pciutils util-linux
  elif need_cmd yum; then
    sudo yum install -y curl ca-certificates gcc make pkgconfig iproute pciutils util-linux
  elif need_cmd apk; then
    sudo apk add --no-cache curl ca-certificates build-base pkgconf iproute2 pciutils util-linux
  else
    echo "no apt/dnf/yum/apk — install gcc/curl yourself"
  fi
}

if ! need_cmd curl; then
  install_pkgs
fi
if ! need_cmd cc && ! need_cmd gcc; then
  install_pkgs
elif need_cmd sudo && [[ "$OFFLINE_BUILD" -eq 0 ]]; then
  install_pkgs || true
fi

if ! need_cmd cargo; then
  echo "installing rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

if ! need_cmd cargo; then
  # shellcheck disable=SC1091
  [[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
fi

if ! need_cmd cargo; then
  echo "cargo not found after rustup"
  exit 1
fi

echo "rustc: $(rustc --version)"
echo "building release..."

cd "$SRC_DIR"
cargo build --release

BIN="$SRC_DIR/target/release/$APP_NAME"
if [[ ! -x "$BIN" ]]; then
  echo "binary missing: $BIN"
  exit 1
fi

echo "installing to $INSTALL_DIR ..."
if [[ -w "$INSTALL_DIR" ]]; then
  install -m 755 "$BIN" "$INSTALL_DIR/$APP_NAME"
else
  sudo install -m 755 "$BIN" "$INSTALL_DIR/$APP_NAME"
fi

echo "ok: $(command -v $APP_NAME || echo $INSTALL_DIR/$APP_NAME)"
echo "  nmos"
echo "  nmos -d 8"
echo "  nmos --json"

if [[ "$RUN_AFTER" -eq 1 ]]; then
  exec "$INSTALL_DIR/$APP_NAME" -d 5
fi
