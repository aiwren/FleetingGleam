# 路线图（2026-10-21 起）

目标顺序：**先把 macOS 端完善并公开，再做 Android 端，且两端选片结果一致。** 本文是执行顺序与范围的约定，细节以各阶段的提交为准；完成一项就在这里打勾。

## 当前进度与交接（2026-10-21 晚）

给接手的人（或新会话里的 AI）看的一段。记忆与审查清单在 `docs/REVIEW-2026-10-21.md`，本文其余部分是计划本身。

**已完成并推送：** A1 全部；A2（导出 / 导入）；A5 的代码部分；A6 的 8/10；学习进度条。

**正在进行：NAS 改为应用内直连的纯 Rust SMB 客户端（替换 NetFS 方案）。**
- 原因：作者要求 Mac 与 Android 共用一份实现，系统挂载只有 Mac 有。
- 已做：`smb2 = "0.34"`（纯 Rust，MIT/Apache，SMB 2/3 签名加密、NTLM/Kerberos、列共享 / 列目录 / stat / 流式读 / 重命名 / 删除）已加入 `apps/desktop/src-tauri/Cargo.toml`；试探程序 `src-tauri/examples/smbspike.rs` 已编译，由作者在自己终端里带密码运行：
  `FG_NAS_HOST=192.168.1.9 FG_NAS_USER=me FG_NAS_PASSWORD='…' cargo run --example smbspike`（作者的 NAS；密码不进仓库、不进对话）。
- 下一步：试探通过后，做 `Source` 接口（本地文件夹 / SMB 共享：列目录、读文件、重命名、建目录、删除），NAS 项目的原图通过客户端流式拉到本地缓存再分析，整理操作经客户端在共享上执行；凭据 Mac 存钥匙串、Android 存 Keystore；然后**删掉** `vision/fg-mount.swift`、`nas.rs` 里的挂载 / 卸载部分与设置页对系统挂载的依赖，只留一条路径。
- 账号与网页端的区别：这台 NAS 钥匙串里只有 5005 端口 http 的密码，SMB 的要单独存（若仍走钥匙串）。

**等作者在 GitHub 上操作：** 发布草稿 Release v0.1.0；上传 `models-v1`（RELEASING.md）；填 `apps/web/src/legal.ts` 的 `CONTACT_EMAIL`；仓库设为公开；之后把 `apps/web/src/content.ts` 的 `release.available` 改为 true 并 `wrangler deploy`（落地页这次的改动尚未部署，作者未确认）。

**待作者拍板：** A4（Apple 美学分蒸馏 + 人脸统一 MediaPipe）会改变 Mac 端分数，开始前要明确同意。Dependabot 的 glib 告警只能在 GitHub 上标"不适用"（Tauri 2.12 仍用 gtk 0.18）。

**未做的小项：** A6 剩两条（设置页按当前项目的风格显示学习状态、CSS 死类清理与 mock 的取消 / stale / 保存失败路径）；两轮审查里的 P2。

**本机环境（不在仓库里）：** 模型对比的工作区 `../FleetingGleam-bakeoff/`（1.7 GB，可删；`work/labels_*.jsonl` 是作者的 68 对判断）；`~/.cache/huggingface` 里 Q-ReAlign Lite / Pro 等权重约 30 GB，可删；codebase-memory-mcp 已注册为本项目的本机 MCP（新会话里 `/mcp` 可见，第一次让 AI "索引这个项目"）。

**接着做时怎么开口：** "继续 FleetingGleam 路线图里『当前进度与交接』的下一步" 即可；所有上下文都在本文、REVIEW-2026-10-21.md 和自动记忆里。

## 阶段 A：macOS 端完善

### A1. 正确性与数据安全（第二轮审查的后端部分，约 1 周；清单见 [REVIEW-2026-10-21.md](REVIEW-2026-10-21.md)）

- [x] 旧扫描仍可能在最后一刻覆盖新项目的决策表：写 `last_photos` 前在锁内再查一次任务号。
- [x] 两个 Swift 边车（fg-vision、fg-image）没有超时：读管道改读线程 + 超时，超时 kill 并重启该进程、该照片按"无信号"处理；含换行的文件名拒发（按行分帧会错位）。
- [x] release 的 CSP 没有 `media-src`，实况照片打包后没画面：补 `media-src 'self' asset: http://asset.localhost`。
- [x] `projects.json` / `styles.json` / `analysis-cache.json` 写入前 fsync；读到有内容但解析失败时先改名备份再新建，前端不再把它当"首次运行"。
- [x] 分批落盘条件 `>=100 张 且 >=30 s` 改为"或"，小项目和慢格式中途退出不再一张没存。
- [x] `analyze()` 与 `measure_eyes()` 对"有脸但没一只眼可量"的记法统一，避免第二次打开时每张这类照片多跑一次 Vision。
- [x] 全尺寸查看器返回原路径前先校验 assetProtocol 白名单，不通过走渲染副本；`<img>` 加 onError 回退。
- [x] 被保留上限截掉的好照片换文案（"好照片，超出保留上限"），不再说"处于边界"。
- [x] `record_label` 去抖、重训在锁外算、`refs` 向量改紧凑编码；`assign_people` 合并循环改并查集、先剔除单脸簇；`search_photos` 不再在锁内克隆全部向量；`imageio.rs` 的开发机路径只进 debug 构建。

### A2. 设置与喜好的导出 / 导入（已完成 2026-10-21）

无账号。导出一个 JSON：应用设置 + 全部风格（名字、是否默认、权重对应的向量模型、每条标注与参考照片**只带向量和保留/淘汰**，不带路径与图片）。导入时同模型直接用权重，不同模型用样本本地重训。项目与每张照片的取舍按路径记，不迁移。这也是 Android 端"把 Mac 上学到的口味带过去"的唯一通道。

### A3. NAS 直连（macOS 端已完成 2026-10-21，改为应用内置客户端；Android 端复用，见阶段 B）

应用内"添加 NAS"：填服务器地址与账号（"测试连接"通过即可保存，不用选共享），密码加密保存在本机（`secrets.rs`，AES-256-GCM，不用系统钥匙串）；之后应用自己建立连接，新建项目时从这台 NAS 的共享开始逐级浏览、任意一层文件夹都可以选，用户全程不碰 Finder，桌面上也不出现挂载卷。
- 实现（`packages/core/src/nas.rs`）：纯 Rust 的 SMB2/3 客户端（`smb2` crate，MIT / Apache-2.0），连接住在一条专用线程里；**不再**走系统挂载（NetFS 的 `fg-mount` 已下掉），所以 Mac 与 Android 共用同一份实现。目前只支持 SMB；WebDAV / NFS 不做。
- 镜像缓存：每台 NAS 在应用数据目录下有一格 `nas/<id>/<共享>/…`，项目的文件夹就是镜像里的本地路径。探测和分析前先同步（只拉新增或变化的照片，按大小 + 修改时间比对；远端已没有的从镜像删掉），后面的分析、查看、导出全部照旧走本地文件。第一次打开显示"正在从 NAS 读取照片 n / N"。
- 淘汰：在共享里移到根目录的 `.FleetingGleam Recycle Bin/<日期>/`，本机副本随之删掉；移除 NAS 删掉本机副本。
- [ ] 镜像缓存的上限与清理（现在"清除缓存"不动镜像；大项目会占满本机磁盘时要能按项目清掉副本）。
- [ ] 已对本地 Samba 验证；还要对真实 NAS（群晖 / 威联通 / Windows 共享）各验一次兼容性。
- 云盘（夸克等）没有官方开放接口，不直连；用其客户端的同步文件夹即可。

### A4. 为跨端一致做准备（2–3 周，**会改变 Mac 端的分数，开始前单独确认**）

Android 没有 Apple Vision，两端要一样就得两端用同样的东西：
1. **Apple 美学分蒸馏成可移植的小头**：Mac 分析过的每张照片都自带 Apple 分，用 CLIP-L/14 向量回归之（799 张上 5 折交叉验证 Spearman 0.84；样本越多越准）。两端都用这个头；Mac 上保留"用 Apple Vision"作为可选项。
2. **人脸统一用 MediaPipe**（检测 + 五官 + 闭眼），不再依赖 Vision 的人脸框；分组规则两端才一致。
3. **解码统一**（HEIC / RAW 用同一套库），清晰度数值两端一致。
做完用第 15 节的配对判断法再验一次，确认没有变差。

### A3b. 云备份（已完成 2026-10-21）

设置 → 备份：把「我的设置与喜好」（和导出文件同一份内容）自动备份到用户自己的云存储，可以有多个备份目标、各有名字（`backup.json` 的 `targets`，目前只有 `r2` 一种；Secret 按目标 id 存在本机加密仓库）。`packages/core/src/r2.rs` 自己做 S3 SigV4 签名（PUT / GET / DELETE / ListObjectsV2），不引入 AWS SDK。设置或喜好变化后 20 秒静默再上传到每个目标（`src/backup.ts`）；桶里 `fleetinggleam/taste.json` 是最新一份，`auto/<日期>.json` 只留最近 N 份（可设）；"立即备份"是手动触发同一次上传；可从任一目标恢复。
- [x] 对着真的 R2 桶验证（用户 2026-10-21 在应用里配置并备份成功）。
- [ ] 以后有账号系统时，桶和令牌由服务端代管，这里的设置页收起来。

### A5. 公开前的收尾（2–3 天，部分需要作者在 GitHub 上操作）

- [ ] 发布 v0.1.0（现在是草稿）；上传 `models-v1`（极致档与 MediaPipe 眼睛包现在必然下载失败）；新手指南在上传前标注"暂不可用"。
- [x] 落地页（2026-10-21）：下载按钮 + 系统要求 + GitHub 入口都已写好，由 `apps/web/src/content.ts` 的 `release.available` 开关控制——**仓库公开并发布 Release 后把它改成 true 再部署**；隐私政策与协议如实说明模型来自 Hugging Face 与 GitHub Releases；OG 分享卡片 / canonical / hreflang / robots.txt / sitemap；404 页；WebGL 上下文丢失后走 DOM 路径；未激活文字对比度；减动效覆盖结尾区块；语言菜单角色。联系邮箱仍空（`legal.ts` 的 `CONTACT_EMAIL`），公开前填。
- [x] CHANGELOG 补上修复；CI 与发布工作流加 `timeout-minutes`、三处版本号一致性校验、措辞统一为"ad-hoc 签名、未公证"、不再生成 updater 产物；文档错字与过期措辞；bug 模板的 Intel 选项标注不支持；落地页依赖固定版本。
- [ ] 公开后：开启私下报告漏洞、Secret scanning、分支保护。

### A6. 前端（第二轮审查的前端部分，约 1 周）

- [x] 对比视图按 `manual` 判定，能真正"确认"系统建议；撤销栈随项目切换清空、`decide` 校验路径属于当前项目。
- [x] 快捷键统一门控：模块 active、无对话框、无查看器时才响应；对比视图不再全局劫持 Tab；瓷砖焦点可见。
- [x] en 的 `bin.confirmTitle/confirmBody` 归位；列表连接用 `Intl.ListFormat`；理由改列表渲染；加 zh / en 键集合一致性测试。
- [x] "清除缓存"后项目重新探测、缩略图缺失有占位；确认文案说明"所有项目需重新分析"。
- [x] `projects.ts` 区分首次 / 解析失败 / 读错误，只有无 `__TAURI_INTERNALS__` 时才退回 localStorage。
- [x] 备注与保留上限滑块防抖提交，persist 合并连续改动。
- [ ] 学习状态按当前项目的风格读取与重置；"重新评估"只在模型生效或更新时出现；工具栏显示本项目的风格。
- [x] 颜色回归 DESIGN：开关 / 进度 / 选中态用墨色，对话框警告不用琥珀，danger 不用红色。
- [ ] 清理 App.css 死类；mock 能走到取消 / stale / 保存失败路径。
- [ ] 其余 P2：停止分析后重新探测；⌘O 时序；无文件夹的空面板；场景画廊窗口化；光台不回退原图。

## 阶段 B：Android 端（原生，约 3–4 个月）

目标：**Mac 端的全部功能在 Android 上都要有**，不做"精简版"。原则：**核心逻辑复用 Rust，界面原生 Kotlin + Jetpack Compose。** 结构按 Material 3 Expressive（导航、返回手势、权限流程），表面用玻璃材质（Android 12+ 的实时模糊），颜色 / 字号 / 间距 / 动效节奏与琥珀只给"保留"等规则直接沿用 DESIGN.md。面向高端机：三档模型都提供。**导航是底部悬浮的液态玻璃标签栏**（作者 2026-10-21 要求：像 iOS 那种悬浮的 tab bar，要苹果的液态玻璃效果），不用贴底的 NavigationBar；页签只有**照片**和**设置**（NAS、模型、备份、外观、语言都在设置里，作者明确不要 NAS 单独占一格）。冒烟骨架里已经按这个结构搭（`MainActivity.kt` 的 `FloatingTabBar`）：现在是 Haze 的实时模糊 + 轻微着色 + 一圈上亮下暗的高光描边 + 阴影；**液态玻璃的折射与高光流动**在第 4 步做：Android 13+ 用 AGSL `RuntimeShader` 对标签栏后面的内容做边缘折射（把背景采样坐标按到边缘的距离做透镜形变）、按压时的光斑随手指移动，Android 12 退化成模糊玻璃，更早的系统不支持（最低 API 31）。

### 功能对齐清单（Mac 端有的，Android 都要）
| 功能 | Android 上怎么做 | 复用 |
|---|---|---|
| 项目：本机文件夹 / 相册 | MediaStore 读相册，也可选任意文件夹（SAF） | 项目模型、决策表与 Mac 同格式 |
| 连拍分组、打分、理由、场景加权 | Rust 核心原样 | `analyze.rs` `decide.rs` `scoring.rs` |
| 人脸、睁眼、人物识别 | MediaPipe（Android 版）；Vision 的部分退到 trait 后面 | `faces.rs` `mediapipe.rs` |
| 三档模型 + 照片搜索 + 场景描述 + 翻译 | ONNX Runtime Android（NNAPI / QNN），同一批模型与清单 | `scorer.rs` `textsearch.rs` `florence.rs` `translate.rs` `modelpacks.rs` |
| 学习喜好、多风格、学习进度 | Rust 核心原样 | `prefs.rs` `taste.rs` |
| 选片界面：照片墙、单张、对比、检查器、筛选、搜索 | Compose 重写，手势版 P / R / X，撤销 | 文案与规则沿用 i18n 与 DESIGN.md |
| 查看器：HDR、实况照片、放大细节 | Android 的 HDR 显示（Ultra HDR / gainmap）、Motion Photo | `hdrview.rs` 的 macOS 部分换掉 |
| 整理：移到文件夹、回收站、导出 | MediaStore / SAF 移动，系统确认框；NAS 上仍走回收文件夹 | `lib.rs` 的 trash / export 逻辑 |
| NAS 直连（SMB） | 同一份 Rust 客户端，镜像缓存放应用私有目录 | `nas.rs` 原样 |
| 云备份（R2，多目标）、自动备份 | 同一份 Rust 客户端；自动触发逻辑搬到 Kotlin 侧或核心 | `r2.rs` 原样 |
| 密码与密钥的本机加密仓库 | 同一份 Rust 实现，文件放应用私有目录 | `secrets.rs` 原样 |
| 手动备份（导出 / 导入文件） | SAF 选文件 | `taste.rs` 原样 |
| 设置：语言、外观、模型管理、缓存、隐私 | Compose 设置页 | 模型下载与校验沿用 |

### 步骤
1. [x] **Rust 核心抽成库**（完成 2026-10-21）：仓库根目录成了 Cargo 工作区，核心在 `packages/core`（crate `fleetinggleam-core`，lib 名 `fg_core`），桌面端 `apps/desktop/src-tauri` 只剩 Tauri 命令、窗口菜单和 macOS 边车；Android 绑定 crate 在 `apps/android/rust`（UniFFI 0.32，`cargo ndk -t arm64-v8a` 编出 `libfg_android.so`），Android 工程骨架在 `apps/android`（Kotlin + Compose + Material 3，Gradle 任务自动跑 cargo-ndk 并生成 Kotlin 绑定）。冒烟：显示核心版本、对应用内生成的示例照片做一次分析（分组 / 打分 / 取舍）、列出一台 NAS 的共享。
   - 平台能力的边界在 `packages/core/src/platform.rs`：`SystemImages`（系统图像解码器，桌面端 `imageio_macos.rs` 的 ImageIO 边车）、`SystemVision`（桌面端 `vision.rs` 的 Apple Vision 边车）、`GpuClipFactory`（桌面端 `clipgpu.rs` 的 Metal 实现）；宿主启动时注册，不注册也能跑。HDR 叠层（`hdrview.rs`）和访达挂载的网络卷（`mounts.rs`）整个留在桌面端。
   - 核心里**仍然残留的平台特有打分信号**（A4 要解决的，这一步只记录不改）：`Photo.sys_score`（Apple 的美学分，`scoring::combine_aesthetic` 里按 `W_SYS_IN_AESTHETIC = 0.6` 与 CLIP 头混合）；`utility` 标记（Vision 判"实用图片"）；`face_count` / `face_min`（Vision 的人脸数与拍摄质量，进 `final_score` 的组内人脸比较）；`eyes_open` 的 "vision" 量表与 MediaPipe 要读的 `eye_boxes`、人物识别要的 `face_marks` 都来自 Vision 的人脸检测。没有 `SystemVision` 的平台（现在的 Android）这些全是空：没有人脸、没有闭眼、美学只有 CLIP 头——所以两端分数一致要等 A4（美学分蒸馏 + 人脸统一 MediaPipe）。
   - 假设（没问过，先按默认做）：最低 Android 12（API 31，玻璃材质要实时模糊，面向高端机）；ONNX Runtime 用 pyke 的 `aarch64-linux-android+nnapi` 预编译包（core 的 Cargo.toml 在 Android 目标上开 `nnapi` 特性），QNN 第 2 步再看；工具链 Gradle 8.13 / AGP 8.12 / Kotlin 2.1.20 / Compose 1.9.3 / Material 3 1.4.0 / JNA 5.17；只编 arm64-v8a。
   - 留给第 3 步：桌面端 `scan_project` 的流程（缓存命中、NAS 同步、分批落盘、补测眼睛 / 人脸 / HDR、取消）还在 `lib.rs` 里，Android 的 `analyze_folder` 是不走缓存的直通版；第 3 步把这段流程搬进核心（进度与取消做成 trait），两端共用。核心里 `std::env::temp_dir()` 的两处（非原生格式的临时渲染、GPU 编码器的标记文件）在 Android 上不可写，到时给核心一个可配置的临时目录。
   - 本机遗留：旧的 `apps/desktop/src-tauri/target`（约 13 GB）不再使用，可以删；新的构建目录是仓库根的 `target/`。
2. [x] **模型层**（完成 2026-10-21）：ONNX Runtime Android 跑的是和 Mac 端完全同一批 ONNX 文件与同一份清单（`packages/core/models.manifest.json`），同样的 SHA-256 校验；下载、删除、进度、取消经 UniFFI 暴露（`packs_status` / `pack_download` / `pack_cancel` / `pack_remove`），Android 的"模型"页签能下载全部 8 个包（Florence-2 与 OPUS-MT 就是 `scenetext` 可选包）。
   - 会话统一从 `packages/core/src/onnx.rs` 建：平台加速器只在这一处接——macOS 的 CoreML（只有 SigLIP 用，和以前一样）、Android 的 NNAPI（NCHW、不开 fp16 松弛）。只有视觉编码器（CLIP / SigLIP）走加速器，而且第一次要在固定输入上和 CPU 比对（余弦 > 0.999），结论记在草稿目录的标记文件里，不一致就只用 CPU（`onnx::verified`，`scorer.rs` 的 `build_accelerated`）。int8 的文本 / 场景 / 翻译模型和小的人脸模型都在 CPU 上跑。
   - **人脸**：没有系统人脸检测的平台用核心库自己的流水线 `facepipe.rs` 作为 `SystemVision`：UltraFace RFB-640（新包 `detect`，1.6 MB，MIT）找脸 → MediaPipe 网格（`eyes` 包）量眼睛、定出人物识别用的五个点（网格 33/133、362/263、1、61、291）→ `faces` 包认人。给出的 `VisionOut` 和 Apple Vision 一样，分析流水线不用区分。Mac 端照旧用 Vision（A4 未动；Mac 设置页不列 `detect`）。
   - 校准：UltraFace 的框比 Vision 的松，MediaPipe 对框敏感；在作者 80 张 Mac 端分析过的照片上对比（`facepipe::tests::agreement_with_apple_vision_on_a_real_cache`），框收到 0.85 后闭眼比例 17%（Mac 端 16%），人脸数 96% 一致。`VisionOut.aesthetic` 改为 `Option`：这条流水线给不出 Apple 的美学分，`sys_score` 为空，美学只剩 CLIP 头——两端分数一致仍要等 A4。
   - 偏离清单的地方（记下，未问）：检测器用的是 UltraFace 而不是 MediaPipe 的 BlazeFace——后者只有 TFLite，转 ONNX 要 tf2onnx（本机没有），而 UltraFace 有现成的 ONNX；以后换只动 `facepipe.rs` 的前处理和清单。QNN 没接：pyke 的预编译包只带 NNAPI，QNN 要 Qualcomm 的 SDK 与自己编 ONNX Runtime；NNAPI 在 Android 15 起被标为废弃，第 6 步做性能对比时再决定是否自己编带 XNNPACK / QNN 的 ONNX Runtime。
   - 核心里 `std::env::temp_dir()` 已换成 `platform::scratch_dir()`，Android 启动时 `init(cacheDir/scratch, 代理)` 设置。核心所有联网（模型下载、云备份）改走 `platform::http_agent`，可带 HTTP 代理：Android 把系统的全局代理交给它（模拟器用 `-http-proxy http://127.0.0.1:7897` 走作者本机的 Clash（这里写的是宿主机地址，`10.0.2.2` 不行；也不要在模拟器里设全局 `http_proxy`），否则模拟器直连不到 Hugging Face）；Mac 端还没把系统代理交给核心，记为 A 阶段的小项。
   - 还没验证的：在真机上 NNAPI 是否真的比 CPU 快（模拟器只有参考实现）；模拟器上只验证了下载 / 校验 / 加载和 CPU 推理。
3. [x] **应用骨架**（完成 2026-10-21）：相册权限与 MediaStore / SAF 读取、项目模型、设置页（含 NAS、备份、导入 Mac 的设置与喜好）、分析用前台服务 + 发热 / 充电策略。
   - 引擎：桌面端 `lib.rs` 里的全部应用流程（`scan_project` 的缓存命中 / NAS 同步 / 分批落盘 / 补测 / 取消，以及决定、项目存取、喜好、备份、NAS、整理、查看、搜索、模型、缓存）搬进 `packages/core/src/engine.rs`，事件经 `Events` trait 发出，事件名与载荷和桌面端前端监听的一字不差；桌面端只剩薄薄一层 Tauri 命令。Android 的 UniFFI 绑定只有一个入口 `FgEngine.call(方法名, JSON 参数)`，方法名与参数名和 Tauri 命令相同，Kotlin 侧照搬桌面端前端的逻辑（`core/` 下的 Session、Projects、Settings、Models、Strategy、Stores 分别对应 Culling.tsx、projects.ts、settings.ts、models.ts、strategy.ts、nas.ts / backup.ts / styles.ts）。
   - 读照片：相册按 MediaStore 的 DATA 列归到文件夹（项目里存的是真实路径，和 Mac 一样）；系统选文件夹（SAF）时能换成 `/storage/...` 路径就直接读，换不了（网盘、SD 卡没权限）就复制进 `files/saf/<id>` 的镜像，打开项目时同步。权限：`READ_MEDIA_IMAGES` + Android 14 的"部分照片"（可再选更多），被永久拒绝时带用户去系统设置、回来自动刷新；通知权限只在第一次开始分析时问，不给也照样分析。不需要位置权限（核心库不读 GPS）。
   - 前台服务：`AnalysisService`，类型 dataSync（Android 15 起 mediaProcessing）。注意 androidx 的 `ServiceCompat.startForeground` 会把 mediaProcessing 屏蔽成"无类型"导致崩溃，直接调系统的 `startForeground`。发热到 SEVERE 或电量 ≤15% 且没充电时暂停（`Scans.kt`），凉下来 / 接上电源 / ≥20% 自动接着分析。
   - 界面：文案由 `scripts/export-i18n.mjs` 从桌面端的 zh-CN.ts / en.ts 导出（每次构建自动跑），Android 才有的与"电脑 → 手机"的改写放在 `assets/i18n/android.<语言>.json` 叠加；字体 Geist / Geist Mono（OFL，随应用打包），颜色、动效和 DESIGN.md 一致。设置页的行字号比正文小一号：Geist 没有中文字形，回落的系统黑体同字号看起来更大（作者反馈偏大）。
   - 已在模拟器（API 36）上走通：相册 → 新建项目 → 准备页 → 前台服务分析（通知进度）→ 照片墙；SAF 选文件夹；项目移进回收站与恢复；设置各分区；导出 / 导入设置与喜好；深色模式。没验证的：真机上的发热 / 电量暂停（模拟器只能靠 adb 模拟）、NAS（作者的 NAS 密码不在这里输入）、R2 备份。
   - 签名（2026-10-22 作者说密钥可以自己生成）：发布密钥库在仓库外 `~/.config/fleetinggleam/release.jks`（RSA-4096、30 年），路径与密码在 `~/.gradle/gradle.properties` 的 `FG_KEYSTORE` / `FG_KEYSTORE_PASSWORD` / `FG_KEY_ALIAS` / `FG_KEY_PASSWORD`；Gradle 有这几项就用它签 release，没有（CI、别的机器）退回调试密钥。**换机器要把这两个文件带过去**，否则签名不同、覆盖安装会失败。上架渠道仍由作者定。
   - 假设（没问过，先按默认做）：release 打开 R8（`proguard-rules.pro` 保留 JNA / UniFFI / kotlinx.serialization）；分析档位的阈值按手机放低（内存 ≥12 GB 且 ≥8 核才用高质量档）；同一个文件夹再建一次项目会得到第二个同名项目，和 Mac 端一致。
4. [x] **选片界面**（完成 2026-10-21）：照片墙、单张、对比、检查器与理由、筛选面板、手势版 P / R / X 与撤销、搜索、风格与学习进度、HDR 与实况照片查看。
   - 选片（`ui/project/Culling.kt`）：状态条（全部 / 保留 / 待复查 / 淘汰，数字是"现在点它墙上会有几张"）、筛选面板（底部弹出：状态、人物头像、场景卡片，三者叠加）、四种浏览方式（瀑布、缩略网格、单张、对比）与四种排序，选中的照片在改判后仍留在视图里直到移开（和桌面端一样）。
   - 手势：照片墙上点一下选中、再点一下或双击放大，选中的格子上的小坞是 ★ / ✕ / 放大（坞的大小跟着格子走）；单张模式里在照片上**上推保留、下拉淘汰**，定了自动到下一张；底部玻璃工具栏有撤销（最近 50 步）。
   - 大图（`Viewer.kt`）：从格子里飞出来、左右翻页、双指 / 双击放大到 10 倍并可平移、放大到超过已解码的像素时按屏幕上那一块从原图解出清晰块（`photo_detail`）、右下角小地图；下拉关闭（飞回格子），上拉看检查器（判定、理由、数据条、同组照片、详细数据、文件信息）；实况照片停稳后静音放一遍，菜单里可重放 / 关闭自动播放；带 HDR 增益图的照片在 Android 14+ 把窗口切到 HDR 色彩模式。
   - 对比（`Compare.kt`）：几张照片按最大面积排开，任一张上双指 / 双击 / 拖动，所有照片同步缩放平移；每张上有综合分、清晰度、曝光、人脸（同组最高的高亮），保留 / 淘汰；可换 2–4 张、按相似组或同场景、底栏左右滑换组、候选图换进最后一格。
   - 搜索：项目内（工具栏放大镜）与首页的全局搜索（按项目分组，点一张打开那个项目并选中它）；索引在后台建、细环显示进度；场景的那句描述也参与搜索。场景页（`SceneGallery.kt`）：每个场景的最佳照片和同场景几张，没装描述模型时可以"为其余场景生成描述"。
   - 液态玻璃：换成 Kyant 的 backdrop 库（Apache-2.0，`io.github.kyant0:backdrop:1.0.6`）——AGSL 透镜折射 + 色散 + 角度高光 + 内阴影，Android 13+；Android 12 退成模糊玻璃。标签栏、选片工具栏、大图上下两条、提示卡、下载胶囊都用 `ui/components/Glass.kt`。作者 2026-10-21 的反馈：大图上那条深色玻璃"很对味"；浅色玻璃在纯白背景上曾像一块白板，已加淡渐变、描边与内阴影；标签栏改为图标在上文字在下，并缩小到接近 iOS 的尺寸；设置页、状态条字号收小一号。
   - **性能**（2026-10-22）：见 `docs/android-ux.md` 第 6 节。模拟器上液态玻璃是冷启动与滚动的大头，加了开关，模拟器上一律关；格子稳定性、事件合并、引擎后台建、去阴影之后 297 张的墙滚动中位 26 ms。真机上的玻璃开销、Baseline Profile 留待作者上真机后再定。
   - **移动端 UX 重做**（2026-10-21，作者反馈"平平无奇、tab bar 怪、授权界面丑、字体和组件偏大"）：方案见 `docs/android-ux.md`。字号阶梯整体收一档（正文 14、行标题 14.5、说明 12、数字 12，大标题 28 只在根页）；按钮不描边、行内用 30dp 胶囊；分组卡片去描边、行间细线从文字起头处开始。标签栏图标在上文字在下，选中态是一块更亮的玻璃而不是实心墨色。首页：大标题 + 右上 ⋯（手动创建、风格、回收站）与 ＋，封面为主的两列卡片，长按出菜单，回收站是推进页。新建项目的弹层是分组列表；没有相册权限时先显示授权说明页（插图、三条承诺、整行"继续"、"改为选一个文件夹"），点继续才弹系统框。设置改成分级列表（根页七行带当前值，点进去是那一组）。项目页顶栏只有 ‹ 项目 · 名字 · 搜索 · ⋯（项目设置、重新评估、风格、重命名），"整理"在底部玻璃条中间。
   - 模型下载可以转到后台：下载对话框里"后台下载"，对话框收起，顶部一粒玻璃胶囊和通知栏显示进度（前台服务 `DownloadService`），点胶囊叫回对话框；等着它的事（分析、搜索）下载完照样继续。
   - 工具链随 backdrop 升级：Kotlin 2.3.10、Compose 1.10.3、kotlinx-serialization 1.9.0（`jvmTarget` 改用 `compilerOptions`）。
   - 没在模拟器上验证的：人物筛选（样张里没有人脸）、实况与 HDR（样张没有）、真机上的手感与性能。整理（导出、系统回收站）是第 5 步。
5. [x] **整理与 NAS**（完成 2026-10-21）：MediaStore / SAF 的移动与回收站（系统确认框）、导出保留照片；NAS 项目的镜像同步与回收。
   - 项目工具栏上的"整理"（`ui/project/OrganizeUi.kt`，逻辑在 `core/Organize.kt`）：导出保留的照片到相册里的「惊鸿/项目名」（Pictures 下，Android 11 起应用可以按路径写自己的文件，写完让媒体库收录），或导出到系统里选的任意文件夹（能换成路径就由核心库复制，换不出——网盘等——就按 Uri 逐张写，重名加序号）。
   - 把淘汰的照片移走（"只移我手动淘汰的 / 包括系统建议的"，和 Mac 一样）：核心库先处理 NAS 上的（移进共享里的回收文件夹）；它交回来的本机照片里，相册里的用 `MediaStore.createTrashRequest` 一次请求系统回收站（系统弹确认框，30 天内能在相册的回收站恢复，拒绝就当没做）；从网盘等选来的镜像文件夹里的，在原处移进根目录的「.FleetingGleam Recycle Bin/日期」（`DocumentsContract.moveDocument`，不支持就复制再删），本机副本随之删掉。为此镜像文件夹现在要读写两种持久权限。已不在的照片从项目的改判里忘掉。
   - 修了一处：整理完重新载入项目时，照片列表和上次一模一样，StateFlow 不再发出，取舍没有重算（全显示 0）；现在相同时手动重算。
   - 已在模拟器上验证：导出到相册（文件在 Pictures/惊鸿/Burst、媒体库已收录）、系统回收站（确认框、文件移走、项目少一张）。没验证的：镜像文件夹的原处回收和导出到网盘文件夹（模拟器上本机文件夹都换得出路径，走不到这条路），NAS 的回收（核心库与 Mac 共用、Mac 端已验证）。
6. [x] **一致性测试**（完成 2026-10-21）：同一组照片两端跑，决策与分组必须一致，作为 CI 固定用例；设置与喜好文件两端互导。
   - 固定用例在 `packages/core/src/consistency.rs`：由代码画出 11 张照片（三组连拍——清晰的、挪了一点的、手抖的、过暗 / 过亮的——加两张单独的场景），按 Android 那条路分析（不注册任何平台能力、只用 CPU、标准档），记下每张的分组、场景、取舍、组内最佳与综合分，和仓库里的基准比：前四项必须一模一样，分数差不超过 0.02。分组和场景记成"组里名字最小的那张"，不依赖编号。
   - 两份基准：`consistency.golden.json`（没有模型，CI 的 `cargo test --lib` 每次都跑）和 `consistency.models.golden.json`（装了标准档 CLIP ViT-B/32 与美学头；`FG_MODELS=apps/desktop/models cargo test -p fleetinggleam-core consistency -- --ignored`）。Android 绑定 crate 有同名测试比同一份基准；`apps/android/scripts/consistency.sh [模型文件夹]` 把它编成 arm64 测试程序推到手机 / 模拟器上跑。2026-10-21 在 Mac（arm64）与 Android 模拟器（arm64，API 36）上两份都通过。更新基准：`FG_WRITE_GOLDEN=1` 再跑一遍并说明原因。
   - 这个用例比的是**同一条可移植的路**在两种系统、两套编译下是否一致；Mac 端默认还会用 Apple Vision 的美学分与人脸（A4 之前两端的分数本来就不同），那部分不在这个用例里——等 A4 把美学分蒸馏成可移植的头、人脸统一 MediaPipe 之后，Mac 的默认路就是这条路。用例里没有人脸，人脸流水线（Android 的 UltraFace + MediaPipe 与 Mac 的 Vision）的一致性另见第 2 步的 80 张对比。
   - 设置与喜好两端互导：两端都是 `fleetinggleam-taste/1`（Mac 写文件，Android 经系统选择器给字节）。`taste_files_go_both_ways_between_mac_and_android` 测 Mac 导出 → Android 导入、Android 导出 → Mac 导入：设置原样带过去、命名风格带过去。采用哪些设置由界面决定：Android 只取各平台通用的几项（外观、学习、搜索、多风格、质量），眼睛固定 MediaPipe，模型档位与硬件照本机。模拟器上也手动导出、再导入过一次。

## 不做的事

- 用户系统 / 云同步：用导出文件代替。
- 云盘直连：夸克无官方接口；阿里云盘 / 百度网盘有官方 OAuth 但仍需整库下载，等有真实需求再说。
- 换通用美学模型：第 15 节已证明现有打分最贴合作者口味。
