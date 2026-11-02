"""aiwren 标志包：从 final/make.py 生成全部 SVG，再导出 PNG、网页图标、头像和社交卡片。

用法：python3 build.py
需要 logo-design 技能的 render_png.py（无头 Chrome 渲染）。
"""
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "final"))
import make as m  # noqa: E402

RENDER = os.path.expanduser("~/.claude/plugins/cache/logo-design-skill/logo-design/1.4.5/skills/logo-design/scripts/render_png.py")


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


def png(svg, out, w, h=None):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    subprocess.run([sys.executable, RENDER, svg, "-o", out, "--width", str(w), "--height", str(h or w)],
                   check=True, stdout=subprocess.DEVNULL)


def tile(inner_g, bg, rx=0, g=m.STD, fill=0.74):
    """方形底板上的标识；rx>0 是圆角底板（256 画布）。"""
    b = f'<rect width="256" height="256" rx="{rx}" fill="{bg}"/>'
    return m.svg(b + m.fit(inner_g, fill=fill, lift=0.0, g=g))


def main():
    svg_dir, png_dir, web = (os.path.join(HERE, d) for d in ("svg", "png", "web"))
    for d in (svg_dir, png_dir, web):
        shutil.rmtree(d, ignore_errors=True)
    for name, text in m.files().items():
        write(os.path.join(svg_dir, name), text)

    small = dict(g=m.SMALL)
    icons = {
        # 网页 / PWA：米白底；favicon 用小尺寸版
        "favicon.svg": tile(m.mark(m.INK, **small), m.CREAM, rx=48, g=m.SMALL, fill=0.86),
        "app-icon.svg": tile(m.mark(m.INK), m.CREAM),
        "app-icon-dark.svg": tile(m.mark(m.INK, **m.DARK), m.INK),
        # 可遮罩图标：标识缩进中间 80 % 的安全圆
        "maskable.svg": tile(m.mark(m.INK), m.CREAM, fill=0.58),
    }
    for name, text in icons.items():
        write(os.path.join(svg_dir, name), text)

    # 透明底 PNG：标识、横版、竖版
    for name in ("symbol", "symbol-dark", "symbol-black", "symbol-white"):
        for s in (256, 512, 1024):
            png(f"{svg_dir}/{name}.svg", f"{png_dir}/{name}-{s}.png", s)
    for name, (w, h) in {"lockup": (1600, 0), "lockup-dark": (1600, 0), "lockup-black": (1600, 0),
                         "lockup-white": (1600, 0), "stacked": (1000, 0), "stacked-dark": (1000, 0)}.items():
        vb = [float(v) for v in open(f"{svg_dir}/{name}.svg").read().split('viewBox="')[1].split('"')[0].split()]
        png(f"{svg_dir}/{name}.svg", f"{png_dir}/{name}-{w}.png", w, round(w * vb[3] / vb[2]))
    # 头像（GitHub 等社交平台，圆形裁切安全）
    for name in ("avatar", "avatar-dark"):
        for s in (460, 1024):
            png(f"{svg_dir}/{name}.svg", f"{png_dir}/{name}-{s}.png", s)

    # 网页图标套装
    os.makedirs(web, exist_ok=True)
    shutil.copy(f"{svg_dir}/favicon.svg", f"{web}/favicon.svg")
    for s in (16, 32, 48):
        png(f"{svg_dir}/symbol-small-tile.svg", f"{web}/favicon-{s}.png", s)
    subprocess.run([sys.executable, RENDER, f"{svg_dir}/symbol-small-tile.svg", "--ico", f"{web}/favicon.ico",
                    "--ico-sizes", "16", "32", "48"], check=True, stdout=subprocess.DEVNULL)
    png(f"{svg_dir}/app-icon.svg", f"{web}/apple-touch-icon.png", 180)
    png(f"{svg_dir}/app-icon.svg", f"{web}/icon-192.png", 192)
    png(f"{svg_dir}/app-icon.svg", f"{web}/icon-512.png", 512)
    png(f"{svg_dir}/maskable.svg", f"{web}/maskable-512.png", 512)
    write(f"{web}/site.webmanifest", """{
  "name": "aiwren",
  "icons": [
    {"src": "/icon-192.png", "sizes": "192x192", "type": "image/png"},
    {"src": "/icon-512.png", "sizes": "512x512", "type": "image/png"},
    {"src": "/maskable-512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable"}
  ],
  "theme_color": "#FBF4EA",
  "background_color": "#FBF4EA",
  "display": "standalone"
}
""")
    write(f"{web}/head-snippet.html", """<link rel="icon" href="/favicon.ico" sizes="48x48">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="manifest" href="/site.webmanifest">
<meta name="theme-color" content="#FBF4EA" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#2A221D" media="(prefers-color-scheme: dark)">
""")

    # 配色卡
    sw = [("墨", m.INK, "#FBF4EA"), ("米白", m.CREAM, "#2A221D"), ("琥珀", m.AMB, "#2A221D"), ("焦糖棕", m.BROWN, "#2A221D")]
    cells = "".join(
        f'<rect x="{i*300}" width="300" height="400" fill="{c}"/>'
        f'<text x="{i*300+28}" y="330" fill="{t}" font-size="34" font-weight="600">{n}</text>'
        f'<text x="{i*300+28}" y="372" fill="{t}" font-size="24">{c}</text>' for i, (n, c, t) in enumerate(sw))
    write(f"{svg_dir}/palette.svg", f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 400" '
          f'font-family="-apple-system, PingFang SC, sans-serif"><title>aiwren 配色</title>{cells}</svg>')
    png(f"{svg_dir}/palette.svg", f"{png_dir}/palette-1200x400.png", 1200, 400)

    # GitHub 仓库社交预览卡 1280×640：深色底 + 横版居中
    lk = open(f"{svg_dir}/lockup-dark.svg").read()
    vb = lk.split('viewBox="')[1].split('"')[0]
    inner = lk.split("</title>", 1)[1].rsplit("</svg>", 1)[0]
    write(f"{svg_dir}/social-card.svg",
          f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 640"><title>aiwren</title>'
          f'<rect width="1280" height="640" fill="{m.INK}"/>'
          f'<svg x="240" y="170" width="800" height="300" viewBox="{vb}">{inner}</svg></svg>')
    png(f"{svg_dir}/social-card.svg", f"{png_dir}/social-card-1280x640.png", 1280, 640)
    print("done")


if __name__ == "__main__":
    main()
