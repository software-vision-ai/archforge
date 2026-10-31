#!/bin/sh
# Scales the application icon from its one source, assets/app-icon/icon.png (the tile filling a
# 1024 px square), into every size and bundle format, reproducibly and at package time, never kept
# in the tree: PNGs, a macOS icon set, a Windows icon file.
# Output: dist/icons/.
. "$(dirname "$0")/common.sh"

require_icon
out="$ROOT/dist/icons"
rm -rf "$out"
mkdir -p "$out/png"
# Up to 32 px the tile fills the square so the beaver stays legible; larger sizes stand on the
# platform icon grid, the tile 824/1024 of the side with a clear margin.
for size in 16 24 32 48 64 128 256 512 1024; do
  grid="--grid"
  [ "$size" -le 32 ] && grid=""
  (cd "$ROOT" && cargo xtask resize "$ROOT/assets/app-icon/icon.png" "$out/png/$size.png" "$size" $grid)
done

# Windows: PNG-encoded entries packed by the task runner.
(cd "$ROOT" && cargo xtask ico "$out/$APP.ico" "$out/png/16.png" "$out/png/24.png" "$out/png/32.png" "$out/png/48.png" "$out/png/64.png" "$out/png/256.png")

# macOS: an icon set, converted when the system tool is present.
if command -v iconutil >/dev/null 2>&1; then
  set_dir="$out/$APP.iconset"
  mkdir -p "$set_dir"
  for size in 16 32 128 256 512; do
    cp "$out/png/$size.png" "$set_dir/icon_${size}x${size}.png"
    double=$((size * 2))
    cp "$out/png/$double.png" "$set_dir/icon_${size}x${size}@2x.png"
  done
  iconutil --convert icns --output "$out/$APP.icns" "$set_dir"
fi

# macOS 26 and later: an Icon Composer document, compiled into the bundle by
# packaging/macos/package.sh where Xcode's actool is present. Its layer is the tile filling the
# canvas, as the system draws the icon's shape itself; the one picture serves both appearances.
icon_doc="$out/AppIcon.icon"
mkdir -p "$icon_doc/Assets"
cp "$ROOT/assets/app-icon/icon.png" "$icon_doc/Assets/icon.png"
cat >"$icon_doc/icon.json" <<'JSON'
{
  "groups" : [
    {
      "layers" : [
        {
          "image-name" : "icon.png",
          "name" : "icon"
        }
      ]
    }
  ],
  "supported-platforms" : {
    "squares" : "shared"
  }
}
JSON
echo "icons: $out"
