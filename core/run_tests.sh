export RUST_BACKTRACE=1;

if cargo fmt --quiet -- --check ; then
	cargo test -- --nocapture
else
	echo "The following files fail lint checks:"
	cargo fmt -- --check -l
	echo "Run cargo fmt" 
	cargo test -- --nocapture
	exit 1
fi
