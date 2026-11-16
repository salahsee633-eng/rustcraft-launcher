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
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            // The M33 5G (and every recent phone) is arm64. Keeping one ABI
            // halves build time and APK size.
            abiFilters += listOf("arm64-v8a")
        }
    }

    sourceSets["main"].jniLibs.srcDirs("src/main/jniLibs")
    sourceSets["main"].manifest.srcFile("src/main/AndroidManifest.xml")

    buildTypes {
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
