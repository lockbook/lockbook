"""Encode capture-editor-demo.cjs frames with Pillow (pip install Pillow).

Usage: python3 scripts/encode-editor-demo.py /tmp/editor-demo static/demos
"""
import json
import shutil
import sys
from pathlib import Path

from PIL import Image

source = Path(sys.argv[1])
output = Path(sys.argv[2])
output.mkdir(parents=True, exist_ok=True)

for theme in ('light', 'dark'):
    directory = source / theme
    manifest = json.loads((directory / 'frames.json').read_text())
    entries = manifest['frames']
    # A shared palette keeps text and caret colors stable throughout the loop.
    samples = entries[::10] + entries[-1:]
    palette_source = Image.new('RGB', (manifest['width'], manifest['height'] * len(samples)))
    for i, entry in enumerate(samples):
        with Image.open(directory / entry['file']) as image:
            palette_source.paste(image.convert('RGB'), (0, i * manifest['height']))
    palette = palette_source.quantize(colors=128, method=Image.Quantize.MEDIANCUT)
    frames = []
    for entry in entries:
        with Image.open(directory / entry['file']) as image:
            frames.append(image.convert('RGB').quantize(palette=palette, dither=Image.Dither.NONE))
    target = output / f'editor-{theme}.gif'
    frames[0].save(target, save_all=True, append_images=frames[1:],
                   duration=[entry['duration'] for entry in entries], loop=0,
                   optimize=True, disposal=1)
    shutil.copyfile(directory / 'poster.png', output / f'editor-{theme}.png')
    with Image.open(target) as image:
        assert image.is_animated and image.size == (manifest['width'], manifest['height'])
        duration = 0
        for i in range(image.n_frames):
            image.seek(i)
            duration += image.info['duration']
        assert duration == sum(entry['duration'] for entry in entries)
        print(f'{target}: {image.n_frames} frames, {duration / 1000:g}s, {target.stat().st_size / 1024:.0f} KiB')
