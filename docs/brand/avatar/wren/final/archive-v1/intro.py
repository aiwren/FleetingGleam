"""aiwren 视频片头：逐帧生成 SVG，用无头 Chrome 截图，再用 ffmpeg 合成 MP4。

用法：python3 intro.py <输出目录> [--fps 30] [--dark]
图形来自 make.py，改了标识重新跑一遍即可。
"""
import math
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

from make import AMB, BROWN, CREAM, INK, W, star, word

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
DUR = 3.6
WIDTH, HEIGHT = 1920, 1080


def clamp(x, a=0.0, b=1.0):
    return max(a, min(b, x))


def seg(t, t0, t1):
    """t 在 [t0, t1] 里走过的比例。"""
    return clamp((t - t0) / (t1 - t0))


def ease_out(x):
    return 1 - (1 - x) ** 3


def ease_in_out(x):
    return 3 * x * x - 2 * x * x * x


def pop(x):
    """弹出：冲过 1 一点再落回。"""
    if x <= 0:
        return 0.0
    if x >= 1:
        return 1.0
    c1 = 1.70158  # easeOutBack
    return 1 + (c1 + 1) * (x - 1) ** 3 + c1 * (x - 1) ** 2


def frame(t, dark):
    bg = INK if dark else CREAM
    line = CREAM if dark else INK  # 弧线、呆毛、字样的颜色
    y, top = 170, 100
    word_svg, end = word(line)
    stage_l, stage_r = 20, end + 24
    lock_cx = (stage_l + stage_r) / 2
    move = ease_in_out(seg(t, 1.95, 2.55))
    shift = (lock_cx - 128) * (1 - move)

    s = []
    # 身体弧线：从左端画到右端（深色版先铺米白的脸）
    a = ease_out(seg(t, 0.05, 0.75))
    if dark and a > 0:
        s.append(f'<path d="M58 {y} A 70 70 0 0 1 198 {y} Z" fill="{CREAM}" opacity="{ease_out(seg(t, 0.35, 0.8)):.3f}"/>')
    # 棕帽：弧线画到一半后从上往下"戴"上
    c = ease_out(seg(t, 0.45, 0.85))
    if c > 0:
        s.append(f'<clipPath id="dome"><path d="M58 {y} A 70 70 0 0 1 198 {y} Z"/></clipPath>'
                 f'<g clip-path="url(#dome)"><path transform="translate(0 {-30*(1-c):.2f})" d="M40 {y-12} C 90 {y-70}, 166 {y-70}, 216 {y-12} L216 0 L40 0 Z" fill="{BROWN}"/></g>')
    if a > 0:
        s.append(f'<path d="M58 {y} A 70 70 0 0 1 198 {y}" pathLength="1" stroke-dasharray="1" stroke-dashoffset="{1-a:.4f}" fill="none" stroke="{line}" stroke-width="{W}" stroke-linecap="round"/>')
    # 大眼睛：依次弹出；1.75 s 眨一下
    blink = seg(t, 1.75, 1.83) if t < 1.83 else 1 - seg(t, 1.83, 1.93)
    ey_k = 1 - 0.88 * clamp(blink)
    ey, er = y - 40, 12.5
    for i, cx in enumerate((101, 155)):
        k = pop(seg(t, 0.6 + i * 0.09, 0.9 + i * 0.09))
        if k > 0:
            hl = f'<circle cx="{cx+4.2}" cy="{ey-4.4}" r="{er*0.32:.1f}" fill="#FFF"/><circle cx="{cx-4.5}" cy="{ey+5}" r="{er*0.13:.1f}" fill="#FFF"/>' if ey_k > 0.5 else ""
            s.append(f'<g transform="translate({cx} {ey}) scale({k:.3f} {k*ey_k:.3f}) translate({-cx} {-ey})"><circle cx="{cx}" cy="{ey}" r="{er}" fill="{INK}"/>{hl}</g>')
    # 嘴
    k = pop(seg(t, 0.85, 1.12))
    if k > 0:
        s.append(f'<g transform="translate(128 {y-19}) scale({k:.3f}) translate(-128 -{y-19})"><path d="M120 {y-24} L128 {y-15} L136 {y-24}" fill="none" stroke="{AMB}" stroke-width="{W-1}" stroke-linecap="round" stroke-linejoin="round"/></g>')
    # 呆毛：往上卷出来
    c = ease_out(seg(t, 1.0, 1.4))
    if c > 0:
        s.append(f'<path d="M127 {top} C 121 {top-20}, 140 {top-30}, 146 {top-24}" pathLength="1" stroke-dasharray="1" stroke-dashoffset="{1-c:.4f}" fill="none" stroke="{line}" stroke-width="{W-1}" stroke-linecap="round"/>')
    # AI 星芒：转着弹出来，一圈光晕散开（"一闪"）
    gx, gy = 155, top - 33
    k = pop(seg(t, 1.3, 1.6))
    if k > 0:
        rot = -90 * (1 - ease_out(seg(t, 1.3, 1.6)))
        s.append(f'<g transform="translate({gx} {gy}) rotate({rot:.1f}) scale({k:.3f}) translate({-gx} {-gy})">{star(gx, gy, 15, AMB)}</g>')
    h = seg(t, 1.36, 1.95)
    if 0 < h < 1:
        s.append(f'<circle cx="{gx}" cy="{gy}" r="{10 + 32*ease_out(h):.2f}" fill="none" stroke="{AMB}" stroke-width="{3*(1-h):.2f}" opacity="{0.8*(1-h):.3f}"/>')
    bird = f'<g transform="translate({shift:.2f} 0)">{"".join(s)}</g>'

    # 字样：从左往右展开；i 的琥珀点最后落下
    wipe = ease_in_out(seg(t, 2.15, 2.85))
    x0 = 236
    body, dot = word_svg.rsplit("<circle", 1)
    dot = "<circle" + dot
    clip = f'<clipPath id="w"><rect x="{x0}" y="0" width="{(end - x0 + 20) * wipe:.2f}" height="400"/></clipPath>'
    words = f'<g clip-path="url(#w)" opacity="{clamp(wipe*1.5):.3f}">{body}</g>' if wipe > 0 else ""
    d = seg(t, 2.75, 3.05)
    if d > 0:
        dy = -40 * (1 - ease_out(d))
        words += f'<g transform="translate(0 {dy:.2f})" opacity="{clamp(d*2):.3f}">{dot}</g>'

    # 画面：标识宽度占画面 58%；小鸟单独出场时镜头推近 1.5 倍，让位给字样时拉回
    vw = stage_r - stage_l
    zoom = 1 + 0.5 * (1 - move)
    scale = WIDTH * 0.58 / vw * zoom
    vh = HEIGHT / scale
    vx = lock_cx - WIDTH / scale / 2
    vy = 124 - vh / 2
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="{vx:.2f} {vy:.2f} {WIDTH/scale:.2f} {vh:.2f}">'
            f'<defs>{clip}</defs><rect x="-5000" y="-5000" width="10000" height="10000" fill="{bg}"/>{bird}{words}</svg>')


def main():
    out = os.path.abspath(sys.argv[1])
    fps = int(sys.argv[sys.argv.index("--fps") + 1]) if "--fps" in sys.argv else 30
    dark = "--dark" in sys.argv
    frames = os.path.join(out, "frames-dark" if dark else "frames")
    os.makedirs(frames, exist_ok=True)
    n = int(DUR * fps)

    def shoot(i):
        svg = os.path.join(frames, f"f{i:04d}.svg")
        png = os.path.join(frames, f"f{i:04d}.png")
        with open(svg, "w") as f:
            f.write(frame(i / fps, dark))
        subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                        f"--window-size={WIDTH},{HEIGHT}", f"--screenshot={png}", f"--user-data-dir={frames}/chrome-{i % 8}", "file://" + svg],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)

    with ThreadPoolExecutor(8) as ex:
        list(ex.map(shoot, range(n)))
    name = "intro-dark.mp4" if dark else "intro.mp4"
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", str(fps), "-i", os.path.join(frames, "f%04d.png"),
                    "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "16", "-movflags", "+faststart", os.path.join(out, name)], check=True)
    print(os.path.join(out, name))


if __name__ == "__main__":
    main()
