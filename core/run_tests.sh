if cargo fmt --quiet -- --check ; then
	cargo test --all --no-fail-fast
else
	echo "The following files fail lint checks:"
	cargo fmt -- --check -l
	echo "Run cargo fmt" 
	cargo test --all --no-fail-fast

	exit 1
fi
