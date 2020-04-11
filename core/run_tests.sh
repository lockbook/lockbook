# export RUST_BACKTRACE=1;

if cargo fmt --quiet -- --check ; then
	cargo test
else
	echo "The following files fail lint checks:"
	cargo fmt -- --check -l
	echo "Run cargo fmt" 
	cargo test
	exit 1
fi
