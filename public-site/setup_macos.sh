#!/bin/bash
set -e

cargo install --locked trunk
brew install zola
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli
cargo install --locked mdbook
