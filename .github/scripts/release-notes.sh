#!/bin/sh
# 生成一个版本的更新说明（Release 正文的"更新内容"和 latest.json 的 notes，两端应用"关于"里显示的就是它）。
#   .github/scripts/release-notes.sh 0.1.1
# 有 release-notes/<版本>/zh-CN.md 就用它（图文版，格式见 release-notes/README.md）：
#   中文在前，然后一行 <!-- en -->，再是 en.md；应用按界面语言只显示其中一半，GitHub 页面上两种语言都在。
#   图片写相对路径（![](wall.webp)），这里换成按标签固定的 raw.githubusercontent.com 地址，发布之后不会再变。
# 没有就退回 CHANGELOG.md 里这个版本的那一节（纯文字）。都没有时输出为空、退出码 1。
set -e
version="$1"
[ -n "$version" ] || { echo "用法: $0 <版本号>" >&2; exit 2; }
repo="${GITHUB_REPOSITORY:-aiwren/FleetingGleam}"
dir="release-notes/$version"
base="https://raw.githubusercontent.com/$repo/v$version/$dir/"
if [ -f "$dir/zh-CN.md" ]; then
  # 只改不以 http 开头的图片地址
  abs() { sed -E "s#!\[([^]]*)\]\(([^):]+)\)#![\1](${base}\2)#g" "$1"; }
  abs "$dir/zh-CN.md"
  if [ -f "$dir/en.md" ]; then
    # 分隔线只给 GitHub 页面看（两种语言上下排着）；应用里会忽略它
    printf '\n---\n\n<!-- en -->\n\n'
    abs "$dir/en.md"
  fi
  exit 0
fi
exec "$(dirname "$0")/changelog-section.sh" "$version"
