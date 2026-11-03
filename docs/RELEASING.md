# 发布流程

## 总览

| 什么时候 | 跑什么 | 结果 |
|---|---|---|
| 每次推送到 `main`、每个 Pull Request | [`ci.yml`](../.github/workflows/ci.yml) | 只做检查：落地页类型检查与构建；桌面前端构建、`cargo check`、`cargo test`（含 Android 绑定 crate 在宿主上的测试）；Android 目标的 `cargo check` 与 Gradle `assembleRelease`。不发布任何东西 |
| 推送到 `main` 且落地页内容有改动 | [`deploy-web.yml`](../.github/workflows/deploy-web.yml) | 构建落地页并部署到 Cloudflare（要先配两个 Cloudflare Secret，见"部署落地页"） |
| 推送形如 `v0.1.0` 的**版本标签** | [`release.yml`](../.github/workflows/release.yml) | 在 macOS 上构建 `.dmg`、在 Linux 上构建正式签名的安卓 `.apk`，创建一个**草稿** GitHub Release，两个都传上去。也能在 Actions 页手动运行，那样只编安卓包检查签名，不碰 Release |

**发布是由标签触发的，不是由某个“发布分支”触发的。** 日常提交到 `main` 不会打包；只有你主动打一个版本标签才会。这是开源项目里最常见的做法：标签就是版本的唯一标记，Release 页面上的每个版本都能对应到一个确切的提交。

Release 一律先创建成**草稿**，你检查过安装包再手动点“Publish”，避免一个有问题的包直接公开。

## 发布一个版本

1. **改版本号**，下面三处必须一致：
   - `apps/desktop/src-tauri/tauri.conf.json` 的 `version`
   - 根目录 `Cargo.toml` 的 `[workspace.package]` 里的 `version`（核心库、桌面端、Android 绑定 crate 都写的是 `version.workspace = true`，跟着它走）
   - `apps/desktop/package.json` 的 `version`

   然后运行一次 `cargo check`，让 `Cargo.lock` 里的版本号跟着更新。

   **Android** 不用另改：`apps/android/app/build.gradle.kts` 的 `versionName` 从根 `Cargo.toml` 读；`versionCode` 是仓库的提交数
   （`git rev-list --count HEAD`），取不到（不在 git 仓库里、浅克隆）就构建失败，不会悄悄退回 1——没有 git 历史时用
   `-PFG_BUILD_NUMBER=<数字>` 明确给出。发布工作流会核对 `versionName` 仍然取自 `Cargo.toml`。
2. **更新 [CHANGELOG.md](../CHANGELOG.md)**：把“未发布”里的内容挪到新版本下面，写上日期。
   **写图文版更新说明**：`release-notes/<版本>/zh-CN.md`、`en.md` 加几张截图（写法见 [release-notes/README.md](../release-notes/README.md)）。两端应用"关于"里显示的、Release 正文里"更新内容"一节都是它；没写就退回 CHANGELOG 那一节的纯文字。
3. **提交并推送**到 `main`，等 CI 变绿。
4. **打标签并推送**（首个公开版本是 `0.1.1`）：

   ```bash
   git tag v0.1.1
   git push origin v0.1.1
   ```

   工作流会先检查标签和 `tauri.conf.json` 的版本是否一致，不一致会直接报错并停下。
5. 在仓库的 **Actions** 页面看构建。成功后，到 **Releases** 里能看到一个草稿，里面有 Apple Silicon（`aarch64`）的 `.dmg`、更新包 `.app.tar.gz` + `.sig` + `latest.json`，以及安卓的 `FleetingGleam-android.apk`。
6. 下载安装包，**自己装一遍、跑一遍**，确认没问题后点 **Publish release**。一发布，已装的应用下次启动就会提示更新（见"应用内更新"）。

带连字符的标签（例如 `v0.2.0-beta.1`）会被标成“预发布”。

标签打错了：

```bash
git tag -d v0.1.0 && git push --delete origin v0.1.0   # 删除标签，同时到 Releases 里删掉对应的草稿
```

## 应用内更新

两端的应用都会自己发现新版本并在"设置 › 关于"里更新，版本来源只有本仓库的 GitHub Releases（已发布的、非预发布的那个）：

- **Mac**（Tauri 的 updater 插件）：读 `https://github.com/aiwren/FleetingGleam/releases/latest/download/latest.json`，里面写着新版本号、更新说明、`.app.tar.gz` 的地址和 minisign 签名。应用只接受用仓库里那把**公钥**（`tauri.conf.json` 的 `plugins.updater.pubkey`）验得过的更新包，然后下载、原地替换 `.app`、提示重启。
- **Android**：读 GitHub 的公开接口 `releases/latest`，拿 `tag_name` 和已装的 `versionName` 比，新的就把 Release 里固定名字的 `FleetingGleam-android.apk` 下到应用私有目录，交给系统安装器（Android 会再确认一次，第一次要允许"安装未知应用"）。覆盖安装要求同一把签名密钥——所以 CI 必须用正式密钥签（见下面"Android 安装包"）。
- 启动后几秒静默检查一次（设置里可关），离线或 GitHub 不通就当没有，不会弹错。**限流**：GitHub 对匿名请求每小时只给 60 次，所以手动"检查更新"之后 60 秒内按钮置灰（"刚检查过"）、不联网；启动时的自动检查 10 分钟内查过就跳过。上次检查时间存在本机，重开应用也绕不过去。更新说明来自 `release-notes/<版本>/`（`.github/scripts/release-notes.sh` 把中英文拼成一份、图片换成按标签固定的地址；应用按界面语言只显示一半），没有就退回 `CHANGELOG.md` 里这个版本的那一节。

### 一次性配置：更新包的签名密钥（必须）

密钥对由 `pnpm tauri signer generate` 生成，**私钥不进仓库**。本机的那份在：

| 文件 | 内容 |
|---|---|
| `~/.config/fleetinggleam/updater.key` | 私钥（已加密） |
| `~/.config/fleetinggleam/updater.key.password` | 私钥的密码 |
| `~/.config/fleetinggleam/updater.key.pub` | 公钥（和 `tauri.conf.json` 里的一致） |

把它们存进仓库 Secrets。**必须是 aiwren 身份**：本机 `gh` 平时登录的是 lcopilot，它对这个仓库没有写 Secret 的权限，会报 `HTTP 403: You must have repository read permissions`。两种做法任选：

- **网页**：用 aiwren 登录 GitHub，打开仓库 Settings → Secrets and variables → Actions → New repository secret，值用 `pbcopy < ~/.config/fleetinggleam/updater.key` 复制后粘贴（密码同理用 `updater.key.password`）。
- **命令行**：`gh auth login` 加登 aiwren，`gh auth switch --user aiwren` 切过去，跑完下面几行再 `gh auth switch --user lcopilot` 切回。屏幕上不显示任何值：

```bash
R=aiwren/FleetingGleam
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo $R < ~/.config/fleetinggleam/updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo $R < ~/.config/fleetinggleam/updater.key.password
gh secret list --repo $R
```

没配这两个 Secret，发布工作流的 Mac 任务会直接失败（没签名的更新包任何已装的应用都不会接受）。**私钥或密码丢了就再也签不出已装应用认的更新包**，只能换公钥发一个用户手动安装的版本；请在密码管理器里另存一份。

### 发布时 CI 做了什么

1. `tauri-action` 构建 `.dmg`，同时（`createUpdaterArtifacts: true`）打出 `FleetingGleam.app.tar.gz` 和它的 `.sig`，一起传进草稿 Release，并生成 `latest.json`。
2. "整理 latest.json 的更新说明"一步把 `latest.json` 的 `notes` 换成上面那份更新说明，重新上传。
3. Release 正文也带上同一段"更新内容"；Android 端显示的就是正文里 `## 更新内容` 之后的部分。

**草稿期间没有人会收到更新**：`releases/latest` 只指向已发布的版本。你点 Publish 之后，已装的应用下次启动就会看到。

### 本地验证更新流程

调试构建可以把清单指到本机（正式包只认 GitHub）：

- Mac：先编两个调试包——当前版本的 `.app`，和用 `--config '{"version":"0.2.0"}'` 编的"新版本"（它的 `.app.tar.gz` 与 `.sig` 就是更新包）。打更新包要私钥：`export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.config/fleetinggleam/updater.key)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.config/fleetinggleam/updater.key.password)"`（tauri-cli 2.11 只认这个变量，值是私钥文件的内容；`scripts/release.sh` 会自动读）。再用 `FG_UPDATE_ENDPOINT=http://127.0.0.1:8787/latest.json` 启动旧的那个（直接跑 `Contents/MacOS/fleetinggleam`），调试构建允许 http。假的 `.app.tar.gz` 用 `pnpm tauri signer sign -k ~/.config/fleetinggleam/updater.key -p "$(cat ~/.config/fleetinggleam/updater.key.password)" <文件>` 签名，签名串写进 `latest.json`。
- Android：调试包读 `files/update_url`：`adb shell "run-as com.fleetinggleam.android sh -c 'echo http://10.0.2.2:8787/release.json > files/update_url'"`，JSON 的格式和 GitHub 的 `releases/latest` 一样（`tag_name`、`body`、`assets[].name/browser_download_url/size`）。调试清单允许明文 HTTP，正式包不允许。
- 界面本身在浏览器里就能看：`http://localhost:1420/?mock&update`（`&update=fail` 看"没查到"）。

## 在仓库里配置

**Settings → Secrets and variables → Actions**。更新包的签名密钥（上一节）是**必须**的；下面这些是**可选**的：不配置，构建照样成功，只是得到未签名的安装包，工作流会在日志里用黄色警告告诉你。

| 名称 | 类型 | 作用 |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Secret | **必须**。更新包的 minisign 私钥与密码（见"应用内更新"） |
| `CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID` | Secret | 落地页自动部署（见"部署落地页"）；不配就只构建不部署 |
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
  --repo aiwren/FleetingGleam \
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

## Android 安装包

**打版本标签时由发布工作流自动编**：`release.yml` 的 `android` 任务在 GitHub 上交叉编译、用正式密钥签名，`android-upload` 等 Mac 任务建好草稿 Release 后，把它以固定文件名 **`FleetingGleam-android.apk`** 放进去。落地页的"下载安卓版"直接指向 `releases/latest/download/FleetingGleam-android.apk`，名字不对按钮就是 404。

### 一次性配置：把正式签名密钥存进仓库 Secrets

必须用和本机同一把密钥（证书 SHA-256 以 `0d258392c804` 开头），手机上已装的正式版才能直接覆盖升级。在**你自己的终端**里跑下面这段（从本机的密钥库和 `~/.gradle/gradle.properties` 读，屏幕上不显示任何值）：

```bash
P=~/.gradle/gradle.properties; R=aiwren/FleetingGleam
base64 -i "$(grep -E '^FG_KEYSTORE=' $P | cut -d= -f2-)" | tr -d '\n' | gh secret set FG_KEYSTORE_BASE64 --repo $R
for n in FG_KEYSTORE_PASSWORD FG_KEY_ALIAS FG_KEY_PASSWORD; do grep -E "^$n=" $P | head -1 | cut -d= -f2- | tr -d '\r\n' | gh secret set $n --repo $R; done
gh secret list --repo $R
```

配好后在 Actions 页手动运行一次 Release 工作流（或 `gh workflow run release.yml --ref main`）：它只编安卓包、作为工作流产物留 7 天，不碰任何 Release；日志里"核对签名并改名"一步会打出证书指纹，和上面的对得上就说明配置对了。缺任何一项 Secret 时安卓任务直接失败，不会把调试密钥签的包放进 Release。

注意：有仓库写权限的人改了工作流就能用这把密钥签名，不要随便给别人写权限。

### 在本机编

```bash
apps/android/gradlew -p apps/android assembleRelease
```

产物在 `apps/android/app/build/outputs/apk/release/`。正式签名的密钥库放在仓库外，路径与密码写在 `~/.gradle/gradle.properties` 的
`FG_KEYSTORE`、`FG_KEYSTORE_PASSWORD`、`FG_KEY_ALIAS`、`FG_KEY_PASSWORD`；没配就用调试密钥签（能装，但不能覆盖正式签名的包）。
构建前置条件见 [apps/android/README.md](../apps/android/README.md)。要手动放进 Release 时同样用固定文件名：

```bash
cp apps/android/app/build/outputs/apk/release/app-release.apk FleetingGleam-android.apk
gh release upload v<版本号> FleetingGleam-android.apk --clobber
```

## 部署落地页

落地页（`apps/web`）构建时会预渲染成静态 HTML，由 Cloudflare 以“静态资源”的方式托管，没有服务端代码。配置在 [`apps/web/wrangler.jsonc`](../apps/web/wrangler.jsonc)。静态资源的请求在 Cloudflare 上免费且不限量。

**自动部署**：[`deploy-web.yml`](../.github/workflows/deploy-web.yml) 在推送到 `main` 且落地页内容有改动时构建并 `wrangler deploy`（只看 `apps/web` 下的 `src/`、`public/`、`package.json`、`vite.config.ts`、`wrangler.jsonc`；改应用、文档、根目录锁文件或工作流本身都不触发，连推几次只有最后一次上线），也能在 Actions 页手动运行（"Deploy web" → Run workflow）。只需一次性配两个 Secret（用 aiwren 身份，同上面"应用内更新"的做法）：

| 名称 | 值从哪来 |
|---|---|
| `CLOUDFLARE_API_TOKEN` | Cloudflare 控制台 → 右上角头像 → My Profile → API Tokens → Create Token → 选 **Edit Cloudflare Workers** 模板，账户选部署落地页的那个，生成后复制（只显示一次） |
| `CLOUDFLARE_ACCOUNT_ID` | `pnpm dlx wrangler@4.146.0 whoami` 打出的 Account ID，或 Cloudflare 控制台 Workers 页右侧的 Account ID |

没配齐时工作流只构建、用黄字提醒，不算失败。

**手动部署**（不想等 CI，或在别的分支上试）：需要先用 `wrangler login` 登录自己的 Cloudflare 账号：

```bash
pnpm build:web
cd apps/web
pnpm dlx wrangler@4.146.0 deploy     # 首次部署前先 pnpm dlx wrangler@4.146.0 login
```

默认地址是 `https://fleetinggleam.<你的子域>.workers.dev`。要用自己的域名，在 Cloudflare 控制台的这个 Worker 里添加自定义域。`workers.dev` 这个域名在部分网络环境（包括中国大陆）下可能访问不稳定，面向国内用户建议绑定自己的域名。

## 当前的限制

- **桌面端只有 macOS**，需要 macOS 15 及以上（原生部分的编译目标是 macOS 15，安装包里的最低系统版本也写的是 15.0）。
- **只有 Apple Silicon。** 推理用的 ONNX Runtime（`ort` 2.0.0-rc.13）只提供 Apple Silicon 的 macOS 预编译库，没有 Intel 的。要出 Intel 版，需要在 CI 里从源码编译 ONNX Runtime，或者换用提供 x86_64 库的版本，再在 `release.yml` 的 `matrix` 里加回 `macos-15-intel`。
- 工作流里用到的第三方 Action 都是按主版本号引用的（例如 `actions/checkout@v4`），需要升级时请手动改。如果你想要更强的供应链安全，可以把它们固定到提交 SHA。
