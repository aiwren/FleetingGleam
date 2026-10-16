## 改了什么

<!-- 说明这个 Pull Request 做了什么、为什么 -->

## 怎么验证的

<!-- 你跑了什么、看了什么。界面改动请附截图 -->

## 检查清单

- [ ] 落地页：`pnpm --filter @fleetinggleam/web exec tsc --noEmit` 和 `pnpm build:web` 通过
- [ ] 桌面端：`pnpm --filter @fleetinggleam/desktop build`、`cargo check`、`cargo test --lib` 通过
- [ ] 新增的用户可见文字放进了 `i18n`，中文和英文都补了
- [ ] 没有提交模型文件、真实照片、密钥或编译产物
- [ ] 如果改了评分或判定逻辑，已在上面说明依据
