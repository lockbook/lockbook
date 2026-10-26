#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

zola build
mdbook build ../docs
mv ../docs/book public/docs
python3 scripts/finalize-site.py public
