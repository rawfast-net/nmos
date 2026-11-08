#!/usr/bin/env bash
set -euo pipefail

test_dir="$(mktemp -d)"
cleanup() {
  rm -f -- "$test_dir/bin/nmos" "$test_dir/install.sh" "$test_dir/nmos-linux-x86_64" "$test_dir/nmos-linux-x86_64.sha256" "$test_dir/packages.log" "$test_dir/installed" "$test_dir/ran" "$test_dir/ca.pem"
  rmdir -- "$test_dir/bin" "$test_dir" 2>/dev/null || true
}
trap cleanup EXIT
mkdir "$test_dir/bin"
cp install.sh "$test_dir/install.sh"
printf '%s\n' '#!/usr/bin/env bash' 'printf ran > "$NMOS_TEST_DIR/ran"' > "$test_dir/nmos-linux-x86_64"
(cd "$test_dir" && sha256sum --text nmos-linux-x86_64 > nmos-linux-x86_64.sha256)
printf 'test\n' > "$test_dir/ca.pem"
export NMOS_TEST_DIR="$test_dir" SSL_CERT_FILE="$test_dir/ca.pem"

uname() { if [[ "$1" == '-s' ]]; then printf Linux; else printf x86_64; fi; }
getconf() { printf 'glibc 2.35\n'; }
lspci() { :; }
sudo() { "$@"; }
apt-get() {
  printf '%s\n' "$*" >> "$NMOS_TEST_DIR/packages.log"
  if [[ "${NMOS_TEST_FAIL:-0}" == 1 && "$1" == install ]]; then return 1; fi
  if [[ "$1" == install ]]; then : > "$NMOS_TEST_DIR/installed"; fi
}
command() {
  if [[ "${1:-}" == '-v' ]]; then
    case "${2:-}" in
      lspci) [[ -e "$NMOS_TEST_DIR/installed" ]]; return ;;
      curl|wget|ip|lscpu|apt-get|sudo) return 0 ;;
      dnf|yum|pacman) return 1 ;;
    esac
  fi
  builtin command "$@"
}
export -f uname getconf lspci sudo apt-get command

INSTALL_DIR="$test_dir/bin" bash "$test_dir/install.sh"
test "$(cat "$test_dir/ran")" = ran
grep -q '^update$' "$test_dir/packages.log"
grep -q '^install -y --no-install-recommends pciutils$' "$test_dir/packages.log"
printf before > "$test_dir/ran"
printf '' > "$test_dir/packages.log"
INSTALL_DIR="$test_dir/bin" bash "$test_dir/install.sh" --no-run
test "$(cat "$test_dir/ran")" = before
test ! -s "$test_dir/packages.log"
rm "$test_dir/installed"
if NMOS_TEST_FAIL=1 INSTALL_DIR="$test_dir/bin" bash "$test_dir/install.sh"; then
  printf 'Ошибка установки пакетов не была обработана\n' >&2
  exit 1
fi
test "$(cat "$test_dir/ran")" = before
getconf() { printf 'glibc 2.31\n'; }
export -f getconf
if INSTALL_DIR="$test_dir/bin" bash "$test_dir/install.sh" --no-run; then
  printf 'Старая glibc не была отклонена\n' >&2
  exit 1
fi
printf 'Установка зависимостей и запуск: OK\n'
