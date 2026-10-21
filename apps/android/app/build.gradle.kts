import org.gradle.api.tasks.Exec

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// 仓库根目录（Cargo 工作区的根）
val repoRoot = rootProject.projectDir.parentFile.parentFile
val rustCrate = "fleetinggleam-android"
val jniLibs = layout.projectDirectory.dir("src/main/jniLibs")
val uniffiOut = layout.buildDirectory.dir("generated/uniffi/kotlin")
// cargo 与 cargo-ndk 在 ~/.cargo/bin；Android Studio 启动的 Gradle 不一定带这个 PATH
val cargoBin = File(System.getProperty("user.home"), ".cargo/bin")
val cargoPath = listOf(cargoBin.absolutePath, System.getenv("PATH") ?: "").joinToString(File.pathSeparator)

android {
    namespace = "com.fleetinggleam.android"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.fleetinggleam.android"
        // 假设：最低 Android 12（API 31）——玻璃材质要实时模糊，面向高端机；记在 docs/ROADMAP.md 阶段 B
        minSdk = 31
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { compose = true }
    sourceSets {
        getByName("main") {
            // UniFFI 生成的 Kotlin 绑定
            java.srcDir(uniffiOut)
        }
    }
    packaging {
        jniLibs { useLegacyPackaging = false }
    }
}

// 1. 用 cargo-ndk 把 Rust 核心编成 arm64-v8a 的 libfg_android.so，放进 jniLibs/
val cargoBuild by tasks.registering(Exec::class) {
    description = "用 cargo-ndk 编译 Rust 核心（arm64-v8a）"
    workingDir = repoRoot
    environment("PATH", cargoPath)
    commandLine("cargo", "ndk", "-t", "arm64-v8a", "-P", "31", "-o", jniLibs.asFile.absolutePath, "build", "-p", rustCrate, "--release")
    inputs.dir(File(repoRoot, "packages/core/src"))
    inputs.dir(File(repoRoot, "apps/android/rust/src"))
    inputs.files(File(repoRoot, "Cargo.toml"), File(repoRoot, "Cargo.lock"), File(repoRoot, "apps/android/rust/Cargo.toml"), File(repoRoot, "packages/core/Cargo.toml"))
    outputs.dir(jniLibs)
}

// 2. 在宿主上再编一份同样的库（debug，带符号），从它里面读 UniFFI 元数据生成 Kotlin 绑定（库模式：不需要 .udl 文件）。
//    不从 Android 的 .so 读，因为 release 配置会 strip 掉元数据所在的符号；两份库的接口元数据是一样的。
val cargoHostBuild by tasks.registering(Exec::class) {
    description = "在宿主上编译 Rust 绑定库（供生成绑定用）"
    workingDir = repoRoot
    environment("PATH", cargoPath)
    commandLine("cargo", "build", "-p", rustCrate, "--lib")
    inputs.dir(File(repoRoot, "apps/android/rust/src"))
    inputs.files(File(repoRoot, "apps/android/rust/Cargo.toml"))
    outputs.upToDateWhen { false }
}

val uniffiBindgen by tasks.registering(Exec::class) {
    description = "由宿主上的绑定库生成 Kotlin 绑定"
    dependsOn(cargoBuild, cargoHostBuild)
    workingDir = repoRoot
    environment("PATH", cargoPath)
    val hostLib = listOf("libfg_android.dylib", "libfg_android.so", "fg_android.dll").map { File(repoRoot, "target/debug/$it") }
    doFirst {
        val lib = hostLib.firstOrNull { it.exists() } ?: error("宿主上的绑定库没编出来：${hostLib.joinToString()}")
        commandLine("cargo", "run", "-p", rustCrate, "--bin", "uniffi-bindgen", "--", "generate", "--library", lib.absolutePath, "--language", "kotlin", "--out-dir", uniffiOut.get().asFile.absolutePath, "--no-format")
    }
    inputs.dir(File(repoRoot, "apps/android/rust/src"))
    outputs.dir(uniffiOut)
    // 生成只要一秒，接口一改绑定就得跟着变：每次都跑
    outputs.upToDateWhen { false }
}

// 3. ONNX Runtime 的预编译包动态链接 NDK 的 C++ 运行库，APK 里要带上它
val copyLibcxx by tasks.registering(Copy::class) {
    description = "把 NDK 的 libc++_shared.so 放进 jniLibs"
    dependsOn(cargoBuild)
    val ndk = android.ndkDirectory
    from(fileTree(File(ndk, "toolchains/llvm/prebuilt")) { include("*/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so") }.files)
    into(jniLibs.dir("arm64-v8a"))
}

tasks.named("preBuild") { dependsOn(uniffiBindgen, copyLibcxx) }

dependencies {
    implementation("androidx.core:core-ktx:1.17.0")
    implementation("androidx.activity:activity-compose:1.11.0")
    implementation("androidx.compose.ui:ui:1.9.3")
    implementation("androidx.compose.foundation:foundation:1.9.3")
    implementation("androidx.compose.material3:material3:1.4.0")
    implementation("androidx.compose.ui:ui-tooling-preview:1.9.3")
    // 悬浮标签栏的玻璃材质：对后面的内容做实时模糊（Android 12+；更早的系统退为半透明）
    implementation("dev.chrisbanes.haze:haze:1.6.10")
    implementation("dev.chrisbanes.haze:haze-materials:1.6.10")
    // UniFFI 生成的 Kotlin 绑定通过 JNA 调 Rust
    implementation("net.java.dev.jna:jna:5.17.0@aar")
}
