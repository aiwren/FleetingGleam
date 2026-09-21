export type Lang = "en" | "zh";

/** Everything the page says. Nothing here claims more than the app does today. */
export const copy = {
  en: {
    home: "/", other: { label: "中文", to: "/zh" },
    name: "FleetingGleam", latin: "",
    title: "FleetingGleam | Local photo culling for your Mac",
    description: "FleetingGleam groups your bursts and picks the best frame on your own computer. Nothing is uploaded.",
    nav: [["Features", "#features"], ["Privacy", "#privacy"], ["How it works", "#flow"]],
    hero: {
      title: ["Thousands of photos.", "Keep the few worth freezing."],
      body: "FleetingGleam groups your bursts and picks the best frame, all on your Mac. Nothing is uploaded.",
      cta: "macOS version coming soon", secondary: "How it works",
      alt: "The FleetingGleam photo wall with an inspector showing the score of the selected photo",
    },
    privacy: {
      text: "Your photos stay on your computer. Analysis, grouping, search and learning your taste all run on this machine. Models download once, only when a feature needs them, and you can delete them any time.",
      points: [["ShieldCheck", "No account, no uploads"], ["Aperture", "Models on demand"], ["Sparkle", "Works offline after setup"]],
    },
    features: {
      title: "It looks over the shoot with you, and it knows your taste.",
      a: { t: "Bursts, grouped for you", d: "Frames of one moment sit together and the best one is marked. Faces, orientation and time gap all count.", alt: "Two frames of one moment side by side, with scores and a suggested best" },
      b: { t: "Scored by kind of scene", d: "Portraits are judged by faces, night shots are not marked down for being dark, sport by focus.", kinds: ["Portrait", "Landscape", "Street", "Night", "Sport", "Macro", "Food", "Stage"] },
      c: { t: "Find by describing", d: "Type what you remember. English and Chinese both work.", phrases: ["group photo on a red background", "an umbrella in the rain", "sunset over the sea"] },
      d: { t: "Learns your taste", d: "Point it at a folder of past picks and it learns in one go. Keep one style for weddings, another for street.", styles: ["Travel", "Street", "Wedding"] },
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
    footer: "FleetingGleam",
  },
  zh: {
    home: "/zh", other: { label: "EN", to: "/" },
    name: "惊鸿", latin: "FleetingGleam",
    title: "惊鸿 FleetingGleam | 本地照片筛选",
    description: "惊鸿在你的电脑上把连拍分好组，挑出最好的一张。照片不上传。",
    nav: [["功能", "#features"], ["隐私", "#privacy"], ["怎么用", "#flow"]],
    hero: {
      title: ["几千张照片，", "只留值得定格的几张。"],
      body: "惊鸿在你的 Mac 上把连拍分好组，挑出最好的一张。照片不上传。",
      cta: "macOS 版即将发布", secondary: "怎么用",
      alt: "惊鸿的照片墙，右侧检查器显示所选照片的评分",
    },
    privacy: {
      text: "你的照片只在你的电脑上。分析、分组、搜索和学习你的喜好，全部在本机完成。模型只在用到某项功能时下载一次，随时可以删除。",
      points: [["ShieldCheck", "无需账号，不上传"], ["Aperture", "模型按需下载"], ["Sparkle", "设置好后可离线使用"]],
    },
    features: {
      title: "陪你一起看片，也懂你的口味。",
      a: { t: "连拍自动分组", d: "同一瞬间的连拍排在一起，最好的一张有标记。人脸、方向和拍摄间隔都会参与判断。", alt: "同一瞬间的两张照片并排，显示评分和推荐" },
      b: { t: "按场景打分", d: "人像看脸，夜景不会因为暗被扣分，运动看对焦。", kinds: ["人像", "风景", "街拍", "夜景", "运动", "微距", "美食", "舞台"] },
      c: { t: "用一句话找照片", d: "输入你记得的样子，中英文都可以。", phrases: ["红底合照", "雨中的伞", "海上日落"] },
      d: { t: "学你的口味", d: "指一个你过去挑好的文件夹，一次学会。婚礼一套风格，街拍另一套。", styles: ["旅行", "街拍", "婚礼"] },
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
    footer: "惊鸿 FleetingGleam",
  },
} as const;
