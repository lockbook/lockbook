#!/bin/sh
set -e

if [[ " $@ " =~ " --deploy " ]]; then
    DEPLOY=true
else
    DEPLOY=false
fi

cd "$(dirname "$0")"

cd trunk; trunk build; mv ../trunk-build/base.html ../templates/

cd ../ ; zola build  ; mv ./trunk-build/* ./public/ ; rm -rf ./trunk-build

cd ../docs ; mdbook build ; mv book ../public-site/public ; cd ../public-site/public ; mv book docs ; cd ..

if [ "$DEPLOY" = true ]; then
    gsutil -m cp -R ./public/* gs://lockbook.net
else 
    cd public && python3 -m http.server 5500
fi
