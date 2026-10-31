#!/usr/bin/env python3
"""Contact sheets of a scenario's captures: one sheet per label prefix, the
`*-on`/`*-off` pairs of each view side by side, labelled, at half size, so a
run's PNGs can be read a module at a time.

    python3 tools/contact_sheet.py target/scenarios/render-sp-XXXX [--scale 0.5]
"""
import argparse
from collections import defaultdict
from pathlib import Path

from PIL import Image, ImageDraw


def sheets(root, scale):
    groups = defaultdict(list)
    for png in sorted(root.glob('*.png')):
        if png.parent.name == 'sheets':
            continue
        stem = png.stem
        label = stem.split('-near')[0].split('-far')[0].split('-pulse')[0]
        groups[label].append(png)
    out = root / 'sheets'
    out.mkdir(exist_ok=True)
    for label, pngs in groups.items():
        images = [Image.open(p).convert('RGB') for p in pngs]
        w = int(max(i.width for i in images) * scale)
        h = int(max(i.height for i in images) * scale)
        cols = 2
        rows = (len(images) + cols - 1) // cols
        sheet = Image.new('RGB', (w * cols, (h + 14) * rows), 'black')
        draw = ImageDraw.Draw(sheet)
        for n, (png, image) in enumerate(zip(pngs, images)):
            x, y = (n % cols) * w, (n // cols) * (h + 14)
            sheet.paste(image.resize((int(image.width * scale), int(image.height * scale))), (x, y + 14))
            draw.text((x + 2, y + 1), png.stem, fill='yellow')
        sheet.save(out / f'{label}.png')
        print(out / f'{label}.png', len(pngs))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--scale', type=float, default=0.5)
    args = parser.parse_args()
    sheets(args.root, args.scale)


if __name__ == '__main__':
    main()
