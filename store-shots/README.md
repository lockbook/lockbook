# Store screenshots

The seven Microsoft Store stills, rendered headlessly from the desktop app and composed with the tools here. `store-vault.md` records every content and design decision; this file is the recipe.

## One-time setup

- Accounts: the stills show `jane`, a showcase account whose vault is the Croatia trip. Her account folder is `~/.lockbook/jane`; `samantha` and `jordan` share notes with her and `travis` is the typed invitee. The folders live only on Travis's machine; `lockbook account export` in one reveals its key for another device.
- Caption font: Segoe UI Semibold, copied from a Windows install into `fonts/seguisb.ttf` (not committed; Microsoft's font). Without it the compositor falls back to the system bold.
- Tools: `for t in ambient compose mask phone sheet slate pdfpages backdrop; do swiftc -O $t.swift -o $t; done` (macOS, CoreImage/CoreText).
- Driver: `cargo build --release -p lockbook-desktop --features win-chrome --example shots` from the repo root. Release, so no debug overlays.

## Rendering a still

From this folder:

```
rm -f ~/.lockbook/jane/ws_persistence.json
LOCKBOOK_PATH=~/.lockbook/jane ../target/release/examples/shots scenes/01-hero.txt out/
```

Each scene in `scenes/` is a list of driver commands (size, ready, sidebar, pane, expand, open, share, sidecar, search, type, key, click, move, wheel, settings, reveal, blur, frames, shot). The shot lands in `out/<name>.png` at 2× the window's point size.

## Backdrop, once

```
./ambient out/bg/dome-warm.png 2560 1440 assets/dubrovnik.jpg 120 1.9 0.10 0.06 90
./slate out/bg/dome-warm.png out/bg/dome.png 0.25 0.5
```

The dome photo blurred warm, lifted at the top until the caption band reaches APCA Lc 90, with a slate floor over the bottom quarter.

## Composing

```
export CAPTION_FONT=fonts/seguisb.ttf
./compose final/01-notes.png      2560 1440 out/bg/dome.png out/01-hero.png       128 300 0.8 -- "The best place for your *notes*." 96 1280 150
./compose final/02-sketch.png     2560 1440 out/bg/dome.png out/02-drawing.png    128 300 0.8 -- "*Sketch* on an infinite canvas." 96 1280 150
./compose final/03-everything.png 2560 1440 out/bg/dome.png out/03-everything.png 128 300 0.8 -- "Your home for *everything*." 96 1280 150
./compose final/04-connected.png  2560 1440 out/bg/dome.png out/04-links.png      128 300 0.8 -- "Your notes, *connected*." 96 1280 150
./phone out/phone.png assets/ios-packing.png 620
./compose final/05-invitation.png 2560 1440 out/bg/dome.png out/05-together.png   853 300 0.8 out/phone.png 116 440 1 -- "*Shared* only by invitation." 96 1280 150
./compose final/06-find.png       2560 1440 out/bg/dome.png out/06-search.png     128 300 0.8 -- "Remember less. *Find* more." 96 1280 150
./mask out/07-keys.png out/07-keys-masked.png 1090 600 2180 740 4 9
./compose final/07-keys.png       2560 1440 out/bg/dome.png out/07-keys-masked.png 128 300 0.8 -- "You hold the *keys*." 96 1280 150
./sheet final/strip.png 4 final/0*.png
```

The starred word is set in the brand blue on its tint. `mask` covers all but the first four words of the account phrase. `assets/ios-packing.png` is the iPhone simulator shot of the packing list (lb-iphone sim seeded with jane's account; see the Rig section of `store-vault.md`).

## Shipping

The finals go to `fastlane/metadata/windows/en-US/images/desktopScreenshots/` with one caption `.txt` beside each, which `lbdev release windows store-screenshots` pushes to a Partner Center submission. The listing text is in `/test/lb-maintainers/store/microsoft-store-listing.md` in the vault.
