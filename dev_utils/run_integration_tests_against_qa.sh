#!/bin/sh

set -a
API_URL="http://qa.lockbook.app"

cd ../integration_tests
cargo test
