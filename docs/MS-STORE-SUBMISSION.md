# Microsoft Store submission for SotF

This is the Microsoft-Store-only path. The standalone `.exe` / `.msix`
sideload downloads (signed with your own self-signed cert and shipped via
GitHub Releases / `sotf.spinorama.org/downloads/`) keep working unchanged.

## Status

| Piece | State |
|---|---|
| App reserved in Partner Center | Done — bundle id `org.spinorama.sotf` |
| Store-assigned Publisher Identity (`CN=817DD0F7-…`) | Done — already in `builds/windows/AppxManifest.xml` |
| Store-assigned Package Name (`SotF.SotF`) | Done — already in manifest |
| `build-msix.ps1 -Arch arm64` | Done — manifest patched to `ProcessorArchitecture="arm64"` automatically |
| `build-release-local.sh` builds both x86_64 and arm64 MSIX | **Done as of this change** (the arm64 path was missing) |
| Partner Center submission (web upload or API) | **TODO — see below** |
| Store metadata (description, screenshots, age rating, privacy URL) | **TODO — Partner Center** |

The remainder is operational.

---

## 1. Build both MSIX files in one go

```bash
./scripts/build-release-local.sh --skip-macos --skip-linux
```

After it finishes, `dist/` should contain both:

- `sotf-desktop-<version>-windows-x86_64.msix`
- `sotf-desktop-<version>-windows-arm64.msix`

Plus the unsigned standalone TUIs:

- `sotf-tui-<version>-windows-x86_64.exe`
- `sotf-tui-<version>-windows-arm64.exe`

The TUI binaries are not part of the Store submission — they ship via the
website mirror only. (See `AppxManifest.xml` line 75 for the rationale: a
second `<Application>` for the TUI either needs `AppListEntry="none"` plus
Microsoft's HeadlessAppBypass waiver, or clutters Start Menu with a
duplicate tile most Store users wouldn't want.)

> One-time setup on the remote Windows builder if you've never built ARM64
> before (the script tries `rustup target add` automatically; this is the
> manual fallback):
>
> 1. `rustup target add aarch64-pc-windows-msvc`
> 2. In Visual Studio Installer → 2022 Build Tools → Modify → enable
>    "MSVC v143 - VS 2022 C++ ARM64/ARM64EC build tools (latest)".

## 2. Code signing — what the Store actually wants

Microsoft Store re-signs every `.msix` after upload with its own
publisher identity. Your local Authenticode signature is **discarded** in
the Store distribution path. Two practical implications:

- You can upload either a **signed** or **unsigned** MSIX. If signed, the
  cert subject must equal the manifest's `Publisher=` (i.e.
  `CN=817DD0F7-95A7-40FF-AD7A-90E15D2F89AD`) or the upload pre-validation
  rejects it. Self-signed certs whose subject CN is the Store-assigned
  publisher GUID work fine.
- Your existing self-signed cert (used for sideload distribution) is
  almost certainly NOT subject `CN=817DD0F7-…`, so signing the
  Store-bound MSIX with it would fail. **Solution**: pass `--skip-sign`
  to `build-release-local.sh` for Store submissions, OR maintain a
  separate cert whose subject matches the publisher GUID.

The `--skip-sign` path is what most Store submitters use. The `.msix`
arrives at Partner Center unsigned, the Store signs it with its own cert
during ingestion, and end users install a Microsoft-signed package. No
end user ever sees your cert in the Store path.

```bash
./scripts/build-release-local.sh --skip-macos --skip-linux --skip-sign
```

## 3. Submit via Partner Center (web — easiest)

1. Log in to <https://partner.microsoft.com/dashboard> and select the SotF
   app. (You already reserved the name; this is just where it lives.)
2. **Pricing and availability**: free, public, all markets — or whatever
   your distribution choice is. Save.
3. **Properties**: category = "Music", subcategory = "Audio Editing", at
   minimum. Privacy policy URL is required as soon as you declare any
   data collection or even just "uses the network". Screenshots and a
   1024×1024 store icon are required.
4. **Age ratings**: complete the IARC questionnaire. SotF will get
   "All ages" if you answer truthfully.
5. **Properties → Product declarations**: declare what's true about the
   app — accesses microphone, camera, network, etc. Each declaration
   maps to capabilities you've already put in the manifest.
6. **Packages**: drag both `sotf-desktop-<version>-windows-x86_64.msix`
   and `sotf-desktop-<version>-windows-arm64.msix` onto the upload
   target. Partner Center auto-detects `ProcessorArchitecture` and
   dedupes by version + arch. Two packages, same version → one
   submission for both architectures.
7. **Store listings**: paste from `dist/store-description.md` (this is
   the same source you used for the macOS App Store filing). Add four
   to ten screenshots, 1280×720 or larger. Include keywords.
8. **Submit for certification**. Microsoft's automated test pass usually
   completes in hours; manual review can take 1–3 business days for a
   first submission.

## 4. Submit via API (automation, optional)

For CI / scripted releases, the [Microsoft Store Submission API](https://learn.microsoft.com/en-us/windows/uwp/monetize/create-and-manage-submissions-using-windows-store-services)
exposes the same workflow over REST. High-level flow:

1. Create an Azure AD app and grant it the "Microsoft Store Services"
   role for your Partner Center tenant. Save the tenant id, client id,
   and client secret somewhere a CI runner can read.
2. Get an OAuth token from `https://login.microsoftonline.com/<tenant>/oauth2/token`.
3. `POST /v1.0/my/applications/<app-id>/submissions` to create a new
   submission. The response contains an Azure Blob SAS URL.
4. Upload a ZIP containing all `.msix` files (and any updated
   metadata / screenshots) to the Blob URL via standard Azure block
   blob PUT.
5. `PATCH` the submission with anything that changed in the JSON
   metadata (description, packages, etc.).
6. `POST .../submissions/<sub-id>/commit` to send for certification.

Most one-person shops stick with the web UI for the first few releases
and only switch to API once the cadence justifies the auth overhead.

## 5. Two architectures: separate MSIX vs MSIXBUNDLE

You're submitting two separate `.msix` files, not a single
`.msixbundle`. That's deliberate:

- A `.msixbundle` packs multiple architectures into one file the Store
  can route. It's slightly nicer for some download mechanics but adds an
  extra `MakeAppx bundle` step and a multi-input AppxBundleManifest.
- Submitting two architecture-specific `.msix` files achieves the same
  end-user result — Partner Center treats them as one application with
  two arch variants. Each end user downloads the matching one.

If you later want a `.msixbundle` (e.g. to expose a single download URL
on the website), it's a one-line `MakeAppx bundle` invocation against
the two `.msix` files. Not required for Store submission.

## 6. Microsoft 365 / business store / TestFlight equivalent

There's no real TestFlight equivalent for the Microsoft Store, but
Partner Center supports two pre-prod distribution paths:

- **Private audience**: list the app as "Private" in Pricing and
  availability, then add specific Microsoft account emails as testers.
  They can install via a Store Promotion Code link before the public
  release.
- **Package Flights**: progressively ship a build to a percentage of
  users. Useful once you have an existing Store presence; less useful
  for the first submission.

## 7. After submission

The Store will email you when the build moves between *Submission
received → In certification → Publishing → In the Store*. Most
rejections at this stage are metadata-related (wrong screenshot
aspect ratio, missing privacy URL, age-rating questionnaire incomplete),
not technical-package issues — `build-release-local.sh` already runs
through Microsoft's `MakeAppx pack` validator, so by the time the
package reaches Partner Center the manifest, capabilities, and
publisher identity are all correct.

Post-publish, end users on Windows 10/11 and Windows-on-ARM
(Surface Pro X, Copilot+ PCs) can install via `ms-windows-store://pdp?productid=<id>`
or by searching "SotF" in the Microsoft Store app. Updates ship through
the same cadence: re-run `build-release-local.sh` with a bumped
`Cargo.toml` version → upload both new `.msix` files → submit.
