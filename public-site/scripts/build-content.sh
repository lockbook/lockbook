#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

# Zola only allows load_data inside the site directory.
trap 'rm -rf .docs' EXIT
rm -rf .docs
cp -R ../docs .docs

zola build
mdbook build ../docs
mv ../docs/book public/docs
