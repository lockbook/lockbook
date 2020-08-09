#!/bin/sh

set -ae

if ! command -v cargo &> /dev/null
then
	echo "You do not have cargo, get the rust toolchain from rustup."
	exit 1
fi

cargo install grcov

API_URL="http://qa.lockbook.app:8000"
CARGO_INCREMENTAL=0
RUSTFLAGS="-Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests -Cpanic=abort"
RUSTDOCFLAGS="-Cpanic=abort"

cd ../core
cargo build
cargo test
grcov ./target/debug/ -s . -t html --llvm --branch --ignore-not-existing -o ./target/debug/coverage/
cd target/debug/coverage/
python -m http.server
