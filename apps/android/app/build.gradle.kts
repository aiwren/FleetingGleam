import org.gradle.api.tasks.Exec

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

// 仓库根目录（Cargo 工作区的根）
val repoRoot = rootProject.projectDir.parentFile.parentFile
val rustCrate = "fleetinggleam-android"
// 版本号只有一处：根 Cargo.toml 的 [workspace.package] version（核心库、桌面端也用它；发版时改那一处，见 docs/RELEASING.md）
val appVersion: String = Regex("""(?m)^version\s*=\s*"([^"]+)"""").find(File(repoRoot, "Cargo.toml").readText())?.groupValues?.get(1)
    ?: throw GradleException("读不到根目录 Cargo.toml 里 [workspace.package] 的 version")
// 内部版本号（build）：仓库的提交次数，每提交一次加一。系统的 versionCode 用它（覆盖安装要求递增），关于页显示成"0.1.0 (105)"。
// 取不到（没有 git、浅克隆）就构建失败：以前退回 1，装出来的包比手机上已有的旧，覆盖安装会被系统拒绝。
// 没有 git 历史时（源码包）用 -PFG_BUILD_NUMBER=<数字> 明确给出。
val buildNumber: Int = (findProperty("FG_BUILD_NUMBER") as String?)?.toIntOrNull() ?: run {
    fun git(vararg args: String): String? = runCatching { providers.exec { commandLine("git", "-C", repoRoot.absolutePath, *args) }.standardOutput.asText.get().trim() }.getOrNull()
    if (git("rev-parse", "--is-shallow-repository") == "true") {
        throw GradleException("浅克隆的提交数不是真正的版本号：拉完整历史（git fetch --unshallow，CI 里 fetch-depth: 0），或用 -PFG_BUILD_NUMBER 给出")
    }
    git("rev-list", "--count", "HEAD")?.toIntOrNull()
        ?: throw GradleException("取不到提交数作 versionCode（不在 git 仓库里？）：用 -PFG_BUILD_NUMBER=<数字> 给出")
}
val jniLibs = layout.projectDirectory.dir("src/main/jniLibs")
val uniffiOut = layout.buildDirectory.dir("generated/uniffi/kotlin")
// cargo 与 cargo-ndk 在 ~/.cargo/bin；Android Studio 启动的 Gradle 不一定带这个 PATH
val cargoBin = File(System.getProperty("user.home"), ".cargo/bin")
val cargoPath = listOf(cargoBin.absolutePath, System.getenv("PATH") ?: "").joinToString(File.pathSeparator)

android {
    namespace = "com.fleetinggleam.android"
    compileSdk = 36
    // NDK：本机用构建插件默认的版本（装在 SDK 里）；CI 的运行环境里没有那个版本，改用 ANDROID_NDK_HOME 指的那份（ci.yml 里设好）
    // （插件会拿版本号去核对这份 NDK，所以版本号也从它的 source.properties 读，不然对不上就当没装）
    System.getenv("ANDROID_NDK_HOME")?.let(::File)?.takeIf { File(it, "source.properties").isFile }?.let { dir ->
        ndkPath = dir.path
        File(dir, "source.properties").readLines().firstOrNull { it.startsWith("Pkg.Revision") }?.substringAfter("=")?.trim()?.let { ndkVersion = it }
    }

    defaultConfig {
        applicationId = "com.fleetinggleam.android"
        // 假设：最低 Android 12（API 31）——玻璃材质要实时模糊，面向高端机；记在 docs/ROADMAP.md 阶段 B
        minSdk = 31
        targetSdk = 36
        versionCode = buildNumber
        versionName = appVersion
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    signingConfigs {
        val ks = (findProperty("FG_KEYSTORE") as String?)?.let(::File)
        if (ks != null && ks.exists()) {
            create("release") {
                storeFile = ks
                storePassword = findProperty("FG_KEYSTORE_PASSWORD") as String?
                keyAlias = (findProperty("FG_KEY_ALIAS") as String?) ?: "fleetinggleam"
                keyPassword = findProperty("FG_KEY_PASSWORD") as String?
            }
        }
    }
    buildTypes {
        release {
            // 用 R8 压缩与优化：Compose 在没优化的包里首帧很慢
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // 正式签名：密钥库在仓库外（~/.config/fleetinggleam/release.jks），路径与密码从 ~/.gradle/gradle.properties 的 FG_* 读；
            // 没配（CI、别的机器）就退回调试密钥，仍能装到真机上测
            signingConfig = if (signingConfigs.findByName("release") != null) signingConfigs.getByName("release") else signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures { compose = true }
    sourceSets {
        getByName("main") {
            // UniFFI 生成的 Kotlin 绑定
            java.srcDir(uniffiOut)
        }
    }
    packaging {
        // 原生库在 APK 里压缩存放（34 MB 的核心库压完约 14 MB，APK 小一半）；代价是安装时解压一份到 lib/，装完多占约 34 MB。
        // 我们的包走 GitHub Release 直接下载，下载体积比安装体积更要紧。
        jniLibs {
            useLegacyPackaging = true
            // 高通 NPU 只用 HTP：DSP、GPU 后端和 ONNX Runtime 的 Java 绑定都用不上（核心库直接调 C 接口）
            excludes += listOf("**/libonnxruntime4j_jni.so", "**/libQnnDsp*.so", "**/libQnnGpu.so")
        }
    }
}

// 1. 用 cargo-ndk 把 Rust 核心编成 arm64-v8a 的 libfg_android.so，放进 jniLibs/
val cargoBuild by tasks.registering(Exec::class) {
    description = "用 cargo-ndk 编译 Rust 核心（arm64-v8a）"
    workingDir = repoRoot
    environment("PATH", cargoPath)
    commandLine("cargo", "ndk", "-t", "arm64-v8a", "-P", "31", "-o", jniLibs.asFile.absolutePath, "build", "-p", rustCrate, "--release")
    // 去掉符号表（cargo 的 strip 不一定落到 cdylib 上，AGP 又只在 NDK 配好时才 strip）：少 12 MB
    doLast {
        val strip = fileTree(File(android.ndkDirectory, "toolchains/llvm/prebuilt")) { include("*/bin/llvm-strip") }.files.firstOrNull()
        val so = File(jniLibs.asFile, "arm64-v8a/libfg_android.so")
        if (strip != null && so.exists()) exec { commandLine(strip.absolutePath, "--strip-all", so.absolutePath) }
    }
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

// 4. 文案：桌面端的 zh-CN.ts / en.ts 导出成 assets/i18n/base.<语言>.json（两端一字不差地共用；没有 node 时用仓库里已导出的那份）
val exportI18n by tasks.registering(Exec::class) {
    description = "把桌面端的文案导出给 Android"
    workingDir = rootProject.projectDir
    environment("PATH", cargoPath + File.pathSeparator + "/opt/homebrew/bin:/usr/local/bin")
    commandLine("node", "--experimental-strip-types", "--no-warnings", "scripts/export-i18n.mjs")
    isIgnoreExitValue = true
    inputs.dir(File(repoRoot, "apps/desktop/src/i18n"))
    outputs.dir(layout.projectDirectory.dir("src/main/assets/i18n"))
}

tasks.named("preBuild") { dependsOn(uniffiBindgen, copyLibcxx, exportI18n) }

dependencies {
    implementation("androidx.core:core-ktx:1.17.0")
    implementation("androidx.activity:activity-compose:1.11.0")
    implementation("androidx.compose.ui:ui:1.10.3")
    implementation("androidx.compose.foundation:foundation:1.10.3")
    implementation("androidx.compose.material3:material3:1.4.0")
    implementation("androidx.compose.ui:ui-tooling-preview:1.10.3")
    implementation("androidx.compose.animation:animation:1.10.3")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.4")
    implementation("androidx.lifecycle:lifecycle-process:2.9.4")
    // 随包的 Baseline Profile（app/src/main/generated/baselineProfiles）在安装时就让 ART 预编译热路径：冷启动少 20–30%
    implementation("androidx.profileinstaller:profileinstaller:1.4.1")
    // 读照片的 EXIF 方向（核心库的解码交给 BitmapFactory 时要自己摆正）
    implementation("androidx.exifinterface:exifinterface:1.3.7")
    // 照片墙的缩略图：异步解码、内存与磁盘缓存、按显示尺寸缩小
    implementation("io.coil-kt.coil3:coil-compose:3.3.0")
    // 隐私政策、使用条款在应用内的半屏浏览器（Chrome Custom Tabs 的局部高度）里打开
    implementation("androidx.browser:browser:1.9.0")
    // 系统文件选择器选的文件夹（SAF）
    implementation("androidx.documentfile:documentfile:1.1.0")
    // 和核心库之间的 JSON（方法的参数与结果、事件载荷、projects.json）
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
    // ONNX Runtime（带高通 QNN 执行器，1.28.0，与核心库编译时的 C 接口版本一致）和它拉进来的高通 QNN 运行库（2.42.0）：
    // 核心库运行时加载 libonnxruntime.so；高通手机上 CLIP 跑在 NPU 上，别的手机 QNN 起不来就回落 CPU（见 onnx.rs / scorer.rs）。
    // 许可：ONNX Runtime 是 MIT；QNN 运行库是高通的 AI Stack License，允许以目标代码形式随应用分发，不许单独分发或逆向（见 docs/THIRD_PARTY.md）
    implementation("com.microsoft.onnxruntime:onnxruntime-android-qnn:1.28.0")
    // 悬浮标签栏的玻璃材质：对后面的内容做实时模糊（Android 12+；更早的系统退为半透明）
    implementation("dev.chrisbanes.haze:haze:1.6.10")
    implementation("dev.chrisbanes.haze:haze-materials:1.6.10")
    // 液态玻璃：对背后内容做透镜折射、高光与模糊（AGSL，Android 13+；Apache-2.0）
    implementation("io.github.kyant0:backdrop:1.0.6")
    // UniFFI 生成的 Kotlin 绑定通过 JNA 调 Rust
    implementation("net.java.dev.jna:jna:5.17.0@aar")
}

// Kotlin 2.3 起 jvmTarget 只能在 compilerOptions 里设
kotlin { compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) } }

// core 包里的数据类（Photo、Decision、Project…）都是不可变的，但带 List 字段，Compose 默认当作"不稳定"，
// 于是每次父级重组照片墙上的每一格都重画。声明稳定后，参数没变的格子会被跳过（滚动和改判时不再重组整面墙）。
composeCompiler { stabilityConfigurationFiles.add(layout.projectDirectory.file("compose_stability.conf")) }


// Baseline Profile 的插件只在收集时挂上（`-PFG_BASELINE=1`）：它一挂上，release 包就会被签成调试密钥（AGP 把被测应用当测试目标）。
// 平时构建不需要它——AGP 自己会把 src/main/baseline-prof.txt 与 startup-prof.txt 合进包里。
// 收集：`./gradlew -PFG_BASELINE=1 :baselineprofile:collectNonMinifiedReleaseBaselineProfile`（连着一台 API 33+ 的设备），
// 然后把 baselineprofile/build/outputs/connected_android_test_additional_output/…/BaselineProfileGenerator_generate-startup-prof.txt
// 复制到 app/src/main/baseline-prof.txt 与 startup-prof.txt，提交进仓库。收集完 AGP 会卸载被测应用。
if (findProperty("FG_BASELINE") != null) {
    apply(plugin = "androidx.baselineprofile")
    extensions.configure<androidx.baselineprofile.gradle.consumer.BaselineProfileConsumerExtension> {
        automaticGenerationDuringBuild = false
        saveInSrc = true
    }
}
