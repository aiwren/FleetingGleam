"""把若干 SVG 按多个尺寸并排放进一页 HTML，再截图看效果（只用于自查）。"""
import sys, os, subprocess
S = "/Users/lcopilot/.claude/plugins/cache/logo-design-skill/logo-design/1.4.5/skills/logo-design/scripts"
out = sys.argv[1]; files = sys.argv[2:]
sizes = [256, 96, 48, 32, 24, 16]
rows = []
for f in files:
    p = os.path.abspath(f)
    cells = "".join(f'<td><img src="file://{p}" width="{s}" height="{s}" style="image-rendering:auto"></td>' for s in sizes)
    rows.append(f"<tr><th>{os.path.relpath(f)}</th>{cells}</tr>")
html = f"<html><body style='margin:0;background:#eee;font:12px sans-serif'><table cellspacing=8>{''.join(rows)}</table></body></html>"
h = out.replace(".png", ".html"); open(h, "w").write(html)
subprocess.run(["python3", f"{S}/render_png.py", h, "-o", out, "--width", "900", "--height", str(40 + 272 * len(files))], check=True)
