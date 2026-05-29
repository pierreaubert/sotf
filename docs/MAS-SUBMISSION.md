# Mac App Store submission for SotF

This is the App-Store-only path. The Developer ID DMG path
(`scripts/build-dmg-sotf.sh` + `scripts/sign-macos.sh`) is unaffected and
keeps shipping to GitHub Releases / `sotf.spinorama.org/downloads/`.

## Status

| Piece | State |
|---|---|
| Apple Distribution cert | In keychain |
| 3rd Party Mac Developer Installer cert | In keychain |
| `builds/macos/entitlements-mas.plist` | Done (sandboxed, hardened-runtime-friendly) |
| `scripts/build-pkg-mas.sh` | Done (preflight + sign + productbuild) |
| Mac App Store Distribution provisioning profile | **TODO — manual** |
| App Store Connect record for `org.spinorama.sotf` | **TODO — manual** |
| Sandbox-safe music-library access (security-scoped bookmarks) | **TODO — code work, see below** |
| Privacy-policy URL + screenshots + metadata | **TODO — App Store Connect** |

The remainder of this doc covers the four TODOs.

---

## 1. Provisioning profile (one-time)

A "Mac App Store Distribution" profile is what binds your bundle id, the
Apple Distribution cert, and the entitlements you're allowed to claim.

1. Make sure the App ID `org.spinorama.sotf` exists at
   <https://developer.apple.com/account/resources/identifiers>. Enable the
   capabilities the entitlements file claims:
   - App Sandbox
   - Camera
   - Microphone
   - Music
2. Create a new provisioning profile at
   <https://developer.apple.com/account/resources/profiles> →
   "Mac App Store Distribution" → pick `org.spinorama.sotf` → pick the
   "Apple Distribution: Pierre Aubert (RTH7ZJXLT6)" cert.
3. Download the `.provisionprofile` file. Save it as
   `builds/macos/sotf-mas.provisionprofile` (or anywhere, and set
   `MAS_PROVISIONING_PROFILE=/path/to/file` in `~/.sotf-release.conf`).

> **Trap to avoid:** when the developer.apple.com UI asks you to pick a
> certificate for this profile, pick **"Apple Distribution: …"** and not
> **"Developer ID Application: …"**. The latter is for the DMG /
> notarization path; if you select it, App Store Connect will reject the
> upload at validation time with the cryptic
> *"Invalid Provisioning Profile. Missing code-signing certificate."*
> `scripts/build-pkg-mas.sh` cross-checks this locally and aborts before
> uploading, but only if the profile is already on disk — pick correctly
> in the UI on the first try and save yourself a round trip.

The build script's preflight will tell you when this step is missing — it
already does, today. After this you can run the script and it will produce
a signed `.pkg`.

## 2. App Store Connect record (one-time)

App Store Connect is the upload target. You can't upload to a record that
doesn't exist.

1. <https://appstoreconnect.apple.com> → My Apps → "+" → New App.
2. Platform: macOS. Name: `SotF`. Primary language: English.
   Bundle ID: pick `org.spinorama.sotf` from the dropdown (this comes from
   the App ID you registered above).
3. SKU: any unique string (`org.spinorama.sotf`).
4. Once created, fill in the App Information page:
   - Privacy policy URL (required for any app that touches the network).
   - App category: Music.
5. Under the version, fill metadata:
   - Description: paste from `dist/store-description.md`.
   - Promotional text, keywords, support URL, marketing URL.
   - Screenshots: 1280×800 minimum, all locales you list.
   - App icon: 1024×1024 PNG (no alpha, no rounded corners — Apple
     adds those).
   - Version number: must match `CFBundleShortVersionString` in the
     uploaded build (i.e. the workspace `Cargo.toml` version).
6. App Privacy: declare data collection. SotF currently:
   - Does not collect personal data.
   - Makes outbound HTTPS calls to `api.spinorama.org` for speaker /
     headphone metadata.
   - Does not use third-party trackers.
   - Stores all preferences locally.
   The honest filing is "Data Not Collected" — but read the App Privacy
   questionnaire carefully and answer truthfully; mistakes here cause
   rejections.

## 3. Sandbox-safe music-library access (code work)

`com.apple.security.app-sandbox = true` blocks the app from touching paths
outside its container unless the user has explicitly granted access via
NSOpenPanel. SotF's current pattern — let the user pick a library
directory once, then re-open it on every launch — does not work in a
sandbox without security-scoped bookmarks.

The fix on macOS:

1. When the user picks a directory via NSOpenPanel, immediately call
   `NSURL bookmarkDataWithOptions:NSURLBookmarkCreationWithSecurityScope`
   on the returned URL.
2. Persist the bookmark blob in `Library/Containers/<bundle>/Data/...`
   (or wherever the existing config goes — the container path is auto-
   sandboxed and writable).
3. On the next launch, resolve each bookmark with
   `NSURL URLByResolvingBookmarkData:options:NSURLBookmarkResolutionWithSecurityScope`.
4. Call `startAccessingSecurityScopedResource` on each resolved URL before
   reading, and `stopAccessingSecurityScopedResource` when done. Failure
   to call these is silent (reads just return ENOENT).

Suggested implementation surface (all macOS-conditional):

- `crates/sotf-player/src/config/library_paths.rs` — abstracted today as
  plain `PathBuf`. Add a parallel `BookmarkedPath` type behind
  `cfg(target_os = "macos")` that holds (path, bookmark_blob).
- One thin Objective-C bridge file under `crates/app-gpui/src/macos/` (or
  reuse `objc2-foundation` already in the dep tree) for the
  `bookmarkData` / `URLByResolvingBookmarkData` APIs.
- Call `start_accessing` in the directory-scan code path; release on scan
  completion.

This is the only meaningful code change required for MAS. Until it's done,
a sandboxed build will only see the user-selected library *during the
session in which they picked it* — usable for review-time testing but not
shippable to real users.

## 4. Build, validate, upload

Once the profile is in place:

```bash
# Build the arm64 binary (you do this anyway for the DMG path).
cargo build --release --target aarch64-apple-darwin -p sotf-gpui --features hal,onnx

# Build, sign, and package for MAS.
./scripts/build-pkg-mas.sh

# The script writes CFBundleVersion from `git rev-list --count HEAD` by
# default. Use `--build-number <integer>` only for manual recovery uploads.

# Validate before upload (catches obvious rejections without burning a
# review slot).
xcrun altool --validate-app \
    -f dist/sotf-desktop-${VERSION}-macos-arm64-mas.pkg \
    -t macos \
    -u <APPLE_ID> \
    -p <APP_SPECIFIC_PASSWORD>

# Upload to App Store Connect.
xcrun altool --upload-app \
    -f dist/sotf-desktop-${VERSION}-macos-arm64-mas.pkg \
    -t macos \
    -u <APPLE_ID> \
    -p <APP_SPECIFIC_PASSWORD>
```

`<APP_SPECIFIC_PASSWORD>` is generated at
<https://appleid.apple.com> → Sign-In and Security → App-Specific
Passwords. Save it in your password manager; `notarytool store-credentials`
also accepts it under the same profile name you already use for
notarization.

Alternative: open Transporter.app, drag the `.pkg` in, click Deliver.
Same effect, GUI instead of CLI.

Once uploaded, App Store Connect needs ~15 min to ingest the build, then
it appears under TestFlight or in the version's build picker, where you
attach it and submit for App Review.

## 5. App Review expectations

Likely review questions / things they'll exercise:

- "How does the user grant access to their music library?" → must
  demonstrate the bookmark flow (point 3 above).
- "Does the app work without admin privileges?" → yes; the .pkg installs
  to `/Applications` and runs sandboxed.
- "Does it phone home?" → only `api.spinorama.org` for speaker / headphone
  metadata. Disclose in the App Privacy section.
- Microphone access on first room-EQ recording → privacy prompt fires
  using `NSMicrophoneUsageDescription`. Make sure the string is honest
  about *why* (room-EQ calibration), not generic.
- Camera access on QR scanner → same pattern with
  `NSCameraUsageDescription`.

Plan for one or two rejection cycles. The first MAS submission almost
always gets bounced for a metadata reason (privacy URL, screenshot size,
description claim Apple disagrees with), not a code reason — the technical
build is the easy part once the four TODOs above are done.
