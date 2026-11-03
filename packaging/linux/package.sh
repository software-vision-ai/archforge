#!/bin/sh
# Linux: a tarball, an AppImage with update information and a zsync file, deb and rpm through nfpm,
# and a Flatpak repackaged from the tarball so every Linux artefact carries identical binaries.
# usage: package.sh <version> <x86_64|aarch64> [--flatpak]
# Tools a release build does not already have are fetched from their official upstream releases
# and verified by SHA-256. The pins below hold unless APPIMAGETOOL_URL and APPIMAGETOOL_SHA256,
# NFPM_URL and NFPM_SHA256, APPIMAGE_RUNTIME_URL and APPIMAGE_RUNTIME_SHA256 or FLATPAK_REMOTE_URL
# are set.
. "$(dirname "$0")/../common.sh"

version=${1:?usage: package.sh <version> <arch> [--flatpak]}
arch=${2:?usage: package.sh <version> <arch> [--flatpak]}
mode=${3:-}
# Pinned 2026-10-24; each hash matches upstream's published digest.
case "$arch" in
  x86_64)
    appimagetool_url=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
    appimagetool_sha256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
    runtime_url=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
    runtime_sha256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d
    nfpm_url=https://github.com/goreleaser/nfpm/releases/download/v2.47.0/nfpm_2.47.0_Linux_x86_64.tar.gz
    nfpm_sha256=0660ca602b2d2d2ae4781a06c692b3eeb9d437ffea05b831d76e41f4a3188783
    ;;
  aarch64)
    appimagetool_url=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-aarch64.AppImage
    appimagetool_sha256=f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158
    runtime_url=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-aarch64
    runtime_sha256=00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444
    nfpm_url=https://github.com/goreleaser/nfpm/releases/download/v2.47.0/nfpm_2.47.0_Linux_arm64.tar.gz
    nfpm_sha256=1c0f5f2999b9a974bfb04fdb0cc3306096de530ac5dbb25d739cc5f5219c919c
    ;;
  *) die "unknown arch $arch; accepted: x86_64, aarch64" ;;
esac
# The Flathub repository, which carries the freedesktop runtime the Flatpak builds on: no recorded
# hash, flatpak trusts it through HTTPS and the GPG key in the file, which then checks the runtime.
FLATPAK_REMOTE_URL=${FLATPAK_REMOTE_URL:-https://dl.flathub.org/repo/flathub.flatpakrepo}

out="$ROOT/dist/linux"
mkdir -p "$out"
tarball="$out/$(artefact_name "$version" linux "$arch" tar.gz)"
here="$ROOT/packaging/linux"

if [ "$mode" = "--flatpak" ]; then
  need flatpak flatpak-builder
  [ -f "$tarball" ] || die "$tarball is missing; the Flatpak repackages the tarball"
  flatpak remote-add --user --if-not-exists runtimes "$FLATPAK_REMOTE_URL"
  flatpak install --user -y runtimes org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08
  work=$(mktemp -d)
  cat >"$work/manifest.yml" <<MANIFEST
app-id: $LINUX_APP_ID
runtime: org.freedesktop.Platform
runtime-version: "24.08"
sdk: org.freedesktop.Sdk
command: $APP
finish-args:
  # Minimal, each with its reason: a window, input, GPU, the user's documents.
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  - --device=dri
  - --filesystem=xdg-documents
modules:
  - name: $APP
    buildsystem: simple
    # The binaries stay the tarball's: without this, flatpak-builder splits their debug data off
    # and rewrites them.
    build-options:
      no-debuginfo: true
    build-commands:
      - cp -r . /app
    sources:
      # Unpacked as it is: the default strips one leading component, which keeps the layout only
      # while every entry starts with ./.
      # With its hash, so flatpak-builder checks the file and its cache follows the tarball's
      # contents, not its path.
      - type: archive
        path: $tarball
        sha256: $(sha256_of "$tarball")
        strip-components: 0
MANIFEST
  flatpak-builder --user --force-clean --state-dir="$work/state" --repo="$work/repo" "$work/build" "$work/manifest.yml"
  for f in "bin/$APP" "share/applications/$LINUX_APP_ID.desktop" \
    "share/doc/$APP/LICENSE-MIT" "share/doc/$APP/LICENSE-APACHE" "share/doc/$APP/NOTICE" \
    "share/doc/$APP/ATTRIBUTION.md" "share/doc/$APP/licenses/fonts/OFL-1.1.txt"; do
    [ -f "$work/build/files/$f" ] || die "the Flatpak lacks /app/$f"
  done
  # Every Linux artefact carries identical binaries.
  mkdir "$work/from-tarball"
  tar -C "$work/from-tarball" -xzf "$tarball" "./bin/$APP"
  for b in "$APP"; do
    [ "$(sha256_of "$work/build/files/bin/$b")" = "$(sha256_of "$work/from-tarball/bin/$b")" ] ||
      die "the Flatpak's bin/$b differs from the tarball's"
  done
  flatpak build-bundle "$work/repo" "$out/$(artefact_name "$version" linux "$arch" flatpak)" "$LINUX_APP_ID"
  rm -rf "$work"
  echo "linux: flatpak in $out"
  exit 0
fi

need cargo tar
cargo build --release --locked -p "$APP" --manifest-path "$ROOT/Cargo.toml"
bin="$TARGET_DIR/release"

# The tree every Linux artefact is made from.
stage=$(mktemp -d)
mkdir -p "$stage/bin" "$stage/share/applications" "$stage/share/metainfo" "$stage/share/doc/$APP"
cp "$bin/$APP" "$stage/bin/"
cp "$here/$APP.desktop" "$stage/share/applications/$LINUX_APP_ID.desktop"
sed "s|__VERSION__|$version|g; s|__DATE__|$(date -u +%Y-%m-%d)|g" "$here/$APP.metainfo.xml.in" >"$stage/share/metainfo/$LINUX_APP_ID.metainfo.xml"
ship_licences "$stage/share/doc/$APP"
require_icon
sh "$ROOT/packaging/icons.sh"
for size in 32 48 64 128 256 512; do
  mkdir -p "$stage/share/icons/hicolor/${size}x${size}/apps"
  cp "$ROOT/dist/icons/png/$size.png" "$stage/share/icons/hicolor/${size}x${size}/apps/$LINUX_APP_ID.png"
done
tar -C "$stage" -czf "$tarball" .
# The tarball is the source of the Flatpak too; every artefact carries the licence texts.
for f in "./bin/$APP" "./share/doc/$APP/LICENSE-MIT" "./share/doc/$APP/ATTRIBUTION.md" "./share/doc/$APP/licenses/fonts/OFL-1.1.txt"; do
  tar -tzf "$tarball" | grep -qx "$f" || die "$tarball lacks $f"
done

# AppImage, with update information and a zsync file.
appimagetool="$out/appimagetool"
fetch_verified "${APPIMAGETOOL_URL:-$appimagetool_url}" "${APPIMAGETOOL_SHA256:-$appimagetool_sha256}" "$appimagetool"
chmod +x "$appimagetool"
# The AppImage runtime is embedded in the image; given here, appimagetool downloads none of its own.
runtime="$out/appimage-runtime"
fetch_verified "${APPIMAGE_RUNTIME_URL:-$runtime_url}" "${APPIMAGE_RUNTIME_SHA256:-$runtime_sha256}" "$runtime"
appdir=$(mktemp -d)/$APP.AppDir
mkdir -p "$appdir/usr"
cp -R "$stage/." "$appdir/usr/"
cp "$here/AppRun" "$appdir/AppRun"
chmod +x "$appdir/AppRun"
cp "$stage/share/applications/$LINUX_APP_ID.desktop" "$appdir/"
cp "$ROOT/dist/icons/png/256.png" "$appdir/$LINUX_APP_ID.png"
appimage="$out/$(artefact_name "$version" linux "$arch" AppImage)"
# The update information names the zsync file, which appimagetool writes beside the image.
(cd "$out" && ARCH=$arch "$appimagetool" --appimage-extract-and-run --runtime-file "$runtime" -u "zsync|$(basename "$appimage").zsync" "$appdir" "$(basename "$appimage")")
[ -f "$appimage.zsync" ] || die "appimagetool wrote no zsync file"
rm -f "$appimagetool" "$runtime"

# deb and rpm through nfpm.
nfpm="$out/nfpm.tar.gz"
fetch_verified "${NFPM_URL:-$nfpm_url}" "${NFPM_SHA256:-$nfpm_sha256}" "$nfpm"
tar -C "$out" -xzf "$nfpm" nfpm
rm -f "$nfpm"
case "$arch" in x86_64) debarch=amd64 ;; aarch64) debarch=arm64 ;; esac
# nfpm expands variables in a few fields only, not in contents or scripts: the configuration is
# filled in here instead.
config=$(mktemp)
sed -e "s|\${STAGE}|$stage|g; s|\${VERSION}|$version|g; s|\${ARCH}|$debarch|g; s|\${POSTINST}|$here/postinst.sh|g" \
  "$here/nfpm.yaml" >"$config"
if grep -q '\${' "$config"; then die "packaging/linux/nfpm.yaml names a variable package.sh does not fill in"; fi
for packager in deb rpm; do
  ext=$packager
  "$out/nfpm" package --config "$config" --packager "$packager" --target "$out/$(artefact_name "$version" linux "$arch" "$ext")"
done
rm -f "$out/nfpm" "$config"
rm -rf "$stage"
echo "linux: $out"
