// 把桌面端的文案（apps/desktop/src/i18n/zh-CN.ts、en.ts）导出成 Android 读的 JSON（app/src/main/assets/i18n/base.<语言>.json）。
// 两端用同一份文案：决定的理由是"消息键 + 参数"，由核心库给出，界面按当前语言渲染，所以文案必须一字不差地共用。
// Android 才有的文案（权限、相册、系统回收站……）在 app/src/main/assets/i18n/android.<语言>.json 里，读的时候叠在上面。
// 用法：node scripts/export-i18n.mjs（Gradle 的 exportI18n 任务会在每次构建前跑）。
import { mkdtempSync, copyFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const src = join(here, "../../desktop/src/i18n");
const out = join(here, "../app/src/main/assets/i18n");
mkdirSync(out, { recursive: true });
// Node 22 能直接读掉类型注解的 TS，只要扩展名是 .mts
const tmp = mkdtempSync(join(tmpdir(), "fg-i18n-"));
for (const [file, lang] of [["zh-CN.ts", "zh-CN"], ["en.ts", "en"]]) {
  const copy = join(tmp, `${lang}.mts`);
  copyFileSync(join(src, file), copy);
  const mod = await import(pathToFileURL(copy).href);
  writeFileSync(join(out, `base.${lang}.json`), JSON.stringify(mod.default, null, 1) + "\n");
  console.log(`i18n: ${lang} -> base.${lang}.json`);
}
