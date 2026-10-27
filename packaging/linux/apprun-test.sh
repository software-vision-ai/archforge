#!/bin/sh
# Smoke test for AppRun: a stand-in binary must receive every argument unchanged.
set -eu
here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/usr/bin"
cp "$here/AppRun" "$work/AppRun"
chmod +x "$work/AppRun"
printf '#!/bin/sh\nfor a in "$@"; do printf "[%%s]" "$a"; done\n' >"$work/usr/bin/archforge"
chmod +x "$work/usr/bin/archforge"
got=$("$work/AppRun" "one" "two words" "")
[ "$got" = "[one][two words][]" ] || { echo "apprun-test: got $got" >&2; exit 1; }
echo "apprun-test: ok"
