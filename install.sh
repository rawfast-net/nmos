#!/usr/bin/env bash
set -euo pipefail
export PATH="${PATH:-/usr/bin:/bin}:/usr/local/sbin:/usr/sbin:/sbin"

INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
BASE_URL="${NMOS_RELEASE_URL:-https://github.com/rawfast-net/nmos/releases/latest/download}"
SCRIPT_DIR=""
if [[ -n "${BASH_SOURCE[0]:-}" && -f "${BASH_SOURCE[0]}" ]]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi
RUN_AFTER=1

for arg in "$@"; do
  case "$arg" in
    --run) RUN_AFTER=1 ;;
    --no-run) RUN_AFTER=0 ;;
    -h|--help) printf 'Использование: bash install.sh [--no-run]\n'; exit 0 ;;
    *) printf 'Неизвестный аргумент: %s\n' "$arg" >&2; exit 1 ;;
  esac
done

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  printf 'Готовая сборка доступна только для Linux x86_64.\n' >&2
  exit 1
fi

LIBC_VERSION="$(getconf GNU_LIBC_VERSION 2>/dev/null || true)"
if [[ ! "$LIBC_VERSION" =~ ^glibc[[:space:]]+([0-9]+)\.([0-9]+) ]]; then
  printf 'Не удалось определить glibc. Для готового бинарника нужна glibc 2.35 или новее.\n' >&2
  exit 1
fi
if (( 10#${BASH_REMATCH[1]} < 2 || (10#${BASH_REMATCH[1]} == 2 && 10#${BASH_REMATCH[2]} < 35) )); then
  printf 'Обнаружена %s. Для готового бинарника нужна glibc 2.35 или новее.\n' "$LIBC_VERSION" >&2
  exit 1
fi

run_as_root() {
  if [[ "$EUID" -eq 0 ]]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    printf 'Для установки пакетов нужны права root или sudo.\n' >&2
    return 1
  fi
}

need_curl=0; need_wget=0; need_coreutils=0; need_ip=0; need_util_linux=0; need_pciutils=0; need_ca=1
command -v curl >/dev/null 2>&1 || need_curl=1
command -v wget >/dev/null 2>&1 || need_wget=1
for cmd in sha256sum install mktemp df cp cat; do
  command -v "$cmd" >/dev/null 2>&1 || need_coreutils=1
done
command -v ip >/dev/null 2>&1 || need_ip=1
command -v lscpu >/dev/null 2>&1 || need_util_linux=1
command -v lspci >/dev/null 2>&1 || need_pciutils=1
for ca_file in "${SSL_CERT_FILE:-}" /etc/ssl/certs/ca-certificates.crt /etc/ssl/cert.pem /etc/ssl/ca-bundle.pem /etc/pki/tls/certs/ca-bundle.crt /etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem; do
  if [[ -n "$ca_file" && -r "$ca_file" ]]; then need_ca=0; break; fi
done

packages=()
PACKAGE_MANAGER=""
if command -v apt-get >/dev/null 2>&1; then PACKAGE_MANAGER=apt
elif command -v dnf >/dev/null 2>&1; then PACKAGE_MANAGER=dnf
elif command -v yum >/dev/null 2>&1; then PACKAGE_MANAGER=yum
elif command -v pacman >/dev/null 2>&1; then PACKAGE_MANAGER=pacman
fi
(( need_curl == 0 )) || packages+=(curl)
(( need_wget == 0 )) || packages+=(wget)
(( need_coreutils == 0 )) || packages+=(coreutils)
(( need_util_linux == 0 )) || packages+=(util-linux)
(( need_pciutils == 0 )) || packages+=(pciutils)
(( need_ca == 0 )) || packages+=(ca-certificates)
if (( need_ip )); then
  if [[ "$PACKAGE_MANAGER" == dnf || "$PACKAGE_MANAGER" == yum ]]; then packages+=(iproute); else packages+=(iproute2); fi
fi

if (( ${#packages[@]} )); then
  printf 'Нужны пакеты: %s\n' "${packages[*]}"
  if [[ "$PACKAGE_MANAGER" == apt ]]; then
    run_as_root apt-get update
    run_as_root apt-get install -y --no-install-recommends "${packages[@]}"
  elif [[ "$PACKAGE_MANAGER" == dnf ]]; then
    run_as_root dnf install -y "${packages[@]}"
  elif [[ "$PACKAGE_MANAGER" == yum ]]; then
    run_as_root yum install -y "${packages[@]}"
  elif [[ "$PACKAGE_MANAGER" == pacman ]]; then
    run_as_root pacman -S --needed --noconfirm "${packages[@]}"
  else
    printf 'Пакетный менеджер не найден. Установите пакеты вручную: %s\n' "${packages[*]}" >&2
    exit 1
  fi
fi

for cmd in curl wget sha256sum install mktemp df cp cat ip lscpu lspci; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    printf 'После установки не найдена команда: %s\n' "$cmd" >&2
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
  if [[ -t 0 ]]; then
    "$INSTALL_DIR/nmos"
  elif { : </dev/tty; } 2>/dev/null; then
    "$INSTALL_DIR/nmos" </dev/tty
  else
    "$INSTALL_DIR/nmos"
  fi
fi
