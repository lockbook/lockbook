#!/bin/sh
set -e

if [[ " $@ " =~ " --deploy " ]]; then
    DEPLOY=true
else
    DEPLOY=false
fi

cd "$(dirname "$0")"

# Build live editor + canvas WASM demos. Output lands in static/wasm/ per
# Trunk.toml. Skip with NO_WASM=1 for fast iteration on HTML/CSS changes
# (in that case the previously-built WASM is reused).
if [ -z "$NO_WASM" ]; then
    (cd trunk && trunk build)
    # Trunk always emits a rendered HTML for its template; we only consume
    # public-site.js + public-site_bg.wasm and serve the page from Zola.
    rm -f static/wasm/base.html
fi

zola build

cd ../docs ; mdbook build ; mv book ../public-site/public ; cd ../public-site/public ; mv book docs ; cd ..

if [ "$DEPLOY" = true ]; then
    gcloud storage cp -r public/* gs://lockbook.net/
    gcloud storage cp -r static/.well-known gs://lockbook.net/
else 
    cd public && python3 -m http.server 5500
fi
