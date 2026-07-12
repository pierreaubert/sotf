# Android builds

# Build SOTF Android Rust cdylib for arm64-v8a
[group('android')]
android-rust:
    cargo ndk -t arm64-v8a -P 26 -o crates/app-android/android/gradle/app/src/main/jniLibs build -p sotf-android --release

# Build SOTF Android APK
[group('android')]
android-apk: android-rust
    cd crates/app-android/android/gradle && ./gradlew assembleRelease -PsotfVersion=$(cd ../../../.. && grep -m1 '^version = ' Cargo.toml | sed -E 's/version = "(.*)"/\1/')

# Clean Android build outputs
[group('android')]
android-clean:
    cd crates/app-android/android/gradle && ./gradlew clean
    rm -rf crates/app-android/android/gradle/app/src/main/jniLibs/arm64-v8a
