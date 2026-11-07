import groovy.json.JsonSlurper

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// Sürüm Cam Dock ile aynı (src-tauri/tauri.conf.json)
val camVersion: String = (JsonSlurper().parse(rootProject.file("../src-tauri/tauri.conf.json")) as Map<*, *>)["version"] as String
val camCode: Int = camVersion.split(".").map { it.toInt() }.let { it[0] * 10000 + it[1] * 100 + it[2] }

android {
    namespace = "com.camdock.phone"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.camdock.phone"
        minSdk = 26
        targetSdk = 35
        versionCode = camCode
        versionName = camVersion
    }

    // İmza anahtarı depoda tutulmaz: CI gizli anahtarlardan (ANDROID_KEYSTORE_B64,
    // ANDROID_KEY_PASSWORD) dosyayı oluşturur. Yoksa hata ayıklama imzası kullanılır.
    val ksFile = file(System.getenv("ANDROID_KEYSTORE") ?: "camdock.jks")
    val ksPass = System.getenv("ANDROID_KEY_PASSWORD")
    signingConfigs {
        if (ksFile.exists() && ksPass != null) {
            create("release") {
                storeFile = ksFile
                storePassword = ksPass
                keyAlias = "camdock"
                keyPassword = ksPass
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}
