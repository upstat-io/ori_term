---
section: 46
title: "macOS App Bundle & Platform Packaging"
status: not-started
tier: 6
goal: "Produce a proper macOS .app bundle so oriterm launches as a native GUI application with dock icon, Cmd+Tab switching, and correct system integration — plus add the macOS build job to CI release pipelines"
inspired_by:
  - "Alacritty extra/osx/Alacritty.app/ (template bundle + Makefile universal binary)"
  - "WezTerm assets/macos/WezTerm.app/ (template bundle + ci/deploy.sh + codesign)"
  - "Ghostty macos/Ghostty.xcodeproj (Xcode-native build)"
depends_on: ["03"]
sections:
  - id: "46.1"
    title: "Info.plist & Bundle Template"
    status: not-started
  - id: "46.2"
    title: "App Icon"
    status: not-started
  - id: "46.3"
    title: "Bundle Assembly Script"
    status: not-started
  - id: "46.4"
    title: "DMG Packaging"
    status: not-started
  - id: "46.5"
    title: "CI Build Jobs (Nightly + Release)"
    status: not-started
  - id: "46.6"
    title: "Section Completion"
    status: not-started
---

# Section 46: macOS App Bundle & Platform Packaging

**Status:** Not Started
**Goal:** oriterm launches as a first-class macOS application — proper dock icon, Cmd+Tab integration, dark mode support, Retina awareness. The nightly and release CI pipelines produce a universal (x86_64 + aarch64) `.app` bundle inside a `.dmg`. Without this, macOS users get a bare binary that launches from Terminal with no dock presence and broken focus behavior.

**Context:** The nightly pipeline (`nightly.yml`) already builds a macOS binary (single-architecture aarch64 tarball), but does not produce a `.app` bundle or DMG. The release pipeline (`release.yml`) has no macOS build job at all. Running the bare `oriterm` binary on macOS inherits the launching terminal's dock icon and process identity, because macOS requires a `.app` bundle with `Info.plist` to recognize a process as a GUI application. Every reference terminal emulator (Alacritty, WezTerm, Ghostty) ships a proper `.app` bundle.

**Crate:** None — this is build infrastructure and CI, not Rust code changes.
**Dependencies:** Section 03 (Cross-Platform) — macOS compiles and runs.

**Reference implementations:**
- **Alacritty** `extra/osx/Alacritty.app/Contents/Info.plist`: Template bundle checked into repo, Makefile assembles universal binary via `lipo`, DMG via `hdiutil`.
- **WezTerm** `assets/macos/WezTerm.app/Contents/Info.plist`: Template bundle, `ci/deploy.sh` assembles + codesigns (no notarization), distributes as ZIP for Homebrew cask.
- **Ghostty** `macos/Ghostty.xcodeproj`: Full Xcode project, icon from PNG source files via asset catalog.

---

## 46.1 Info.plist & Bundle Template

**File(s):** `assets/macos/OriTerm.app/Contents/Info.plist`

Create the `.app` bundle template directory structure checked into the repo. This is a static template — the build script copies binaries into it.

- [ ] Create directory structure:
  ```
  assets/macos/OriTerm.app/
  └── Contents/
      ├── Info.plist
      ├── MacOS/
      │   └── .gitkeep    (git doesn't track empty dirs; removed at build time)
      └── Resources/
          └── .gitkeep    (git doesn't track empty dirs; removed at build time)
  ```

- [ ] Write `Info.plist` with required keys (modeled after Alacritty + WezTerm):
  ```xml
  <?xml version="1.0" encoding="UTF-8"?>
  <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
  <plist version="1.0">
  <dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>

    <key>CFBundleExecutable</key>
    <string>oriterm</string>

    <key>CFBundleIdentifier</key>
    <string>com.oriterm.app</string>

    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>

    <key>CFBundleName</key>
    <string>OriTerm</string>

    <key>CFBundleDisplayName</key>
    <string>OriTerm</string>

    <key>CFBundlePackageType</key>
    <string>APPL</string>

    <key>CFBundleShortVersionString</key>
    <string>__VERSION__</string>

    <key>CFBundleVersion</key>
    <string>1</string>

    <key>CFBundleIconFile</key>
    <string>oriterm.icns</string>

    <key>CFBundleSupportedPlatforms</key>
    <array>
      <string>MacOSX</string>
    </array>

    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>

    <key>LSApplicationCategoryType</key>
    <string>public.app-category.utilities</string>

    <key>NSHighResolutionCapable</key>
    <true/>

    <key>NSMainNibFile</key>
    <string></string>

    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>

    <key>NSRequiresAquaSystemAppearance</key>
    <string>NO</string>

    <key>NSAppleEventsUsageDescription</key>
    <string>An application in OriTerm would like to access AppleScript.</string>

    <key>NSCalendarsUsageDescription</key>
    <string>An application in OriTerm would like to access calendar data.</string>

    <key>NSCameraUsageDescription</key>
    <string>An application in OriTerm would like to access the camera.</string>

    <key>NSContactsUsageDescription</key>
    <string>An application in OriTerm wants to access your contacts.</string>

    <key>NSLocationAlwaysUsageDescription</key>
    <string>An application in OriTerm would like to access your location information, even in the background.</string>

    <key>NSLocationUsageDescription</key>
    <string>An application in OriTerm would like to access your location information.</string>

    <key>NSLocationWhenInUseUsageDescription</key>
    <string>An application in OriTerm would like to access your location information while active.</string>

    <key>NSMicrophoneUsageDescription</key>
    <string>An application in OriTerm would like to access your microphone.</string>

    <key>NSRemindersUsageDescription</key>
    <string>An application in OriTerm would like to access your reminders.</string>

    <key>NSBluetoothAlwaysUsageDescription</key>
    <string>An application in OriTerm wants to use Bluetooth.</string>

    <key>NSDocumentsFolderUsageDescription</key>
    <string>An application in OriTerm would like to access your Documents folder.</string>

    <key>NSDownloadsFolderUsageDescription</key>
    <string>An application in OriTerm would like to access your Downloads folder.</string>

    <key>NSLocalNetworkUsageDescription</key>
    <string>An application in OriTerm would like to access the local network.</string>

    <key>NSSystemAdministrationUsageDescription</key>
    <string>An application in OriTerm requires elevated permissions.</string>
  </dict>
  </plist>
  ```

- [ ] Verify `CFBundleShortVersionString` matches workspace version in `Cargo.toml` (currently `0.1.0-alpha.3`) — the build script replaces the `__VERSION__` placeholder at assembly time.

---

## 46.2 App Icon

**File(s):** `assets/oriterm.iconset/`, `assets/macos/OriTerm.app/Contents/Resources/oriterm.icns`

macOS requires an `.icns` icon file inside the bundle. The standard approach is to maintain source PNGs in an `.iconset` directory and convert with `iconutil`.

Existing icon assets in `assets/`: `icon.svg` (master source), `icon-16.png`, `icon-32.png`, `icon-48.png` (Linux desktop entries; not used in `.iconset`), `icon-64.png`, `icon-128.png`, `icon-256.png`, `icon.ico` (Windows). The `oriterm_ui` build script (`oriterm_ui/build.rs`) decodes `assets/icon-256.png` to RGBA at build time for the embedded window icon (`load_icon()` in `oriterm_ui/src/window/mod.rs`).

- [ ] **Step 1: Generate missing high-resolution PNGs** from `assets/icon.svg` using `rsvg-convert` (from `librsvg`). These must exist before the `.iconset` can be assembled:
  ```bash
  rsvg-convert -w 512 -h 512 assets/icon.svg -o assets/icon-512.png
  rsvg-convert -w 1024 -h 1024 assets/icon.svg -o assets/icon-1024.png
  ```
  Alternative: `inkscape --export-type=png --export-width=512 assets/icon.svg`. Either tool works; `rsvg-convert` is lighter-weight. Check these into the repo alongside the other `assets/icon-*.png` files.

- [ ] **Step 2: Assemble the `.iconset` directory** from existing and newly generated PNGs:
  ```
  assets/oriterm.iconset/
  ├── icon_16x16.png          (from assets/icon-16.png)
  ├── icon_16x16@2x.png      (32x32, from assets/icon-32.png)
  ├── icon_32x32.png          (from assets/icon-32.png)
  ├── icon_32x32@2x.png      (64x64, from assets/icon-64.png)
  ├── icon_128x128.png        (from assets/icon-128.png)
  ├── icon_128x128@2x.png    (256x256, from assets/icon-256.png)
  ├── icon_256x256.png        (from assets/icon-256.png)
  ├── icon_256x256@2x.png    (512x512, from assets/icon-512.png)
  ├── icon_512x512.png        (512x512, from assets/icon-512.png)
  └── icon_512x512@2x.png    (1024x1024, from assets/icon-1024.png)
  ```

- [ ] **Step 3: Generate `.icns`** from `.iconset`:
  ```bash
  iconutil -c icns assets/oriterm.iconset -o assets/macos/OriTerm.app/Contents/Resources/oriterm.icns
  ```
  Note: `iconutil` is macOS-only. The generated `.icns` is checked into the repo so non-macOS contributors don't need this tool. Regenerate only when the icon source changes.

- [ ] **Step 4: Check the generated `.icns`** into the repo at `assets/macos/OriTerm.app/Contents/Resources/oriterm.icns`.

- [ ] **Step 5: Add `.gitattributes`** entry to mark `.icns` as binary (prevents git text diff/line-ending corruption):
  ```
  *.icns binary
  ```
  `.gitattributes` does not yet exist at the repo root; create it.

- [ ] **Step 6: Verify visual consistency** — the same source PNGs are used for both the `.icns` and the embedded window icon (`load_icon()` in `oriterm_ui/src/window/mod.rs`, sourced from `assets/icon-256.png` via `oriterm_ui/build.rs`). The dock icon and window icon must match.

---

## 46.3 Bundle Assembly Script

**File(s):** `scripts/build-macos-bundle.sh`

A shell script that assembles the `.app` bundle from the template and compiled binaries. Used by CI and local builds.

- [ ] Create `scripts/` directory and `scripts/build-macos-bundle.sh`:
  ```bash
  #!/usr/bin/env bash
  set -euo pipefail

  # Usage: ./scripts/build-macos-bundle.sh [--universal] [--release]
  # Produces: target/macos/OriTerm.app/

  RELEASE_FLAG=""
  UNIVERSAL=false
  PROFILE="debug"

  for arg in "$@"; do
    case $arg in
      --release) RELEASE_FLAG="--release"; PROFILE="release" ;;
      --universal) UNIVERSAL=true ;;
    esac
  done

  APP_DIR="target/macos/OriTerm.app"
  CONTENTS="$APP_DIR/Contents"

  # Clean and copy template
  rm -rf "$APP_DIR"
  cp -R assets/macos/OriTerm.app "$APP_DIR"

  # Remove .gitkeep files from template copy
  # (git needs these to track empty dirs, but they don't belong in the bundle)
  find "$APP_DIR" -name .gitkeep -delete

  # Ensure target directories exist (safety net if .gitkeep removal happened)
  mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"

  # Update version in Info.plist from Cargo.toml (__VERSION__ placeholder)
  VERSION=$(sed -n '/\[workspace\.package\]/,/^\[/{ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p }' Cargo.toml)
  sed -i.bak "s|__VERSION__|$VERSION|" "$CONTENTS/Info.plist"
  rm -f "$CONTENTS/Info.plist.bak"

  if $UNIVERSAL; then
    # Build both architectures (--workspace ensures both oriterm and oriterm-mux are built)
    MACOSX_DEPLOYMENT_TARGET="11.0" cargo build --workspace $RELEASE_FLAG --target x86_64-apple-darwin
    MACOSX_DEPLOYMENT_TARGET="11.0" cargo build --workspace $RELEASE_FLAG --target aarch64-apple-darwin

    # Create universal binaries with lipo
    for bin in oriterm oriterm-mux; do
      lipo \
        "target/x86_64-apple-darwin/$PROFILE/$bin" \
        "target/aarch64-apple-darwin/$PROFILE/$bin" \
        -create -output "$CONTENTS/MacOS/$bin"
    done
  else
    # Single-architecture build (host arch only)
    MACOSX_DEPLOYMENT_TARGET="11.0" cargo build --workspace $RELEASE_FLAG

    for bin in oriterm oriterm-mux; do
      cp "target/$PROFILE/$bin" "$CONTENTS/MacOS/$bin"
    done
  fi

  # Strip release binaries (macOS strip handles universal/fat binaries correctly)
  if [ "$PROFILE" = "release" ]; then
    strip "$CONTENTS/MacOS/oriterm"
    strip "$CONTENTS/MacOS/oriterm-mux"
  fi

  # Ad-hoc code sign (required for aarch64, good practice for x86_64)
  # Remove any existing signature first, then re-sign (Alacritty pattern)
  # NOTE: --deep is deprecated by Apple for new submissions but still works for
  # ad-hoc signing. When/if we move to Developer ID signing, sign each binary
  # individually instead of using --deep.
  codesign --remove-signature "$APP_DIR" 2>/dev/null || true
  codesign --force --deep --sign - "$APP_DIR"

  echo "Built: $APP_DIR (version $VERSION, profile=$PROFILE, universal=$UNIVERSAL)"
  ```

- [ ] `chmod +x scripts/build-macos-bundle.sh`

- [ ] Validate the assembled bundle (manual verification on macOS):
  - Both binaries exist: `ls -la target/macos/OriTerm.app/Contents/MacOS/{oriterm,oriterm-mux}`
  - For universal builds: `lipo -info target/macos/OriTerm.app/Contents/MacOS/oriterm` shows both architectures
  - `open target/macos/OriTerm.app` launches with dock icon
  - Cmd+Tab shows "OriTerm" with icon
  - `codesign --verify target/macos/OriTerm.app` succeeds
  - System dark/light mode is respected (`NSRequiresAquaSystemAppearance` = `NO`)
  - No `.gitkeep` files remain in the assembled bundle

> **Note:** This script is macOS-only (requires `codesign`, `lipo`, `strip`). It cannot run on Linux/Windows. CI runs it on `macos-latest` runners only.

---

## 46.4 DMG Packaging

**File(s):** `scripts/build-macos-dmg.sh`

Create a DMG disk image containing the `.app` bundle and an Applications symlink (standard macOS drag-to-install pattern). This must exist before the CI jobs in 46.5, which invoke `./scripts/build-macos-dmg.sh`.

- [ ] Create `scripts/build-macos-dmg.sh`:
  ```bash
  #!/usr/bin/env bash
  set -euo pipefail

  APP_DIR="target/macos/OriTerm.app"

  if [ ! -d "$APP_DIR" ]; then
    echo "ERROR: $APP_DIR not found. Run build-macos-bundle.sh first."
    exit 1
  fi

  # Verify the bundle is signed before packaging into DMG
  if ! codesign --verify "$APP_DIR" 2>/dev/null; then
    echo "WARNING: $APP_DIR is not signed. DMG will contain unsigned app."
  fi

  # Extract version
  VERSION=$(sed -n '/\[workspace\.package\]/,/^\[/{ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p }' Cargo.toml)

  # Determine output name
  SHORT_SHA=$(git rev-parse --short=7 HEAD 2>/dev/null || echo "unknown")
  DATE=$(date -u +%Y%m%d)

  if [ -n "${GITHUB_REF_NAME:-}" ] && [[ "${GITHUB_REF_NAME}" == v* ]]; then
    # Tagged release
    DMG_NAME="oriterm-${GITHUB_REF_NAME}-macos-universal.dmg"
    VOLUME_NAME="OriTerm ${VERSION}"
  else
    # Nightly
    DMG_NAME="oriterm-nightly-${DATE}-${SHORT_SHA}-macos-universal.dmg"
    VOLUME_NAME="OriTerm Nightly"
  fi

  # Create staging directory
  STAGING="target/macos/dmg-staging"
  rm -rf "$STAGING"
  mkdir -p "$STAGING"
  cp -R "$APP_DIR" "$STAGING/"
  ln -sf /Applications "$STAGING/Applications"

  # Build DMG
  hdiutil create "$DMG_NAME" \
    -volname "$VOLUME_NAME" \
    -fs HFS+ \
    -srcfolder "$STAGING" \
    -ov -format UDZO

  rm -rf "$STAGING"

  echo "Created: $DMG_NAME"
  ```

- [ ] `chmod +x scripts/build-macos-dmg.sh`

---

## 46.5 CI Build Jobs (Nightly + Release)

**File(s):** `.github/workflows/nightly.yml`, `.github/workflows/release.yml`, `.github/workflows/ci.yml`

Upgrade the existing nightly macOS job to produce a universal `.app` bundle + DMG (currently single-arch tarball), and add a new macOS build job to the release pipeline. Both produce universal binaries (x86_64 + aarch64).

**Cross-compilation note:** `macos-latest` runners are Apple Silicon (M-series). Building `x86_64-apple-darwin` on an aarch64 runner is cross-compilation. This is well-supported by Rust (Apple ships a universal SDK), but if any native C dependencies (wgpu, winit) fail to cross-compile, the fallback is to use `macos-13` (Intel) for x86_64 and `macos-latest` for aarch64, then `lipo` them in a separate step (doubles runner cost).

### Nightly Pipeline

- [ ] Modify the existing `build-macos` job in `.github/workflows/nightly.yml` to build a universal `.app` bundle and DMG (currently builds single-arch aarch64 tarball):
  ```yaml
  build-macos:
    name: Build macOS (Universal)
    if: github.event.workflow_run.conclusion == 'success'
    runs-on: macos-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event.workflow_run.head_sha }}
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-apple-darwin,aarch64-apple-darwin
      - uses: Swatinem/rust-cache@v2
        with:
          key: nightly-macos
      - name: Build universal bundle
        run: ./scripts/build-macos-bundle.sh --universal --release
      - name: Package DMG
        run: ./scripts/build-macos-dmg.sh
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: oriterm-nightly-macos-universal
          path: oriterm-nightly-*-macos-universal.dmg
  ```

- [ ] Update the nightly release's download step — the artifact name changes from `oriterm-nightly-macos-aarch64` to `oriterm-nightly-macos-universal` (the `needs` array already includes `build-macos`). The `release` job uses `merge-multiple: true` for `download-artifact`, so no download step changes are needed — the new artifact name is picked up automatically. Verify that `sha256sum oriterm-*` in the checksums step includes the `.dmg` file (it will, since the filename starts with `oriterm-`).

### Release Pipeline

- [ ] Add `build-macos` job to `.github/workflows/release.yml`:
  ```yaml
  build-macos:
    name: Build macOS (Universal)
    needs: validate
    runs-on: macos-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-apple-darwin,aarch64-apple-darwin
      - uses: Swatinem/rust-cache@v2
      - name: Build universal bundle
        run: ./scripts/build-macos-bundle.sh --universal --release
      - name: Package DMG
        run: ./scripts/build-macos-dmg.sh
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: oriterm-macos-universal
          path: oriterm-${{ github.ref_name }}-macos-universal.dmg
  ```

- [ ] Add `build-macos` to the release job's `needs` array (currently `needs: [build-linux, build-windows]`): `needs: [build-linux, build-windows, build-macos]`

- [ ] Add a macOS artifact download step to the release job (matching the per-platform pattern used for Linux and Windows):
  ```yaml
  - name: Download macOS artifact
    uses: actions/download-artifact@v4
    with:
      name: oriterm-macos-universal
      path: artifacts
  ```

- [ ] Verify that the release job's `files: artifacts/*` glob and `sha256sum oriterm-*` checksums step include the macOS `.dmg` — both already match, so no additional changes are needed beyond the download step and `needs` update.

### CI Pipeline

- [ ] Optionally add `clippy-macos` job to `.github/workflows/ci.yml` (the existing `test-macos` job already verifies that the workspace compiles on macOS):
  ```yaml
  clippy-macos:
    name: Clippy (macOS)
    runs-on: macos-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --workspace -- -D warnings
  ```

---

## 46.6 Section Completion

- [ ] `assets/macos/OriTerm.app/Contents/Info.plist` exists with correct keys
- [ ] `assets/macos/OriTerm.app/Contents/Resources/oriterm.icns` exists (valid icon)
- [ ] `scripts/build-macos-bundle.sh` produces a working `.app` that launches with dock icon
- [ ] `scripts/build-macos-dmg.sh` produces a valid `.dmg`
- [ ] `.github/workflows/nightly.yml` `build-macos` job upgraded to universal `.app` bundle + DMG (currently single-arch tarball)
- [ ] `.github/workflows/release.yml` includes new `build-macos` job and publishes DMG
- [ ] `.github/workflows/ci.yml` has macOS coverage (existing `test-macos` job; optionally add `clippy-macos`)
- [ ] Universal binary (x86_64 + aarch64) works on both Intel and Apple Silicon Macs
- [ ] `codesign --verify` passes on the assembled bundle
- [ ] DMG opens in Finder with drag-to-Applications layout
- [ ] `.gitattributes` marks `*.icns` as binary
- [ ] No `.gitkeep` files leak into the assembled bundle
- [ ] `Info.plist` `__VERSION__` placeholder is correctly replaced at build time
- [ ] `./build-all.sh`, `./clippy-all.sh`, `./test-all.sh` still green (no regressions)

**Known Limitations / Future Work (out of scope for this section):**
- **Code signing with Apple Developer ID**: This section uses ad-hoc signing (`--sign -`). Distributing via the Mac App Store or passing Gatekeeper without user override requires an Apple Developer certificate and notarization. WezTerm's `ci/deploy.sh` shows the pattern (import certificate from CI secret, `codesign --options runtime`, but no notarization).
- **Notarization**: `xcrun notarytool submit` + `xcrun stapler staple`. Requires Apple Developer Program membership ($99/year). Neither Alacritty nor WezTerm notarizes.
- **Homebrew cask formula**: Both Alacritty and WezTerm distribute via Homebrew casks. A `oriterm.rb` cask formula pointing to the GitHub release DMG would be a separate task.
- **`CFBundleDocumentTypes`**: WezTerm registers shell script file types (`.sh`, `.zsh`, `.bash`, `.fish`) and folder types so it can be set as the default terminal. Consider adding this in a future section.
- **Sparkle (auto-update framework)**: Some macOS apps use Sparkle for in-app updates. Not needed for now.
- **`CFBundleVersion` auto-increment**: Currently hardcoded to `"1"`. Mac App Store submissions require a monotonically increasing build number (e.g., git commit count or CI run number). Not needed for ad-hoc distribution.
- **`codesign --deep` deprecation**: Apple has deprecated `--deep` for new code signing submissions. When moving to Developer ID signing, sign each Mach-O binary individually rather than using `--deep` on the bundle.

**Exit Criteria:** The nightly and release CI pipelines produce a macOS `.dmg` containing a universal `OriTerm.app` that launches as a proper macOS GUI application with dock icon, Cmd+Tab switching, and system dark mode integration. Running the DMG build locally via `scripts/build-macos-bundle.sh --universal --release && scripts/build-macos-dmg.sh` produces the same result.
