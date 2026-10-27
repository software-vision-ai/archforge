#!/bin/sh
# FreeBSD: a tarball built inside a FreeBSD 14 virtual machine, with a content check.
# usage: package.sh <version>
. "$(dirname "$0")/../common.sh"

version=${1:?usage: package.sh <version>}
need cargo tar
[ "$(uname -s)" = "FreeBSD" ] || die "run this inside FreeBSD"
cargo build --release --locked -p "$APP" --manifest-path "$ROOT/Cargo.toml"
out="$ROOT/dist/freebsd"
mkdir -p "$out"
stage=$(mktemp -d)
mkdir -p "$stage/bin" "$stage/share/doc/$APP"
cp "$TARGET_DIR/release/$APP" "$stage/bin/"
ship_licences "$stage/share/doc/$APP"
tarball="$out/$(artefact_name "$version" freebsd x86_64 tar.gz)"
tar -C "$stage" -czf "$tarball" .
rm -rf "$stage"
for f in "./bin/$APP" "./share/doc/$APP/LICENSE-MIT" "./share/doc/$APP/ATTRIBUTION.md" "./share/doc/$APP/licenses/fonts/OFL-1.1.txt"; do
  tar -tzf "$tarball" | grep -qx "$f" || die "$tarball lacks $f"
done
echo "freebsd: $tarball"
