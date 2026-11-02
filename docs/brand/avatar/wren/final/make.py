"""aiwren 标识（定稿）：冒头的鹪鹩，戴一顶焦糖棕翻边毛线帽、大圆眼、琥珀小嘴，呆毛从帽顶卷出、挑着一颗琥珀 AI 星芒。

运行 `python3 make.py [输出目录]` 写出全部 SVG。片头动画（intro.py）也从这里取图形。
上一版（细棕帽、v 字嘴）的脚本和成品留在 archive-v1/。

相对上一版的精修（logo-design 技能体检后）：
- 所有线条同一线宽；
- 棕帽改成真正的毛线帽：圆顶 + 一圈伸出头外的翻边，16 px 也认得出；
- 眼睛、嘴下移（更幼态），只留一颗大眼神光；嘴改成圆角实心小三角（v 字粗线在小尺寸会糊成爱心）；
- 星芒和呆毛拉开间隙；画面里放大并按光学中心摆放；
- 反白版线条减细 6 %（抵消深底上的光渗）；另画一个 ≤ 32 px 专用的小尺寸版。
"""
import os
import sys

INK, CREAM, AMB, BROWN = "#2A221D", "#FBF4EA", "#E8A043", "#B9824B"
W = 12  # 线宽（256 画布）

# 骨架：头是圆心 (128, Y) 半径 R 的半圆弧
Y, R = 170, 70
TOP = Y - R
HAT_UP = 9  # 帽顶比头顶高出的量，帽子才有自己的高度
LIFT = HAT_UP + 3  # 呆毛和星芒跟着帽顶抬高

# 每个尺寸档的几何参数：标准版和 ≤ 32 px 小尺寸版
STD = dict(w=W, er=13.5, ex=26, ey=Y - 28, beak=(9, 9), beak_y=Y - 14,
           curl=f"M128 {TOP} C 121 {TOP-20}, 139 {TOP-31}, 147 {TOP-25}", star=(160, TOP - 38, 15.5), hl=True)
SMALL = dict(w=19, er=14.5, ex=25, ey=Y - 26, beak=(11, 11), beak_y=Y - 12,
             curl=f"M128 {TOP} C 124 {TOP-14}, 136 {TOP-22}, 143 {TOP-19}", star=(164, TOP - 34, 21), hl=False)


def star(cx, cy, r, fill):
    """四角星芒（AI 的通用记号，也是惊鸿的标志形）：四个尖、内凹的弧边。"""
    k = r * 0.24
    return (f'<path d="M{cx:g} {cy-r:g} Q{cx+k:g} {cy-k:g} {cx+r:g} {cy:g} Q{cx+k:g} {cy+k:g} {cx:g} {cy+r:g} '
            f'Q{cx-k:g} {cy+k:g} {cx-r:g} {cy:g} Q{cx-k:g} {cy-k:g} {cx:g} {cy-r:g} Z" fill="{fill}"/>')


def hat_paths(g=STD, w=None):
    """毛线帽：(帽顶, 翻边) 两段路径。帽顶比头外沿大一圈、盖住弧线；翻边伸出头外 7 个单位，下沿离眼睛顶端 7 个单位。"""
    w = w or g["w"]
    o = R + w / 2  # 头弧线的外沿
    rc = o + 3
    hb = g["ey"] - g["er"] - 7  # 翻边下沿
    ct = hb - 13  # 翻边上沿
    bottom = ct + 1
    hx = (rc ** 2 - (Y - HAT_UP - bottom) ** 2) ** 0.5
    crown = f"M{128-hx:.2f} {bottom:g} A{rc:g} {rc:g} 0 0 1 {128+hx:.2f} {bottom:g} Z"
    ow = (o ** 2 - (Y - hb) ** 2) ** 0.5 + 7
    cuff = (f"M{128-ow+6.5:.2f} {ct:g} H{128+ow-6.5:.2f} A6.5 6.5 0 0 1 {128+ow-6.5:.2f} {hb:g} "
            f"H{128-ow+6.5:.2f} A6.5 6.5 0 0 1 {128-ow+6.5:.2f} {ct:g} Z")
    return crown, cuff


def silhouette(w):
    """弧线的外轮廓（深色版的脸）：外沿半圆 + 圆角收底。"""
    o = R + w / 2
    return (f"M{128-o:g} {Y} A {o:g} {o:g} 0 0 1 {128+o:g} {Y} A {w/2:g} {w/2:g} 0 0 1 {128+R} {Y+w/2:g} "
            f"L{128-R} {Y+w/2:g} A {w/2:g} {w/2:g} 0 0 1 {128-o:g} {Y} Z")


def parts(ink, amber=AMB, cap=BROWN, line=None, face=None, g=STD, thin=1.0):
    """小鸟的各个部件，按出场顺序：脸、身体弧线、帽子、眼睛、嘴、呆毛、星芒。

    `line`：弧线和呆毛的颜色（默认同 `ink`）；`face`：深色版的脸（填满弧线外轮廓，不再单画弧线）；
    `thin`：线宽系数，反白版用 0.94。`cap=None` 是单色版：帽子用 `ink`，不画眼神光。
    """
    line = line or ink
    w = g["w"] * thin
    d = {"face": f'<path d="{silhouette(w)}" fill="{face}"/>' if face else ""}
    d["arc"] = "" if face else (f'<path d="M{128-R} {Y} A {R} {R} 0 0 1 {128+R} {Y}" fill="none" stroke="{line}" '
                                f'stroke-width="{w:g}" stroke-linecap="round"/>')
    crown, cuff = hat_paths(g, w)
    hc = cap or ink
    d["hat"] = f'<path d="{crown} {cuff}" fill="{hc}" stroke="{hc}" stroke-width="1.5" stroke-linejoin="round"/>'
    eyes = ""
    er, ey = g["er"], g["ey"]
    for cx in (128 - g["ex"], 128 + g["ex"]):
        eyes += f'<circle cx="{cx}" cy="{ey}" r="{er:g}" fill="{ink}"/>'
        if cap and g["hl"]:  # 一颗大眼神光，单色版不要
            eyes += f'<circle cx="{cx+er*0.36:.1f}" cy="{ey-er*0.36:.1f}" r="{er*0.36:.1f}" fill="#FFF"/>'
    d["eyes"] = eyes
    bw, bh = g["beak"]
    by = g["beak_y"]
    d["beak"] = (f'<path d="M{128-bw} {by} L{128+bw} {by} L128 {by+bh} Z" fill="{amber}" '
                 f'stroke="{amber}" stroke-width="5" stroke-linejoin="round"/>')
    d["curl"] = (f'<path transform="translate(0 {-LIFT})" d="{g["curl"]}" fill="none" stroke="{line}" '
                 f'stroke-width="{w:g}" stroke-linecap="round"/>')
    sx, sy, sr = g["star"]
    d["gleam"] = star(sx, sy - LIFT, sr, amber)
    return d


def mark(ink, **kw):
    return "".join(parts(ink, **kw).values())


DARK = dict(line=CREAM, face=CREAM, thin=0.94)  # 深色底：米白的脸，墨色的大眼睛，线略细


def bounds(g=STD):
    """标识的外框（含线宽），用来缩放和光学居中。"""
    w = g["w"]
    sx, sy, sr = g["star"]
    return 128 - R - w / 2, sy - sr - LIFT, 128 + R + w / 2, Y + w / 2


def fit(inner, size=256, fill=0.84, lift=0.02, g=STD):
    """把标识按外框放进 size 见方：宽或高占 fill，视觉中心比几何中心高 lift。"""
    x0, y0, x1, y1 = bounds(g)
    k = size * fill / max(x1 - x0, y1 - y0)
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    return f'<g transform="translate({size/2:g} {size*(0.5-lift):.2f}) scale({k:.4f}) translate({-cx:.2f} {-cy:.2f})">{inner}</g>'


def svg(x, bg=None, vb="0 0 256 256", title="aiwren"):
    b = f'<rect x="-1000" y="-1000" width="3000" height="3000" fill="{bg}"/>' if bg else ""
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}"><title>{title}</title>{b}{x}</svg>'


def word(ink, x0=252, base=Y, xh=58, sw=W, dot=AMB):
    """单线构造的 aiwren 字样：圆头圆角，和小鸟同样的线宽；i 的点是琥珀色。返回 (svg, 右端 x)。"""
    t = base - xh
    r = xh / 2
    cy = base - r
    p = []
    x = x0
    gap = 15
    p.append(f'<circle cx="{x+r}" cy="{cy}" r="{r}"/><path d="M{x+2*r} {t} V{base}"/>')  # a
    x += 2 * r + gap
    p.append(f'<path d="M{x} {t} V{base}"/>')  # i
    dt = (x, t - 20)
    x += gap
    w = 64  # w
    p.append(f'<path d="M{x} {t} L{x+w*0.24} {base} L{x+w*0.5} {t+14} L{x+w*0.76} {base} L{x+w} {t}"/>')
    x += w + gap
    p.append(f'<path d="M{x} {base} V{t}"/><path d="M{x} {t+24} C{x} {t+6}, {x+14} {t}, {x+32} {t+2}"/>')  # r
    x += 32 + gap - 4
    p.append(f'<path d="M{x+1} {cy} H{x+2*r-1} A{r-1} {r-1} 0 1 0 {x+2*r-7} {cy+r*0.62}"/>')  # e
    x += 2 * r + gap
    p.append(f'<path d="M{x} {base} V{t} M{x} {t+26} C{x} {t+8}, {x+12} {t}, {x+26} {t} C{x+42} {t}, {x+52} {t+8}, {x+52} {t+26} V{base}"/>')  # n
    x += 52
    g = f'<g fill="none" stroke="{ink}" stroke-width="{sw:.1f}" stroke-linecap="round" stroke-linejoin="round">{"".join(p)}</g>'
    g += f'<circle cx="{dt[0]}" cy="{dt[1]}" r="{sw*0.75:.1f}" fill="{dot}"/>'
    return g, x


def bird_for(ink, mono):
    if mono:
        return mark(ink, amber=ink, cap=None)
    if ink == CREAM:
        return mark(INK, **DARK)
    return mark(ink)


PAD = 24


def lockup(ink, bg=None, mono=False):
    """横版：小鸟 + 字样，小鸟弧线的两脚和字样基线齐平。"""
    g, end = word(ink, dot=ink if mono else AMB)
    x0, y0, x1, y1 = bounds()
    return svg(bird_for(ink, mono) + g, bg, f"{x0-PAD:g} {y0-PAD:g} {end + W/2 + PAD - (x0-PAD):g} {y1 - y0 + 2*PAD:g}")


def stacked(ink, bg=None, mono=False):
    """竖版：小鸟在上，字样在下居中，字样缩到 0.62。"""
    g, end = word(ink, dot=ink if mono else AMB)
    ww = end - 252 + W
    x0, y0, x1, y1 = bounds()
    gap, k = 40, 0.62
    word_g = f'<g transform="translate(128 {y1+gap}) scale({k}) translate({-(252 - W/2 + ww/2):.1f} {-(Y-58-26):.1f})">{g}</g>'
    h = y1 + gap + (58 + 26 + W) * k
    vw = max(x1 - x0, ww * k) + 2 * PAD
    return svg(bird_for(ink, mono) + word_g, bg, f"{128 - vw/2:.1f} {y0-PAD:g} {vw:.1f} {h - y0 + 2*PAD:.1f}")


def files():
    small = dict(g=SMALL)
    return {
        "symbol.svg": svg(fit(mark(INK))),
        "symbol-dark.svg": svg(fit(mark(INK, **DARK))),
        "symbol-black.svg": svg(fit(mark("#000", amber="#000", cap=None))),
        "symbol-white.svg": svg(fit(mark("#FFF", amber="#FFF", cap=None, thin=0.94))),
        "avatar.svg": svg(fit(mark(INK), fill=0.74, lift=0.0), CREAM),
        "avatar-dark.svg": svg(fit(mark(INK, **DARK), fill=0.74, lift=0.0), INK),
        # ≤ 32 px 用：线更粗、星芒更大、无眼神光
        "symbol-small.svg": svg(fit(mark(INK, **small), fill=0.9, lift=0.0, **small)),
        "symbol-small-tile.svg": svg(fit(mark(INK, **small), fill=0.84, lift=0.0, **small), CREAM),
        "symbol-small-dark.svg": svg(fit(mark(INK, **small, **{**DARK, "thin": 1.0}), fill=0.84, lift=0.0, **small), INK),
        "lockup.svg": lockup(INK),
        "lockup-dark.svg": lockup(CREAM),  # 透明底，放在深色底上用
        "lockup-black.svg": lockup("#000", mono=True),
        "lockup-white.svg": lockup("#FFF", mono=True),
        "stacked.svg": stacked(INK),
        "stacked-dark.svg": stacked(CREAM),
        "stacked-black.svg": stacked("#000", mono=True),
        "wordmark.svg": svg(word(INK)[0], None, f"{252-W/2-PAD:g} {Y-58-26-PAD:g} {word(INK)[1]-252+W+2*PAD:g} {58+26+W/2+2*PAD:g}"),
    }


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.abspath(__file__))
    os.makedirs(out, exist_ok=True)
    for name, s in files().items():
        with open(os.path.join(out, name), "w") as f:
            f.write(s)
        print(name)
