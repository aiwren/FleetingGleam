# FleetingGleam 惊鸿

本地运行的照片筛选工具。照片不上传，只有模型文件按需下载。

```
apps/
  desktop/   Tauri 2 + React + Rust 桌面应用（macOS 优先）
  web/       落地页，TanStack Start
packages/    以后放前后端共用的类型和 manifest 格式
docs/        设计与保护方案
branding/    Logo 与图标源文件
```

## 开发

```bash
pnpm install
pnpm dev:desktop   # 桌面应用（tauri dev）
pnpm dev:web       # 落地页，http://localhost:3000
pnpm build:web
```

桌面应用的浏览器预览：`pnpm desktop dev`，再打开 `http://localhost:1420/?mock`。

模型（`apps/desktop/models/`）、封装后的模型（`dist-models/`）和密钥不进仓库，见 `.gitignore`。
发布流程见 `apps/desktop/scripts/release.sh` 与 `docs/PROTECTION.md`。
