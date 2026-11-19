#!/bin/bash
# Builds "Event Studio.app" for macOS and a disk image of it with the guides beside it.
#
#   packaging/macos/bundle.sh
#
# Run on a Mac with Rust (https://www.rust-lang.org/tools/install). The program is built for
# Apple Silicon and for Intel and joined into one; a target that is not installed is added with
# rustup. TARGETS="aarch64-apple-darwin" builds for this Mac's kind only.
# The result is in dist/macos: the app, and EventStudio-<version>-macOS.dmg.
#
# The app is signed "ad hoc" (no Apple Developer ID), which is what lets it start on Apple
# Silicon at all; a copy that came over the internet still has to be let through once in
# System Settings > Privacy & Security (see the README).
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
TARGETS=${TARGETS:-"aarch64-apple-darwin x86_64-apple-darwin"}
OUT=dist/macos
APP="$OUT/Event Studio.app"
IMAGE="$OUT/EventStudio-$VERSION-macOS.dmg"

rm -rf "$OUT"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# The program, for each kind of Mac, as one file.
BUILT=()
for target in $TARGETS; do
  rustup target add "$target" >/dev/null
  MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release --target "$target"
  BUILT+=("target/$target/release/event-studio")
done
lipo -create -output "$APP/Contents/MacOS/EventStudio" "${BUILT[@]}"

# Its icon: the program draws it (src/icon_art.rs), macOS wants it in ten sizes.
ICONS="$OUT/AppIcon.iconset"
mkdir -p "$ICONS"
"$APP/Contents/MacOS/EventStudio" --write-icon "$OUT/icon-1024.png"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$OUT/icon-1024.png" --out "$ICONS/icon_${size}x${size}.png" >/dev/null
  sips -z "$((size * 2))" "$((size * 2))" "$OUT/icon-1024.png" --out "$ICONS/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONS" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONS" "$OUT/icon-1024.png"

# What macOS reads about the app, the guides and the licences.
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
GUIDES=("사용안내.txt" "Guide.txt" "使用说明.txt" "使い方.txt")
cp "${GUIDES[@]}" LICENSE THIRD-PARTY-NOTICES.md THIRD-PARTY-LICENSES.txt "$APP/Contents/Resources/"

codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

# The disk image: the app, a way to the Applications folder, and the guides.
STAGE="$OUT/image"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
cp "${GUIDES[@]}" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Event Studio $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$IMAGE" >/dev/null
rm -rf "$STAGE"

lipo -archs "$APP/Contents/MacOS/EventStudio"
echo "$IMAGE"
