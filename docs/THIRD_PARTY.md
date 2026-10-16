# 第三方内容与许可

本项目自己的代码以 [AGPL-3.0-or-later](../LICENSE) 发布。下面列出的内容**不属于**本项目，各自适用自己的许可。

> **请先读这一条。** 下表里“许可”一栏，是根据上游仓库的公开声明整理的，**写这份文档时没有逐一回到上游核对**。你要再分发这些模型（比如把它们托管在自己的服务器上，让应用下载）之前，请逐项回到上游确认许可，尤其是标了“待核实”的。

## 模型

模型文件不在本仓库里，应用在第一次用到某项功能时才下载。文件名与模型包的对应关系见 `apps/desktop/src-tauri/src/modelpacks.rs`。

| 用途 | 模型 | 上游 | 许可 |
|---|---|---|---|
| 分析（标准 / 高质量） | CLIP ViT-B/32、ViT-L/14 视觉编码器 | OpenAI CLIP；ONNX 版本来自 Hugging Face 上的 `Xenova/clip-vit-base-patch32`、`Xenova/clip-vit-large-patch14` | MIT |
| 分析（极致，实验） | SigLIP So400m 视觉编码器 | Google | Apache-2.0 |
| 美学评分头 | `improved-aesthetic-predictor` 的线性头 | christophschuhmann / LAION | 待核实 |
| 美学评分头（极致档） | Aesthetic Predictor V2.5 的评分头 | discus0434 | **AGPL-3.0** |
| 照片搜索 | 多语言文本编码器（`text_multilingual_*`，int8） | 具体来源待补充 | 待核实 |
| 眼睛检测 | MediaPipe Face Landmarker 与 Blendshapes | Google MediaPipe | Apache-2.0 |
| 人物识别 | SFace（`face_recognition_sface_2021dec`） | OpenCV Zoo | Apache-2.0 |
| 场景描述 | Florence-2（int8） | Microsoft | MIT |
| 场景描述的翻译 | OPUS-MT（`opusmt_*`，int8） | Helsinki-NLP | **CC BY 4.0**，再分发需要署名 |

关于 **Aesthetic Predictor V2.5 是 AGPL-3.0**：本项目选用 AGPL-3.0-or-later，与它是兼容的。反过来要注意，如果将来想把本项目改成闭源或更宽松的许可证，必须先处理这一项（换掉这个评分头，或者另行获得授权）。

## 代码库

下面列的是主要的直接依赖，完整的依赖及其许可见 `pnpm-lock.yaml` 和 `apps/desktop/src-tauri/Cargo.lock`。

| 依赖 | 用途 | 许可 |
|---|---|---|
| [Tauri](https://tauri.app) 2 | 桌面应用框架 | MIT / Apache-2.0 |
| [ort](https://github.com/pykeio/ort) | ONNX Runtime 的 Rust 封装 | MIT / Apache-2.0 |
| [tokenizers](https://github.com/huggingface/tokenizers) | 文本分词 | Apache-2.0 |
| React、Vite | 前端 | MIT |
| [Motion](https://motion.dev) | 动画 | MIT |
| i18next、react-i18next | 国际化 | MIT |
| TanStack Start / Router、Tailwind CSS | 落地页 | MIT |
| Phosphor Icons | 落地页图标 | MIT |
| Geist、Geist Mono（经 Fontsource） | 字体 | SIL OFL 1.1 |

查看全部依赖的许可：

```bash
pnpm licenses list                          # 前端
cd apps/desktop/src-tauri && cargo tree     # Rust 依赖树（用 cargo-license 可列出许可）
```

macOS 上的 Vision、ImageIO、Metal Performance Shaders Graph 是系统自带的框架，通过 Swift 调用，不随应用分发。

## 示例照片

落地页和截图里的示例照片（`apps/web/public/` 下的 `photos/`、`wall/`、`shots/`）来自 [Lorem Picsum](https://picsum.photos)，遵循 [Unsplash 许可](https://unsplash.com/license)。它们**不是 AGPL 授权的内容**，仅用于展示；请不要把它们单独抽出来当作图库再分发。

## 名称与标识

“FleetingGleam”“惊鸿”这个名称，以及 `branding/` 里的 Logo 和图标，不在 AGPL 的授权范围内。欢迎你 fork 和修改代码，但请不要用这些名称和标识来暗示你的衍生版本是官方版本。
