import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(__dirname, "..");
const srcDir = path.join(webDir, "dist", "client");
const dstDir = path.join(webDir, "dist", "pages-cn");

const OVERSEAS_ORIGIN = "https://gleam.novx.si";
const CN_ORIGIN = "https://gleam.tinkerai.cn";
const TEXT_EXTS = new Set([".html", ".js", ".xml", ".txt", ".json"]);

if (!fs.existsSync(srcDir)) {
  console.error(`Missing build output: ${srcDir}. Run "pnpm build:web" first.`);
  process.exit(1);
}

fs.rmSync(dstDir, { recursive: true, force: true });
fs.cpSync(srcDir, dstDir, { recursive: true });

let replacedCount = 0;
function rewriteOrigins(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      rewriteOrigins(full);
    } else if (entry.isFile() && TEXT_EXTS.has(path.extname(entry.name).toLowerCase())) {
      const text = fs.readFileSync(full, "utf8");
      if (text.includes(OVERSEAS_ORIGIN)) {
        fs.writeFileSync(full, text.replaceAll(OVERSEAS_ORIGIN, CN_ORIGIN), "utf8");
        replacedCount++;
      }
    }
  }
}
rewriteOrigins(dstDir);

const routesJson = {
  version: 1,
  include: ["/*"],
  exclude: [
    "/assets/*",
    "/home/*",
    "/shots/*",
    "/favicon.png",
    "/logo.png",
    "/og.png",
    "/robots.txt",
    "/sitemap.xml",
  ],
};
fs.writeFileSync(
  path.join(dstDir, "_routes.json"),
  JSON.stringify(routesJson, null, 2) + "\n",
  "utf8",
);

const workerJs = `const TARGET_ORIGIN = "${OVERSEAS_ORIGIN}";
const SOURCE_HOST = "${new URL(CN_ORIGIN).hostname}";

// 豁免常见搜索引擎爬虫与社交卡片抓取器，避免影响国内/海外收录与分享预览
const BOT_RE =
  /bot|crawl|spider|slurp|bingpreview|baiduspider|bytespider|sogou|360spider|yisouspider|googlebot|bingbot|duckduckbot|yandex|applebot|facebookexternalhit|twitterbot|linkedinbot|whatsapp|telegrambot|slackbot|discordbot/i;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    // 仅处理 GET/HEAD 且访问主域名 gleam.tinkerai.cn（不拦截 *.pages.dev 预览地址）
    if (
      (request.method === "GET" || request.method === "HEAD") &&
      url.hostname === SOURCE_HOST
    ) {
      // 调试逃生参数：?no_redirect=1 不跳转（与 gleam.novx.si 保持一致）
      if (url.searchParams.get("no_redirect") !== "1") {
        const ua = request.headers.get("user-agent") || "";
        const country = request.cf && request.cf.country;

        // 明确识别为海外 IP（排除 CN 及未知 XX / Tor T1），且非爬虫时，302 跳转至海外官网
        if (
          country &&
          country !== "CN" &&
          country !== "XX" &&
          country !== "T1" &&
          !BOT_RE.test(ua)
        ) {
          const acceptLang = (request.headers.get("accept-language") || "").toLowerCase();
          const primaryLang = acceptLang.split(",")[0] || "";
          const prefersZh = primaryLang.startsWith("zh");

          let targetPath = url.pathname;
          // 非中文浏览器的海外访客访问 /zh 或 /zh/... 时，自动切到海外英文对应路径
          if (!prefersZh && (targetPath === "/zh" || targetPath.startsWith("/zh/"))) {
            targetPath = targetPath.replace(/^\\/zh/, "") || "/";
          }

          const targetUrl = \`\${TARGET_ORIGIN}\${targetPath}\${url.search}\`;
          return new Response(null, {
            status: 302,
            headers: {
              Location: targetUrl,
              "Cache-Control": "private, no-store",
              Vary: "CF-IPCountry, Accept-Language",
            },
          });
        }
      }
    }

    const response = await env.ASSETS.fetch(request);
    const contentType = response.headers.get("content-type") || "";
    if (contentType.includes("text/html")) {
      const next = new Response(response.body, response);
      next.headers.append("Vary", "CF-IPCountry");
      return next;
    }
    return response;
  },
};
`;
fs.writeFileSync(path.join(dstDir, "_worker.js"), workerJs, "utf8");

console.log(
  `Prepared CN Pages bundle at ${path.relative(webDir, dstDir)} (rewrote ${replacedCount} files, added _routes.json and _worker.js).`,
);
