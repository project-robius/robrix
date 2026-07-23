#!/usr/bin/env bash
set -euo pipefail

# Build, sign, and package the Robrix iOS .ipa, then optionally upload it to
# TestFlight. This is the CI-runnable version of the local testflight.sh flow:
# it builds with distribution flags, compiles the asset catalog, patches
# Info.plist (compliance flag, numeric version, unique build number), re-signs,
# packages the .ipa, and uploads via altool.
#
# Config (env vars; the release.yml `for_ios` job supplies these from secrets):
#   IOS_SIGNING_IDENTITY           cert common-name to sign with (default "Apple Distribution")
#   IOS_PROVISIONING_PROFILE_UUID  installed .mobileprovision UUID (required)
#   ASC_KEY_ID / ASC_ISSUER_ID     App Store Connect API key ids (required to upload)
#   IOS_UPLOAD_TESTFLIGHT          "true" to upload after packaging (default false)
#   TESTFLIGHT_BUILD_NUMBER        CFBundleVersion (default: Pacific-time timestamp)
#   ORG / APP                      makepad org + app (default rs.robius / robrix)
#
# altool auto-discovers the API key from ~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

ORG="${ORG:-rs.robius}"
APP="${APP:-robrix}"
SIGNING_IDENTITY="${IOS_SIGNING_IDENTITY:-Apple Distribution}"
PROFILE_UUID="${IOS_PROVISIONING_PROFILE_UUID:?set IOS_PROVISIONING_PROFILE_UUID}"
BUILD_NUMBER="${TESTFLIGHT_BUILD_NUMBER:-$(TZ=America/Los_Angeles date +%Y%m%d.%H%M)}"
APP_BUNDLE="$REPO_ROOT/target/apple/makepad-apple-app/aarch64-apple-ios/release/${APP}.app"

# Resolve the signing cert's SHA-1 from its common name, so nothing is hard-coded.
CERT_SHA1="$(security find-identity -v -p codesigning \
  | awk -v id="$SIGNING_IDENTITY" 'index($0, id) {print $2; exit}')"
if [[ -z "$CERT_SHA1" ]]; then
    echo "Error: no code-signing identity matching '$SIGNING_IDENTITY' in the keychain." >&2
    exit 1
fi

echo "==> Identity: $SIGNING_IDENTITY ($CERT_SHA1)"
echo "==> Profile:  $PROFILE_UUID"
echo "==> Build:    $BUILD_NUMBER"

# ---- 1. Build (run-device errors with "device not found"; the .app is still built) ----
CARGO_PROFILE_RELEASE_DEBUG=false \
CARGO_PROFILE_RELEASE_STRIP=symbols \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
CARGO_PROFILE_RELEASE_LTO=fat \
TESTFLIGHT_BUILD_NUMBER="$BUILD_NUMBER" \
cargo makepad apple ios --org="$ORG" --app="$APP" \
  --profile="$PROFILE_UUID" \
  --cert="$CERT_SHA1" \
  --device=IPhone \
  run-device -p "$APP" --release || true

if [[ ! -d "$APP_BUNDLE" ]]; then
    echo "Error: app bundle not built at $APP_BUNDLE" >&2
    exit 1
fi

# Fail fast if the binary wasn't linked against the iOS 26+ SDK, so a wrong
# Xcode is caught here instead of after the ~20-min package + upload.
SDK_VER=$(xcrun vtool -show-build "$APP_BUNDLE/$APP" 2>/dev/null | awk '$1=="sdk"{print $2; exit}')
if [[ -n "$SDK_VER" ]]; then
    echo "==> Linked iOS SDK: $SDK_VER"
    if [[ "${SDK_VER%%.*}" -lt 26 ]]; then
        echo "Error: built against iOS SDK $SDK_VER; Apple requires 26+. Active Xcode: $(xcode-select -p)" >&2
        exit 1
    fi
else
    echo "==> Warning: could not read the linked iOS SDK version; skipping the SDK check." >&2
fi

# ---- 2. Compile asset catalog ----
xcrun actool ./packaging/ios/icons/Assets.xcassets \
  --compile "$APP_BUNDLE" \
  --platform iphoneos \
  --minimum-deployment-target 14.0 \
  --app-icon AppIcon \
  --output-partial-info-plist /tmp/AssetInfo.plist

# ---- 3. Patch Info.plist ----
PLIST="$APP_BUNDLE/Info.plist"
/usr/libexec/PlistBuddy -c "Merge /tmp/AssetInfo.plist" "$PLIST"

set_or_add () {  # key, type, value
  /usr/libexec/PlistBuddy -c "Add :$1 $2 $3" "$PLIST" 2>/dev/null \
    || /usr/libexec/PlistBuddy -c "Set :$1 $3" "$PLIST"
}
set_or_add CFBundlePackageType string APPL
set_or_add CFBundleIconName string AppIcon
/usr/libexec/PlistBuddy -c "Add :UILaunchScreen dict" "$PLIST" 2>/dev/null || true
set_or_add UILaunchScreen:UIImageName string AppIcon60x60
set_or_add UILaunchScreen:UIColorName string LaunchScreenBackground
set_or_add ITSAppUsesNonExemptEncryption bool false

VERSION=$(cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name=="robrix") | .version' \
  | sed 's/-.*$//')
echo "Version $VERSION (build $BUILD_NUMBER)"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$PLIST"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILD_NUMBER" "$PLIST"

# ---- 4. Extract entitlements from the App Store profile ----
security cms -D -i "$HOME/Library/MobileDevice/Provisioning Profiles/${PROFILE_UUID}.mobileprovision" \
  > /tmp/profile.plist
/usr/libexec/PlistBuddy -x -c "Print :Entitlements" /tmp/profile.plist > /tmp/entitlements.plist

# ---- 5. Re-sign (steps 2-3 invalidated the signature) ----
codesign --force --sign "$CERT_SHA1" \
  --entitlements /tmp/entitlements.plist \
  --timestamp=none \
  "$APP_BUNDLE"

# ---- 6. Verify ----
codesign -vvv "$APP_BUNDLE"
codesign -d --entitlements - "$APP_BUNDLE"

# ---- 7. Package .ipa ----
BUILD_DIR="$REPO_ROOT/target/apple/makepad-apple-app/aarch64-apple-ios/release"
cd "$BUILD_DIR"
IPA_NAME="Robrix-${VERSION}-ios.ipa"
rm -rf Payload "$IPA_NAME"
ditto "${APP}.app" "Payload/${APP}.app"
ditto -c -k --sequesterRsrc --keepParent Payload "$IPA_NAME"
ls -lh "$IPA_NAME"
echo "==> Packaged $BUILD_DIR/$IPA_NAME"

# ---- 8. Upload to TestFlight (optional) ----
if [[ "${IOS_UPLOAD_TESTFLIGHT:-false}" == "true" ]]; then
    : "${ASC_KEY_ID:?set ASC_KEY_ID to upload}"
    : "${ASC_ISSUER_ID:?set ASC_ISSUER_ID to upload}"
    echo "==> Uploading $IPA_NAME to TestFlight..."
    xcrun altool --upload-app --type ios \
      --file "$IPA_NAME" \
      --apiKey "$ASC_KEY_ID" \
      --apiIssuer "$ASC_ISSUER_ID"
    echo "==> Uploaded to TestFlight."
else
    echo "==> Skipping TestFlight upload (IOS_UPLOAD_TESTFLIGHT != true)."
fi
