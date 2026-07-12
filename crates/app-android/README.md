# sotf-android (lib: `sotf_android`)

Android app shell for the SOTF music player. Compiles to a shared library
(`.so`) loaded by a `NativeActivity` Gradle project.

## Architecture

```text
NativeActivity -> libsotf_android.so -> android_main() -> GPUI app -> PlaceholderView
```

## Dependencies

- `gpui` / `gpui-android` -- GPUI framework with Android Vulkan backend
- `gpui-ui-kit` -- UI components
- `sotf-player` / `sotf-engine` -- Future: player logic and audio engine

## Building

Prerequisites:

- Android SDK installed and `ANDROID_HOME` set.
- Android NDK installed and `ANDROID_NDK_HOME` set.
- `cargo-ndk` installed (`cargo install cargo-ndk`).

Build the APK from the workspace root:

```bash
./scripts/build-apk.sh
```

Or manually:

```bash
just android-apk
```

The unsigned APK is copied to `dist/sotf-android-<version>.apk`.

## License

See the root workspace `LICENSE` file.
