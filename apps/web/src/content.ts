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
    nav: [["Privacy", "#privacy"], ["Features", "#features"], ["Compare", "#zoom"], ["How it works", "#flow"]],
    cta: { soon: "Coming soon", ready: "Download" },
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
      eyebrow: "Privacy",
      title: "Nothing is uploaded.",
      text: "Your photos stay on your own devices. Analysis, search and learning your taste all run on your Mac or phone. Models download once, only when needed.",
      points: [["shield", "No account, no uploads"], ["aperture", "Models on demand"], ["offline", "Works offline after setup"]],
    },
    features: {
      label: "Features",
      progress: ["Bursts grouped", "Scored by scene", "Find by describing", "Your taste"],
      alts: { field: "Sunset over a field", boat: "A small boat on calm water", village: "White houses on a hill", forest: "A stream in the forest", st: "Street sample", wd: "Wedding sample" },
      zoom: "View larger: ",
      a: { t: "Bursts, grouped for you.", d: "Frames of one moment sit together and the best one is marked. Faces, orientation and time gap all count.",
        aria: "Grouping demo: 12 sample photos fall into 3 groups, the best of each marked", group: "Group {n}", frames: "4 frames", best: "Best" },
      b: { t: "Scored by kind of scene.", d: "Fourteen kinds of scene, each weighed differently. One standard is not laid over every photo.",
        kinds: ["Portrait", "People", "Landscape", "Architecture", "Food", "Animal", "Object", "Document", "Street", "Night", "Sport", "Macro", "Stage", "Vehicle"],
        rules: { 0: "Faces first: is the face sharp?", 9: "Never marked down for being dark.", 10: "Focus first: was the subject caught?" } as Record<number, string>,
        other: "Weighed in its own way.",
        sr: "Fourteen kinds of scene: portrait, people, landscape, architecture, food, animal, object, document, street, night, sport, macro, stage and vehicle. Portraits by faces, sport by focus, and a night shot is not marked down for being dark." },
      c: { t: "Find by describing.", d: "Type what you remember. English and Chinese both work, and photos are found by what is in them. Try it here.",
        label: "Describe the picture", placeholder: "e.g. a small boat on calm water", clear: "Clear search", suggest: "Sample descriptions",
        all: "12 sample photos", found: "Found", foundUnit: "", none: "Nothing found", sample: "Sample photos",
        empty: ["No photo matches that description.", "Try other words, like “a stream in the forest”, or clear it to see them all."],
        phrases: ["sunset over a field", "a small boat on calm water", "white houses on a hill", "a stream in the forest"] },
      d: { t: "Learns your taste.", d: "Point it at a folder of past picks and it learns in one go. Keep one style for weddings, another for street.",
        pick: "Choose a style", kept: "Kept", note: ["The “", "” style keeps ", " of 4"],
        styles: [["Travel", ["field", "village", "boat", "forest"], [0, 1, 2]], ["Street", ["st0", "st1", "st2", "st3"], [1, 3]], ["Wedding", ["wd0", "wd1", "wd2", "wd3"], [0, 2, 3]]] },
    },
    zoom: {
      eyebrow: "Compare and decide", title: "Zoom in, then decide.", lead: "Compare frames of one moment up close, and see where each score comes from.",
      aria: "The score of the selected photo, sample data", selected: "Selected photo", sample: "Sample data",
      metrics: [["Overall", 76], ["Subject sharpness", 92], ["Exposure", 84]],
      keys: [["P", "Keep"], ["R", "Review"], ["X", "Reject"], ["⌘Z", "Undo"]],
    },
    honest: { title: ["A score is a hint,", "not a verdict."], body: "It can tell you which frame is technically stronger. A good photograph is sometimes imperfect, like a sleeping child with closed eyes, so the decision stays yours." },
    flow: {
      eyebrow: "How it works",
      title: "Open a folder. It does the rest.",
      // {keys} is where the three keys are drawn
      steps: [
        ["Pick a folder", "On your Mac or phone, or a share on your NAS. Thumbnails appear first and scores fill in as the analysis runs in the background."],
        ["Review each group", "Zoom to compare frames of one moment, or on a Mac decide from the keyboard with {keys}."],
        ["Organise what you kept", "Copy the keepers into a new folder or your photo album. On a Mac the rest can go to the Trash, where they can be recovered."],
      ],
      aria: "The steps in motion: 8 sample photos, 4 kept, 3 rejected, 1 to review",
      folder: "Sample folder", ready: "Ready", analysing: "Analysing", photo: "Photo", of: "of", unit: "", keptN: "Kept", trashN: "Trash",
      trash: "Trash", kept: "Kept", reject: "Rejected", review: "Review", photoAlt: "Sample photo",
    },
    final: {
      title: "Be first to try it.", body: "The Mac and Android versions are being polished and the downloads will appear here.", label: "Mac and Android versions coming soon",
      readyTitle: "Free, open source, yours to keep.", readyBody: "Download it for your Mac or Android phone, or read the source on GitHub. Your photos never leave your devices.",
      download: "Download for macOS", downloadAndroid: "Download for Android", source: "Source on GitHub",
      reqs: [
        "Mac: needs Apple Silicon (M-series) and macOS 15 or later.",
        "Android: needs Android 12 or later on a 64-bit ARM phone.",
        "No account. All analysis runs on your device, and it works offline once set up.",
      ],
      readyReqs: [
        "Mac: needs Apple Silicon (M-series) and macOS 15 or later. The build is not notarised yet: before the first launch, run `xattr -cr /Applications/FleetingGleam.app` once in Terminal.",
        "Android: needs Android 12 or later on a 64-bit ARM phone. It is not in an app store: after the download, allow your browser to install apps when Android asks.",
        "No account. All analysis runs on your device, and it works offline once set up.",
      ],
    },
    lightbox: { label: "Photo, enlarged", close: "Close", fallback: "Sample photo" },
    footer: { rights: "© 2026 FleetingGleam contributors. Free software under the AGPL-3.0-or-later licence.", note: "Sample photos on this page are from Lorem Picsum, under the Unsplash license." },
  },
  zh: {
    home: "/zh", other: { label: "EN", to: "/" },
    theme: "切换明暗主题",
    name: "惊鸿", latin: "FleetingGleam",
    title: "惊鸿 FleetingGleam | Mac 与安卓上的本地选片",
    description: "惊鸿在你自己的 Mac 或安卓手机上把连拍分好组，挑出最好的一张。照片不上传。",
    nav: [["隐私", "#privacy"], ["功能", "#features"], ["对比", "#zoom"], ["怎么用", "#flow"]],
    cta: { soon: "即将发布", ready: "下载" },
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
      eyebrow: "隐私",
      title: "照片不上传。",
      text: "你的照片只在你自己的设备上。分析、搜索和学习你的喜好，全部在你的 Mac 或手机上完成。模型只在用到时下载一次。",
      points: [["shield", "无需账号，不上传"], ["aperture", "模型按需下载"], ["offline", "设置好后可离线使用"]],
    },
    features: {
      label: "功能",
      progress: ["连拍分组", "按场景打分", "一句话找照片", "学你的口味"],
      alts: { field: "田野上的日落", boat: "平静水面上的小船", village: "山坡上的白色房屋", forest: "森林里的溪流", st: "街拍示例照片", wd: "婚礼示例照片" },
      zoom: "放大查看：",
      a: { t: "连拍自动分组。", d: "同一瞬间的连拍排在一起，最好的一张有标记。人脸、方向和拍摄间隔都会参与判断。",
        aria: "分组演示：12 张示例照片归成 3 组，每组最好的一张带保留标记", group: "第 {n} 组", frames: "4 张", best: "最佳" },
      b: { t: "按场景打分。", d: "十四种场景，各有各的评分侧重。同一套标准不会套在所有照片上。",
        kinds: ["人像", "人物场景", "风景", "建筑", "美食", "动物", "物品", "文档", "街拍", "夜景", "运动", "微距", "舞台演出", "交通工具"],
        rules: { 0: "看脸，人脸是否清晰最重要。", 9: "不会因为暗被扣分。", 10: "看对焦，主体抓住了才算数。" } as Record<number, string>,
        other: "有自己的评分侧重。",
        sr: "十四种场景：人像、人物场景、风景、建筑、美食、动物、物品、文档、街拍、夜景、运动、微距、舞台演出、交通工具。人像看脸，运动看对焦，夜景不会因为暗被扣分。" },
      c: { t: "用一句话找照片。", d: "输入你记得的样子，中英文都可以，按照片里的内容来找。可以直接在这里试。",
        label: "描述画面", placeholder: "例如：平静水面上的小船", clear: "清空搜索", suggest: "示例描述",
        all: "共 12 张示例照片", found: "找到", foundUnit: " 张", none: "没有找到", sample: "示例照片",
        empty: ["没有找到符合描述的照片。", "换个说法试试，比如「森林里的溪流」，或清空后查看全部。"],
        phrases: ["田野上的日落", "平静水面上的小船", "山坡上的白色房屋", "森林里的溪流"] },
      d: { t: "学你的口味。", d: "指一个你过去挑好的文件夹，一次学会。婚礼一套风格，街拍另一套。",
        pick: "选择风格", kept: "保留", note: ["「", "」这套风格留下 ", " / 4 张"],
        styles: [["旅行", ["field", "village", "boat", "forest"], [0, 1, 2]], ["街拍", ["st0", "st1", "st2", "st3"], [1, 3]], ["婚礼", ["wd0", "wd1", "wd2", "wd3"], [0, 2, 3]]] },
    },
    zoom: {
      eyebrow: "对比与决定", title: "放大，再决定。", lead: "放大对比同一瞬间的照片，分数从哪来一目了然。",
      aria: "所选照片的评分，示例数据", selected: "所选照片", sample: "示例数据",
      metrics: [["总分", 76], ["主体清晰度", 92], ["曝光", 84]],
      keys: [["P", "保留"], ["R", "待复查"], ["X", "淘汰"], ["⌘Z", "撤销"]],
    },
    honest: { title: ["评分是参考，", "不是裁判。"], body: "它能告诉你哪一张技术上更好。好照片有时并不完美，比如闭着眼睛睡着的孩子，所以决定始终在你手里。" },
    flow: {
      eyebrow: "怎么用",
      title: "打开文件夹，剩下的交给它。",
      steps: [
        ["选一个文件夹", "Mac 或手机上的文件夹，或者 NAS 上的共享都行。缩略图先出现，评分在后台分析时陆续补上。"],
        ["看每一组的推荐", "放大对比同一瞬间的照片，在 Mac 上也可以用键盘 {keys} 决定。"],
        ["整理你保留的照片", "把留下的复制到新文件夹或手机相册。在 Mac 上，其余的可以移进废纸篓，随时找回。"],
      ],
      aria: "使用过程演示：8 张示例照片，保留 4 张，淘汰 3 张，1 张待复查",
      folder: "示例文件夹", ready: "准备", analysing: "分析中", photo: "第", of: "/", unit: " 张", keptN: "已保留", trashN: "废纸篓",
      trash: "废纸篓", kept: "保留", reject: "淘汰", review: "待复查", photoAlt: "示例照片",
    },
    final: {
      title: "第一时间试用。", body: "Mac 版和安卓版正在打磨，发布后下载链接会放在这里。", label: "Mac 与安卓版即将发布",
      readyTitle: "免费、开源、随你用。", readyBody: "下载 Mac 版或安卓版，或者去 GitHub 看源码。照片始终不离开你自己的设备。",
      download: "下载 macOS 版", downloadAndroid: "下载安卓版", source: "GitHub 源码",
      reqs: [
        "Mac：需要 Apple Silicon（M 系列芯片）和 macOS 15 及以上。",
        "安卓：需要 Android 12 及以上的 64 位 ARM 手机。",
        "无需账号，分析全部在你的设备上完成，设置好后可离线使用。",
      ],
      readyReqs: [
        "Mac：需要 Apple Silicon（M 系列芯片）和 macOS 15 及以上。这个版本还没有经过苹果公证：第一次打开前，在「终端」里执行一次 `xattr -cr /Applications/FleetingGleam.app`。",
        "安卓：需要 Android 12 及以上的 64 位 ARM 手机。它不在应用商店里，下载后按系统提示允许浏览器安装应用即可。",
        "无需账号，分析全部在你的设备上完成，设置好后可离线使用。",
      ],
    },
    lightbox: { label: "放大查看照片", close: "关闭", fallback: "示例照片" },
    footer: { rights: "© 2026 惊鸿 FleetingGleam 贡献者。以 AGPL-3.0-or-later 许可证开源。", note: "页面中的示例照片来自 Lorem Picsum，遵循 Unsplash 授权。" },
  },
} as const;

/** What a sample photograph shows, by its key (the alt text and the lightbox caption). */
export function altOf(lang: Lang, k: string): string {
  const a = copy[lang].features.alts;
  if (k.startsWith("st")) return `${a.st} ${+k[2] + 1}`;
  if (k.startsWith("wd")) return `${a.wd} ${+k[2] + 1}`;
  return a[k as "field" | "boat" | "village" | "forest"];
}
