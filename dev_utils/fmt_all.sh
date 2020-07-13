#!/bin/sh

set -e

cd ../core
cargo fmt

cd ../server
cargo fmt

cd ../clients/cli
cargo fmt

cd ../../integration_tests
cargo fmt
