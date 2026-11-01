// A build of its own, so it runs without the Android SDK: `./gradlew -p core test`.
// The app includes it (`includeBuild("core")` in the root settings) and depends on
// `com.yaca:core`, which Gradle substitutes with this project.
pluginManagement {
    repositories {
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
    versionCatalogs {
        create("libs") {
            from(files("../gradle/libs.versions.toml"))
        }
    }
}

rootProject.name = "core"
