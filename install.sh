#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
BASE_URL="${NMOS_RELEASE_URL:-https://github.com/rawfast-net/nmos/releases/latest/download}"
SCRIPT_DIR=""
if [[ -n "${BASH_SOURCE[0]:-}" && -f "${BASH_SOURCE[0]}" ]]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi
RUN_AFTER=0

for arg in "$@"; do
  case "$arg" in
    --run) RUN_AFTER=1 ;;
    -h|--help) printf 'Использование: bash install.sh [--run]\n'; exit 0 ;;
    *) printf 'Неизвестный аргумент: %s\n' "$arg" >&2; exit 1 ;;
  esac
done

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  printf 'Готовая сборка доступна только для Linux x86_64.\n' >&2
  exit 1
fi

for cmd in curl sha256sum install mktemp; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    printf 'Не найдена команда: %s\n' "$cmd" >&2
    exit 1
  fi
done

TMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TMP_DIR"' EXIT
if [[ -n "$SCRIPT_DIR" && -f "$SCRIPT_DIR/nmos-linux-x86_64" && -f "$SCRIPT_DIR/nmos-linux-x86_64.sha256" ]]; then
  cp -- "$SCRIPT_DIR/nmos-linux-x86_64" "$TMP_DIR/nmos-linux-x86_64"
  cp -- "$SCRIPT_DIR/nmos-linux-x86_64.sha256" "$TMP_DIR/nmos-linux-x86_64.sha256"
else
  printf 'Загрузка готовой сборки...\n'
  for file in nmos-linux-x86_64 nmos-linux-x86_64.sha256; do
    if ! curl --fail --location --silent --show-error --proto '=https' --tlsv1.2 "$BASE_URL/$file" -o "$TMP_DIR/$file"; then
      printf 'Не удалось загрузить %s. Проверьте наличие файлов в последнем релизе.\n' "$file" >&2
      exit 1
    fi
  done
fi
CHECKSUM="$(cat "$TMP_DIR/nmos-linux-x86_64.sha256")"
if [[ ! "$CHECKSUM" =~ ^[0-9a-fA-F]{64}[[:space:]][[:space:]]nmos-linux-x86_64$ ]]; then
  printf 'Некорректный файл контрольной суммы.\n' >&2
  exit 1
fi
(cd "$TMP_DIR" && sha256sum --check nmos-linux-x86_64.sha256)
if [[ -w "$INSTALL_DIR" ]]; then
  install -m 755 "$TMP_DIR/nmos-linux-x86_64" "$INSTALL_DIR/nmos"
elif command -v sudo >/dev/null 2>&1; then
  sudo install -m 755 "$TMP_DIR/nmos-linux-x86_64" "$INSTALL_DIR/nmos"
else
  printf 'Нет прав для записи в %s. Запустите от root или задайте INSTALL_DIR.\n' "$INSTALL_DIR" >&2
  exit 1
fi
printf 'Установлено: %s/nmos\n' "$INSTALL_DIR"
if [[ "$RUN_AFTER" -eq 1 ]]; then
  exec "$INSTALL_DIR/nmos"
fi
