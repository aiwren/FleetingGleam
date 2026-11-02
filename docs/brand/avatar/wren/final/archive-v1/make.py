"""aiwren 标识的生成脚本：冒头的鹪鹩，棕色头顶、大圆眼，呆毛上挑着一颗琥珀 AI 星芒。

运行 `python3 make.py` 在本目录写出全部 SVG。片头动画（intro.py）也从这里取图形。
"""

INK, CREAM, AMB, BG = "#2A221D", "#FBF4EA", "#E8A043", "#FBF4EA"
W = 11  # 线宽（256 画布）


BROWN = "#B9824B"  # 头顶的棕帽（鹪鹩的棕色头顶）


def star(cx, cy, r, fill):
    """四角星芒（AI 的通用记号，也是惊鸿的标志形）：四个尖、内凹的弧边。"""
    k = r * 0.22
    return (f'<path d="M{cx} {cy-r} Q{cx+k} {cy-k} {cx+r} {cy} Q{cx+k} {cy+k} {cx} {cy+r} '
            f'Q{cx-k} {cy+k} {cx-r} {cy} Q{cx-k} {cy-k} {cx} {cy-r} Z" fill="{fill}"/>')


def parts(ink, w=W, amber=AMB, cap=BROWN, uid="d", line=None, face=None):
    """小鸟的各个部件，按出场顺序：（脸）、棕帽、身体弧线、眼睛、嘴、呆毛、星芒。

    `line`：弧线和呆毛的颜色（默认同 `ink`）；`face`：头部里面填一块颜色（深色底用米白，眼睛仍是墨色）。
    """
    line = line or ink
    y = 170  # 弧的两端
    top = y - 70
    er = 12.5 + (w - 11) * 0.25  # 大圆眼
    ey = y - 40
    hl = "#FFF"
    eyes = ""
    for cx in (101, 155):
        eyes += f'<circle cx="{cx}" cy="{ey}" r="{er:.1f}" fill="{ink}"/>'
        if cap:  # 眼神光（单色版不要）
            eyes += f'<circle cx="{cx+4.2}" cy="{ey-4.4}" r="{er*0.32:.1f}" fill="{hl}"/>'
            if w < 15:
                eyes += f'<circle cx="{cx-4.5}" cy="{ey+5}" r="{er*0.13:.1f}" fill="{hl}"/>'
    d = {"face": f'<path d="M58 {y} A 70 70 0 0 1 198 {y} Z" fill="{face}"/>' if face else ""}
    d["cap"] = (f'<clipPath id="{uid}"><path d="M58 {y} A 70 70 0 0 1 198 {y} Z"/></clipPath>'
                f'<path clip-path="url(#{uid})" d="M40 {y-12} C 90 {y-70}, 166 {y-70}, 216 {y-12} L216 0 L40 0 Z" fill="{cap}"/>') if cap else ""
    d["arc"] = f'<path d="M58 {y} A 70 70 0 0 1 198 {y}" fill="none" stroke="{line}" stroke-width="{w}" stroke-linecap="round"/>'
    d["eyes"] = eyes
    # 粗线版的 v 字嘴会糊成一颗爱心，改成实心小三角
    d["beak"] = (f'<path d="M120 {y-24} L128 {y-15} L136 {y-24}" fill="none" stroke="{amber}" stroke-width="{w-1}" stroke-linecap="round" stroke-linejoin="round"/>'
                 if w < 15 else
                 f'<path d="M119 {y-25} L137 {y-25} L128 {y-14} Z" fill="{amber}" stroke="{amber}" stroke-width="3" stroke-linejoin="round"/>')
    d["curl"] = f'<path d="M127 {top} C 121 {top-20}, 140 {top-30}, 146 {top-24}" fill="none" stroke="{line}" stroke-width="{w-1}" stroke-linecap="round"/>'
    d["gleam"] = star(155, top - 33, 15 + (w - 11) * 0.4, amber)
    return d


def mark(ink, w=W, amber=AMB, cap=BROWN, uid="d", **kw):
    return "".join(parts(ink, w, amber, cap, uid, **kw).values())


DARK = dict(line=CREAM, face=CREAM)  # 深色底：米白的头，墨色的大眼睛


def scaled(inner, k=1.14, cy=128):
    """放大并让视觉中心略高于画面正中。"""
    return f'<g transform="translate(128 {cy}) scale({k}) translate(-128 -124)">{inner}</g>'


def svg(x, bg=None, vb="0 0 256 256"):
    b = f'<rect x="-1000" y="-1000" width="3000" height="3000" fill="{bg}"/>' if bg else ""
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}"><title>aiwren</title>{b}{x}</svg>'


def word(ink, x0=252, base=168, xh=58, sw=W * 1.14):
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
    dot = (x, t - 20)
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
    g += f'<circle cx="{dot[0]}" cy="{dot[1]}" r="8" fill="{AMB}"/>'
    return g, x


def lockup(ink, bg=None):
    g, end = word(ink)
    bird = mark(INK, **DARK) if ink == CREAM else mark(ink)
    return svg(scaled(bird) + g, bg, f"20 40 {end + 44 - 20:.0f} 176")


if __name__ == "__main__":
    files = {
        "symbol.svg": svg(scaled(mark(INK))),
        "symbol-dark.svg": svg(scaled(mark(INK, **DARK))),
        "symbol-black.svg": svg(scaled(mark("#000", amber="#000", cap=None))),
        "avatar.svg": svg(scaled(mark(INK), 1.18, 128), BG),
        "avatar-dark.svg": svg(scaled(mark(INK, **DARK), 1.18, 128), INK),
        # ≤ 32 px 用：线更粗
        "symbol-small.svg": svg(scaled(mark(INK, w=17), 1.2, 128), BG),
        "lockup.svg": lockup(INK),
        "lockup-dark.svg": lockup(CREAM),
    }
    for name, s in files.items():
        open(name, "w").write(s)
    print("\n".join(files))
