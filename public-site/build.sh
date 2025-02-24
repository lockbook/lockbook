#!/bin/bash
set -e

cd "$(dirname "$0")"

cd trunk && trunk build  && mv ../trunk-build/base.html ../templates/

cd ../ && zola build  && mv ./trunk-build/* ./public/ && rm -rf ./trunk-build

cd public && python3 -m http.server 5500