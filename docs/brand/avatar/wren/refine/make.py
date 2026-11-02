"""aiwren 标识（精修稿）：冒头的鹪鹩，焦糖棕头顶、大圆眼、琥珀小嘴，呆毛上挑着一颗琥珀 AI 星芒。

相对 final/make.py 的精修：
- 全部线条统一同一线宽（呆毛原来细 1 个单位，属于"差一点"的不一致）；
- 棕帽从一条细缝加厚成头顶的一整块月牙，16 px 也能看到棕色；
- 眼睛下移、加大，只留一颗大眼神光（原来那颗 3.6 单位的小光点在小尺寸会消失）；
- 嘴改成圆角实心小三角（粗线 v 字在小尺寸会糊成爱心）；
- 星芒和呆毛之间拉开间隙，32 px 下不粘连；
- 画面里放大并按光学中心摆放（标识原来只占画布三分之一）；
- 反白版线条减细 6 %（抵消深底上的光渗）；
- 另画一个 ≤ 32 px 专用的小尺寸版。

运行 `python3 make.py [输出目录]` 写出全部 SVG。片头动画（intro.py）也从这里取图形。
"""
import os
import sys

INK, CREAM, AMB, BROWN = "#2A221D", "#FBF4EA", "#E8A043", "#B9824B"
W = 12  # 线宽（256 画布）

# 骨架：头是圆心 (128, Y) 半径 R 的半圆弧
Y, R = 170, 70
TOP = Y - R


def star(cx, cy, r, fill):
    """四角星芒（AI 的通用记号，也是惊鸿的标志形）：四个尖、内凹的弧边。"""
    k = r * 0.24
    return (f'<path d="M{cx:g} {cy-r:g} Q{cx+k:g} {cy-k:g} {cx+r:g} {cy:g} Q{cx+k:g} {cy+k:g} {cx:g} {cy+r:g} '
            f'Q{cx-k:g} {cy+k:g} {cx-r:g} {cy:g} Q{cx-k:g} {cy-k:g} {cx:g} {cy-r:g} Z" fill="{fill}"/>')


# 每个尺寸档的几何参数：标准版和 ≤ 32 px 小尺寸版
STD = dict(w=W, er=13.5, ex=26, ey=Y - 28, cap_c=118, cap_s=128, beak=(9, 9), beak_y=Y - 14,
           curl=(f"M128 {TOP} C 121 {TOP-20}, 139 {TOP-31}, 147 {TOP-25}"), star=(160, TOP - 38, 15.5), hl=True, hat="beanie")
SMALL = dict(w=19, er=14.5, ex=25, ey=Y - 26, cap_c=122, cap_s=130, beak=(11, 11), beak_y=Y - 12,
             curl=(f"M128 {TOP} C 124 {TOP-14}, 136 {TOP-22}, 143 {TOP-19}"), star=(164, TOP - 34, 21), hl=False, hat="beanie")


def cap_path(c, s, rad):
    """棕帽的闭合路径：下沿是一条三次贝塞尔（两端高 s、中段控制点高 c），上沿是半径 rad 的头顶圆弧。"""
    P = [(30, s), (84, c), (172, c), (226, s)]

    def at(t, pts=P):
        a = pts
        while len(a) > 1:
            a = [((1 - t) * p[0] + t * q[0], (1 - t) * p[1] + t * q[1]) for p, q in zip(a, a[1:])]
        return a[0]

    def outside(t):
        x, y = at(t)
        return (x - 128) ** 2 + (y - Y) ** 2 > rad ** 2

    lo, hi = 0.0, 0.5  # 左端在圆外，中点在圆内，二分求交点
    for _ in range(60):
        m = (lo + hi) / 2
        lo, hi = (m, hi) if outside(m) else (lo, m)
    t0, t1 = lo, 1 - lo

    # 取 [t0, t1] 这一段曲线（de Casteljau 两次切分）
    def split(pts, t):
        a, left, right = pts, [pts[0]], [pts[-1]]
        while len(a) > 1:
            a = [((1 - t) * p[0] + t * q[0], (1 - t) * p[1] + t * q[1]) for p, q in zip(a, a[1:])]
            left.append(a[0])
            right.insert(0, a[-1])
        return left, right

    _, rest = split(P, t0)
    seg, _ = split(rest, (t1 - t0) / (1 - t0))
    f = lambda p: f"{p[0]:.2f} {p[1]:.2f}"
    return (f"M{f(seg[0])} C{f(seg[1])} {f(seg[2])} {f(seg[3])} "
            f"A{rad:g} {rad:g} 0 0 0 {f(seg[0])} Z")


HAT_UP = {"beanie": 9, "side": 6, "front": 6}


def hat_path(kind, g, w):
    """更像帽子的三种头顶：盖在头的弧线上面，帽子有明确的下沿。下沿 hb 离眼睛顶端留 7 个单位。

    beanie：毛线帽，圆顶 + 一圈伸出头外的翻边；side：侧戴的鸭舌帽，帽舌朝左伸出去；
    front：正戴的鸭舌帽，帽舌是一片压在眉上的弯月。
    """
    o = R + w / 2  # 头弧线的外沿
    rc = o + 3  # 帽顶比头外沿大一圈，盖住弧线
    hb = g["ey"] - g["er"] - 7

    up = HAT_UP[kind]  # 帽顶比头顶高出的量，帽子才有自己的高度，不像锅盖

    def dome(bottom):
        hx = (rc ** 2 - (Y - up - bottom) ** 2) ** 0.5
        return f"M{128-hx:.2f} {bottom:g} A{rc:g} {rc:g} 0 0 1 {128+hx:.2f} {bottom:g} Z"

    def wide(y):  # 头外沿在高度 y 处的半宽
        return (o ** 2 - (Y - y) ** 2) ** 0.5

    if kind == "beanie":
        ct = hb - 13
        ow = wide(hb) + 7
        cuff = (f"M{128-ow+6.5:.2f} {ct:g} H{128+ow-6.5:.2f} A6.5 6.5 0 0 1 {128+ow-6.5:.2f} {hb:g} "
                f"H{128-ow+6.5:.2f} A6.5 6.5 0 0 1 {128-ow+6.5:.2f} {ct:g} Z")
        return dome(ct + 1) + " " + cuff
    if kind == "side":
        top, bot = hb - 13, hb - 1
        # 和帽顶同为顺时针，重叠处才不会被非零环绕规则挖空
        bill = (f"M{128-wide(bot):.2f} {bot:g} H{38+6:g} A6 6 0 0 1 {38:g} {bot-6:g} "
                f"A6 6 0 0 1 {44:g} {top:g} H{128-wide(bot):.2f} Z")
        return dome(hb - 1) + " " + bill
    if kind == "front":
        vt = hb - 10
        ow = wide(vt) + 9
        visor = (f"M{128-ow:.2f} {vt:g} Q128 {vt-7:g} {128+ow:.2f} {vt:g} "
                 f"Q128 {hb+9:g} {128-ow:.2f} {vt:g} Z")
        return dome(vt + 1) + " " + visor
    raise ValueError(kind)


def parts(ink, amber=AMB, cap=BROWN, line=None, face=None, g=STD, thin=1.0):
    """小鸟的各个部件，按出场顺序：（脸）、棕帽、身体弧线、眼睛、嘴、呆毛、星芒。

    `line`：弧线和呆毛的颜色（默认同 `ink`）；`face`：头部里面填一块颜色（深色底用米白，眼睛仍是墨色）；
    `thin`：线宽系数，反白版用 0.94。`cap=None` 是单色版：不画棕帽和眼神光。
    """
    line = line or ink
    w = g["w"] * thin
    dome = f"M{128-R} {Y} A {R} {R} 0 0 1 {128+R} {Y} Z"
    # 深色版：脸填到弧线外沿、圆角收底（就是弧线的外轮廓），棕帽也铺到外沿，不再单画弧线
    o = R + w / 2
    sil = (f"M{128-o:g} {Y} A {o:g} {o:g} 0 0 1 {128+o:g} {Y} A {w/2:g} {w/2:g} 0 0 1 {128+R} {Y+w/2:g} "
           f"L{128-R} {Y+w/2:g} A {w/2:g} {w/2:g} 0 0 1 {128-o:g} {Y} Z")
    d = {"face": f'<path d="{sil}" fill="{face}"/>' if face else ""}
    # 棕帽：头顶的一块月牙，下沿比头的弧线平，所以中间厚、两侧收尖；直接算出闭合路径，不用裁切蒙版
    hat = g.get("hat")
    if not hat:
        d["cap"] = f'<path d="{cap_path(g["cap_c"], g["cap_s"], o if face else R)}" fill="{cap}"/>' if cap else ""
    d["arc"] = "" if face else f'<path d="M{128-R} {Y} A {R} {R} 0 0 1 {128+R} {Y}" fill="none" stroke="{line}" stroke-width="{w:g}" stroke-linecap="round"/>'
    if hat:  # 帽子盖在弧线上面；单色版用墨色实心
        d["cap"] = f'<path d="{hat_path(hat, g, w)}" fill="{cap or ink}" stroke="{cap or ink}" stroke-width="1.5" stroke-linejoin="round"/>'
    eyes = ""
    er, ey = g["er"], g["ey"]
    for cx in (128 - g["ex"], 128 + g["ex"]):
        eyes += f'<circle cx="{cx}" cy="{ey}" r="{er:g}" fill="{ink}"/>'
        if cap and g["hl"]:  # 一颗大眼神光，单色版不要
            eyes += f'<circle cx="{cx+er*0.36:.1f}" cy="{ey-er*0.36:.1f}" r="{er*0.36:.1f}" fill="#FFF"/>'
    d["eyes"] = eyes
    # 嘴：圆角实心小三角，尖朝下
    bw, bh = g["beak"]
    by = g["beak_y"]
    d["beak"] = (f'<path d="M{128-bw} {by} L{128+bw} {by} L128 {by+bh} Z" fill="{amber}" '
                 f'stroke="{amber}" stroke-width="5" stroke-linejoin="round"/>')
    lift = HAT_UP[hat] + 3 if hat else 0  # 戴帽时呆毛和星芒跟着帽顶抬高一点
    d["curl"] = f'<path transform="translate(0 {-lift})" d="{g["curl"]}" fill="none" stroke="{line}" stroke-width="{w:g}" stroke-linecap="round"/>'
    sx, sy, sr = g["star"]
    d["gleam"] = star(sx, sy - lift, sr, amber)
    return d


def mark(ink, **kw):
    return "".join(parts(ink, **kw).values())


DARK = dict(line=CREAM, face=CREAM, thin=0.94)  # 深色底：米白的头，墨色的大眼睛，线略细


def bounds(g=STD):
    """标识的外框（含线宽），用来缩放和光学居中。"""
    w = g["w"]
    sx, sy, sr = g["star"]
    lift = HAT_UP[g["hat"]] + 3 if g.get("hat") else 0
    x0 = 38 if g.get("hat") == "side" else 128 - R - w / 2
    return x0, sy - sr - lift, 128 + R + w / 2, Y + w / 2


def fit(inner, size=256, fill=0.84, lift=0.02, g=STD):
    """把标识按外框放进 size 见方：宽或高占 fill，视觉中心比几何中心高 lift。"""
    x0, y0, x1, y1 = bounds(g)
    k = size * fill / max(x1 - x0, y1 - y0)
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    return f'<g transform="translate({size/2:g} {size*(0.5-lift):.2f}) scale({k:.4f}) translate({-cx:.2f} {-cy:.2f})">{inner}</g>'


def svg(x, bg=None, vb="0 0 256 256", title="aiwren"):
    b = f'<rect x="-1000" y="-1000" width="3000" height="3000" fill="{bg}"/>' if bg else ""
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}"><title>{title}</title>{b}{x}</svg>'


def word(ink, x0=252, base=Y, xh=58, sw=W * 1.14, dot=AMB):
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


def lockup(ink, bg=None, mono=False):
    """横版：小鸟 + 字样，小鸟弧线的两脚和字样基线齐平。"""
    g, end = word(ink, sw=W, dot=ink if mono else AMB)
    if mono:
        bird = mark(ink, amber=ink, cap=None)
    elif ink == CREAM:
        bird = mark(INK, **DARK)
    else:
        bird = mark(ink)
    x0, y0, x1, y1 = bounds()
    pad = 24
    return svg(bird + g, bg, f"{x0-pad:g} {y0-pad:g} {end + W/2 + pad - (x0-pad):g} {y1 - y0 + 2*pad:g}")


def stacked(ink, bg=None, mono=False):
    """竖版：小鸟在上，字样在下居中。"""
    g, end = word(ink, sw=W, dot=ink if mono else AMB)
    ww = end - 252 + W
    if mono:
        bird = mark(ink, amber=ink, cap=None)
    elif ink == CREAM:
        bird = mark(INK, **DARK)
    else:
        bird = mark(ink)
    x0, y0, x1, y1 = bounds()
    gap = 40
    k = 0.62  # 字样相对小鸟的大小
    word_g = f'<g transform="translate(128 {y1+gap}) scale({k}) translate({-(252 - W/2 + ww/2):.1f} {-(Y-58-26):.1f})">{g}</g>'
    h = y1 + gap + (58 + 26 + W) * k
    pad = 24
    vw = max(x1 - x0, ww * k) + 2 * pad
    return svg(bird + word_g, bg, f"{128 - vw/2:.1f} {y0-pad:g} {vw:.1f} {h - y0 + 2*pad:.1f}")


def files():
    return {
        "symbol.svg": svg(fit(mark(INK))),
        "symbol-dark.svg": svg(fit(mark(INK, **DARK))),
        "symbol-black.svg": svg(fit(mark("#000", amber="#000", cap=None))),
        "symbol-white.svg": svg(fit(mark("#FFF", amber="#FFF", cap=None, thin=0.94))),
        "avatar.svg": svg(fit(mark(INK), fill=0.74, lift=0.0), CREAM),
        "avatar-dark.svg": svg(fit(mark(INK, **DARK), fill=0.74, lift=0.0), INK),
        # ≤ 32 px 用：线更粗、眼更大、无眼神光、星芒更大更远
        "symbol-small.svg": svg(fit(mark(INK, g=SMALL), fill=0.9, lift=0.0, g=SMALL)),
        "symbol-small-tile.svg": svg(fit(mark(INK, g=SMALL), fill=0.84, lift=0.0, g=SMALL), CREAM),
        "symbol-small-dark.svg": svg(fit(mark(INK, g=SMALL, **{**DARK, "thin": 1.0}), fill=0.84, lift=0.0, g=SMALL), INK),
        "lockup.svg": lockup(INK),
        "lockup-dark.svg": lockup(CREAM, INK),
        "lockup-black.svg": lockup("#000", mono=True),
        "stacked.svg": stacked(INK),
        "stacked-dark.svg": stacked(CREAM, INK),
    }


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "."
    os.makedirs(out, exist_ok=True)
    for name, s in files().items():
        with open(os.path.join(out, name), "w") as f:
            f.write(s)
        print(name)
