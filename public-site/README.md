Hey welcome to lockbook's public site. 

technologies used:
- [zola](https://www.getzola.org/): a static website generator to power the blog 
- [trunk](https://trunkrs.dev/): packages workspace into a wasm build 
- [tailwind](https://trunkrs.dev/): css styling

## Build 

to build the website, use trunk to package workspace into a wasm binary included in a html page. then this html page is used as a zola template to be leveraged in zola static site generation pipeline. 
All of this can be done though `./build.sh` which will also start a development server on [http://localhost:5500/](http://localhost:5500/)

ensure that you have `trunk`, `zola` and added `wasm` as rust compilation target. this can be done through `./setup_macos.sh`

## Deploy

to deploy to gcp, run `./build.sh --deploy`

you might have to setup gcloud auth `gcloud auth login`

## Mobile editor recording

Devices whose primary pointer supports both hover and fine input keep the
interactive demo. Other devices show a 30-second recording of the real WASM
editor typing a partitioned todo list, reordering a task, and folding a section.
This choice uses `(hover: hover) and (pointer: fine)`, independent of screen size.
The recording follows the page's light/dark theme and includes a pause button;
reduced-motion users see the completed note as a still image.

To regenerate it, serve a built site locally, install Playwright and Pillow in
your tooling environment, then run from `public-site`:

```sh
node scripts/capture-editor-demo.cjs http://127.0.0.1:5500 /tmp/editor-demo
python3 scripts/encode-editor-demo.py /tmp/editor-demo static/demos
```

Set `CHROME_EXECUTABLE` to use an installed Chrome, or install Playwright's
Chromium. The capture uses the existing `/wasm/` files and real keyboard/mouse
input, clears the initial demo text, and verifies the resulting Markdown.
The recording uses 70 ms per ordinary character and 150 ms per Markdown syntax
character. The link URL is inserted with a real clipboard paste event.
A recording-only pointer overlay follows the actual mouse events and
shows clicks and dragging. Change the typing sequence in
`capture-editor-demo.cjs` to make a new take.
