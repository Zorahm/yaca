import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlin.serialization)
}

group = "com.yaca"
version = "1.0.0-beta"

// Compiled by whatever JDK runs Gradle, for Java 17 bytecode: the app's D8 reads
// it, and a toolchain would download a second JDK for nothing.
java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
        allWarningsAsErrors.set(true)
    }
}

dependencies {
    api(libs.kotlinx.coroutines.core)
    api(libs.kotlinx.serialization.json)
    api(libs.okhttp)

    testImplementation(libs.kotlin.test)
    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.okhttp.mockwebserver)
    testImplementation(libs.okhttp.tls)
}

tasks.test {
    // The fixtures the Rust side writes (`crates/yaca-remote/tests/fixtures.rs`)
    // are read from the source tree, not copied: a stale copy would test nothing.
    systemProperty("yaca.fixtures", layout.projectDirectory.dir("src/test/resources/fixtures").asFile.path)
}
