#!/bin/sh
#
# on nix the package of interest is librsvg
rsvg-convert logo.svg -o logo.png -w 150

# Windows store tiles, each plated asset at its own size (macOS sips; resvg from cargo or brew)
A=../../utils/lbdev/src/releaser/windows/msix-assets
for f in $(ls $A/*.png | grep -v unplated); do
  s=$(sips -g pixelWidth "$f" | awk '/pixelWidth/{print $2}')
  resvg -w "$s" -h "$s" store-tile.svg "$f"
done
