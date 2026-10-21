# 拾光（选片）调研笔记

调研日期：2026-09-21。标注「未验证」的是我的判断或没能实测的内容，不是来源里的结论。

## 1. 专业摄影师怎么选片

来源：[Narrative 选片 FAQ](https://narrative.so/blog/photo-culling-complete-faq)、[Imagen 选片指南](https://imagen-ai.com/valuable-tips/what-is-culling-in-photography/)、[NANPA 评委 25 问](https://nanpa.org/2020/09/25/twenty-five-questions-to-think-like-a-photo-competition-judge/)、[比赛评分标准](https://www.micimage.com/contests/photo-contest-judging)。

专业选片是分层的，不是一个总分：

| 层 | 问的问题 | 性质 |
|---|---|---|
| 1 技术门槛 | 主体对焦了吗？有没有手抖/运动模糊？曝光能不能救？ | 硬性淘汰 |
| 2 冗余 | 同一瞬间连拍了 8 张，哪一张是最好的？ | 每组只留 1 张 |
| 3 主体与表情 | 有没有闭眼、怪表情？主体在做有趣的事吗？ | 人像、抓拍权重高 |
| 4 构图 | 主体位置、干扰物、背景、地平线是否水平、视角是否最佳 | 判断 |
| 5 光与影调 | 光线质量、影调层次、色彩是否和谐、有没有对比 | 判断 |
| 6 故事与冲击力 | 有没有情绪和主题？独特吗？还是只是“到此一游”的记录？ | 最难，最主观 |

要点：
- 事件类摄影，技术门槛和冗余决定大部分取舍；人像类，表情和情感连接比技术更重要。摄影师会在开始前先定好自己的优先级。
- 技术不完美但抓住真实瞬间的照片，也值得留。所以“低技术分”不应等于“直接淘汰”，而应进入“待定”。
- 比赛评分的一个例子（总分 80）：明暗 15、清晰 15、构图 20、风格 15、技术 15。

对旅行照片特别重要的一点：**“一组能讲完这次旅行的照片”**。选出来的应该覆盖不同地点、人物、细节，而不是 20 张相似的最佳日落。

## 2. 开源选片项目怎么做

来源：[Culld](https://github.com/codykhadev/culld)、[PhotoSort-AI](https://github.com/CAPTAINIFZY/PhotoSort-AI)、[Phaicull](https://github.com/omelkes/phaicull)。

- **Culld**：只用两个指标，拉普拉斯方差（清晰度）和眼睛纵横比 EAR（是否闭眼）。连拍分组用感知哈希，组内取“最清晰且睁眼”的。README 没有公开阈值。
- **PhotoSort-AI**：0 到 100 分，权重为清晰度 0.25、曝光 0.15、人脸质量 0.20、构图 0.20、独特性 0.20；扣分项为模糊 0.30、闭眼 0.20、重复 0.40；≥75 保留，50 到 75 复查，<50 淘汰；用 CLIP 512 维向量给近似帧聚类。这些权重是该项目自己的经验值，没有证据表明它们经过验证。
- 共同点：都是“规则加少量模型”，都没有针对旅行照片；都有三态（保留、复查、淘汰）。
- 商业产品（Aftershoot、FilterPixel 的 DeepCull）的差异化卖点是**学习你个人的取舍风格**。

## 3. 我们现在效果差的原因（诊断，未验证）

1. **全图拉普拉斯方差会误判虚化**：浅景深、大光圈的照片主体清晰、背景虚化，全图方差偏低，会被当成模糊；而噪点多、纹理复杂的照片反而得高分。相关讨论见 [模糊检测与景深](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7961744/)。应当只在主体区域（显著性、人脸）测清晰度。
2. **美学模型选得弱**：我用的是 CLIP ViT-B/32 加一个 512→1 的线性头（LAION 版本）。它训练自 AVA、SAC、logos 数据，原本是给图像生成训练集做过滤的，不是给“同一次旅行里排序”设计的，B/32 版本的精度也比 L/14 低。
3. **没有人脸、表情、闭眼**：这是人像和旅行合影里最影响观感的因素，现在完全没处理。
4. **权重是拍脑袋的**：AI 分 0.7 加技术分 0.3，以及 3 到 7 分映射到 0 到 1，都没有用真实数据校准。
5. **没有“故事覆盖度”**：只按分数取前 30%，会选出大量相似场景。
6. **二元结论**：只有保留和淘汰，没有“待复查”，边界照片被硬分类。

## 4. 可用的本地模型与接口

| 选项 | 输出 | 优点 | 风险和待确认 |
|---|---|---|---|
| **Apple Vision 美学评分** `VNCalculateImageAestheticsScoresRequest` | overallScore（-1 到 1）、isUtility（截图、文档、收据） | 系统内置，无需下载模型，考虑了模糊、曝光、色彩、构图、主体，还能识别“实用图片” | 需要 macOS 15+；本机 15.7.7。**默认路径在命令行会失败，强制 CPU 可用，见第 8 节实测** |
| **Apple Vision 人脸质量** `VNDetectFaceCaptureQualityRequest` | 0 到 1 | 综合曝光、姿态、模糊、表情、闭眼、遮挡 | 同上 |
| Rust 调用 Apple Vision | `objc2-vision` crate，含这两个请求 | 不需要 Swift 边车 | 需要确认在 Tauri 里的线程和内存表现 |
| [Aesthetic Predictor V2.5](https://github.com/discus0434/aesthetic-predictor-v2-5) | 1 到 10，5.5 以上算很好 | 基于 SigLIP，比 V2 覆盖更广 | **AGPL-3.0**，若将来要闭源分发会有许可问题；没有现成 ONNX |
| [pyiqa / IQA-PyTorch](https://github.com/chaofengc/IQA-PyTorch) | TOPIQ、MUSIQ、NIMA 等 | 专门的技术质量和美学模型库 | 需要自己转 ONNX；各模型许可不同 |
| [ArtiMuse](https://arxiv.org/pdf/2507.14533) | 细粒度美学打分加专家级解释 | 论文称在多个数据集上超过 Q-Align | 基于大模型，体积和许可未查 |
| Q-ReAlign（pyiqa 2026 年 6 月加入） | 视觉打分器，0.8B / 4B / 9B 三个规格 | 0.8B 的 mini 版可能适合本地 | 未评测 |
| 本地视觉大模型（如 [Qwen2.5-VL / Qwen3-VL](https://contracollective.com/blog/local-vision-llm-apple-silicon-mlx-qwen-vl-moondream-2026)） | 文字判断加理由 | 3B 的 4-bit 约 2.2 GB；在 Mac 上 MLX 比 GGUF 快约 10% 到 40% | 逐张跑太慢，只适合对每组前几名做精排和写理由 |

## 5. 建议的评分架构

```
导入 → 解码/缩略图
     → 第 1 层 技术门槛（主体区域清晰度、曝光、闭眼、isUtility）  → 淘汰 / 继续
     → 第 2 层 分组（CLIP 或 DINOv2 向量，聚类相似帧）
     → 第 3 层 组内排序（Apple 美学 + 人脸质量 + CLIP 头，多个信号合成）
     → 第 4 层 覆盖度（在每个场景聚类里挑代表，避免全是同一类）
     → 第 5 层（可选）本地 VLM 对每组前 K 名精排，并写“为什么”
输出：保留 / 待复查 / 淘汰 三态，每张有可解释的理由
```

**个性化闭环**：现在每张照片已经有 512 维 CLIP 向量。用户按 P（保留）和 X（淘汰）的每次操作，都是训练数据。累计几十到几百次后，在向量上训练一个轻量分类器（如逻辑回归），叠加到评分里，让程序逐渐学会你的口味。这也是 FilterPixel 的 DeepCull 的核心思路。

**评测**：需要一批带人工标注的照片，度量“模型的保留集合和你的保留集合重合了多少”，然后再调权重。没有这个，所有调参都是猜。

## 6. 动效调研

来源：[View Transitions API 共享元素过渡](https://www.smashingmagazine.com/2023/12/view-transitions-api-ui-animations-part1/)、[GSAP Flip 网格布局](https://tympanus.net/codrops/2026/01/20/animating-responsive-grid-layout-transitions-with-gsap-flip/)、[ThumbHash 占位图](https://evanw.github.io/thumbhash/)。

| 效果 | 用在哪 | 实现 |
|---|---|---|
| 共享元素过渡 | 点击缩略图后，照片从原位置平滑放大成大图，关闭时飞回 | View Transitions API 或 FLIP（GSAP Flip 自 3.9 起免费，Framer Motion 的 layout 动画也可以） |
| 网格重排动画 | 切换 全部/精选/淘汰、改排序时，照片平滑移动到新位置而不是跳变 | FLIP，只用 transform |
| 单张分析进度 | 每张照片上一圈进度环或一道扫描线，并显示当前阶段（解码、清晰度、AI 评分） | 后端按阶段推送事件，前端用 CSS 动画 |
| 模糊到清晰的渐进加载 | 缩略图出现前先显示一个色块模糊图，再淡入 | ThumbHash（比 BlurHash 更紧凑，细节更多） |
| 分数入场 | 分数从 0 滚动到目标值，评分条用弹簧曲线展开 | 弹簧物理，只动 transform 和 opacity |
| 保留标记 | 对勾用描边动画画出来；淘汰只用形状标签和轻微缩小，不用灰度（见 DESIGN.md 第 9 节） | SVG stroke-dashoffset |

性能原则：只用 transform 和 opacity 做动画；尊重“减少动态效果”系统设置。View Transitions 在 Safari 18 起支持（未验证 Tauri 的 WebView 版本是否启用）。

## 7. 待确认与下一步

1. 已完成：Apple Vision 实测（第 8 节）、开始确认页和逐张进度、评分重构（第 9 节）。
2. **仍缺**：用户的旅行照片和手选的“好/不好”，用来建立评测集并校准第 9 节的所有常数。
3. 之后：个性化学习（用 P/R/X 操作训练向量分类器）、Windows 端的替代信号（人脸质量、美学）、本地 VLM 对每组前几名精排。

## 8. Apple Vision 实测（2026-09-21）

只在一张图库照片（pexels）及其变体上测试，样本极小，只能说明趋势。

- 默认走神经网络引擎（ANE）的路径在命令行和沙盒里都会失败（`espresso plan` / `ANECF` 错误），**强制用 CPU 计算设备就能跑**，每张约 150 到 600 毫秒。`fg-vision` 助手就是这样做的：先试默认路径，失败就固定用 CPU。
- 美学分：原图 +0.61；轻度模糊 +0.59；重度模糊 +0.49；**欠曝 -2EV / -3.5EV 反而升到 +0.71 / +0.73**（偏爱暗调氛围）；过曝 +2EV / +3.5EV 降到 +0.58 / +0.53；只裁中心 25% 且没有主体，降到 -0.03。
  - 结论：不能单靠 Apple 分数，曝光必须由我们自己的规则把关。
- 人脸质量对模糊很敏感（0.30 → 0.26 → 0.09），但绝对值很小，适合在**同一组连拍里相对比较**，不适合当绝对分。
- 对 Windows：Apple 信号只是可选的一路。没有 helper 时程序只用内置信号，评分逻辑不依赖它。

## 9. 当前评分（未校准，全部在 `src-tauri/src/scoring.rs`）

- 主体清晰度：8×8 分块，取最清晰约 12% 分块的拉普拉斯能量（先轻微去噪），对数映射。原图与模糊变体校准：原图 1.00；σ=0.8 → 0.81；σ=1.5 → 0.34；σ≥2.5 → 0。
- 曝光：只惩罚高光和暗部裁切，不惩罚夜景本身偏暗。
- 美学：本地 CLIP 头与 Apple 分数按 0.4 / 0.6 混合（没有 Apple 时只用 CLIP）。
- 基础分 = 0.45 美学 + 0.35 清晰度 + 0.20 曝光；同组有人脸时，再用组内相对的人脸质量占 20%；疑似虚焦、截图/文档、过曝、过暗有乘法惩罚。
- 分组：dHash 或 CLIP 余弦 ≥ 0.93 且拍摄时间相差不超过 15 分钟（读 EXIF）；“场景”用 0.86 的宽松相似度，选片时同场景每多选一张扣 0.06，保证覆盖整次旅行。
- 三态：保留、待复查、淘汰，每张有多条可读理由。

## 10. 性能实测（2026-09-21，M1 Max，一张 5000×6469 的照片）

| 阶段 | 优化前 | 优化后 | 说明 |
|---|---|---|---|
| 解码原图 | 429 ms | 115 ms | 第一次含文件缓存冷启动 |
| 缩到 1024 | 779 ms | 44 ms | 改用 SIMD 的 `fast_image_resize` |
| 清晰度（分块） | 12 ms | 6 ms | |
| CLIP（CPU，int8） | 83 ms | 29 ms | 开发编译也提到了 opt-level 2/3 |
| CLIP（CoreML 提供者） | 164 ms，加载 3.7 s | 74 ms，加载 1.0 s | **仍慢于 CPU**：int8 量化模型的大部分算子 CoreML 不支持，会回退到 CPU |
| Apple 美学与人脸 | 341 ms（喂原图） | 142 ms（喂 1024 缩略副本） | 本环境下被迫用 CPU |

- 结论：目前**基本全是 CPU**。`auto` 只在存在 fp16 模型时才给 CLIP 启用 CoreML；否则用 CPU 的 int8。fp16 版约 176 MB（`Xenova/clip-vit-base-patch32` 的 `vision_model_fp16.onnx`），下载前需用户同意。
- 之前分析 301 张显示“约剩 258 秒”，主因是 `tauri dev` 的 Rust 代码没有优化编译，其次是缩图算法慢。
- 同时发现并修复：没有读取 EXIF 方向，竖拍照片会横着显示。

## 11. 命名调研（2026-09-21）

- “拾光”已被多款照片类应用使用：[拾光相册](https://apps.apple.com/us/app/%E6%8B%BE%E5%85%89%E7%9B%B8%E5%86%8C-ai-%E7%9B%B8%E5%86%8C%E7%AE%A1%E7%90%86%E4%B8%8E%E7%85%A7%E7%89%87%E8%A7%86%E9%A2%91%E5%90%88%E6%88%90/id6767172105?l=zh-Hans-CN)、胶片拾光、[拾光说](https://apps.apple.com/qa/app/%E6%8B%BE%E5%85%89%E8%AF%B4-ai%E7%9B%B8%E5%86%8C%E7%AE%A1%E5%AE%B6%E4%B8%8E%E7%85%A7%E7%89%87%E6%95%B4%E7%90%86/id6748812430?l=ar)（定位是识别重要瞬间、清理相似照片、推荐最佳照片，和本应用几乎重合）。因此功能模块不应继续叫“拾光”。
- 搜索没有发现名为“隙光”“须臾”的照片类应用，但这个检索覆盖有限，**不能代替应用商店和商标库的正式查询**。
- 候选：隙光（云隙间漏下的一线光，也呼应胶片摄影的“漏光”）、光阴（光与影，也是时间）、须臾、浮光、一瞥、拾瞬。

补充（同日）：
- “流光”已有流光相机、流光日记；**“流影”是一款现成的 AI 选片应用**（评估构图、光线、时机、主体、故事感），与我们定位相近，要避开。
- “星标相册 StarAlbum”是用星级筛选照片的应用：星标本身在选片领域常见，所以我们的标记用 logo 里的四角“光芒”形状，并配上专属的光带、光圈动效，而不是通用的五角星评级。
- 未搜到名为“露光”“星芒”“拾萤”的照片类应用（检索覆盖有限，仍需正式查询）。

## 12. fp16 与 CoreML 实测（2026-09-21，用户已同意下载）
`vision_model_fp16.onnx`（176,080,659 字节，来自 Xenova/clip-vit-base-patch32）已放入 `models/clip_vit_b32_vision_fp16.onnx`。30 张 3200 万像素照片的端到端吞吐：
- 不含 Apple 分析：CPU int8 50 ms/张，CoreML fp16 65 ms/张。
- **含 Apple 分析（真实流程）：CPU int8 209 ms/张，CoreML fp16 120 ms/张，快约 1.7 倍。**
- 单独跑模型：CPU int8 35.5 ms，CoreML fp16 43 ms。
结论：自动模式默认用 fp16 + CoreML。正式打包时需要把 `models/` 和 `bin/fg-vision` 作为资源打进应用（目前只在开发目录里）。

## 13. CLIP ViT-L/14 + improved aesthetic predictor（2026-09-21，用户已授权下载）
- 下载：`Xenova/clip-vit-large-patch14` 的 `vision_model_fp16.onnx`（608,690,866 字节，约 609 MB）和 `christophschuhmann/improved-aesthetic-predictor` 的 `sac+logos+ava1-l14-linearMSE.pth`（3.7 MB）。
- 评分头结构（已对照官方 `simple_inference.py` 确认）：768→1024→128→64→16→1，**所有 ReLU 被注释掉**，因此等价于单个线性打分。我把五层线性变换离线合成为一个 768 维权重和一个偏置（偏置 5.24），放在 `models/aesthetic_head_vit_l_14.json`。输入是 L2 归一化的图像向量。
- 对同一张照片的变体：标准档（B/32）0.78 → 轻度模糊 0.71 → 重度模糊 0.40；高质量档（L/14）0.54 → 0.46 → 0.21。L/14 对模糊的相对反应更强（降 61% 对 49%）。单张耗时约 500-800 ms 对约 45 ms。
- **这只证明它对明显问题更敏感，不能证明选片更准。** 是否更符合你的口味，要用真实照片和你的手动选择来判断。
- 该评分头的原始训练目标是 AVA / SAC / LAION-Logos 的美学评分，对“旅行照片是否好”并非专门训练。

## 14. 美学评分模型调研（2026-10-20）

目标：找比现有三档更好、许可允许随 AGPL 项目再分发、能用 ONNX Runtime 本地运行、评真实照片的美学模型。基线：pyiqa 官方基准里，与现用"高质量档"同款的 CLIP ViT-L/14 + LAION 头在 AVA 测试集上 SRCC 0.665 / PLCC 0.666（https://iqa-pytorch.readthedocs.io/en/latest/benchmark.html ）。现用"标准档"（B/32 头）与"极致档"（v2.5）没有公开的 AVA 数字。

### 两条硬事实

1. **pyiqa / IQA-PyTorch 整体不可用**：代码许可是 PolyForm Noncommercial 1.0.0，权重仓库 `chaofengc/IQA-PyTorch-Weights` 是 CC-BY-NC-SA-4.0。非商业条款与 AGPL 不兼容，所以它收录的 TOPIQ-IAA（AVA SRCC 0.791，小模型里最好）、重训的 NIMA、转换的 MUSIQ 都只能排除。
2. **多模态大模型打分器分数明显更高**（Q-Align 0.82、Q-ReAlign 0.80–0.83），但体积都是 GB 级，只有 Q-ReAlign-Mini-0.8B 有现实的 ONNX 路径。

### 候选

| 模型 | 许可（核实到的） | 大小 | AVA SRCC | ONNX | 结论 |
|---|---|---|---|---|---|
| [Q-ReAlign-Mini-0.8B](https://huggingface.co/q-future/Q-ReAlign-Mini-0.8B)（Qwen3.5-VL 0.8B） | HF 模型卡 apache-2.0；GitHub 仓库无 LICENSE 文件 | bf16 2.2 GB；按 [onnx-community/Qwen3.5-0.8B-ONNX](https://huggingface.co/onnx-community/Qwen3.5-0.8B-ONNX) 的配方量化后约 853 MB | 0.797（README） | 无官方 ONNX，同架构基座有社区导出；decoder 用到 `com.microsoft` contrib 算子，需确认 ort rc.13 支持；只能 CPU | **推荐作为第四档"精排"**，只对每组前几名跑 |
| [VILA-R](https://github.com/google-research/google-research/tree/master/vila)（Google，CoCa） | 代码 Apache-2.0；GCS 权重无单独声明 | 完整 checkpoint 1.55 GB（含文本解码器）；TF Hub 版大小未核实 | 0.774（[IAA-LQ 对比表](https://arxiv.org/pdf/2309.02861)，VILA 补充材料同值） | TF Hub SavedModel → tf2onnx，难度中 | 可试，替换/并列高质量档 |
| [MUSIQ-AVA](https://github.com/google-research/google-research/tree/master/musiq)（Google） | 同上 | 163 MB（JAX .npz） | 0.726 | 多尺度动态 patch，导出最费事；不能借用 pyiqa 的实现 | 可试，收益小 |
| [TANet](https://github.com/woshidandan/TANet) | Apache-2.0 | 仓库里没找到权重 | 0.758（二手引用） | PyTorch 常规结构，易导出 | 先找到权重再说 |
| [rsinema/aesthetic-scorer](https://huggingface.co/rsinema/aesthetic-scorer) | MIT | 350 MB（整体微调的 B/32） | 无指标 | 易导出 | 可试：输出整体/技术/构图/光线/色彩/景深/内容 7 维，正好对应"告诉你为什么" |
| idealo NIMA MobileNet | Apache-2.0 | 十几 MB | 0.609 | 容易 | 排除，弱于现有 |

### 排除（查过）

pyiqa 全家（许可）；CLIP-IQA（AVA 0.338，不是美学）；LIQE、MANIQA（技术质量，不评美学）；Q-Align/OneAlign、ArtiMuse（15.9 GB）、AesExpert、Q-Insight、VisualQuality-R1、UniPercept、ROC4MLLM、Q-ReAlign-Lite/Pro（都 >1 GB 且无 ONNX）；HF 上所有 SigLIP2 "aesthetic" 头（要么训练自动漫/插画，要么无指标无说明，**没有一个面向真实照片且带指标的**）；HPS/ImageReward/PickScore（AI 生成图偏好）。只有论文没权重：QPT V2、IAA-LQ、FGAesthetics、Real-time Burst Photo Selection。

连拍选片数据集：Chang 2016 "Automatic Triage for a Photo Series"（15,545 张/5,953 组）下载站已无法访问，2026 年论文称其"只部分开源"；FGAesthetics（含连拍分组）未发布。**目前没有现成的"同场景多张选最佳"模型或完整数据集。**

### 结论

- 没有能直接替换现有三档的"小、干净、更准"的现成模型。小模型里最准的被非商业许可挡住；更准的都是大模型。
- 最可行的升级是 Q-ReAlign-Mini-0.8B 做精排档（AVA 0.797 对现有 0.665），代价是自己导出并量化 ONNX、验证 ort 算子支持、CPU 上单张秒级（未实测）。
- 自己在现有 SigLIP / CLIP-L 向量上训头仍是最便宜的路，前提是先有自己照片的标注评测集（见第 5 节的个性化闭环）。用户决定暂缓。

### 未核实

Q-ReAlign 的 GitHub 许可文件与 ort 算子支持；VILA/MUSIQ 的 TF Hub 模型大小与 GCS 权重许可；TANet 权重位置；rsinema 模型用的 PARA 数据集许可；Chang 2016 数据集是否仍可申请。pyiqa 基准数字是其作者自测，部分对比值是二手引用。

## 15. 美学模型实测（2026-10-21）

第 14 节是看论文数字；这一节是在作者自己的 799 张照片（三个文件夹：出游、日常、婚礼筹备）上，由作者本人做配对判断得出的结果。评测工具在仓库外，照片与分数不进仓库。

**方法。** 每个候选模型给 799 张各打一分；在"两个模型意见相反"的地方抽配对（同一连拍里两家首选不同的两张，以及不同场景里一家看好一家看低的两张），作者只看照片选"哪张更好"，页面不显示任何模型信息。共判断 68 对（11 对"差不多"不计），统计每个模型"分数高的那张正好是作者选的"的比例。注意这是专挑分歧处的难题集，数字只能相互比较，不代表日常一致率。

**候选。** 应用现有信号：LAION-L14 头、Apple Vision 美学分、二者的合成分、最终分（含清晰度/曝光/人脸）。外部：TOPIQ-IAA（Swin / Res50）、NIMA（Inception-ResNet-v2 / VGG16）、MUSIQ-AVA、aesthetic-predictor v2.5、rsinema 七维（CLIP B/32 微调）、Q-ReAlign-Mini 0.8B 与 Lite 4B（Qwen3.5-VL）。Q-ReAlign-Pro 9B 的 18.8 GB bf16 权重在 32 GB 的 M1 Max 上装不下（交换区占满），未能参评。

**结果（57 对有高下）。** 应用最终分 61%，Apple Vision 60%，合成分 58%，rsinema 56%，Q-ReAlign-Mini 54%，LAION 头 54%；TOPIQ、MUSIQ、NIMA、v2.5 均低于 50%。把 NIMA 加进合成分：连拍 9:9、跨场景 3:9（作者站原合成分）。用作者的判断学带权叠加（逻辑回归 + 留一法）：没有任何组合稳定超过最终分单独使用。大模型在 M1 Max 上 3.5–15 秒/张，也没有换来更高的一致率。

**结论。** 现有打分已经是这批候选里最贴合作者口味的，**不换模型、不叠加**。剩下的提升空间不在换一个通用美学模型，而在用作者自己的留/弃标注去训练应用内"学习我的喜好"那个头（向量已经在缓存里），这需要日常使用中积累更多标注。
