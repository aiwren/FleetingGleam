#!/bin/sh
# 从 CHANGELOG.md 里摘出一个版本的那一节（不含"## [x.y.z]"标题行本身，到下一个"## "为止）。
# 发布工作流用它生成 Release 正文和 latest.json 的 notes（两端应用里"更新说明"显示的就是这段）。
#   .github/scripts/changelog-section.sh 0.1.0 [CHANGELOG.md]
# 找不到这个版本时输出为空、退出码 1，工作流会退回到一句指向 CHANGELOG 的链接。
set -e
version="$1"
file="${2:-CHANGELOG.md}"
[ -n "$version" ] || { echo "用法: $0 <版本号> [CHANGELOG.md]" >&2; exit 2; }
out=$(awk -v v="$version" '
  /^## / { if (found) exit; found = index($0, "[" v "]") > 0; next }
  # 文件末尾的链接定义（[0.1.0]: https://…）不是内容
  found && !/^\[[^]]*\]: / { print }
' "$file" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}')
[ -n "$out" ] || exit 1
printf '%s\n' "$out"
