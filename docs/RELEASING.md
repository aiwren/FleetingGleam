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
5. 在仓库的 **Actions** 页面看构建。成功后，到 **Releases** 里能看到一个草稿，里面有 Apple Silicon（`aarch64`）和 Intel（`x64`）两个 `.dmg`。
6. 下载安装包，**自己装一遍、跑一遍**，确认没问题后点 **Publish release**。

带连字符的标签（例如 `v0.2.0-beta.1`）会被标成“预发布”。

标签打错了：

```bash
git tag -d v0.1.0 && git push --delete origin v0.1.0   # 删除标签，同时到 Releases 里删掉对应的草稿
```

## 在仓库里配置

**Settings → Secrets and variables → Actions**。下面全部是**可选**的：不配置，构建照样成功，只是得到的包有缺陷，工作流会在日志里用黄色警告告诉你缺了什么。

| 名称 | 类型 | 作用 |
|---|---|---|
| `FG_MODEL_BASE` | Variable | 模型文件托管的 `https://` 地址。**不配置的话，应用不知道去哪里下载模型，装好后模型下载会失败** |
| `FG_SEAL_KEY` | Secret | 模型封装用的密钥，必须和你封装托管模型时用的是同一个，见下 |
| `APPLE_CERTIFICATE` | Secret | Developer ID Application 证书（`.p12` 文件的 base64） |
| `APPLE_CERTIFICATE_PASSWORD` | Secret | 上述证书的密码 |
| `APPLE_SIGNING_IDENTITY` | Secret | 例如 `Developer ID Application: 你的名字 (TEAMID)` |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Secret | 用于公证；`APPLE_PASSWORD` 是 Apple ID 的**App 专用密码** |

**没有签名会怎样？** 没配 Apple 证书时，安装包是未签名的，用户首次打开会被 Gatekeeper 拦住，需要手动放行（[新手上手指南](GETTING_STARTED.md#1-安装)里写了怎么做）。要让所有人双击就能用，需要 Apple 开发者账号（每年 99 美元）并配上面的 Secret。

## 模型怎么提供给用户

模型文件很大，**不放进仓库，也不放进安装包**。应用装好之后，在用到某项功能时才去 `FG_MODEL_BASE` 下载，下载的是封装后的文件，校验通过才会使用（见 [PROTECTION.md](PROTECTION.md)）。

**CI 只构建应用，不封装也不托管模型**，这一步由你在本地做一次，之后每次发版都复用：

```bash
# 1. 把明文模型放进 apps/desktop/models/（不提交到 git）
# 2. 用同一个密钥封装，输出到 apps/desktop/dist-models/（里面有 manifest.json）
cd apps/desktop/src-tauri
FG_SEAL_KEY=<你的密钥> cargo run --release --example seal_models -- ../models ../dist-models
# 3. 把 dist-models/ 整个文件夹上传到一个能公开下载的地方，把地址配成 FG_MODEL_BASE
```

要点：

- **密钥要保存好，并且每个版本都用同一个。** 密钥是编译进应用的，用密钥 A 封装的模型，只有用密钥 A 构建的应用才解得开。换了密钥就要重新封装、重新上传。生成密钥：`openssl rand -hex 32`。**不要把它提交进仓库。**
- **放哪里**：任何支持直接下载的静态托管都行，比如对象存储（Cloudflare R2、阿里云 OSS、S3）。另一个可以考虑的办法是把 `dist-models/` 里的文件作为一个单独 Release（例如标签 `models-v1`）的附件，那么地址就是 `https://github.com/lcopilot/FleetingGleam/releases/download/models-v1`；**这个做法我们没有实际验证过**，并且 GitHub 对单个附件有 2 GiB 的上限，用之前请先试一遍。
- **确认模型许可允许你再分发。** 你托管的是别人训练的模型，每个都有自己的许可，见 [THIRD_PARTY.md](THIRD_PARTY.md)。其中有的要求署名，有的许可还没有核实。

## 本地构建一个发布包

不想用 CI 时，可以在本地跑与 CI 等价的流程：

```bash
cd apps/desktop
FG_SEAL_KEY=<密钥> FG_MODEL_BASE=<模型地址> sh scripts/release.sh
```

它会构建辅助程序、封装模型、构建应用，并检查安装包里没有混进模型文件。

## 当前的限制

- **只有 macOS**，需要 macOS 15 及以上（原生部分的编译目标是 macOS 15）。
- Intel 版本在 `macos-15-intel` 上构建。GitHub 以后如果调整这个运行器，需要改 `release.yml` 里的 `matrix`。
- 工作流里用到的第三方 Action 都是按主版本号引用的（例如 `actions/checkout@v4`），需要升级时请手动改。如果你想要更强的供应链安全，可以把它们固定到提交 SHA。
