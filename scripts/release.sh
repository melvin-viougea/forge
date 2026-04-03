#!/bin/bash
set -e

# Usage: ./scripts/release.sh 0.2.0 "Description of changes"
VERSION="${1:?Usage: ./scripts/release.sh <version> <description>}"
DESCRIPTION="${2:-Release v$VERSION}"

echo "==> Building Forge v$VERSION (release)..."
cargo build --release

echo "==> Creating Forge.app bundle..."
APP_DIR="target/release/Forge.app"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp target/release/forge "$APP_DIR/Contents/MacOS/forge"

cat > "$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Forge</string>
    <key>CFBundleDisplayName</key><string>Forge</string>
    <key>CFBundleIdentifier</key><string>com.melvin.forge</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleExecutable</key><string>forge</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Copy icon if exists
if [ -f "AppIcon.icns" ]; then
    cp AppIcon.icns "$APP_DIR/Contents/Resources/AppIcon.icns"
fi

echo "==> Compressing..."
cd target/release
tar -czf "Forge-v${VERSION}-macos-arm64.tar.gz" "Forge.app"

echo "==> Creating DMG..."
STAGING=$(mktemp -d)
cp -r Forge.app "$STAGING/"
ln -s /Applications "$STAGING/Applications"
hdiutil create -volname "Forge v${VERSION}" -srcfolder "$STAGING" -ov -format UDZO "Forge-v${VERSION}.dmg"
cd ../..

echo "==> Publishing GitHub Release v$VERSION..."
gh release create "v$VERSION" \
    --title "Forge v$VERSION" \
    --notes "$DESCRIPTION" \
    "target/release/Forge-v${VERSION}-macos-arm64.tar.gz" \
    "target/release/Forge-v${VERSION}.dmg"

echo "==> Done! Release v$VERSION published."
echo "    https://github.com/melvin-viougea/forge/releases/tag/v$VERSION"

# Pour toi (publier une mise à jour) :
# 1. Fais tes modifications + commit
# 2. Met à jour la version dans updater.rs (CURRENT_VERSION)
# 3. Lance le script de release :
# ./scripts/release.sh 0.2.0 "Description des changements"
#
# xattr -cr /Applications/Forge.app
