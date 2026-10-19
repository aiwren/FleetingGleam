# 发布流程

## 总览

| 什么时候 | 跑什么 | 结果 |
|---|---|---|
| 每次推送到 `main`、每个 Pull Request | [`ci.yml`](../.github/workflows/ci.yml) | 只做检查：落地页类型检查与构建；桌面前端构建、`cargo check`、`cargo test`。不发布任何东西 |
| 推送形如 `v0.1.0` 的**版本标签** | [`release.yml`](../.github/workflows/release.yml) | 在 macOS 上构建安装包，创建一个**草稿** GitHub Release，把 `.dmg` 传上去 |

**发布是由标签触发的，不是由某个“发布分支”触发的。** 日常提交到 `main` 不会打包；只有你主动打一个版本标签才会。这是开源项目里最常见的做法：标签就是版本的唯一标记，Release 页面上的每个版本都能对应到一个确切的提交。

Release 一律先创建成**草稿**，你检查过安装包再手动点“Publish”，避免一个有问题的包直接公开。

## 发布一个版本

1. **改版本号**，下面三处必须一致：
   - `apps/desktop/src-tauri/tauri.conf.json` 的 `version`
   - `apps/desktop/src-tauri/Cargo.toml` 的 `version`
   - `apps/desktop/package.json` 的 `version`

   然后运行一次 `cargo check`，让 `Cargo.lock` 里的版本号跟着更新。
2. **更新 [CHANGELOG.md](../CHANGELOG.md)**：把“未发布”里的内容挪到新版本下面，写上日期。
3. **提交并推送**到 `main`，等 CI 变绿。
4. **打标签并推送**（首个版本是 `0.1.0`）：

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

   工作流会先检查标签和 `tauri.conf.json` 的版本是否一致，不一致会直接报错并停下。
5. 在仓库的 **Actions** 页面看构建。成功后，到 **Releases** 里能看到一个草稿，里面有 Apple Silicon（`aarch64`）的 `.dmg`。
6. 下载安装包，**自己装一遍、跑一遍**，确认没问题后点 **Publish release**。

带连字符的标签（例如 `v0.2.0-beta.1`）会被标成“预发布”。

标签打错了：

```bash
git tag -d v0.1.0 && git push --delete origin v0.1.0   # 删除标签，同时到 Releases 里删掉对应的草稿
```

## 在仓库里配置

**Settings → Secrets and variables → Actions**。下面全部是**可选**的：不配置，构建照样成功，只是得到未签名的安装包，工作流会在日志里用黄色警告告诉你。

| 名称 | 类型 | 作用 |
|---|---|---|
| `APPLE_CERTIFICATE` | Secret | Developer ID Application 证书（`.p12` 文件的 base64） |
| `APPLE_CERTIFICATE_PASSWORD` | Secret | 上述证书的密码 |
| `APPLE_SIGNING_IDENTITY` | Secret | 例如 `Developer ID Application: 你的名字 (TEAMID)` |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Secret | 用于公证；`APPLE_PASSWORD` 是 Apple ID 的**App 专用密码** |

**没有签名会怎样？** 没配 Apple 证书时，安装包用 ad-hoc 签名（`tauri.release.conf.json` 里的 `signingIdentity: "-"`，免费，但不能公证）。用户下载后 macOS 会提示“已损坏”或“无法验证开发者”，需要在终端执行一次 `xattr -cr /Applications/FleetingGleam.app`（Release 说明和[新手上手指南](GETTING_STARTED.md#1-安装)里都写了）。要让所有人双击就能用，需要 Apple 开发者账号（每年 99 美元）并配上面的 Secret；配好之后，记得把 `release.yml` 里 Release 说明的那条命令删掉。

## 模型怎么提供给用户

模型**不放进仓库，也不放进安装包**，由应用在用到时自己下载。大部分直接从 Hugging Face 下载，地址、大小和 SHA-256 都写在应用内置的清单里，所以**发布应用不需要配置任何模型服务器**。详见 [MODELS.md](MODELS.md)。

只有 3 个文件例外：SigLIP 视觉模型（857 MB）和两个 MediaPipe 眼睛模型（共 6.8 MB）。上游没有现成可用的版本，是我们转换的，清单里把它们的地址指向本仓库的 Release `models-v1`。**这个 Release 需要在仓库公开之后上传一次**，之后每次发版都不用再管（除非更换这几个模型）：

```bash
# 在放着这三个文件的机器上，进入 apps/desktop/models/
gh release create models-v1 \
  siglip_so400m_vision_fp16.onnx face_landmarks.onnx face_blendshapes.onnx \
  --repo lcopilot/FleetingGleam \
  --title "模型文件（models-v1）" \
  --notes "FleetingGleam 的应用在第一次用到“极致分析”和“MediaPipe 眼睛检测”时会从这里下载这几个文件，不需要手动下载。来源和转换方式见 docs/MODELS.md。"
```

要点：

- 上传前先核对：`shasum -a 256 <文件>` 必须和 `models.manifest.json` 里的值一致，否则应用会拒绝这个文件。
- 必须是**已发布**的 Release（不能是草稿），应用才能匿名下载。仓库还是私有时，这些地址打不开。
- 标签 `models-v1` 不符合 `v0.1.0` 这种格式，**不会触发**发布工作流。
- GitHub 对单个附件的上限是 2 GiB，857 MB 的 SigLIP 文件在范围内。
- 在上传之前，“极致分析”和“MediaPipe 眼睛检测”下载会失败，其余功能不受影响。

## 本地构建一个发布包

不想用 CI 时，可以在本地跑与 CI 等价的流程：

```bash
cd apps/desktop
sh scripts/release.sh
```

它会构建辅助程序、构建应用，并检查安装包里没有混进模型文件。

## 部署落地页

落地页（`apps/web`）构建时会预渲染成静态 HTML，由 Cloudflare 以“静态资源”的方式托管，没有服务端代码。配置在 [`apps/web/wrangler.jsonc`](../apps/web/wrangler.jsonc)。静态资源的请求在 Cloudflare 上免费且不限量。

部署（或更新）一次，需要先用 `wrangler login` 登录自己的 Cloudflare 账号：

```bash
pnpm build:web
cd apps/web
pnpm dlx wrangler@4.146.0 deploy     # 首次部署前先 pnpm dlx wrangler@4.146.0 login
```

默认地址是 `https://fleetinggleam.<你的子域>.workers.dev`。要用自己的域名，在 Cloudflare 控制台的这个 Worker 里添加自定义域。`workers.dev` 这个域名在部分网络环境（包括中国大陆）下可能访问不稳定，面向国内用户建议绑定自己的域名。

## 当前的限制

- **只有 macOS**，需要 macOS 15 及以上（原生部分的编译目标是 macOS 15，安装包里的最低系统版本也写的是 15.0）。
- **只有 Apple Silicon。** 推理用的 ONNX Runtime（`ort` 2.0.0-rc.13）只提供 Apple Silicon 的 macOS 预编译库，没有 Intel 的。要出 Intel 版，需要在 CI 里从源码编译 ONNX Runtime，或者换用提供 x86_64 库的版本，再在 `release.yml` 的 `matrix` 里加回 `macos-15-intel`。
- 工作流里用到的第三方 Action 都是按主版本号引用的（例如 `actions/checkout@v4`），需要升级时请手动改。如果你想要更强的供应链安全，可以把它们固定到提交 SHA。
