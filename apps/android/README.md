# FleetingGleam Android 端

Kotlin + Jetpack Compose（Material 3）的原生应用；所有核心逻辑在共用的 Rust 库 [`packages/core`](../../packages/core) 里，经 [`rust/`](rust)（UniFFI 绑定 crate）编成 `libfg_android.so` 给 Kotlin 调用。

和 Mac 端功能对齐的完整应用。两个页签（照片、设置）在底部悬浮的液态玻璃标签栏里：

- **照片**：项目首页（项目卡片、回收站）；新建项目从手机相册、文件夹、网盘（系统的文件选择器）或 NAS 选照片；分析在前台服务里跑，边分析边出结果，过热或低电时自动暂停；照片墙、同组对比、大图、检查器、按场景浏览和用一句话找照片；整理时把保留的照片导出到相册或文件夹、NAS。
- **设置**：分析方式与模型档位、模型下载与删除（下载后按 SHA-256 校验，放在应用私有目录 `files/models/`）、NAS、风格与喜好、云备份、诊断导出、关于。
- 高通芯片的手机上，模型可以通过随应用附带的高通运行库跑在 NPU（HTP）上；别的手机用 CPU。


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
