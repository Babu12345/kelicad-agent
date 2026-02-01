#!/bin/bash

# KeliCAD Agent - macOS Build, Sign & Notarize Script
#
# Usage: ./scripts/build-macos.sh
#
# This script will:
# 1. Load credentials from ../.env.local automatically
# 2. Build the Tauri app (which signs and notarizes the .app)
# 3. Verify the DMG signature
# 4. Copy to public/downloads
#
# Prerequisites:
# 1. Developer ID Application certificate installed in Keychain
# 2. ../.env.local file with:
#    - APPLE_SIGNING_IDENTITY
#    - APPLE_ID
#    - APPLE_PASSWORD
#    - APPLE_TEAM_ID

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Navigate to kelicad-agent directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

echo -e "${GREEN}KeliCAD Agent - macOS Build Script${NC}"
echo "======================================"

# Auto-load credentials from .env.local
ENV_FILE="../.env.local"
if [ -f "$ENV_FILE" ]; then
    echo -e "${BLUE}Loading credentials from .env.local...${NC}"

    # Parse .env.local safely (handle values with special characters)
    while IFS='=' read -r key value; do
        # Skip comments and empty lines
        [[ "$key" =~ ^#.*$ ]] && continue
        [[ -z "$key" ]] && continue

        # Remove leading/trailing whitespace from key
        key=$(echo "$key" | xargs)

        # Only export Apple-related variables if not already set
        case "$key" in
            APPLE_SIGNING_IDENTITY|APPLE_ID|APPLE_PASSWORD|APPLE_TEAM_ID)
                if [ -z "${!key}" ]; then
                    export "$key=$value"
                fi
                ;;
        esac
    done < "$ENV_FILE"
fi

# Try to auto-detect signing identity if not set
if [ -z "$APPLE_SIGNING_IDENTITY" ]; then
    DETECTED_IDENTITY=$(security find-identity -v -p codesigning | grep "Developer ID Application" | head -1 | sed 's/.*"\(.*\)".*/\1/')
    if [ -n "$DETECTED_IDENTITY" ]; then
        echo -e "${BLUE}Auto-detected signing identity: ${DETECTED_IDENTITY}${NC}"
        export APPLE_SIGNING_IDENTITY="$DETECTED_IDENTITY"
    fi
fi

# Validate required variables
echo -e "\n${BLUE}Checking credentials...${NC}"
MISSING_VARS=()

if [ -z "$APPLE_SIGNING_IDENTITY" ]; then
    MISSING_VARS+=("APPLE_SIGNING_IDENTITY")
fi
if [ -z "$APPLE_TEAM_ID" ]; then
    MISSING_VARS+=("APPLE_TEAM_ID")
fi
if [ -z "$APPLE_ID" ]; then
    MISSING_VARS+=("APPLE_ID")
fi
if [ -z "$APPLE_PASSWORD" ]; then
    MISSING_VARS+=("APPLE_PASSWORD")
fi

if [ ${#MISSING_VARS[@]} -ne 0 ]; then
    echo -e "${RED}Error: Missing required environment variables:${NC}"
    for var in "${MISSING_VARS[@]}"; do
        echo "  - $var"
    done
    echo ""
    echo "Add these to ../.env.local or export them before running this script."
    echo ""
    echo "To find your signing identity:"
    echo "  security find-identity -v -p codesigning"
    exit 1
fi

echo -e "  ${GREEN}✓${NC} APPLE_SIGNING_IDENTITY: ${APPLE_SIGNING_IDENTITY:0:50}..."
echo -e "  ${GREEN}✓${NC} APPLE_TEAM_ID: $APPLE_TEAM_ID"
echo -e "  ${GREEN}✓${NC} APPLE_ID: $APPLE_ID"
echo -e "  ${GREEN}✓${NC} APPLE_PASSWORD: [set]"

# Step 1: Build with Tauri (handles signing and notarization)
echo -e "\n${GREEN}Step 1/3: Building Tauri app (includes signing & notarization)...${NC}"
echo -e "${YELLOW}This may take a few minutes...${NC}"

APPLE_SIGNING_IDENTITY="$APPLE_SIGNING_IDENTITY" \
APPLE_ID="$APPLE_ID" \
APPLE_PASSWORD="$APPLE_PASSWORD" \
APPLE_TEAM_ID="$APPLE_TEAM_ID" \
npx @tauri-apps/cli build

# Check if build succeeded
APP_PATH="src-tauri/target/release/bundle/macos/KeliCAD Agent.app"
DMG_PATH="src-tauri/target/release/bundle/dmg/KeliCAD Agent_1.0.0_aarch64.dmg"

if [ ! -d "$APP_PATH" ]; then
    echo -e "${RED}Build failed - app bundle not found${NC}"
    exit 1
fi

if [ ! -f "$DMG_PATH" ]; then
    echo -e "${RED}Build failed - DMG not found${NC}"
    exit 1
fi

echo -e "${GREEN}✓ App built, signed, and notarized by Tauri${NC}"

# Step 2: Verify the DMG
echo -e "\n${GREEN}Step 2/3: Verifying DMG signature...${NC}"

# Check code signature
if codesign -v "$DMG_PATH" 2>/dev/null; then
    echo -e "  ${GREEN}✓${NC} Code signature valid"
else
    echo -e "  ${RED}✗${NC} Code signature invalid"
    exit 1
fi

# Check notarization (Gatekeeper)
if spctl -a -t open --context context:primary-signature "$DMG_PATH" 2>/dev/null; then
    echo -e "  ${GREEN}✓${NC} Notarization verified (Gatekeeper approved)"
else
    echo -e "  ${YELLOW}⚠${NC} Gatekeeper check failed - DMG may not be notarized"
    echo "    Users may see a warning when opening the DMG"
fi

# Step 3: Copy to public downloads
echo -e "\n${GREEN}Step 3/3: Copying to public downloads...${NC}"
DOWNLOADS_DIR="../public/downloads"
mkdir -p "$DOWNLOADS_DIR"
cp "$DMG_PATH" "$DOWNLOADS_DIR/"

# Show final info
DMG_SIZE=$(du -h "$DOWNLOADS_DIR/KeliCAD Agent_1.0.0_aarch64.dmg" | cut -f1)

echo -e "\n${GREEN}======================================${NC}"
echo -e "${GREEN}Build complete!${NC}"
echo -e "${GREEN}======================================${NC}"
echo ""
echo "DMG: $DOWNLOADS_DIR/KeliCAD Agent_1.0.0_aarch64.dmg"
echo "Size: $DMG_SIZE"
echo ""
echo -e "${BLUE}Verification commands:${NC}"
echo "  codesign -dv --verbose=4 \"$DMG_PATH\""
echo "  spctl -a -t open --context context:primary-signature \"$DMG_PATH\""
