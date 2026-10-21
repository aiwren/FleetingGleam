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

### A3. NAS 直连（macOS 端已完成 2026-10-21；Android 端见阶段 B）

应用内"添加 NAS"：填 `smb://`（也支持 WebDAV、NFS）与账号，密码存钥匙串；之后应用自己建立连接，新建项目时直接从 NAS 里选文件夹，断线自动重连，用户全程不碰 Finder，桌面上也不出现挂载卷。
- macOS：通过系统的 SMB 客户端（NetFS 接口，内核级 SMB2/3，Finder 用的同一套）由应用调用，挂在应用自己的私有目录下，现有分析管线不变；不在应用里自写 SMB 协议栈（Rust 生态里纯 SMB 客户端不成熟）。
- Android：没有系统级挂载，用成熟的 Java SMB 库（smbj / jcifs-ng）在应用内实现同样的"添加 NAS"，文件流式读到本地缓存再分析；WebDAV 走 HTTP。
- 网络卷没有废纸篓 → 共享内可恢复的回收站文件夹；NAS 不在线时识别为"未连接"并可一键重连；首轮分析要完整读一遍原图，界面提前说明耗时。
- 云盘（夸克等）没有官方开放接口，不直连；用其客户端的同步文件夹即可。

### A4. 为跨端一致做准备（2–3 周，**会改变 Mac 端的分数，开始前单独确认**）

Android 没有 Apple Vision，两端要一样就得两端用同样的东西：
1. **Apple 美学分蒸馏成可移植的小头**：Mac 分析过的每张照片都自带 Apple 分，用 CLIP-L/14 向量回归之（799 张上 5 折交叉验证 Spearman 0.84；样本越多越准）。两端都用这个头；Mac 上保留"用 Apple Vision"作为可选项。
2. **人脸统一用 MediaPipe**（检测 + 五官 + 闭眼），不再依赖 Vision 的人脸框；分组规则两端才一致。
3. **解码统一**（HEIC / RAW 用同一套库），清晰度数值两端一致。
做完用第 15 节的配对判断法再验一次，确认没有变差。

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

原则：**核心逻辑复用 Rust，界面原生 Kotlin + Jetpack Compose。** 结构按 Material 3 Expressive（导航、返回手势、权限流程），表面用玻璃材质（Android 12+ 的实时模糊），颜色 / 字号 / 间距 / 动效节奏与琥珀只给"保留"等规则直接沿用 DESIGN.md。面向高端机：三档模型都提供。

1. **Rust 核心抽成库**（1–2 周）：macOS 依赖（Vision、ImageIO）退到 trait 后面，cargo-ndk + UniFFI 编出 Android 库；A4 完成后核心里已没有平台特有的打分信号。
2. **模型层**（2–3 周）：ONNX Runtime Android（NNAPI / QNN），同一批 ONNX 文件与清单、同样的 SHA-256 校验；MediaPipe 人脸；Florence-2 与 OPUS-MT 作为可选包。
3. **应用骨架**（2 周）：相册权限与 MediaStore 读取、项目模型、设置与"导入 Mac 的设置与喜好"、分析用前台服务 + 发热 / 充电策略。
4. **选片界面**（4–6 周）：照片墙、对比、检查器与理由、手势版 P / R / X、搜索、风格与学习进度。
5. **整理**（2 周）：MediaStore 的移动与回收站（系统确认框）、导出保留照片。
6. **一致性测试**：同一组照片两端跑，决策与分组必须一致，作为 CI 固定用例。

## 不做的事

- 用户系统 / 云同步：用导出文件代替。
- 云盘直连：夸克无官方接口；阿里云盘 / 百度网盘有官方 OAuth 但仍需整库下载，等有真实需求再说。
- 换通用美学模型：第 15 节已证明现有打分最贴合作者口味。
