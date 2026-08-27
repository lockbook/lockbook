#!/bin/sh
set -e

if [[ " $@ " =~ " --deploy " ]]; then
    DEPLOY=true
else
    DEPLOY=false
fi

cd "$(dirname "$0")"

# Build the editor + canvas WASM demos into static/wasm/ (see trunk/Trunk.toml).
# NO_WASM=1 reuses the previous build — fast iteration on HTML/CSS.
if [ -z "$NO_WASM" ]; then
    (cd trunk && trunk build)
    # Trunk always renders its target template; we only consume the js + wasm.
    rm -f static/wasm/base.html
fi

zola build

cd ../docs ; mdbook build ; mv book ../public-site/public ; cd ../public-site/public ; mv book docs ; cd ..

if [ "$DEPLOY" = true ]; then
    # The wasm is large and highly compressible — serve it encoded, and cache it
    # hard. Without -z GCS stores it identity-encoded and every visitor pays the
    # full uncompressed size.
    gcloud storage cp -r -z js,wasm,html,css,svg,xml public/* gs://lockbook.net/
    gcloud storage cp -r static/.well-known gs://lockbook.net/
else
    cd public && python3 -m http.server 5500
fi
