"""Convert oracle framebuffer dumps (*.vga) to indexed PNG files.

Usage: python tools/scripts/oracle-fb2png.py <fb-dir-or-file> [...] [--out DIR] [--sheet N]

A dump is a 768-byte VGA DAC palette (256 x R,G,B, 6-bit values 0..63) followed by width x height
indexed pixels, row by row. Width and height are in the file name: f<frame>_<W>x<H>[_tag].vga.
See P:\\msdos\\TD2192\\re-work\\dosbox\\ORACLE.md.

PNGs are written next to the dumps (or into --out) and must stay in the RE work area: they are
renders of original game data and never go into the repository. --sheet N also writes a contact
sheet with N columns (sheet.png) for a quick look.

Palette expansion: 8-bit = (v << 2) | (v >> 4), the usual 6-to-8-bit VGA mapping.
Requires Pillow (scratch tooling only, not a project dependency).
"""
import argparse
import pathlib
import re
import sys

from PIL import Image

NAME = re.compile(r'_(\d+)x(\d+)(?:_|\.vga$)')


def load(path: pathlib.Path) -> Image.Image:
    m = NAME.search(path.name)
    if not m:
        raise ValueError(f'{path.name}: no WxH in name')
    w, h = int(m.group(1)), int(m.group(2))
    data = path.read_bytes()
    if len(data) != 768 + w * h:
        raise ValueError(f'{path.name}: size {len(data)} != 768 + {w}*{h}')
    pal = [((v & 63) << 2) | ((v & 63) >> 4) for v in data[:768]]
    img = Image.frombytes('P', (w, h), data[768:])
    img.putpalette(pal)
    return img


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('inputs', nargs='+')
    ap.add_argument('--out')
    ap.add_argument('--sheet', type=int, default=0)
    a = ap.parse_args()
    files = []
    for i in a.inputs:
        p = pathlib.Path(i)
        files += sorted(p.glob('*.vga')) if p.is_dir() else [p]
    if not files:
        print('no .vga files', file=sys.stderr)
        return 1
    imgs = []
    for f in files:
        img = load(f)
        out_dir = pathlib.Path(a.out) if a.out else f.parent / 'png'
        out_dir.mkdir(parents=True, exist_ok=True)
        img.save(out_dir / (f.stem + '.png'))
        imgs.append(img)
    print(f'{len(files)} PNG(s) written')
    if a.sheet and imgs:
        cols = a.sheet
        w = max(i.width for i in imgs)
        h = max(i.height for i in imgs)
        rows = (len(imgs) + cols - 1) // cols
        sheet = Image.new('RGB', (w * cols, h * rows))
        for k, img in enumerate(imgs):
            sheet.paste(img.convert('RGB'), ((k % cols) * w, (k // cols) * h))
        target = (pathlib.Path(a.out) if a.out else files[0].parent / 'png') / 'sheet.png'
        sheet.save(target)
        print(f'contact sheet: {target}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
