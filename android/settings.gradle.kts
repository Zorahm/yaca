// The phone client of Yaca. Two builds: `core` is plain Kotlin/JVM (the wire,
// the feed reducer, the connection) and is built and tested without the Android
// SDK; `app` is the Compose application on top of it.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "yaca-android"

includeBuild("core")
include(":app")
