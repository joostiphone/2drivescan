"""Generates assets/icon.svg – the 2DriveScan app icon (sunburst + "2")."""
import math, colorsys

S = 1024
C = S / 2

def polar(r, a):
    return C + r * math.cos(a), C + r * math.sin(a)

def arc_path(r0, r1, a0, a1, gap=6):
    p0 = gap / 2 / r1
    p1 = gap / 2 / r0
    o0, o1 = a0 + p0, a1 - p0
    i0, i1 = a0 + p1, a1 - p1
    large = 1 if (a1 - a0) > math.pi else 0
    x0, y0 = polar(r1, o0); x1, y1 = polar(r1, o1)
    x2, y2 = polar(r0, i1); x3, y3 = polar(r0, i0)
    return (f"M{x0:.2f},{y0:.2f} A{r1},{r1} 0 {large} 1 {x1:.2f},{y1:.2f} "
            f"L{x2:.2f},{y2:.2f} A{r0},{r0} 0 {large} 0 {x3:.2f},{y3:.2f} Z")

# Brand palette, shared with the app's sunburst (ui/js/colors.js).
PALETTE = ["#7c5cff", "#b64dff", "#ff4fa3", "#ff6b6b", "#ff9f43", "#ffd43b",
           "#5ee6a8", "#3ddbd9", "#4dabf7", "#7c5cff"]

def hex2rgb(h):
    return [int(h[i:i+2], 16) for i in (1, 3, 5)]

def color(t, lighten):
    t = t % 1.0
    x = t * (len(PALETTE) - 1)
    i = int(x)
    f = x - i
    a, b = hex2rgb(PALETTE[i]), hex2rgb(PALETTE[min(i + 1, len(PALETTE) - 1)])
    rgb = [a[k] + (b[k] - a[k]) * f for k in range(3)]
    rgb = [c + (255 - c) * lighten for c in rgb]
    return "#" + "".join(f"{int(c):02x}" for c in rgb)

# Ring layout: (inner, outer, segments as fractions)
rings = [
    (170, 262, [0.34, 0.22, 0.17, 0.12, 0.09, 0.06]),
    (268, 330, [0.20, 0.10, 0.12, 0.07, 0.03, 0.10, 0.06, 0.08, 0.05, 0.04, 0.06, 0.03, 0.03]),
    (336, 378, [0.12, 0.05, 0.06, 0.02, 0.08, 0.04, 0.03, 0.07, 0.03, 0.05, 0.04, 0.06, 0.03, 0.02]),
]
start = -math.pi / 2
paths = []
for depth, (r0, r1, segs) in enumerate(rings):
    a = start
    total = 1.0 if depth == 0 else sum(segs) / (0.98 if depth == 1 else 0.82)
    for f in segs:
        span = 2 * math.pi * f / total
        mid = (a + span / 2 - start) / (2 * math.pi)
        paths.append(f'<path d="{arc_path(r0, r1, a, a + span, 7)}" fill="{color(mid, depth * 0.14)}" />')
        a += span

svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{S}" height="{S}" viewBox="0 0 {S} {S}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#2a1d6e"/>
      <stop offset="0.55" stop-color="#121236"/>
      <stop offset="1" stop-color="#070914"/>
    </linearGradient>
    <radialGradient id="glow" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="#8b6cff" stop-opacity="0.55"/>
      <stop offset="0.6" stop-color="#3a2ca0" stop-opacity="0.15"/>
      <stop offset="1" stop-color="#000" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="shade" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0.3" stop-color="#fff" stop-opacity="0.18"/>
      <stop offset="1" stop-color="#000" stop-opacity="0.25"/>
    </radialGradient>
    <linearGradient id="core" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#1d1846"/>
      <stop offset="1" stop-color="#0c0b22"/>
    </linearGradient>
    <linearGradient id="two" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ffffff"/>
      <stop offset="1" stop-color="#c9d6ff"/>
    </linearGradient>
    <clipPath id="body"><rect x="100" y="100" width="824" height="824" rx="185" ry="185"/></clipPath>
    <filter id="soft" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="14"/>
    </filter>
  </defs>
  <g clip-path="url(#body)">
    <rect x="100" y="100" width="824" height="824" fill="url(#bg)"/>
    <circle cx="{C}" cy="{C}" r="420" fill="url(#glow)"/>
    <g>{''.join(paths)}</g>
    <circle cx="{C}" cy="{C}" r="380" fill="url(#shade)" style="mix-blend-mode:soft-light"/>
    <circle cx="{C}" cy="{C}" r="162" fill="url(#core)"/>
    <circle cx="{C}" cy="{C}" r="162" fill="none" stroke="#ffffff" stroke-opacity="0.10" stroke-width="3"/>
    <text x="{C}" y="{C + 72}" text-anchor="middle" font-family="Inter Display, Inter, sans-serif"
          font-weight="800" font-size="210" fill="url(#two)" letter-spacing="-6">2</text>
    <rect x="100" y="100" width="824" height="824" rx="185" ry="185" fill="none"
          stroke="#ffffff" stroke-opacity="0.08" stroke-width="4"/>
  </g>
</svg>'''
open("assets/icon.svg", "w").write(svg)
print("wrote assets/icon.svg")
