#!/bin/sh
# Renders the application icon from its vector source into every raster size and bundle format,
# reproducibly and at package time, never kept in the tree: PNGs, a macOS icon set, a Windows icon file.
# Output: dist/icons/.
. "$(dirname "$0")/common.sh"

require_icon
need rsvg-convert
out="$ROOT/dist/icons"
rm -rf "$out"
mkdir -p "$out/png"
# Up to 32 px the full-bleed small icon keeps the mark legible; larger sizes use the macOS grid.
for size in 16 24 32 48 64 128 256 512 1024; do
  svg="$ROOT/assets/app-icon/icon.svg"
  [ "$size" -le 32 ] && svg="$ROOT/assets/app-icon/icon-small.svg"
  rsvg-convert --width "$size" --height "$size" "$svg" -o "$out/png/$size.png"
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

# macOS 26 and later: an Icon Composer document carrying the light and the dark appearance,
# compiled into the bundle by packaging/macos/package.sh where Xcode's actool
# is present. Each source's 824 px tile is scaled to fill the 1024 px canvas, as the system draws
# the icon's shape itself.
icon_doc="$out/AppIcon.icon"
mkdir -p "$icon_doc/Assets"
for name in icon icon-dark; do
  sed -e '1s|$|<g transform="matrix(1.242718 0 0 1.242718 -124.2718 -124.2718)">|' -e 's|</svg>|</g></svg>|' \
    "$ROOT/assets/app-icon/$name.svg" >"$icon_doc/Assets/$name.svg"
done
cat >"$icon_doc/icon.json" <<'JSON'
{
  "groups" : [
    {
      "layers" : [
        {
          "image-name-specializations" : [
            { "value" : "icon.svg" },
            { "appearance" : "dark", "value" : "icon-dark.svg" }
          ],
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
