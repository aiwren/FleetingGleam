# 参与贡献

谢谢你愿意花时间在这个项目上。Issue、讨论、文档修正和代码都欢迎，中文或英文都可以。

## 先聊再写

- **小改动**（错别字、明显的 bug、文案）：直接提 Pull Request。
- **较大的改动**（新功能、改评分逻辑、改界面结构）：请先开一个 Issue 说说你的想法，避免你写了很久却和项目方向不一致。
- 不确定的问题可以直接在 Issue 里问。

## 开发环境

| 需要 | 说明 |
|---|---|
| macOS 15+ | 桌面应用的原生部分用 Swift 写，目前只能在 macOS 上完整构建 |
| Xcode 命令行工具 | 提供 `swiftc`，`xcode-select --install` |
| Rust stable | [rustup.rs](https://rustup.rs) |
| Node.js 22 | |
| pnpm 11 | 版本由根目录 `package.json` 的 `packageManager` 指定，用 `corepack enable` 即可 |

```bash
git clone https://github.com/lcopilot/FleetingGleam.git
cd FleetingGleam
pnpm install
```

### 常用命令

```bash
pnpm dev:web          # 落地页 http://localhost:3150
pnpm dev:desktop      # 桌面应用（tauri dev）
pnpm desktop dev      # 只启动桌面前端，配合 http://localhost:1420/?mock 做浏览器预览
sh apps/desktop/scripts/build-vision.sh   # 构建 Apple Vision 辅助程序（可选）
```

### 没有模型怎么开发？

模型体积很大，**不放在仓库里**，应用平时是第一次用到时才下载。开发时有两种办法：

1. **只改界面**：用浏览器预览模式（`pnpm desktop dev`，打开 `/?mock`）。它用绘制出来的示例照片和模拟的分析过程，不需要 Rust，也不需要任何模型。
2. **要跑真实分析**：把模型文件放进 `apps/desktop/models/`（这个目录已被 `.gitignore` 忽略）。只有 debug 构建会读取这个目录。需要哪些文件、文件名是什么，见 `apps/desktop/src-tauri/src/modelpacks.rs` 里的 `PACKS`；每个文件从哪里下载见 [docs/MODELS.md](docs/MODELS.md)，许可见 [docs/THIRD_PARTY.md](docs/THIRD_PARTY.md)。

> 目前还没有一键下载模型的脚本。如果你愿意写一个，那会是一份很受欢迎的贡献。

## 提交之前请先运行

这些就是 CI 会跑的检查，在本地先过一遍可以少来回一轮：

```bash
# 落地页
pnpm --filter @fleetinggleam/web exec tsc --noEmit
pnpm build:web

# 桌面端：前端必须先构建，Rust 编译时会读取它
pnpm --filter @fleetinggleam/desktop build
cd apps/desktop/src-tauri
cargo check --locked
cargo test --locked --lib      # 需要模型的测试默认被忽略，不影响结果
```

## 约定

- **设计**：改界面之前请读 [DESIGN.md](DESIGN.md)。几条最容易踩的：金色只用来表示“保留”；被淘汰的照片只是变小变浅，**不变成黑白**；动效克制，不为装饰而循环（见 DESIGN.md 第 10 节）。
- **文案**：用户能看到的文字不要写死在组件里，放进 `apps/desktop/src/i18n/`（中文 `zh-CN.ts` 和英文 `en.ts` 两份都要补）。落地页的文案在 `apps/web/src/content.ts`。
- **跨平台**：Windows 版本在计划中，所以核心流程不要依赖只有 macOS 才有的接口；macOS 专属的能力（比如 Apple Vision）做成可选的辅助程序，没有它应用也要能正常工作。
- **改评分**：评分常数目前是手工调的、没有校准。如果你改了评分或判定逻辑，请在 Pull Request 里说明依据，最好附上改动前后的对比。
- **不要提交**：模型文件、真实的个人照片、密钥和证书、编译产物（`target/`、`dist/`、`src-tauri/bin/`）。`.gitignore` 已经挡住了大部分，提交前请再看一眼 `git status`。
- **提交信息**：中文或英文都可以，说清楚“改了什么、为什么”。

## Pull Request

1. Fork 本仓库，从 `main` 拉一个分支。
2. 改动尽量聚焦：一个 Pull Request 做一件事。
3. 按上面的清单跑过检查，在 Pull Request 里写清楚改了什么、怎么验证的。界面改动请附截图。
4. 维护者会尽快回复；可能会请你调整，这很正常。

## 许可

向本项目提交代码，即表示你同意你的贡献以与项目相同的许可证发布：**AGPL-3.0-or-later**（见 [LICENSE](LICENSE)）。

## 行为准则

参与即表示你同意遵守 [行为准则](CODE_OF_CONDUCT.md)。
