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
    // Tauri Android currently requests Jackson 2.15.3. Pin a patched,
    // API-compatible line for the release runtime until upstream updates it.
    implementation("com.fasterxml.jackson.core:jackson-databind:2.18.9")
}

// Resolve the exact same dependency graph on every developer machine and CI
// runner. Refresh intentionally with Gradle's `--write-locks` flag.
dependencyLocking {
    lockAllConfigurations()
}
