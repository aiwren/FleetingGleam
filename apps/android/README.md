# FleetingGleam Android 端

Kotlin + Jetpack Compose（Material 3）的原生应用；所有核心逻辑在共用的 Rust 库 [`packages/core`](../../packages/core) 里，经 [`rust/`](rust)（UniFFI 绑定 crate）编成 `libfg_android.so` 给 Kotlin 调用。目标与步骤见 [docs/ROADMAP.md](../../docs/ROADMAP.md) 的"阶段 B"。

现在是**第 1、2 步的冒烟骨架**，两个页签（照片、设置）在底部悬浮的液态玻璃标签栏里（作者要求像 iOS 的悬浮 tab bar；现在是 [Haze](https://github.com/chrisbanes/haze) 的实时模糊 + 高光描边，折射在第 4 步用 AGSL 加）：

- **照片**：对应用内生成的示例照片做一次分析（分组、打分、取舍；装了模型就用 CLIP 打分、找脸量眼睛），显示用了哪个模型、跑在哪（CPU / NNAPI）。
- **设置 → 模型**：和 Mac 端同一份清单的 8 个模型包，下载（带进度、可取消）、删除；下载后按 SHA-256 校验，放在应用私有目录 `files/models/`。
- **设置 → NAS**：用内置的 SMB 客户端列出一台 NAS 的共享。

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
- ONNX Runtime 用 pyke 的 `aarch64-linux-android+nnapi` 预编译包；CLIP 在 NNAPI 上跑之前先和 CPU 比对一致（`packages/core/src/onnx.rs`），不一致就只用 CPU；QNN 没接（见路线图第 2 步）。
- 人脸：没有系统人脸检测，用核心库的 `facepipe.rs`（UltraFace 找脸 + MediaPipe 网格），需要 `detect` 与 `eyes` 两个包。

## 加接口

在 `rust/src/lib.rs` 里用 `#[uniffi::export]` 导出函数、`#[derive(uniffi::Record)]` 定义数据、`#[uniffi::export(foreign)]` 定义 Kotlin 实现的回调；重新 `assembleDebug` 即可，绑定自动更新。宿主上可以直接 `cargo test -p fleetinggleam-android` 测这些接口。
