trap "kill 0" EXIT
set -a 
set -e
source ../containers/qa.env

cargo build --manifest-path ../core/Cargo.toml
cargo test --manifest-path ../core/Cargo.toml

grcov ../core/target/debug/ -s . -t html --llvm --branch --ignore-not-existing -o ./core-tests-coverage/ 

cd ../server/
cargo build
cargo run & cargo test --manifest-path ../integration_tests/Cargo.toml
mv ../integration_tests/target/debug/deps/*.gcda ./target/debug/deps/
grcov ./target/debug/ -s . -t html --llvm --branch --ignore-not-existing -o ../integration_tests/integration_tests-coverage/ 
