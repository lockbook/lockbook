#!/bin/sh
set -e
cd "$(dirname "$0")"

if [ -z "$NO_WASM" ]; then
    (cd trunk && trunk build)
    rm -f static/wasm/base.html
fi

zola build -o public --force

cd ../docs && mdbook build && rm -rf ../public-site/public/docs && mv book ../public-site/public/docs && cd ../public-site

REV=$(git rev-parse --short HEAD)
STAGE=$(mktemp -d)
cp -R public/. "$STAGE"/
touch "$STAGE/.nojekyll"
printf 'User-agent: *\nDisallow: /\n' > "$STAGE/robots.txt"

cd "$STAGE"
git init -q -b main
git add -A
git commit -q -m "staging build @ $REV"
git push -q -f git@github.com:lockbook/lockbook.github.io main
cd - >/dev/null
rm -rf "$STAGE"
echo "deployed https://lockbook.github.io/ @ $REV"
