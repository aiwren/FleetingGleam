export type Lang = "en" | "zh";

/** Everything the page says. Nothing here claims more than the app does today. */
export const copy = {
  en: {
    home: "/", other: { label: "中文", to: "/zh" },
    theme: "Switch light or dark theme",
    name: "FleetingGleam", latin: "",
    title: "FleetingGleam | Local photo culling for your Mac",
    description: "FleetingGleam groups your bursts and picks the best frame on your own computer. Nothing is uploaded.",
    nav: [["Features", "#features"], ["Privacy", "#privacy"], ["How it works", "#flow"]],
    menu: "Menu",
    hero: {
      title: ["Thousands of photos.", "A few worth freezing."],
      body: "FleetingGleam groups your bursts and picks the best frame, all on your Mac. Nothing is uploaded.",
      cta: "See how it works",
      alt: "The FleetingGleam photo wall with an inspector showing the score of the selected photo",
      marks: [["Focus", "0.97 on the hull"], ["Composition", "subject on the thirds"], ["Exposure", "balanced, no clipping"], ["Colour", "teal against orange"]],
      kept: "Kept",
    },
    read: { title: "It reads each photo the way you would.", body: "Focus, framing, exposure and colour are measured on the picture itself, and the best frame of a moment takes the mark." },
    wall: { title: "Watch it choose.", body: "Scroll, and a real pass runs: bursts sit together, and the keeper gets the mark.", kept: "kept", note: "Sample photos. Frames 8 to 10 are one moment." },
    privacy: {
      text: "Your photos stay on your computer. Analysis, search and learning your taste all run on this machine. Models download once, only when needed.",
      points: [["ShieldCheck", "No account, no uploads"], ["Aperture", "Models on demand"], ["Sparkle", "Works offline after setup"]],
    },
    features: {
      title: "Sees the shoot the way you do.",
      a: { t: "Bursts, grouped for you", d: "Frames of one moment sit together and the best one is marked. Faces, orientation and time gap all count.", alt: "Two frames of one moment side by side, with scores and a suggested best" },
      b: { t: "Scored by kind of scene", d: "Fourteen kinds of scene, each weighed differently. Portraits by faces, sport by focus, and a night shot is not marked down for being dark.", kinds: ["Portrait", "People", "Landscape", "Architecture", "Food", "Animal", "Object", "Document", "Street", "Night", "Sport", "Macro", "Stage", "Vehicle"] },
      c: { t: "Find by describing", d: "Type what you remember. English and Chinese both work, and photos are found by what is in them.", phrases: [["sunset over a field", "field"], ["a small boat on calm water", "boat"], ["white houses on a hill", "village"], ["green forest and stream", "forest"]] },
      d: { t: "Learns your taste", d: "Point it at a folder of past picks and it learns in one go. Keep one style for weddings, another for street.",
        styles: [["Travel", ["field", "village", "boat", "forest"], [0, 1, 2]], ["Street", ["st0", "st1", "st2", "st3"], [1, 3]], ["Wedding", ["wd0", "wd1", "wd2", "wd3"], [0, 2, 3]]] },
    },
    flow: {
      title: "Open a folder. It does the rest.",
      steps: [
        ["Pick a folder", "Thumbnails appear first and scores fill in as the analysis runs in the background."],
        ["Review each group", "Zoom to compare frames of one moment, or decide from the keyboard with P, R and X."],
        ["Organise what you kept", "Move keepers into a new folder and the rest to the Trash, where they can be recovered."],
      ],
    },
    honest: { title: "A score is a hint, not a verdict.", body: "It can tell you which frame is technically stronger. A good photograph is sometimes imperfect, like a sleeping child with closed eyes, so the decision stays yours." },
    final: { title: "Be first to try it.", body: "The macOS version is being polished and the download will appear here. Windows follows.", label: "macOS version coming soon" },
    footer: { rights: "© 2026 FleetingGleam. All rights reserved.", note: "Sample photos in screenshots are from Lorem Picsum, under the Unsplash license." },
  },
  zh: {
    home: "/zh", other: { label: "EN", to: "/" },
    theme: "切换明暗主题",
    name: "惊鸿", latin: "FleetingGleam",
    title: "惊鸿 FleetingGleam | 本地照片筛选",
    description: "惊鸿在你的电脑上把连拍分好组，挑出最好的一张。照片不上传。",
    nav: [["功能", "#features"], ["隐私", "#privacy"], ["怎么用", "#flow"]],
    menu: "菜单",
    hero: {
      title: ["几千张照片，", "只留值得定格的几张。"],
      body: "惊鸿在你的 Mac 上把连拍分好组，挑出最好的一张。照片不上传。",
      cta: "了解怎么用",
      alt: "惊鸿的照片墙，右侧检查器显示所选照片的评分",
      marks: [["对焦", "船身 0.97"], ["构图", "主体落在三分线"], ["曝光", "均衡，无过曝"], ["色彩", "青绿衬橙红"]],
      kept: "已保留",
    },
    read: { title: "像你一样看每一张照片。", body: "对焦、构图、曝光和色彩都落在画面本身上量，同一瞬间里最好的那张会得到标记。" },
    wall: { title: "看它怎么选。", body: "往下滚，一次真实的筛选就会开始：同一瞬间的连拍排在一起，留下的那张会被标记。", kept: "已保留", note: "示例照片。第 8 至 10 张是同一瞬间。" },
    privacy: {
      text: "你的照片只在你的电脑上。分析、搜索和学习你的喜好，全部在本机完成。模型只在用到时下载一次。",
      points: [["ShieldCheck", "无需账号，不上传"], ["Aperture", "模型按需下载"], ["Sparkle", "设置好后可离线使用"]],
    },
    features: {
      title: "按你的眼光看片。",
      a: { t: "连拍自动分组", d: "同一瞬间的连拍排在一起，最好的一张有标记。人脸、方向和拍摄间隔都会参与判断。", alt: "同一瞬间的两张照片并排，显示评分和推荐" },
      b: { t: "按场景打分", d: "十四种场景，各有各的评分侧重。人像看脸，运动看对焦，夜景不会因为暗被扣分。", kinds: ["人像", "人物场景", "风景", "建筑", "美食", "动物", "物品", "文档", "街拍", "夜景", "运动", "微距", "舞台演出", "交通工具"] },
      c: { t: "用一句话找照片", d: "输入你记得的样子，中英文都可以，按照片里的内容来找。", phrases: [["田野上的日落", "field"], ["平静水面上的小船", "boat"], ["山坡上的白色房屋", "village"], ["森林里的溪流", "forest"]] },
      d: { t: "学你的口味", d: "指一个你过去挑好的文件夹，一次学会。婚礼一套风格，街拍另一套。",
        styles: [["旅行", ["field", "village", "boat", "forest"], [0, 1, 2]], ["街拍", ["st0", "st1", "st2", "st3"], [1, 3]], ["婚礼", ["wd0", "wd1", "wd2", "wd3"], [0, 2, 3]]] },
    },
    flow: {
      title: "打开文件夹，剩下的交给它。",
      steps: [
        ["选一个文件夹", "缩略图先出现，评分在后台分析时陆续补上。"],
        ["看每一组的推荐", "放大对比同一瞬间的照片，或用键盘 P、R、X 决定。"],
        ["整理你保留的照片", "把留下的移到新文件夹，其余进废纸篓，随时可以找回。"],
      ],
    },
    honest: { title: "评分是参考，不是裁判。", body: "它能告诉你哪一张技术上更好。好照片有时并不完美，比如闭着眼睛睡着的孩子，所以决定始终在你手里。" },
    final: { title: "第一时间试用。", body: "macOS 版正在打磨，发布后下载链接会放在这里，Windows 版随后推出。", label: "macOS 版即将发布" },
    footer: { rights: "© 2026 惊鸿 FleetingGleam. 保留所有权利。", note: "截图中的示例照片来自 Lorem Picsum，遵循 Unsplash 授权。" },
  },
} as const;
