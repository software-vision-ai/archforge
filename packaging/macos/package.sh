#!/bin/sh
# macOS: a universal binary in an application bundle, in a disk image.
# usage: package.sh <version> [--sign]
# Unsigned by default, so a dry run needs no secrets; --sign runs only in a signing environment:
# codesign with the entitlements, notarise, staple.
. "$(dirname "$0")/../common.sh"

version=${1:?usage: package.sh <version> [--sign]}
sign=${2:-}
need cargo lipo hdiutil osascript
out="$ROOT/dist/macos"
mkdir -p "$out"
bundle="$out/$APP_NAME.app"

if [ "$sign" != "--sign" ]; then
  for target in x86_64-apple-darwin aarch64-apple-darwin; do
    cargo build --release --locked --target "$target" -p "$APP" --manifest-path "$ROOT/Cargo.toml"
  done
  rm -rf "$bundle"
  mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
  lipo -create -output "$bundle/Contents/MacOS/$APP" \
    "$TARGET_DIR/x86_64-apple-darwin/release/$APP" "$TARGET_DIR/aarch64-apple-darwin/release/$APP"
  sed -e "s|__VERSION__|$version|g; s|__BUNDLE_ID__|$BUNDLE_ID|g" "$ROOT/packaging/macos/Info.plist.in" >"$bundle/Contents/Info.plist"
  # Inside the bundle, so signing covers them and a copy of the application carries them.
  ship_licences "$bundle/Contents/Resources"
  if [ -f "$ROOT/assets/app-icon/icon.png" ]; then
    sh "$ROOT/packaging/icons.sh"
    # macOS 26 and later show the icon from the asset catalog actool compiles from the Icon
    # Composer document, with its own .icns for earlier systems; without actool the bundle carries
    # the .icns alone.
    if xcrun --find actool >/dev/null 2>&1; then
      xcrun actool "$ROOT/dist/icons/AppIcon.icon" --compile "$bundle/Contents/Resources" --platform macosx \
        --minimum-deployment-target 11.0 --app-icon AppIcon --output-partial-info-plist "$out/icon-info.plist" >/dev/null
      plutil -replace CFBundleIconFile -string AppIcon "$bundle/Contents/Info.plist"
      plutil -replace CFBundleIconName -string AppIcon "$bundle/Contents/Info.plist"
    else
      echo "package: Xcode's actool is missing; the bundle carries the .icns icon only" >&2
      cp "$ROOT/dist/icons/$APP.icns" "$bundle/Contents/Resources/$APP.icns"
    fi
  else
    echo "package: no application icon yet; the bundle uses the system's generic icon" >&2
  fi
  # Copies between machines can drop executable bits; the bundle travels to the signing
  # environment as a tarball.
  tar -C "$out" -czf "$out/bundle.tar.gz" "$APP_NAME.app"
else
  # Always the bundle from its tarball, which keeps the executable bits: a bundle lying beside it
  # may have come through an upload that dropped them.
  [ -f "$out/bundle.tar.gz" ] || die "$out/bundle.tar.gz is missing; download the unsigned build first"
  # Unpacked apart, so nothing in the tarball can land beside the bundle; only the bundle moves in.
  unpacked=$(mktemp -d)
  tar -C "$unpacked" -xzf "$out/bundle.tar.gz"
  [ -d "$unpacked/$APP_NAME.app" ] || die "$out/bundle.tar.gz holds no $APP_NAME.app"
  rm -rf "$bundle"
  mv "$unpacked/$APP_NAME.app" "$bundle"
  rm -rf "$unpacked"
  [ -x "$bundle/Contents/MacOS/$APP" ] || die "$bundle/Contents/MacOS/$APP is not executable after unpacking"
  need codesign xcrun security
  keychain="$RUNNER_TEMP/signing.keychain-db"
  security create-keychain -p "" "$keychain"
  security set-keychain-settings "$keychain"
  security unlock-keychain -p "" "$keychain"
  echo "$MACOS_CERTIFICATE_P12" | base64 --decode >"$RUNNER_TEMP/cert.p12"
  security import "$RUNNER_TEMP/cert.p12" -k "$keychain" -P "$MACOS_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
  security list-keychains -d user -s "$keychain"
  codesign --force --options runtime --timestamp --entitlements "$ROOT/packaging/macos/entitlements.plist" \
    --sign "$MACOS_SIGNING_IDENTITY" "$bundle"
fi

# The disk image: the bundle, a link to the applications folder, and, when installer artwork exists
# as packaging/macos/background.svg, a background rendered at 1x and 2x.
# The window layout is applied by dmg-layout.applescript; if it cannot run (no window session),
# the image is still built, plain, and the script says so. The volume name carries no version,
# because the layout embeds it.
dmg="$out/$(artefact_name "$version" macos universal dmg)"
work=$(mktemp -d)
staging="$work/staging"
mkdir -p "$staging"
cp -R "$bundle" "$staging/"
ln -s /Applications "$staging/Applications"
background=no
if [ -f "$ROOT/packaging/macos/background.svg" ]; then
  need rsvg-convert tiffutil
  mkdir -p "$staging/.background"
  rsvg-convert --width 660 --height 400 "$ROOT/packaging/macos/background.svg" -o "$work/background.png"
  rsvg-convert --width 1320 --height 800 "$ROOT/packaging/macos/background.svg" -o "$work/background-2x.png"
  tiffutil -cathidpicheck "$work/background.png" "$work/background-2x.png" -out "$staging/.background/background.tiff"
  background=yes
fi
hdiutil create -volname "$APP_NAME" -srcfolder "$staging" -fs HFS+ -format UDRW -ov "$work/rw.dmg"
mount="$work/mount"
mkdir -p "$mount"
hdiutil attach "$work/rw.dmg" -readwrite -noverify -noautoopen -mountpoint "$mount" >/dev/null
if ! osascript "$ROOT/packaging/macos/dmg-layout.applescript" "$APP_NAME" "$APP_NAME" "$background"; then
  echo "package: the disk-image layout could not be applied (no window session?); the image is plain" >&2
fi
sync
hdiutil detach "$mount" >/dev/null
rm -f "$dmg"
hdiutil convert "$work/rw.dmg" -format UDZO -o "$dmg" >/dev/null
rm -rf "$work"

if [ "$sign" = "--sign" ]; then
  codesign --force --timestamp --sign "$MACOS_SIGNING_IDENTITY" "$dmg"
  echo "$MACOS_NOTARY_KEY_P8" | base64 --decode >"$RUNNER_TEMP/notary.p8"
  xcrun notarytool submit "$dmg" --key "$RUNNER_TEMP/notary.p8" --key-id "$MACOS_NOTARY_KEY_ID" --issuer "$MACOS_NOTARY_ISSUER" --wait
  xcrun stapler staple "$dmg"
fi
echo "macos: $dmg"
