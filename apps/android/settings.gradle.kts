// FleetingGleam Android 端：Kotlin + Jetpack Compose（Material 3），核心逻辑在 Rust 库 libfg_android.so（见 rust/ 与 app/build.gradle.kts）。
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
rootProject.name = "FleetingGleam"
include(":app")
// Baseline Profile 的生成模块（只在生成时用，不进包）
include(":baselineprofile")
