#!/bin/bash
set -e

if [[ " $@ " =~ " --deploy " ]]; then
    DEPLOY=true
else
    DEPLOY=false
fi

cd "$(dirname "$0")"

cd trunk && trunk build && mv ../trunk-build/base.html ../templates/

cd ../ && zola build  && mv ./trunk-build/* ./public/ && rm -rf ./trunk-build

if [ "$DEPLOY" = true ]; then
    gsutil -m cp -R ./public/* gs://lockbook.net
else 
    cd public && python3 -m http.server 5500
fi