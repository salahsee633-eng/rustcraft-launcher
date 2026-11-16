plugins {
    id("com.android.application")
}

android {
    namespace = "org.rustcraft.launcher"
    compileSdk = 35
    ndkVersion = "27.0.12077973"

    defaultConfig {
        applicationId = "org.rustcraft.launcher"
        minSdk = 26
        targetSdk = 35
        versionCode = 2
        versionName = "0.1.0"

        ndk {
            // The M33 5G (and every recent phone) is arm64. Keeping one ABI
            // halves build time and APK size.
            abiFilters += listOf("arm64-v8a")
        }
    }

    sourceSets["main"].jniLibs.srcDirs("src/main/jniLibs")
    sourceSets["main"].manifest.srcFile("src/main/AndroidManifest.xml")

    // A stable preview key, checked in on purpose: GitHub Actions creates a
    // random debug key per run, which would force an uninstall (and wipe the
    // game data) on every update. This key is for sideloaded previews only;
    // a Play/release build will use its own private key.
    signingConfigs {
        create("preview") {
            storeFile = file("rustcraft-debug.keystore")
            storePassword = "android"
            keyAlias = "rustcraftdebug"
            keyPassword = "android"
        }
    }

    buildTypes {
        debug {
            signingConfig = signingConfigs.getByName("preview")
        }
        release {
            isMinifyEnabled = false
        }
    }

    // JNI libraries must be extracted: the JRE loader dlopen()s them from the
    // filesystem, not from the APK.
    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
