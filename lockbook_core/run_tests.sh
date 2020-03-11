if cargo fmt --quiet -- --check ; then
	cargo test -- --nocapture
else
	echo "The following files fail lint checks:"
	cargo fmt -- --check -l
	echo "Run cargo fmt" 
	exit 1
fi
