# FleetingGleam Android 端

Kotlin + Jetpack Compose（Material 3）的原生应用；所有核心逻辑在共用的 Rust 库 [`packages/core`](../../packages/core) 里，经 [`rust/`](rust)（UniFFI 绑定 crate）编成 `libfg_android.so` 给 Kotlin 调用。目标与步骤见 [docs/ROADMAP.md](../../docs/ROADMAP.md) 的"阶段 B"。

现在是**第 1 步的冒烟骨架**：显示核心库版本、对应用内生成的示例照片做一次分析（分组、打分、取舍）、列出一台 NAS 的共享。

## 构建

需要：Android Studio（或 SDK + NDK 26/27）、JDK 17、Rust 稳定版，以及

```bash
rustup target add aarch64-linux-android && cargo install cargo-ndk
```

然后

```bash
./gradlew assembleDebug
```

Gradle 会先跑 `cargo ndk … build -p fleetinggleam-android --release`（产物放到 `app/src/main/jniLibs/arm64-v8a/`，不提交），再用库里的元数据生成 Kotlin 绑定到 `app/build/generated/uniffi/kotlin/`（包名 `uniffi.fleetinggleam`）。只编 arm64-v8a。

装到模拟器（arm64 镜像）或手机：`./gradlew installDebug`。

## 假设（见路线图）

- 最低 Android 12（API 31）：玻璃材质要实时模糊，面向高端机。
- ONNX Runtime 用 pyke 的 `aarch64-linux-android+nnapi` 预编译包；QNN 等第 2 步再看。

## 加接口

在 `rust/src/lib.rs` 里用 `#[uniffi::export]` 导出函数、`#[derive(uniffi::Record)]` 定义数据、`#[uniffi::export(foreign)]` 定义 Kotlin 实现的回调；重新 `assembleDebug` 即可，绑定自动更新。宿主上可以直接 `cargo test -p fleetinggleam-android` 测这些接口。
