export type Lang = "en" | "zh";

/** Where the site lives (absolute links in the share card, canonical and sitemap); change when a domain is bound. */
export const SITE = "https://fleetinggleam.fleetinggleam-web.workers.dev";

/**
 * The release state the page shows. `available` is flipped to true once the repository is public and a Release is published:
 * until then the download section says "coming soon" and no link points at GitHub, where a visitor would only find a 404.
 */
export const release = {
  available: false,
  repo: "https://github.com/lcopilot/FleetingGleam",
  download: "https://github.com/lcopilot/FleetingGleam/releases/latest",
  /** The APK is uploaded to every Release under this fixed name (docs/RELEASING.md), so the link always gets the latest one. */
  android: "https://github.com/lcopilot/FleetingGleam/releases/latest/download/FleetingGleam-android.apk",
};

/** Everything the page says. Nothing here claims more than the app does today. */
export const copy = {
  en: {
    home: "/", other: { label: "中文", to: "/zh" },
    theme: "Switch light or dark theme",
    name: "FleetingGleam", latin: "",
    title: "FleetingGleam | Local photo culling for Mac and Android",
    description: "FleetingGleam groups your bursts and picks the best frame on your own Mac or Android phone. Nothing is uploaded.",
    nav: [["Features", "#features"], ["Privacy", "#privacy"], ["How it works", "#flow"]],
    menu: "Menu",
    hero: {
      title: ["Thousands of photos.", "A few worth freezing."],
      body: "FleetingGleam is a photo culling app for Mac and Android. It groups your bursts, picks the best frame in each, and shows you why. All on your own device, nothing uploaded.",
      cta: "See how it works",
      alt: "The FleetingGleam photo wall with an inspector showing the score of the selected photo",
      steps: ["Groups your bursts", "Picks the best frame", "Shows you why"],
      // The scene on the first screen: three takes of one moment, one kept. The figures are the ones the app's own sample run shows.
      scene: { kept: "Kept", best: "Best", overall: "Overall", score: 76, reasons: [["Subject sharpness", 92], ["Exposure", 84]], why: "Of the three worth keeping, this one is the sharpest.", sample: "Sample photos" },
    },
    privacy: {
      text: "Your photos stay on your own devices. Analysis, search and learning your taste all run on your Mac or phone. Models download once, only when needed.",
      points: [["ShieldCheck", "No account, no uploads"], ["Aperture", "Models on demand"], ["Sparkle", "Works offline after setup"]],
    },
    features: {
      title: "Sees the shoot the way you do.",
      a: { t: "Bursts, grouped for you", d: "Frames of one moment sit together and the best one is marked. Faces, orientation and time gap all count.", alt: "Frames of one moment grouped together, the best of each marked with a gold star" },
      b: { t: "Scored by kind of scene", d: "Fourteen kinds of scene, each weighed differently. Portraits by faces, sport by focus, and a night shot is not marked down for being dark.", kinds: ["Portrait", "People", "Landscape", "Architecture", "Food", "Animal", "Object", "Document", "Street", "Night", "Sport", "Macro", "Stage", "Vehicle"] },
      c: { t: "Find by describing", d: "Type what you remember. English and Chinese both work, and photos are found by what is in them.", phrases: [["sunset over a field", "field"], ["a small boat on calm water", "boat"], ["white houses on a hill", "village"], ["green forest and stream", "forest"]] },
      d: { t: "Learns your taste", d: "Point it at a folder of past picks and it learns in one go. Keep one style for weddings, another for street.",
        styles: [["Travel", ["field", "village", "boat", "forest"], [0, 1, 2]], ["Street", ["st0", "st1", "st2", "st3"], [1, 3]], ["Wedding", ["wd0", "wd1", "wd2", "wd3"], [0, 2, 3]]] },
    },
    flow: {
      title: "Open a folder. It does the rest.",
      steps: [
        ["Pick a folder", "On your Mac or phone, or a share on your NAS. Thumbnails appear first and scores fill in as the analysis runs in the background."],
        ["Review each group", "Zoom to compare frames of one moment, or decide from the keyboard with P, R and X."],
        ["Organise what you kept", "Copy the keepers into a new folder or your photo album. On a Mac the rest can go to the Trash, where they can be recovered."],
      ],
    },
    honest: { title: "A score is a hint, not a verdict.", body: "It can tell you which frame is technically stronger. A good photograph is sometimes imperfect, like a sleeping child with closed eyes, so the decision stays yours." },
    final: {
      title: "Be first to try it.", body: "The Mac and Android versions are being polished and the downloads will appear here.", label: "Mac and Android versions coming soon",
      readyTitle: "Free, open source, yours to keep.", readyBody: "Download it for your Mac or Android phone, or read the source on GitHub. Your photos never leave your devices.",
      download: "Download for macOS", downloadAndroid: "Download for Android", source: "Source on GitHub",
      req: "Needs a Mac with Apple Silicon (M-series) and macOS 15 or later. The build is not notarised yet: before the first launch, run `xattr -cr /Applications/FleetingGleam.app` once in Terminal. The Android app needs Android 12 or later on a 64-bit ARM phone. It is not in an app store: after the download, allow your browser to install apps when Android asks.",
    },
    footer: { rights: "© 2026 FleetingGleam contributors. Free software under the AGPL-3.0-or-later licence.", note: "Sample photos in screenshots are from Lorem Picsum, under the Unsplash license." },
  },
  zh: {
    home: "/zh", other: { label: "EN", to: "/" },
    theme: "切换明暗主题",
    name: "惊鸿", latin: "FleetingGleam",
    title: "惊鸿 FleetingGleam | Mac 与安卓上的本地选片",
    description: "惊鸿在你自己的 Mac 或安卓手机上把连拍分好组，挑出最好的一张。照片不上传。",
    nav: [["功能", "#features"], ["隐私", "#privacy"], ["怎么用", "#flow"]],
    menu: "菜单",
    hero: {
      title: ["几千张照片，", "只留值得定格的几张。"],
      body: "惊鸿是一款 Mac 和安卓手机上的选片软件：把连拍自动分好组，挑出每组最好的一张，并告诉你为什么。全程在你自己的设备上完成，照片不上传。",
      cta: "了解怎么用",
      alt: "惊鸿的照片墙，右侧检查器显示所选照片的评分",
      steps: ["连拍自动分组", "挑出最好的一张", "告诉你为什么"],
      scene: { kept: "保留", best: "最佳", overall: "总分", score: 76, reasons: [["主体清晰度", 92], ["曝光", 84]], why: "值得留下的三张里，这张最清晰。", sample: "示例照片" },
    },
    privacy: {
      text: "你的照片只在你自己的设备上。分析、搜索和学习你的喜好，全部在你的 Mac 或手机上完成。模型只在用到时下载一次。",
      points: [["ShieldCheck", "无需账号，不上传"], ["Aperture", "模型按需下载"], ["Sparkle", "设置好后可离线使用"]],
    },
    features: {
      title: "按你的眼光看片。",
      a: { t: "连拍自动分组", d: "同一瞬间的连拍排在一起，最好的一张有标记。人脸、方向和拍摄间隔都会参与判断。", alt: "同一瞬间的照片排在一起，每组最好的一张带金色星标" },
      b: { t: "按场景打分", d: "十四种场景，各有各的评分侧重。人像看脸，运动看对焦，夜景不会因为暗被扣分。", kinds: ["人像", "人物场景", "风景", "建筑", "美食", "动物", "物品", "文档", "街拍", "夜景", "运动", "微距", "舞台演出", "交通工具"] },
      c: { t: "用一句话找照片", d: "输入你记得的样子，中英文都可以，按照片里的内容来找。", phrases: [["田野上的日落", "field"], ["平静水面上的小船", "boat"], ["山坡上的白色房屋", "village"], ["森林里的溪流", "forest"]] },
      d: { t: "学你的口味", d: "指一个你过去挑好的文件夹，一次学会。婚礼一套风格，街拍另一套。",
        styles: [["旅行", ["field", "village", "boat", "forest"], [0, 1, 2]], ["街拍", ["st0", "st1", "st2", "st3"], [1, 3]], ["婚礼", ["wd0", "wd1", "wd2", "wd3"], [0, 2, 3]]] },
    },
    flow: {
      title: "打开文件夹，剩下的交给它。",
      steps: [
        ["选一个文件夹", "Mac 或手机上的文件夹，或者 NAS 上的共享都行。缩略图先出现，评分在后台分析时陆续补上。"],
        ["看每一组的推荐", "放大对比同一瞬间的照片，或用键盘 P、R、X 决定。"],
        ["整理你保留的照片", "把留下的复制到新文件夹或手机相册。在 Mac 上，其余的可以移进废纸篓，随时找回。"],
      ],
    },
    honest: { title: "评分是参考，不是裁判。", body: "它能告诉你哪一张技术上更好。好照片有时并不完美，比如闭着眼睛睡着的孩子，所以决定始终在你手里。" },
    final: {
      title: "第一时间试用。", body: "Mac 版和安卓版正在打磨，发布后下载链接会放在这里。", label: "Mac 与安卓版即将发布",
      readyTitle: "免费、开源、随你用。", readyBody: "下载 Mac 版或安卓版，或者去 GitHub 看源码。照片始终不离开你自己的设备。",
      download: "下载 macOS 版", downloadAndroid: "下载安卓版", source: "GitHub 源码",
      req: "需要 Apple Silicon（M 系列芯片）的 Mac 和 macOS 15 及以上。这个版本还没有经过苹果公证：第一次打开前，在「终端」里执行一次 `xattr -cr /Applications/FleetingGleam.app`。安卓版需要 Android 12 及以上的 64 位 ARM 手机；它不在应用商店里，下载后按系统提示允许浏览器安装应用即可。",
    },
    footer: { rights: "© 2026 惊鸿 FleetingGleam 贡献者。以 AGPL-3.0-or-later 许可证开源。", note: "截图中的示例照片来自 Lorem Picsum，遵循 Unsplash 授权。" },
  },
} as const;
