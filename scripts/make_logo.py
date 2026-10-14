#!/usr/bin/env python3

import argparse
import math
import random
from pathlib import Path

SIZE = 1024

PAPER = "#f5efe0"
PAPER_DEEP = "#e7dcc3"
TORN = "#fbf8f0"
SUN = "#c9442c"
SUN_FIBER = "#eaa38f"
FAR = "#a9b8c7"
MID = "#566f91"
NEAR = "#1e2a45"


def wobble(rng, amplitude, harmonics=(2, 3, 5, 7, 11, 17)):
    terms = [(amplitude / k**0.7, k, rng.uniform(0, math.tau)) for k in harmonics]
    return lambda t: sum(a * math.sin(k * t + phase) for a, k, phase in terms)


def polygon(points):
    return "M" + "L".join(f"{x:.1f} {y:.1f}" for x, y in points) + "Z"


def smooth_closed(points):
    n = len(points)
    d = f"M{points[0][0]:.1f} {points[0][1]:.1f}"
    for i in range(n):
        p0, p1, p2, p3 = points[(i - 1) % n], points[i], points[(i + 1) % n], points[(i + 2) % n]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
        d += f"C{c1[0]:.1f} {c1[1]:.1f} {c2[0]:.1f} {c2[1]:.1f} {p2[0]:.1f} {p2[1]:.1f}"
    return d + "Z"


def squircle(cx, cy, half, power, rng, jitter, points=120):
    noise = wobble(rng, jitter)
    out = []
    for i in range(points):
        t = math.tau * i / points
        c, s = math.cos(t), math.sin(t)
        x = half * math.copysign(abs(c) ** (2 / power), c)
        y = half * math.copysign(abs(s) ** (2 / power), s)
        length = math.hypot(x, y) or 1
        push = noise(t)
        out.append((cx + x + x / length * push, cy + y + y / length * push))
    return smooth_closed(out)


def ridge(rng, base, amp, freqs, edge, step=5):
    phases = [rng.uniform(0, math.tau) for _ in freqs]
    weights = (0.56, 0.30, 0.14)
    slow = wobble(rng, edge * 0.5, harmonics=(3, 5, 9))
    body, fringe = [], []
    x = -24.0
    while x <= SIZE + 24:
        u = x / SIZE
        height = sum(
            w * (1 - abs(math.sin(math.pi * u * f + p)))
            for w, f, p in zip(weights, freqs, phases)
        )
        y = base - amp * height + rng.gauss(0, 1.3)
        body.append((x, y))
        fringe.append((x, y - edge - slow(u * math.tau) - abs(rng.gauss(0, edge * 0.45))))
        x += step
    return body, fringe


def closed_ridge(points, bottom=SIZE + 80):
    return [(points[0][0], bottom)] + points + [(points[-1][0], bottom)]


def disc(rng, cx, cy, radius, jitter, points=96):
    noise = wobble(rng, jitter)
    out = []
    for i in range(points):
        t = math.tau * i / points
        r = radius + noise(t) + rng.gauss(0, jitter * 0.12)
        out.append((cx + math.cos(t) * r, cy + math.sin(t) * r))
    return polygon(out)


def fibers(rng, count, palette, region, length=(24, 120), angle_sigma=50):
    x0, y0, x1, y1 = region
    parts = []
    for _ in range(count):
        x, y = rng.uniform(x0, x1), rng.uniform(y0, y1)
        size = rng.uniform(*length)
        angle = rng.gauss(math.radians(-8), math.radians(angle_sigma))
        dx, dy = math.cos(angle) * size, math.sin(angle) * size
        bend = rng.uniform(-0.3, 0.3) * size
        cx, cy = x + dx / 2 - math.sin(angle) * bend, y + dy / 2 + math.cos(angle) * bend
        parts.append(
            f'<path d="M{x:.1f} {y:.1f}Q{cx:.1f} {cy:.1f} {x + dx:.1f} {y + dy:.1f}" stroke="{rng.choice(palette)}" '
            f'stroke-width="{rng.choice((1.1, 1.5, 2.0, 2.6))}" stroke-opacity="{rng.uniform(0.2, 0.6):.2f}"/>'
        )
    return "".join(parts)


def build(seed, shown, detail=True):
    rng = random.Random(seed)
    sheet = squircle(SIZE / 2, SIZE / 2, 448, 5.2, rng, 5.0)
    step = 5 if detail else 16

    far_body, far_fringe = ridge(rng, 600, 170, (1.7, 3.1, 5.3), 9, step)
    mid_body, mid_fringe = ridge(rng, 712, 150, (1.3, 2.6, 4.7), 10, step)
    near_body, near_fringe = ridge(rng, 842, 110, (1.1, 2.2, 3.9), 11, step)
    layers = [
        ("far", far_body, far_fringe, FAR, ("#d7e0e9", "#8a9db1")),
        ("mid", mid_body, mid_fringe, MID, ("#9fb3cc", "#3f5677")),
        ("near", near_body, near_fringe, NEAR, ("#6f86a8", "#121b30")),
    ]

    sun_cx, sun_cy, sun_r = 652, 372, 150
    sun_edge = disc(rng, sun_cx, sun_cy, sun_r + 9, 5.5)
    sun_body = disc(rng, sun_cx, sun_cy, sun_r, 3.2)

    defs, art = [], []
    defs.append(f'<clipPath id="sheet"><path d="{sheet}"/></clipPath>')
    defs.append(
        '<radialGradient id="vignette" cx="50%" cy="42%" r="70%">'
        f'<stop offset="0.5" stop-color="{PAPER}" stop-opacity="0"/><stop offset="1" stop-color="{PAPER_DEEP}" stop-opacity="0.9"/></radialGradient>'
    )
    defs.append(
        '<linearGradient id="mist" x1="0" y1="0" x2="0" y2="1">'
        '<stop offset="0" stop-color="#fff" stop-opacity="0"/><stop offset="0.55" stop-color="#fbf8f0" stop-opacity="0.5"/>'
        '<stop offset="1" stop-color="#fbf8f0" stop-opacity="0"/></linearGradient>'
    )
    defs.append(
        '<filter id="lift" x="-10%" y="-10%" width="120%" height="125%">'
        '<feDropShadow dx="0" dy="16" stdDeviation="20" flood-color="#2a1d10" flood-opacity="0.24"/></filter>'
    )
    defs.append(
        '<filter id="layer" x="-5%" y="-30%" width="110%" height="160%">'
        '<feDropShadow dx="2" dy="-5" stdDeviation="7" flood-color="#0d1424" flood-opacity="0.34"/></filter>'
    )
    defs.append(
        '<filter id="sunshadow" x="-30%" y="-30%" width="160%" height="160%">'
        '<feDropShadow dx="3" dy="6" stdDeviation="8" flood-color="#40130a" flood-opacity="0.28"/></filter>'
    )
    if detail:
        defs.append(
            '<filter id="grain" x="0" y="0" width="100%" height="100%">'
            '<feTurbulence type="fractalNoise" baseFrequency="0.85" numOctaves="3" seed="5"/>'
            '<feColorMatrix type="matrix" values="0 0 0 0 0.30  0 0 0 0 0.22  0 0 0 0 0.12  0 0 0 0.20 -0.02"/></filter>'
        )

    art.append(f'<path d="{sheet}" fill="{PAPER}" filter="url(#lift)"/>')
    art.append('<g clip-path="url(#sheet)">')
    art.append(f'<rect width="{SIZE}" height="{SIZE}" fill="url(#vignette)"/>')
    if detail:
        art.append(
            '<g fill="none" stroke-linecap="round">'
            + fibers(rng, 150, ("#e0d6bc", "#d2c7a9", "#efe8d6"), (40, 40, SIZE - 40, 560))
            + "</g>"
        )

    defs.append(f'<clipPath id="sun"><path d="{sun_body}"/></clipPath>')
    art.append(f'<g filter="url(#sunshadow)"><path d="{sun_edge}" fill="{TORN}"/></g>')
    art.append(f'<path d="{sun_body}" fill="{SUN}"/>')
    if detail:
        art.append(
            '<g clip-path="url(#sun)" fill="none" stroke-linecap="round">'
            + fibers(rng, 70, (SUN_FIBER, "#b23520", "#e07a62"), (sun_cx - sun_r, sun_cy - sun_r, sun_cx + sun_r, sun_cy + sun_r), (20, 90))
            + "</g>"
        )

    for index, (name, body, fringe, color, fiber_palette) in enumerate(layers):
        if index == 1:
            art.append(f'<rect x="0" y="470" width="{SIZE}" height="260" fill="url(#mist)"/>')
        body_d, fringe_d = polygon(closed_ridge(body)), polygon(closed_ridge(fringe))
        defs.append(f'<clipPath id="{name}"><path d="{body_d}"/></clipPath>')
        art.append(f'<g filter="url(#layer)"><path d="{fringe_d}" fill="{TORN}"/></g>')
        art.append(f'<path d="{body_d}" fill="{color}"/>')
        if detail:
            top = min(y for _, y in body)
            art.append(
                f'<g clip-path="url(#{name})" fill="none" stroke-linecap="round">'
                + fibers(rng, 80, fiber_palette, (0, top, SIZE, SIZE), (22, 130), 38)
                + "</g>"
            )

    if detail:
        art.append(f'<rect width="{SIZE}" height="{SIZE}" filter="url(#grain)"/>')
    art.append("</g>")

    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}" width="{shown}" height="{shown}" role="img" aria-label="Washi">\n'
        "  <title>Washi 和紙</title>\n"
        f"  <defs>{''.join(defs)}</defs>\n  " + "\n  ".join(art) + "\n</svg>\n"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--all", action="store_true", help="app-icon.svg・docs/assets/logo.svg・docs/favicon.svg をまとめて作る")
    parser.add_argument("--out", default="docs/assets/logo.svg", help="--all を使わないときの出力先")
    parser.add_argument("--size", type=int, default=160, help="表示サイズ（px）。--all を使わないときだけ効く")
    parser.add_argument("--simple", action="store_true", help="紙の目と繊維を省く。--all を使わないときだけ効く")
    parser.add_argument("--seed", type=int, default=11, help="縁・繊維の乱数の種")
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    jobs = (
        [(root / "app-icon.svg", 1024, True), (root / "docs/assets/logo.svg", 160, True), (root / "docs/favicon.svg", 160, False)]
        if args.all
        else [(Path(args.out), args.size, not args.simple)]
    )
    for out, size, detail in jobs:
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(build(args.seed, size, detail), encoding="utf-8")
        print(f"wrote {out} ({out.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
