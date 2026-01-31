#!/bin/bash

# KeliCAD Agent - macOS Build, Sign & Notarize Script
#
# Prerequisites:
# 1. Developer ID Application certificate installed in Keychain
# 2. App-specific password from appleid.apple.com
# 3. Environment variables set (or will prompt):
#    - APPLE_SIGNING_IDENTITY
#    - APPLE_ID
#    - APPLE_PASSWORD
#    - APPLE_TEAM_ID

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}KeliCAD Agent - macOS Build Script${NC}"
echo "======================================"

# Check for required environment variables
if [ -z "$APPLE_SIGNING_IDENTITY" ]; then
    echo -e "${RED}Error: APPLE_SIGNING_IDENTITY environment variable is required${NC}"
    echo "Find your identity with: security find-identity -v -p codesigning"
    echo "Example: export APPLE_SIGNING_IDENTITY=\"Developer ID Application: Your Name (TEAMID)\""
    exit 1
fi

if [ -z "$APPLE_TEAM_ID" ]; then
    echo -e "${RED}Error: APPLE_TEAM_ID environment variable is required${NC}"
    exit 1
fi

# Navigate to kelicad-agent directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

echo -e "\n${GREEN}Step 1: Building Tauri app...${NC}"
APPLE_SIGNING_IDENTITY="$APPLE_SIGNING_IDENTITY" npx @tauri-apps/cli build

# Check if build succeeded
APP_PATH="src-tauri/target/release/bundle/macos/KeliCAD Agent.app"
if [ ! -d "$APP_PATH" ]; then
    echo -e "${RED}Build failed - app bundle not found${NC}"
    exit 1
fi

echo -e "${GREEN}App built and signed successfully${NC}"

# Create DMG manually (since Tauri's DMG bundler can be flaky)
echo -e "\n${GREEN}Step 2: Creating DMG...${NC}"
DMG_DIR="src-tauri/target/release/bundle/dmg"
DMG_PATH="$DMG_DIR/KeliCAD Agent_1.0.0_aarch64.dmg"

mkdir -p "$DMG_DIR"
rm -f "$DMG_PATH" 2>/dev/null || true

hdiutil create -volname "KeliCAD Agent" \
    -srcfolder "$APP_PATH" \
    -ov -format UDZO \
    "$DMG_PATH"

echo -e "\n${GREEN}Step 3: Signing DMG...${NC}"
codesign --sign "$APPLE_SIGNING_IDENTITY" "$DMG_PATH"

# Notarization (requires APPLE_ID and APPLE_PASSWORD)
if [ -n "$APPLE_ID" ] && [ -n "$APPLE_PASSWORD" ]; then
    echo -e "\n${GREEN}Step 4: Submitting for notarization...${NC}"
    xcrun notarytool submit "$DMG_PATH" \
        --apple-id "$APPLE_ID" \
        --password "$APPLE_PASSWORD" \
        --team-id "$APPLE_TEAM_ID" \
        --wait

    echo -e "\n${GREEN}Step 5: Stapling notarization ticket...${NC}"
    xcrun stapler staple "$DMG_PATH"

    echo -e "\n${GREEN}Notarization complete!${NC}"
else
    echo -e "\n${YELLOW}Skipping notarization - APPLE_ID and APPLE_PASSWORD not set${NC}"
    echo "To notarize manually, run:"
    echo "  xcrun notarytool submit \"$DMG_PATH\" --apple-id YOUR_ID --password YOUR_PASSWORD --team-id $APPLE_TEAM_ID --wait"
    echo "  xcrun stapler staple \"$DMG_PATH\""
fi

# Copy to public downloads
echo -e "\n${GREEN}Step 6: Copying to public downloads...${NC}"
DOWNLOADS_DIR="../public/downloads"
mkdir -p "$DOWNLOADS_DIR"
cp "$DMG_PATH" "$DOWNLOADS_DIR/"

# Show final info
DMG_SIZE=$(du -h "$DOWNLOADS_DIR/KeliCAD Agent_1.0.0_aarch64.dmg" | cut -f1)
echo -e "\n${GREEN}======================================"
echo -e "Build complete!"
echo -e "======================================${NC}"
echo "DMG: $DOWNLOADS_DIR/KeliCAD Agent_1.0.0_aarch64.dmg"
echo "Size: $DMG_SIZE"
echo ""
echo "Verify signature with:"
echo "  codesign -dv --verbose=4 \"$DMG_PATH\""
echo ""
echo "Verify notarization with:"
echo "  spctl -a -t open --context context:primary-signature \"$DMG_PATH\""
