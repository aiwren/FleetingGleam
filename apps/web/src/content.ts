export type Lang = "zh" | "en";

/** Everything the page says, in both languages. Nothing here claims more than the app does today. */
export const copy = {
  zh: {
    name: "惊鸿",
    latin: "FleetingGleam",
    title: "惊鸿 FleetingGleam · 本地照片筛选",
    description: "旅行归来几千张照片，惊鸿在你的电脑上帮你分好连拍、挑出最好的一张。照片不上传，全程本地处理。",
    nav: { features: "功能", privacy: "隐私", flow: "怎么用", download: "下载", other: { label: "EN", to: "/en" } },
    hero: {
      kicker: "照片筛选 · 本地运行",
      title: ["几千张照片，", "只留下值得定格的那几张。"],
      body: "旅行回来，同一个瞬间连拍了十几张。惊鸿在你的电脑上把它们分好组，比较清晰度、曝光和表情，告诉你每组里哪一张最好。照片始终留在你的电脑里。",
      cta: "macOS 版即将发布",
      note: "Windows 版随后推出",
      secondary: "了解怎么工作",
    },
    proof: ["全程本地处理", "不上传任何照片", "按需下载模型，之后可离线使用"],
    privacy: {
      kicker: "隐私",
      title: "你的照片，只在你的电脑上。",
      body: "分析、分组、搜索、学习你的喜好，全部在本机完成。惊鸿只会在你第一次用到某项功能时，下载对应的模型文件，之后不再需要网络。",
      points: [
        ["不上传", "没有云端相册，没有账号，照片不会离开这台电脑。"],
        ["按需下载", "模型不随安装包一起给你，用到哪个才下载哪个，不用的可以随时删掉。"],
        ["喜好只存本机", "你的选片习惯保存在本地文件里，可以随时清空。"],
      ],
    },
    features: {
      kicker: "功能",
      title: "像有个懂你口味的助手，坐在你旁边看片。",
      items: [
        { t: "连拍自动分组", d: "把同一瞬间的连拍归到一起，每组标出最好的那张。人数、方向、拍摄间隔都会参与判断，不会把两个不同的场景混在一起。" },
        { t: "分场景打分", d: "人像看脸和眼睛，风景看曝光和整体清晰，运动看对焦，夜景不会因为暗被扣分。同一组连拍里有人闭眼，会优先推荐睁眼的那张。" },
        { t: "用一句话找照片", d: "输入“红底合照”“雨中的伞”这样的描述就能找到对应的照片，中英文都可以。" },
        { t: "学你的口味", d: "你保留和淘汰的选择会慢慢教会它你喜欢什么样的照片。也可以直接指一个你过去挑好的文件夹，让它一次学会。" },
        { t: "多种选片风格", d: "街拍和婚礼的口味不一样。在设置里可以建立多套风格，每个项目选用适合的那一套。" },
      ],
    },
    flow: {
      kicker: "怎么用",
      title: "打开文件夹，剩下的交给它。",
      steps: [
        ["选一个文件夹", "拖进来或从菜单打开。惊鸿在后台逐张分析，缩略图先出现，评分陆续补上。"],
        ["看每一组的推荐", "同一瞬间的照片排在一起，最好的一张有标记。放大对比，或者直接用键盘 P、R、X 决定。"],
        ["整理你保留的照片", "把留下的照片整理到新文件夹，被淘汰的移到废纸篓，随时可以找回。"],
      ],
    },
    honest: {
      title: "评分是参考，不是裁判。",
      body: "自动评分只能告诉你“技术上哪一张更好”，好照片有时恰好不完美，比如闭着眼睛睡着的孩子。所以最终的决定始终在你手里，它给的是排序和理由。",
    },
    cta: { title: "等它上线时，第一个知道。", body: "macOS 版正在打磨中。发布之后，这里会放上下载链接。", button: "macOS 版即将发布" },
    footer: "© 惊鸿 FleetingGleam",
    mock: { keep: "保留", out: "淘汰", best: "推荐", group: "同一瞬间 · 6 张", name: "选片" },
  },
  en: {
    name: "FleetingGleam",
    latin: "",
    title: "FleetingGleam · Local photo culling",
    description: "Thousands of photos from a trip: FleetingGleam groups the bursts and picks the best frame on your own computer. Nothing is uploaded.",
    nav: { features: "Features", privacy: "Privacy", flow: "How it works", download: "Download", other: { label: "中文", to: "/" } },
    hero: {
      kicker: "Photo culling · runs locally",
      title: ["Thousands of photos.", "Keep only the ones worth freezing."],
      body: "You come home with a dozen frames of every moment. FleetingGleam groups them on your computer, compares sharpness, exposure and faces, and tells you which frame in each group is the best. Your photos never leave the machine.",
      cta: "macOS version coming soon",
      note: "Windows to follow",
      secondary: "See how it works",
    },
    proof: ["Processed entirely on-device", "No photo is ever uploaded", "Models download on demand, then work offline"],
    privacy: {
      kicker: "Privacy",
      title: "Your photos stay on your computer.",
      body: "Analysis, grouping, search and learning your taste all happen on this machine. FleetingGleam downloads a model file only the first time a feature needs it, and after that it needs no network.",
      points: [
        ["No uploads", "No cloud library, no account. Photos do not leave this computer."],
        ["Models on demand", "Models are not bundled with the installer. Each is fetched when first needed, and you can delete the ones you do not use."],
        ["Taste stays local", "Your culling habits are saved in a local file that you can clear at any time."],
      ],
    },
    features: {
      kicker: "Features",
      title: "Like an assistant who knows your taste, looking over the shoot with you.",
      items: [
        { t: "Bursts grouped for you", d: "Frames of the same moment are put together and the best one is marked. Number of people, orientation and time gap all count, so two different scenes are not mixed up." },
        { t: "Scored by kind of scene", d: "Portraits are judged by faces and eyes, landscapes by exposure and overall sharpness, sport by focus. A night shot is not marked down for being dark. If someone blinks in a burst, the frame with open eyes is preferred." },
        { t: "Find photos by describing them", d: "Type “a group photo on a red background” or “an umbrella in the rain”. English and Chinese both work." },
        { t: "Learns your taste", d: "What you keep and reject slowly teaches it what you like. Or point it at a folder of past picks and it learns from that in one go." },
        { t: "Several culling styles", d: "Street and weddings call for different tastes. Create as many styles as you like in Settings and pick one per project." },
      ],
    },
    flow: {
      kicker: "How it works",
      title: "Open a folder. It does the rest.",
      steps: [
        ["Pick a folder", "Drag it in or open it from the menu. Photos are analysed in the background: thumbnails appear first, scores fill in as they finish."],
        ["Review each group", "Frames of one moment sit together with the best marked. Zoom to compare, or decide from the keyboard with P, R and X."],
        ["Organise what you kept", "Move the keepers into a new folder and the rest to the Trash, where they can always be recovered."],
      ],
    },
    honest: {
      title: "A score is a hint, not a verdict.",
      body: "Automatic scoring can tell you which frame is technically stronger. A good photograph is sometimes imperfect, like a sleeping child with closed eyes. So the decision is always yours; you get a ranking and the reasons.",
    },
    cta: { title: "Be the first to know when it ships.", body: "The macOS version is being polished. The download link will appear here once it is out.", button: "macOS version coming soon" },
    footer: "© FleetingGleam",
    mock: { keep: "Kept", out: "Out", best: "Best", group: "One moment · 6 frames", name: "Cull" },
  },
} as const;
