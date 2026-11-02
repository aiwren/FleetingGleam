/** 应用里"关于"用到的链接。网站地址与 apps/web/src/content.ts 的 SITE 一致；换域名时两处一起改。 */
export const SITE = "https://fleetinggleam.fleetinggleam-web.workers.dev";
export const REPO = "https://github.com/aiwren/FleetingGleam";
export const links = (zh: boolean) => ({
  site: zh ? `${SITE}/zh` : SITE,
  source: REPO,
  issues: `${REPO}/issues`,
  privacy: zh ? `${SITE}/zh/privacy` : `${SITE}/privacy`,
  terms: zh ? `${SITE}/zh/terms` : `${SITE}/terms`,
});
