# 模型

应用本身**不带分析用的模型**：它们体积很大，只在你**第一次用到某项功能时**才下载，下载完之后完全离线。这份文档说明每个模型文件从哪里来、有多大、什么许可，以及怎么验证它们没被换过。

## 它们从哪里下载

应用里内置了一份清单：[`packages/core/models.manifest.json`](../packages/core/models.manifest.json)。每个文件在里面都有固定的**下载地址、大小和 SHA-256**。下载完成后应用会核对大小和 SHA-256，**对不上就丢弃**，不会使用一个被损坏或被替换过的文件。

文件分三类：

1. **直接来自公开上游（Hugging Face）**：下载地址固定在某个提交上（地址里带完整的提交号），上游之后再更新也不会悄悄变化。我们不托管这些文件，也没有自己的模型服务器。
2. **随应用分发的小文件**（共 3.7 MB，在 [`packages/core/model-data/`](../packages/core/model-data/)）：上游没有现成的、应用能直接读的版本，是我们把权重转成了应用需要的格式。因为很小，直接编进应用，不需要下载。
3. **我们转换后放在本仓库 Release 里的文件**（标签 `models-v1`）：同样是上游没有现成版本，转换出来的文件又比较大，所以放在 GitHub Release 的附件里。清单里固定了它们的 SHA-256，换成别的内容应用会拒绝。

应用联网只做一件事：下载这些文件。照片和照片的任何信息都不会发出去。

## 清单

“需下载”不包括随应用分发的文件。SHA-256 只列前 12 位，完整值在清单里。

### 标准分析（`standard`），需下载约 265 MB

| 文件 | 大小 | 获取方式 | 来源 | 许可 | SHA-256（前 12 位） |
|---|---|---|---|---|---|
| `clip_vit_b32_vision_fp16.onnx` | 176.1 MB | Hugging Face | Xenova/clip-vit-base-patch32 · onnx/vision_model_fp16.onnx | MIT | `35c4e0fb0aee` |
| `clip_vit_b32_vision_int8.onnx` | 88.6 MB | Hugging Face | Xenova/clip-vit-base-patch32 · onnx/vision_model_int8.onnx | MIT | `0ab0c1b3ace7` |
| `aesthetic_head_vit_b_32.json` | 11 KB | 随应用分发 | LAION-AI/aesthetic-predictor · sa_0_4_vit_b_32_linear.pth（权重转成 JSON） | MIT | `0a0f77e477c5` |

### 高质量分析（`high`），需下载约 609 MB

| 文件 | 大小 | 获取方式 | 来源 | 许可 | SHA-256（前 12 位） |
|---|---|---|---|---|---|
| `clip_vit_l14_vision_fp16.onnx` | 608.7 MB | Hugging Face | Xenova/clip-vit-large-patch14 · onnx/vision_model_fp16.onnx | MIT | `6e6b9e280b73` |
| `aesthetic_head_vit_l_14.json` | 16 KB | 随应用分发 | christophschuhmann/improved-aesthetic-predictor · sac+logos+ava1-l14-linearMSE.pth（各层合并为一个线性层，存成 JSON） | Apache-2.0 | `7ab153941d31` |

### 极致分析（实验）（`ultra`），需下载约 857 MB

| 文件 | 大小 | 获取方式 | 来源 | 许可 | SHA-256（前 12 位） |
|---|---|---|---|---|---|
| `siglip_so400m_vision_fp16.onnx` | 857.2 MB | 本仓库 Release | onnx-community/siglip-so400m-patch14-384-ONNX · onnx/model.onnx（抽出视觉部分并转为 fp16） | Apache-2.0 | `637a65ed8c1e` |
| `aesthetic_v25_head.json` | 24 KB | 随应用分发 | discus0434/aesthetic-predictor-v2-5 · aesthetic_predictor_v2_5.pth（各层合并为一个线性层，存成 JSON） | AGPL-3.0 | `1cd29370e105` |

### 照片搜索（`search`），需下载约 137 MB

| 文件 | 大小 | 获取方式 | 来源 | 许可 | SHA-256（前 12 位） |
|---|---|---|---|---|---|
| `text_multilingual_qint8.onnx` | 135.3 MB | Hugging Face | sentence-transformers/clip-ViT-B-32-multilingual-v1 · onnx/model_qint8_arm64.onnx | Apache-2.0 | `3bed77e83926` |
| `text_multilingual_tokenizer.json` | 2.0 MB | Hugging Face | sentence-transformers/clip-ViT-B-32-multilingual-v1 · tokenizer.json | Apache-2.0 | `5b4e1a8171c8` |
| `text_multilingual_dense.bin` | 1.6 MB | 随应用分发 | sentence-transformers/clip-ViT-B-32-multilingual-v1 · 2_Dense/model.safetensors（取出 512×768 的权重） | Apache-2.0 | `e5f6548cbcef` |

### MediaPipe 眼睛检测（`eyes`），需下载约 7 MB

| 文件 | 大小 | 获取方式 | 来源 | 许可 | SHA-256（前 12 位） |
|---|---|---|---|---|---|
| `face_landmarks.onnx` | 4.9 MB | 本仓库 Release | Google MediaPipe · face_landmarker.task 里的 face_landmarks_detector.tflite（tf2onnx 转换） | Apache-2.0 | `7e9a19e5ce2a` |
| `face_blendshapes.onnx` | 1.9 MB | 本仓库 Release | Google MediaPipe · face_landmarker.task 里的 face_blendshapes.tflite（tf2onnx 转换） | Apache-2.0 | `dd4eff02b5e3` |

### 人物识别（`faces`），需下载约 39 MB

| 文件 | 大小 | 获取方式 | 来源 | 许可 | SHA-256（前 12 位） |
|---|---|---|---|---|---|
| `face_recognition_sface_2021dec.onnx` | 38.7 MB | Hugging Face | opencv/face_recognition_sface · face_recognition_sface_2021dec.onnx | Apache-2.0 | `0ba9fbfa01b5` |

### 场景描述（`scenetext`），需下载约 390 MB

| 文件 | 大小 | 获取方式 | 来源 | 许可 | SHA-256（前 12 位） |
|---|---|---|---|---|---|
| `florence_vision_encoder_int8.onnx` | 93.7 MB | Hugging Face | onnx-community/Florence-2-base-ft · onnx/vision_encoder_int8.onnx | MIT | `d7876c1ab0f7` |
| `florence_embed_tokens_int8.onnx` | 39.4 MB | Hugging Face | onnx-community/Florence-2-base-ft · onnx/embed_tokens_quantized.onnx | MIT | `6b2258db1c8e` |
| `florence_encoder_model_int8.onnx` | 43.7 MB | Hugging Face | onnx-community/Florence-2-base-ft · onnx/encoder_model_quantized.onnx | MIT | `f4ad7a68f1fb` |
| `florence_decoder_model_merged_int8.onnx` | 98.2 MB | Hugging Face | onnx-community/Florence-2-base-ft · onnx/decoder_model_merged_quantized.onnx | MIT | `f22f52f980c3` |
| `florence_tokenizer.json` | 2.3 MB | Hugging Face | onnx-community/Florence-2-base-ft · tokenizer.json | MIT | `d69dcdb2323e` |
| `opusmt_encoder_model_int8.onnx` | 52.7 MB | Hugging Face | Xenova/opus-mt-en-zh · onnx/encoder_model_int8.onnx | Apache-2.0 | `262c0319bd0d` |
| `opusmt_decoder_model_merged_int8.onnx` | 60.0 MB | Hugging Face | Xenova/opus-mt-en-zh · onnx/decoder_model_merged_int8.onnx | Apache-2.0 | `84f71e0787bd` |
| `opusmt_tokenizer.json` | 2.3 MB | 随应用分发 | Xenova/opus-mt-en-zh · tokenizer.json（去掉分词器读不懂的文本归一化步骤） | Apache-2.0 | `9b213671b4e7` |

## 派生文件是怎么做出来的

下面这些不是上游原样的文件，是我们转换的。**转换结果以清单里的 SHA-256 为准**，不保证你重新转换能得到逐字节相同的文件。

- **美学评分头（3 个 `aesthetic_*.json`）**：上游是 PyTorch 的 `.pth` 权重。B/32 的是一个线性层，直接取出 512 个权重和一个偏置；L/14 和 v2.5 的是几个中间没有激活函数的线性层，数学上等于一个线性层，所以把它们合并成一个权重向量和一个偏置。
- **`text_multilingual_dense.bin`**：取自上游 `2_Dense/model.safetensors` 里形状为 512×768 的那个张量，按小端 float32 原样写出。
- **`opusmt_tokenizer.json`**：就是上游的 `tokenizer.json`，只去掉了 `normalizer` 一项（一个 `tokenizers` 库读不了的预编译字符映射表），其他字段完全相同。对普通英文句子没有影响。
- **`siglip_so400m_vision_fp16.onnx`**：上游 `onnx-community/siglip-so400m-patch14-384-ONNX` 只有 3.5 GB 的完整模型（文本和视觉两部分，`model_fp16.onnx` 是个空文件），应用只需要视觉部分。我们用 `onnx.utils.extract_model` 从输入 `pixel_values` 到输出 `image_embeds` 抽出视觉图，再用 ONNX Runtime 的 `convert_float_to_float16`（保留输入输出为 float32）转成 fp16。
- **`face_landmarks.onnx`、`face_blendshapes.onnx`**：Google MediaPipe 的 `face_landmarker.task`（[官方下载地址](https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task)）是个 zip，里面有 TFLite 格式的 `face_landmarks_detector.tflite` 和 `face_blendshapes.tflite`。我们用 `python -m tf2onnx.convert --tflite <文件> --output <文件>.onnx --opset 17` 转成了 ONNX。

## 开发时怎么用模型

- 把模型文件放进 `apps/desktop/models/`（已在 `.gitignore` 里，不会被提交）。**debug 构建**发现这个文件夹里有 `clip_vit_b32_vision_int8.onnx` 时，就直接用它，不走下载。
- 想在开发时测试下载流程：`FG_FORCE_DOWNLOAD=1 pnpm dev:desktop`，应用会像用户那样下载到自己的数据目录。
- 随应用分发的文件不用放：它们在应用里。
- 完整的下载测试（从 Hugging Face 下载人物识别包，约 39 MB，校验并加载）默认不跑，需要时：

  ```bash
  cd apps/desktop/src-tauri
  cargo test --lib downloads_verifies_and_loads -- --ignored --nocapture
  ```

## 新增或更换一个模型

1. 拿到文件，算出大小和 SHA-256：`shasum -a 256 <文件>`，`stat -f %z <文件>`（macOS）。
2. 如果来自 Hugging Face，把地址固定到一个提交：`https://huggingface.co/<仓库>/resolve/<完整提交号>/<路径>`，**不要用 `main`**。
3. 在 `models.manifest.json` 里加一项（`url`、`size`、`sha256`，另外写上 `source` 和 `license`）；在 `modelpacks.rs` 的 `PACKS` 里把文件加进某个包。
4. 跑 `cargo test --lib`：其中一个测试会检查清单和各个包是否一一对应、随应用分发的文件是否和清单一致。
5. 在 [THIRD_PARTY.md](THIRD_PARTY.md) 里补上来源和许可。**确认许可允许你这样分发**，有的要求署名。

## 发布前要做的一件事

清单里有 3 个文件的地址指向本仓库的 Release `models-v1`（SigLIP 和两个 MediaPipe 模型）。**这个 Release 需要有人上传一次**，在那之前，“极致分析”和“MediaPipe 眼睛检测”会下载失败（其余功能不受影响）。步骤见 [RELEASING.md](RELEASING.md#模型怎么提供给用户)。
