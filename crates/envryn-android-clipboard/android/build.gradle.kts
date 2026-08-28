plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.envryn.clipboard"
    compileSdk = 36

    defaultConfig { minSdk = 29 }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions { jvmTarget = "1.8" }
}

dependencies {
    implementation(project(":tauri-android"))
}

// Resolve the exact same dependency graph on every developer machine and CI
// runner. Refresh intentionally with Gradle's `--write-locks` flag.
dependencyLocking {
    lockAllConfigurations()
}
