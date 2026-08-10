import java.io.FileInputStream
import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// F-Droid / release signing — keystore.properties or env (never commit secrets)
val keystorePropsFile = rootProject.file("keystore.properties")
val keystoreProps = Properties().apply {
    if (keystorePropsFile.exists()) {
        load(FileInputStream(keystorePropsFile))
    }
}

fun prop(name: String, env: String = name): String? =
    keystoreProps.getProperty(name)
        ?: System.getenv(env)

android {
    namespace = "dev.xstrawman.junk"
    compileSdk = 35

    defaultConfig {
        applicationId = "dev.xstrawman.junk"
        minSdk = 26
        targetSdk = 35
        versionCode = 20
        versionName = "0.2.0-arcade"
        vectorDrawables.useSupportLibrary = true
    }

    signingConfigs {
        // F-Droid binary-repo style: you own the key.
        // Official F-Droid.org rebuilds and uses their key instead.
        create("release") {
            val storePath = prop("storeFile", "JUNK_KEYSTORE")
            if (storePath != null) {
                val f = file(storePath)
                if (f.exists()) {
                    storeFile = f
                    storePassword = prop("storePassword", "JUNK_KEYSTORE_PASSWORD")
                    keyAlias = prop("keyAlias", "JUNK_KEY_ALIAS") ?: "junk"
                    keyPassword = prop("keyPassword", "JUNK_KEY_PASSWORD")
                        ?: prop("storePassword", "JUNK_KEYSTORE_PASSWORD")
                }
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            val releaseSigning = signingConfigs.findByName("release")
            if (releaseSigning?.storeFile != null) {
                signingConfig = releaseSigning
            }
        }
        debug {
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(composeBom)
    androidTestImplementation(composeBom)

    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.animation:animation")
    debugImplementation("androidx.compose.ui:ui-tooling")

    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    // YouTube / peer-tube / etc. stream URL extraction (no Python yt-dlp on phone)
    implementation("com.github.TeamNewPipe:NewPipeExtractor:0.24.6")
}
