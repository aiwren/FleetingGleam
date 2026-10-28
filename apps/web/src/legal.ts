import type { Lang } from "./content";

export type Section = { h: string; p?: readonly string[]; ul?: readonly string[] };
export type Doc = { title: string; updated: string; intro: string; sections: readonly Section[] };

/** 联系运营方的邮箱。留空时，联系方式一节改为指向项目的 GitHub Issues（仓库公开后才显示链接）。 */
export const CONTACT_EMAIL = "";

export const legal: Record<Lang, { privacy: Doc; terms: Doc; labels: { updated: string; contact: string; contactMissing: string; contactPending: string; issues: string; back: string; privacy: string; terms: string } }> = {
  en: {
    labels: { updated: "Last updated", contact: "Contact", contactMissing: "Questions about this policy or these terms, and requests about your data, can be raised in the Issues of the project's GitHub repository. Because the app collects nothing about you, there is no personal data held by us to access or delete.", contactPending: "The project's GitHub repository will open with the first public release; until then, this section will be updated with a contact route.", issues: "GitHub Issues", back: "Back to home", privacy: "Privacy Policy", terms: "Terms of Use" },
    privacy: {
      title: "Privacy Policy",
      updated: "October 28, 2026",
      intro: "FleetingGleam is built so that your photos never leave your own devices. This policy explains what the app and this website do and do not do with information.",
      sections: [
        { h: "The short version", ul: ["Your photos are analysed on your own Mac or Android phone and are never uploaded to us.", "There is no account, and the app does not collect analytics or usage data.", "The app goes online only to download model files the first time a feature needs them, and to reach a NAS or a backup bucket that you set up yourself.", "This website uses no cookies and no analytics."] },
        { h: "What the app does with your photos", p: ["When you open a folder, FleetingGleam reads the photos in it and analyses them locally: sharpness, exposure, faces, similarity and a visual score. Original files are not modified. Organising photos moves or copies files only when you ask it to.", "If you add a NAS in Settings, the app connects to it with the address and account you give, copies the photos you open into a cache on your device, and analyses them there. Organising on a NAS moves files into a recycle folder on that NAS, and exporting to a NAS writes copies there."] },
        { h: "What is stored on your device", p: ["To avoid repeating work, the app keeps the following in its own data folder on your Mac or phone:"], ul: ["Thumbnails and larger previews of your photos, and copies of photos opened from a NAS.", "Analysis results such as scores and numeric descriptions of each photo.", "Your keep, review and reject decisions, your projects, and the taste the app has learned from them.", "Settings, and the model files you have downloaded.", "The NAS passwords and backup keys you enter, encrypted with a key that is created on your device and never leaves it.", "Logs and crash reports. They stay on your device. They are shared only if you export a diagnostics file and send it yourself, and passwords, keys and account names are removed from that file."] },
        { h: "What we collect", p: ["We do not collect your photos, file names, analysis results, decisions or any personal information. None of the data above is sent to us or to anyone else."] },
        { h: "Your NAS and your backups", p: ["A NAS you add is reached directly over your network with the SMB protocol. Nothing about it is sent to us.", "Cloud backup is off unless you set it up. If you do, the app uploads your settings and the taste it has learned, without any photos, project names or file paths, to a Cloudflare R2 bucket that you own, using the keys you created in your own Cloudflare account. We have no access to that bucket, and Cloudflare's terms apply to it."] },
        { h: "Model downloads", p: ["Small models come with the app. The larger models that power search, scoring and scene detection are not bundled with the installer. When you first use a feature that needs one, the app asks you and downloads it straight from the public place that publishes it: Hugging Face (huggingface.co) for most models, and the GitHub Releases of this project for a few converted files. We run no model server of our own. Like any download, the request reveals your IP address, the time and the name of the file to that service (Hugging Face or GitHub) and its hosting provider. The app does not send any photo, identifier or usage information with the request."] },
        { h: "System frameworks and permissions", p: ["On macOS the app may use Apple's on-device frameworks, such as Vision, for face and image analysis. On Android phones with a Qualcomm chip, the app may run models on the phone's neural processor through Qualcomm's runtime, which is included in the app. Both run locally.", "On Android the app asks for access to your photos so it can analyse them, for notifications so it can show the progress of a long analysis, and to be exempt from battery optimisation so that analysis is not stopped halfway. It uses no Google Play services."] },
        { h: "This website", p: ["This website is a static site. It sets no cookies and uses no analytics or advertising trackers. Your language and light or dark theme choice are stored in your browser's local storage so they are remembered, and never sent anywhere. Our hosting provider may keep standard server logs, such as IP address and requested page, for security and operation."] },
        { h: "Your choices", p: ["In Settings you can clear the analysis cache, reset what the app has learned, delete downloaded models, clear the local copies of a NAS, and remove a NAS or a backup target together with its saved password or keys. Removing the app's data folder, or uninstalling the app on Android, deletes everything it has stored. You can also disable photo search and other model features at any time."] },
        { h: "Children", p: ["FleetingGleam is not directed at children, and because it collects no personal information it does not knowingly collect any from children."] },
        { h: "Changes to this policy", p: ["If the app or this policy changes in a way that affects your privacy, we will update this page and the date above. Material changes will also be described in the release notes."] },
      ],
    },
    terms: {
      title: "Terms of Use",
      updated: "October 28, 2026",
      intro: "These terms apply to your use of the FleetingGleam application and this website. By installing or using them you agree to these terms.",
      sections: [
        { h: "The software", p: ["FleetingGleam is an application for Mac and Android that helps you group, compare and select photographs on your own devices. It is free software: the source code is published on GitHub under the GNU Affero General Public License, version 3 or later (AGPL-3.0-or-later). You may use, study, modify and share it under that licence, and nothing in these terms takes those rights away."] },
        { h: "What these terms cover", ul: ["The FleetingGleam application for Mac and Android, and this website.", "They do not cover the model files the application downloads. Those come from Hugging Face and GitHub under their own terms and licences; we do not host them. Check each model's licence before you redistribute it.", "You may not use the application or the website in a way that breaks the law or infringes the rights of others."] },
        { h: "Your photos", p: ["Your photos remain yours. We claim no rights in them, and they are never uploaded to us. You are responsible for having the right to process the photos you open. Because organising can move files and send them to the Trash, keep backups of photos you care about."] },
        { h: "Scores are suggestions", p: ["Scores, groupings, search results and recommendations are automatic estimates. They can be wrong. Decisions about which photos to keep or delete are yours, and we are not responsible for photos you choose to discard."] },
        { h: "Models and third-party components", p: ["The application uses models and open-source components that are provided under their own licences. Those licences apply to those parts alongside these terms. Models are downloaded on demand and may be updated or withdrawn."] },
        { h: "Updates and availability", p: ["We may change, update or discontinue features or the website at any time. The application may not be available on every platform or in every region."] },
        { h: "No warranty", p: ["The application and website are provided as they are and as available, without warranties of any kind, whether express or implied, including fitness for a particular purpose, accuracy of results and uninterrupted or error-free operation, to the extent the law allows."] },
        { h: "Limitation of liability", p: ["To the extent the law allows, we are not liable for indirect, incidental or consequential damages, or for loss of data or photographs, arising from your use of the application or website. Where liability cannot be excluded, it is limited to the amount you paid for the application in the twelve months before the claim, or zero if it was free."] },
        { h: "Ending these terms", p: ["You may stop using the application at any time by uninstalling it. Your rights under the AGPL do not depend on these terms and continue as that licence says. We may limit access to the website or the model downloads if they are abused. The disclaimers and limits of liability continue after that."] },
        { h: "Governing law", p: ["These terms are governed by the laws of the place where the operator of FleetingGleam is established, without regard to conflict of law rules, unless the mandatory law of your country of residence says otherwise."] },
        { h: "Changes to these terms", p: ["We may update these terms. The date above shows the latest version, and continuing to use the application after an update means you accept it."] },
      ],
    },
  },
  zh: {
    labels: { updated: "最后更新", contact: "联系方式", contactMissing: "关于本政策或本协议的疑问，以及与你数据有关的请求，可以在项目 GitHub 仓库的 Issues 中提出。由于应用不收集任何关于你的信息，我们手中没有可供查阅或删除的个人数据。", contactPending: "项目的 GitHub 仓库会随首个公开版本开放；在此之前，本节会补上联系途径。", issues: "GitHub Issues", back: "返回首页", privacy: "隐私政策", terms: "用户协议" },
    privacy: {
      title: "隐私政策",
      updated: "2026 年 10 月 28 日",
      intro: "惊鸿（FleetingGleam）的设计原则是：你的照片不会离开你自己的设备。本政策说明应用和本网站会怎样处理、以及不会处理哪些信息。",
      sections: [
        { h: "简短版本", ul: ["照片在你自己的 Mac 或安卓手机上分析，不会上传给我们。", "没有账号，应用不收集统计或使用数据。", "应用只在两种情况下联网：第一次用到某项功能时下载所需的模型文件；连接你自己设置的 NAS 或备份存储桶。", "本网站不使用 Cookie，也不使用统计工具。"] },
        { h: "应用如何处理你的照片", p: ["打开文件夹后，惊鸿会读取其中的照片并在本机分析：清晰度、曝光、人脸、相似度和视觉评分。原始文件不会被修改。只有在你要求整理时，应用才会移动或复制文件。", "如果你在设置里添加了 NAS，应用会用你提供的地址和账号连接它，把你打开的照片复制到本机的缓存里再分析。在 NAS 上整理时，照片会移进那台 NAS 上的回收文件夹；导出到 NAS 时，会把副本写到那里。"] },
        { h: "保存在你设备上的内容", p: ["为避免重复计算，应用会把以下内容保存在自己位于你 Mac 或手机上的数据文件夹里："], ul: ["照片的缩略图和较大的预览图，以及从 NAS 打开的照片的副本。", "分析结果，例如评分和每张照片的数值描述。", "你的保留、待复查、淘汰决定，你的项目，以及应用据此学到的偏好。", "设置，以及你已下载的模型文件。", "你填写的 NAS 密码和备份密钥。它们用一把在你设备上生成、从不离开设备的密钥加密保存。", "日志和崩溃记录。它们只留在你的设备上；只有你自己导出诊断文件并发出去时才会被分享，而且导出的文件里会去掉密码、密钥和账号名。"] },
        { h: "我们收集什么", p: ["我们不收集你的照片、文件名、分析结果、决定或任何个人信息。上述数据不会发送给我们或任何第三方。"] },
        { h: "你的 NAS 和备份", p: ["你添加的 NAS 通过你的网络用 SMB 协议直接连接，和它有关的任何信息都不会发给我们。", "云备份默认关闭，只有你自己设置后才会启用。启用后，应用会把你的设置和学到的喜好上传到你自己的 Cloudflare R2 存储桶，不含任何照片、项目名或文件路径，用的是你在自己的 Cloudflare 账户里创建的密钥。我们无法访问这个存储桶，它适用 Cloudflare 的条款。"] },
        { h: "模型下载", p: ["几个很小的模型随应用一起安装。支持搜索、评分和场景识别的较大模型不随安装包提供。当你第一次使用需要它的功能时，应用会先询问你，再直接从发布它的公开站点下载：大部分模型来自 Hugging Face（huggingface.co），少数转换过的文件来自本项目的 GitHub Releases；我们没有自己的模型服务器。与任何下载一样，这个请求会让该站点（Hugging Face 或 GitHub）及其托管服务商看到你的 IP 地址、请求时间和文件名。应用不会随请求发送任何照片、标识符或使用信息。"] },
        { h: "系统框架与权限", p: ["在 macOS 上，应用可能使用苹果的本机框架（例如 Vision）做人脸和图像分析。在使用高通芯片的安卓手机上，应用可能通过随应用附带的高通运行库，用手机的神经网络处理器运行模型。两者都在本机运行。", "在安卓上，应用会申请读取照片的权限以便分析，申请通知权限用来显示较长分析的进度，并请求不受电池优化限制，以免分析中途被系统停掉。应用不使用 Google Play 服务。"] },
        { h: "本网站", p: ["本网站是静态网站，不设置 Cookie，不使用统计或广告追踪。你选择的语言和明暗主题保存在浏览器的本地存储中，仅用于记住偏好，不会发送到任何地方。托管服务商可能出于安全和运行目的保留标准的服务器日志，例如 IP 地址和访问的页面。"] },
        { h: "你的选择", p: ["在设置中，你可以清除分析缓存、重置应用学到的偏好、删除已下载的模型、清掉某台 NAS 在本机的副本，以及移除某台 NAS 或某个备份目标并一并删掉保存的密码或密钥。删除应用的数据文件夹（安卓上卸载应用）即可清除它保存的全部内容。你也可以随时关闭照片搜索等依赖模型的功能。"] },
        { h: "未成年人", p: ["惊鸿不面向儿童。由于它不收集任何个人信息，也不会有意收集儿童的信息。"] },
        { h: "本政策的变更", p: ["如果应用或本政策的变化会影响你的隐私，我们会更新本页面和上方日期，重大变化也会写入版本说明。"] },
      ],
    },
    terms: {
      title: "用户协议",
      updated: "2026 年 10 月 28 日",
      intro: "本协议适用于你对惊鸿应用和本网站的使用。安装或使用即表示你同意本协议。",
      sections: [
        { h: "软件", p: ["惊鸿是一款用于 Mac 和安卓的应用，帮助你在自己的设备上分组、对比和挑选照片。它是自由软件：源代码在 GitHub 上公开，采用 GNU Affero 通用公共许可证第 3 版或更高版本（AGPL-3.0-or-later）。你可以按该许可证使用、研究、修改和分享它，本协议不会剥夺这些权利。"] },
        { h: "本协议涵盖什么", ul: ["惊鸿的 Mac 版、安卓版应用，以及本网站。", "不包括应用下载的模型文件。它们来自 Hugging Face 与 GitHub，适用各自的条款和许可，我们不托管这些文件；再分发前请先核对各模型的许可。", "你不得以违反法律或侵犯他人权利的方式使用本应用或本网站。"] },
        { h: "你的照片", p: ["照片始终归你所有。我们对其不主张任何权利，也不会收到它们。你应确保有权处理你打开的照片。由于整理功能可能移动文件并将其放入废纸篓，请为重要的照片保留备份。"] },
        { h: "评分只是建议", p: ["评分、分组、搜索结果和推荐都是自动估计，可能出错。保留还是删除哪些照片由你决定，对于你选择丢弃的照片，我们不承担责任。"] },
        { h: "模型与第三方组件", p: ["本应用使用的模型和开源组件适用其各自的许可，这些许可与本协议一并适用于相应部分。模型按需下载，可能被更新或撤回。"] },
        { h: "更新与可用性", p: ["我们可能随时更改、更新或停止提供部分功能或本网站。本应用不一定在所有平台或地区提供。"] },
        { h: "不作保证", p: ["在法律允许的范围内，本应用和网站按现状提供，不附带任何明示或暗示的保证，包括对特定用途的适用性、结果的准确性，以及运行不中断或无错误。"] },
        { h: "责任限制", p: ["在法律允许的范围内，对于因使用本应用或网站而产生的间接、附带或后果性损害，或数据、照片的丢失，我们不承担责任。无法排除责任时，责任以你在索赔前十二个月内为本应用支付的金额为限；如果应用免费，则为零。"] },
        { h: "协议的终止", p: ["你可以随时卸载应用以停止使用。你依据 AGPL 享有的权利不取决于本协议，并按该许可证的规定继续有效。如果网站或模型下载被滥用，我们可以限制访问。免责和责任限制在此之后仍然有效。"] },
        { h: "适用法律", p: ["本协议受惊鸿运营方所在地的法律管辖，不适用其冲突法规则，但你居住国的强制性法律另有规定的除外。"] },
        { h: "协议的变更", p: ["我们可能更新本协议。上方日期标明最新版本，更新后继续使用即表示你接受。"] },
      ],
    },
  },
};
