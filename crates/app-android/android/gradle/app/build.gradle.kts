plugins {
    id("com.android.application")
}

android {
    namespace = "org.spinorama.sotf.android"
    compileSdk = 35

    defaultConfig {
        applicationId = "org.spinorama.sotf.android"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = project.findProperty("sotfVersion")?.toString() ?: "0.1.0"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }

        manifestPlaceholders["nativeLibraryName"] = "sotf_android"
    }

    buildTypes {
        debug {
            isDebuggable = true
            isJniDebuggable = true
        }
        release {
            isMinifyEnabled = false
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    packaging {
        jniLibs {
            keepDebugSymbols += listOf("*/arm64-v8a/libsotf_android.so")
        }
    }

    lint {
        checkReleaseBuilds = false
    }
}
