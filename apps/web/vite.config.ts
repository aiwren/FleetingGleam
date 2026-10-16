import { defineConfig } from "vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import tailwindcss from "@tailwindcss/vite";
import viteReact from "@vitejs/plugin-react";

export default defineConfig({
  server: { port: 3150 },
  resolve: { tsconfigPaths: true },
  plugins: [
    tailwindcss(),
    // 页面全是静态内容，构建时直接生成 HTML，可以放在任何静态托管上（Cloudflare 等），不需要服务端运行时
    tanstackStart({ prerender: { enabled: true, failOnError: true } }),
    viteReact(),
  ],
});
