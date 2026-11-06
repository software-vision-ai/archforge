#!/bin/sh
# Shared helpers for the packaging scripts. Source it: . "$(dirname "$0")/../common.sh"
# Every script resolves paths from the repository root, never from the caller's directory,
# and writes only under dist/.

set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
case "$ROOT" in */packaging) ROOT=$(dirname -- "$ROOT") ;; esac
APP=archforge
# The command-line tool agents run, which the application is to start for an agent from its in-app
# chat on; the macOS and Linux packages place it beside the application's executable, where
# archforge_control::cli_path finds it.
CLI="$APP-cli"
APP_NAME="Archforge"
BUNDLE_ID="ai.software-vision.archforge"
# Flatpak and AppStream allow a hyphen only in an id's last segment, so Linux spells the domain's
# hyphen as an underscore, the freedesktop rule for such domains.
LINUX_APP_ID="ai.software_vision.archforge"
TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}

die() {
  echo "package: $*" >&2
  exit 1
}

# Fails with a clear message when a tool is missing.
need() {
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || die "$tool is required and not on PATH"
  done
}

# Artefact names: <app>-<version>-<os>-<arch>.<ext>
artefact_name() {
  printf '%s-%s-%s-%s.%s' "$APP" "$1" "$2" "$3" "$4"
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

# Downloads $1 to $3 and verifies its SHA-256 against $2; a mismatch deletes the file.
fetch_verified() {
  [ -n "$1" ] && [ -n "$2" ] || die "a download address and its SHA-256 are required (the pins in the calling script)"
  need curl
  curl -fsSL "$1" -o "$3"
  got=$(sha256_of "$3")
  if [ "$got" != "$2" ]; then
    rm -f "$3"
    die "$3: SHA-256 $got does not match the recorded $2"
  fi
}

# The application icon: icon.png, the one picture every size is scaled from. Packaging that needs
# it stops here when it is missing.
require_icon() {
  [ -f "$ROOT/assets/app-icon/icon.png" ] || die "assets/app-icon/icon.png is missing: the application icon is scaled from it, with its row in ATTRIBUTION.toml"
}

# The licences and notices every package carries: the project's licences
# and NOTICE, ATTRIBUTION.md, and the texts under licenses/, among them those of the fonts the
# binary embeds, whose licences ask for their text with every copy.
ship_licences() {
  mkdir -p "$1"
  cp "$ROOT/LICENSE-MIT" "$ROOT/LICENSE-APACHE" "$ROOT/NOTICE" "$ROOT/ATTRIBUTION.md" "$1/"
  cp -R "$ROOT/licenses" "$1/"
}

mkdir -p "$ROOT/dist"
